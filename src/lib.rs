use std::{cell::UnsafeCell, mem::MaybeUninit, time::Duration};

#[cfg(feature = "mem-tracker")]
pub mod interpose;
pub mod jmp;
#[cfg(feature = "mem-tracker")]
pub mod mem;
pub mod watchdog;

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("task timed out")]
    TimedOut,
}

struct TimeoutData {
    #[cfg(feature = "mem-tracker")]
    pub(crate) mem: mem::MemTracker,

    pub(crate) jump_env: UnsafeCell<Option<MaybeUninit<jmp::JmpBuf>>>,
    pub(crate) outer: *mut TimeoutData,
}

impl Default for TimeoutData {
    fn default() -> Self {
        Self {
            outer: std::ptr::null_mut(),
            jump_env: UnsafeCell::new(None),
            #[cfg(feature = "mem-tracker")]
            mem: mem::MemTracker::default(),
        }
    }
}

thread_local! {
    pub static TIMEOUT_DATA: UnsafeCell<*mut TimeoutData> = UnsafeCell::new(std::ptr::null_mut());
}

pub(crate) unsafe fn get_current_timeout_data() -> Result<*mut TimeoutData, ()> {
    // we are actually calling get_timeout_data in thread destructor (because of free)
    // which is a bad place to access the TLS. we should just return Err instead of
    // panicking there.
    let Ok(x) = TIMEOUT_DATA.try_with(|x| x.get()) else {
        return Err(());
    };
    if x.is_null() {
        return Err(());
    }
    Ok(*x)
}

pub(crate) fn set_current_timeout_data(data: *mut TimeoutData) -> Result<(), ()> {
    match TIMEOUT_DATA.try_with(|x| {
        let x = unsafe { &mut *x.get() };
        *x = data;
    }) {
        Ok(_) => Ok(()),
        Err(_) => Err(()),
    }
}

pub fn timeout_cpu<R, F: Fn() -> R>(task: F, timeout: Duration) -> Result<R, Error> {
    // TODO: resources?
    /*
       can this be a solution:
       - track every open fd in processes
       - interpose sources of new fd and add to a list based on tid
       - close all of them after failure
    */
    // TODO: follow threads?
    /*
       we can interpose thread creation but the problem is how to combine these timers?
       - can we use cgroups somehow?
       - is a monitor thread (or a smaller cputime for inaccurate periodically check) only solution?
       - https://github.com/godzie44/BugStalker ? tokio oracle?
    */
    // TODO: async?
    /*
       async hooks in tokio? https://discord.com/channels/500028886025895936/500336333500448798/1369206090037723196
       how tokio-console works? https://github.com/tokio-rs/console/tree/main/tokio-console#tasks-list
       how tracing works?
       https://docs.rs/tokio/latest/tokio/runtime/struct.Builder.html#method.on_before_task_poll
    */
    // TODO: overhead of everything and how to test for memory leaks and stuff?

    let data = unsafe { get_current_timeout_data().unwrap() };

    let mut td = TimeoutData::default();
    let mut outer: *mut TimeoutData = std::ptr::null_mut(); /* put the outer here if needed */

    if data.is_null() {
        outer = data;
        td.outer = outer;
        set_current_timeout_data(&mut td as _).unwrap();
    } else {
        set_current_timeout_data(&mut td as _).unwrap();
    }
    let data = unsafe { get_current_timeout_data().unwrap() };
    std::mem::forget(td);

    let buf = unsafe { &mut *(*data).jump_env.get() };
    *buf = Some(MaybeUninit::uninit());
    let j_val = unsafe { jmp::sigsetjmp(buf.as_mut().unwrap().as_mut_ptr(), 1) };

    static mut DATA: [*mut TimeoutData; 10] = [std::ptr::null_mut(); 10];
    match j_val {
        0 => {
            let watch = watchdog::Watchdog::new(Box::new(move || {
                let buf = unsafe { &mut *(*data).jump_env.get() };
                unsafe {
                    // TODO: not thread-safe
                    #[allow(static_mut_refs)]
                    for (i, x) in DATA.iter_mut().enumerate() {
                        if x.is_null() {
                            *x = data;
                            // we can't use 0. see  jmp::siglongjmp docs
                            jmp::siglongjmp(buf.as_mut().unwrap().as_mut_ptr(), (i + 1) as _);
                        }
                    }
                    panic!("out of space")
                }
            }));
            watch.arm(timeout);
            let r = task();
            watch.disarm();

            // we are allocating watchdog inside the mem tracker
            // so we should de-allocate it before calling the MemTracker
            // Drop. preventing a double free
            drop(watch);

            #[cfg(feature = "mem-tracker")]
            {
                // timer didn't trigger so we actually know current data is ours
                let data = unsafe { &mut *get_current_timeout_data().unwrap() };
                data.mem.free_all();
            }

            // recover the outer
            if !outer.is_null() {
                set_current_timeout_data(outer).unwrap();
            }

            return Ok(r);
        }
        watchdog_data_i => {
            // ... this causes double free, but it should be ok... right?
            // drop(task);

            // here we don't know if current data is related the timer that triggred it.
            // because of that we send the pointer to Timeout data in watchdog data callback
            // so we can recover from that one.
            let watchdog_data_i = watchdog_data_i - 1;
            #[allow(static_mut_refs)]
            let data = unsafe {
                let x = DATA[watchdog_data_i as usize];
                DATA[watchdog_data_i as usize] = std::ptr::null_mut();
                x
            };
            let data = unsafe { &mut *data };
            #[cfg(feature = "mem-tracker")]
            data.mem.free_all();

            // we can't trust the stack at this point
            // so we should use data.outer
            // recover the outer
            if !data.outer.is_null() {
                set_current_timeout_data(data.outer).unwrap();
            }
            return Err(Error::TimedOut);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_nesting() {
        // inner smaller than outer
        let r = timeout_cpu(
            || timeout_cpu(|| loop {}, Duration::from_millis(100)),
            Duration::from_millis(100),
        );

        assert!(matches!(r, Ok(Err(Error::TimedOut))));

        // outer smaller than inner
        let r = timeout_cpu(
            || timeout_cpu(|| loop {}, Duration::from_millis(100)),
            Duration::from_millis(50),
        );
        assert!(matches!(r, Err(Error::TimedOut)));
    }

    #[test]
    fn test_basic_functionality_works() {
        fn test(timeout: Duration) {
            let r = timeout_cpu(|| loop {}, timeout);
            assert_eq!(r, Err(Error::TimedOut));

            let r = timeout_cpu(|| 1, timeout);
            assert_eq!(r, Ok(1));
        }

        test(Duration::from_millis(100));
        test(Duration::from_millis(500));
        test(Duration::from_millis(300));
        test(Duration::from_millis(1000));
    }
}

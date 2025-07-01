use std::{cell::UnsafeCell, mem::MaybeUninit, time::Duration};

pub mod jmp;
pub mod watchdog;

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("task timed out")]
    TimedOut,
}

thread_local! {
    pub static JMP_ENV: UnsafeCell<Option<MaybeUninit<jmp::JmpBuf>>> = UnsafeCell::new(None);
}

pub fn timeout_cpu<R, F: Fn() -> R>(task: F, timeout: Duration) -> Result<R, Error> {
    // TODO: allocations? / memory leaks
    /*
       https://github.com/EmbarkStudios/crash-handling/pull/8/files#diff-b04454ec1d15f45a222fc624d25df3492d207d9fdc209ef57815385a3a13a3d7
       similar to crash handler crate i guess we have to interpose malloc instead of using global allocator.
       because task() may call into non rust binary.

    */
    // TODO: resources?
    /*
       is this a solution:
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
    // TODO: nested timeout calls in a thread?
    /*
       instead of one static jump buffer we should just create a tree of them with an ID
    */
    // TODO: overhead of everything and how to test for memory leaks and stuff?

    let buf = unsafe { &mut *JMP_ENV.with(|x| x.get()) };
    *buf = Some(MaybeUninit::uninit());
    let j_val = unsafe { jmp::sigsetjmp(buf.as_mut().unwrap().as_mut_ptr(), 1) };

    match j_val {
        0 => {
            let watch = watchdog::Watchdog::new(Box::new(|| {
                let buf = unsafe { &mut *JMP_ENV.with(|x| x.get()) };
                unsafe {
                    jmp::siglongjmp(buf.as_mut().unwrap().as_mut_ptr(), 1);
                }
            }));
            watch.arm(timeout);
            let r = task();
            watch.disarm();
            return Ok(r);
        }
        _ => {
            // ... this may cause double free, but it should be ok... right?
            drop(task);
            return Err(Error::TimedOut);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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

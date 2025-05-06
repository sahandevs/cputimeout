use nix::libc::{
    clockid_t, gettid, sigaction, sigevent, siginfo_t, timer_create, timer_delete, timer_settime,
    timer_t, CLOCK_THREAD_CPUTIME_ID, SA_SIGINFO, SIGALRM, SIGEV_THREAD_ID,
};
use std::{ffi::c_void, time::Duration};

pub struct Watchdog {
    timer_id: timer_t,
    callback: *mut Box<dyn Fn()>,
}

type CallBack = Box<dyn Fn()>;

extern "C" fn handler(_: i32, info: &mut siginfo_t, _: *mut c_void) {
    let callback = unsafe { &mut *((info.si_value().sival_ptr) as *mut CallBack) };
    callback();
}

impl Watchdog {
    pub fn new(callback: CallBack) -> Self {
        let mut action: sigaction = unsafe { std::mem::zeroed() };
        action.sa_flags = SA_SIGINFO;
        action.sa_sigaction = handler as usize;
        unsafe { sigaction(SIGALRM, &action, std::ptr::null_mut()) };
        let tid = unsafe { gettid() };

        let clockid: clockid_t = CLOCK_THREAD_CPUTIME_ID;

        let mut timer_id: timer_t = unsafe { std::mem::zeroed() };

        let mut sev: sigevent = unsafe { std::mem::zeroed() };
        sev.sigev_notify = SIGEV_THREAD_ID;
        sev.sigev_signo = SIGALRM;
        sev.sigev_notify_thread_id = tid;
        let callback = Box::into_raw(Box::new(callback));
        sev.sigev_value.sival_ptr = callback as *mut c_void;
        unsafe { timer_create(clockid, &mut sev, &mut timer_id) };
        Watchdog { timer_id, callback }
    }

    pub fn arm(&self, timeout: Duration) -> bool {
        if timeout.is_zero() {
            return false;
        }
        let time = &mut unsafe { std::mem::zeroed::<nix::libc::itimerspec>() };
        time.it_value.tv_sec = timeout.as_secs() as _;
        time.it_value.tv_nsec = timeout.subsec_nanos() as _;
        unsafe {
            timer_settime(self.timer_id, 0, time, std::ptr::null_mut());
        }

        true
    }

    pub fn disarm(&self) {
        let time = &mut unsafe { std::mem::zeroed::<nix::libc::itimerspec>() };
        unsafe {
            timer_settime(self.timer_id, 0, time, std::ptr::null_mut());
        }
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        unsafe {
            timer_delete(self.timer_id);
            drop(Box::from_raw(self.callback));
        }
    }
}

// https://github.com/EmbarkStudios/crash-handling/pull/8/files#diff-b04454ec1d15f45a222fc624d25df3492d207d9fdc209ef57815385a3a13a3d7
//
use crate::{get_current_timeout_data, interpose};
use libc::c_void;

interpose!(c"malloc", fn REAL_MALLOC = malloc (size: usize) -> *mut c_void |r| {
    match unsafe { get_current_timeout_data() } {
        Ok(d) => {
            if d.is_null() { return r; };
            unsafe { &mut *d }.mem.track(r);
            r
        },
        Err(_) => r
    }
});

interpose!(c"free", fn REAL_FREE = free (ptr: *mut c_void) |r| {
    match unsafe { get_current_timeout_data() } {
        Ok(d) => {
            if d.is_null() { return r; };
            unsafe { &mut *d }.mem.free(ptr);
            r
        },
        Err(_) => r
    }
});

pub(crate) struct MemTracker {
    // TODO: use real_malloc instead
    allocations: [*mut c_void; 100],
}

impl MemTracker {
    fn track(&mut self, ptr: *mut c_void) {
        for t in self.allocations.iter_mut() {
            if *t == 0 as _ {
                *t = ptr;
                return;
            }
        }
        unsafe {
            libc::exit(81);
        }
    }

    fn free(&mut self, ptr: *mut c_void) {
        for t in self.allocations.iter_mut() {
            if *t == ptr {
                *t = 0 as _;
                return;
            }
        }
    }

    pub fn free_all(&mut self) {
        for ptr in self.allocations {
            if !ptr.is_null() {
                unsafe { (REAL_FREE.unwrap())(ptr) };
            }
        }
    }
}

impl Default for MemTracker {
    fn default() -> Self {
        Self {
            allocations: [0 as *mut c_void; 100],
        }
    }
}

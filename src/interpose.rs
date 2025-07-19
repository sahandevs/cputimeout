#[macro_export]
macro_rules! interpose {
    ($name_c:expr, fn $real_name:ident = $name:ident  ($($p_name:ident : $p_type:ty),*) $(-> $p_ret: ty)? |$r:ident| $body:expr) => {

        static mut $real_name: Option<fn($($p_name: $p_type),*) $(-> $p_ret)*> = None;
        #[unsafe(no_mangle)]
        pub extern "C" fn $name($($p_name: $p_type),*) $(-> $p_ret)* {
            static INIT: parking_lot::Once = parking_lot::Once::new();
            INIT.call_once(|| unsafe {
                const RTLD_NEXT: *mut c_void = -1isize as *mut c_void;
                let ptr = nix::libc::dlsym(RTLD_NEXT, $name_c.as_ptr().cast());
                if !ptr.is_null() {
                    $real_name = Some(std::mem::transmute::<*mut c_void, _>(ptr));
                }

            });

            #[allow(static_mut_refs)]
            let real = unsafe {
                $real_name.as_ref().unwrap()
            };
            let $r = real($($p_name),*) ;

            $body
        }
    };
}

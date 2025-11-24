use libc::*;

cfg_if! {
    if #[cfg(ossl110)] {
        pub enum OPENSSL_STACK {}
    } else if #[cfg(libressl390)] {
        pub enum _STACK {}
    } else {
        #[repr(C)]
        pub struct _STACK {
            pub num: c_int,
            pub data: *mut *mut c_char,
            pub sorted: c_int,
            pub num_alloc: c_int,
            pub comp: Option<unsafe extern "C" fn(*const c_void, *const c_void) -> c_int>,
        }
    }
}

cfg_if! {
    if #[cfg(ossl110)] {
        extern "C" {
            pub fn OPENSSL_sk_num(stack: *const OPENSSL_STACK) -> c_int;
            pub fn OPENSSL_sk_value(stack: *const OPENSSL_STACK, idx: c_int) -> *mut c_void;

            pub fn OPENSSL_sk_new_null() -> *mut OPENSSL_STACK;
            pub fn OPENSSL_sk_free(st: *mut OPENSSL_STACK);
            pub fn OPENSSL_sk_pop_free(
                st: *mut OPENSSL_STACK,
                free: Option<unsafe extern "C" fn(*mut c_void)>,
            );
            pub fn OPENSSL_sk_push(st: *mut OPENSSL_STACK, data: *const c_void) -> c_int;
            pub fn OPENSSL_sk_pop(st: *mut OPENSSL_STACK) -> *mut c_void;
        }
    } else {
        extern "C" {
            pub fn sk_num(st: *const _STACK) -> c_int;
            pub fn sk_value(st: *const _STACK, n: c_int) -> *mut c_void;

            pub fn sk_new_null() -> *mut _STACK;
            pub fn sk_free(st: *mut _STACK);
            pub fn sk_pop_free(st: *mut _STACK, free: Option<unsafe extern "C" fn(*mut c_void)>);
            pub fn sk_push(st: *mut _STACK, data: *mut c_void) -> c_int;
            pub fn sk_pop(st: *mut _STACK) -> *mut c_void;
        }
    }
}

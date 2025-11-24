use crate::prelude::*;

/* DIFF(main): module removed in de76fee6 */

// DragonFlyBSD's __error function is declared with "static inline", so it must
// be implemented in the libc crate, as a pointer to a static thread_local.
f! {
    #[deprecated(since = "0.2.77", note = "Use `__errno_location()` instead")]
    pub fn __error() -> *mut c_int {
        &mut errno
    }
}

extern "C" {
    #[thread_local]
    pub static mut errno: c_int;
}

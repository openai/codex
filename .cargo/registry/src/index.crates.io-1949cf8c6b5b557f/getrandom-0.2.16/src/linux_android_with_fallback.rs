//! Implementation for Linux / Android with `/dev/urandom` fallback
use crate::{
    lazy::LazyBool,
    util_libc::{getrandom_syscall, last_os_error, sys_fill_exact},
    {use_file, Error},
};
use core::mem::MaybeUninit;

pub fn getrandom_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    // getrandom(2) was introduced in Linux 3.17
    static HAS_GETRANDOM: LazyBool = LazyBool::new();
    if HAS_GETRANDOM.unsync_init(is_getrandom_available) {
        sys_fill_exact(dest, getrandom_syscall)
    } else {
        use_file::getrandom_inner(dest)
    }
}

fn is_getrandom_available() -> bool {
    if getrandom_syscall(&mut []) < 0 {
        match last_os_error().raw_os_error() {
            Some(libc::ENOSYS) => false, // No kernel support
            // The fallback on EPERM is intentionally not done on Android since this workaround
            // seems to be needed only for specific Linux-based products that aren't based
            // on Android. See https://github.com/rust-random/getrandom/issues/229.
            #[cfg(target_os = "linux")]
            Some(libc::EPERM) => false, // Blocked by seccomp
            _ => true,
        }
    } else {
        true
    }
}

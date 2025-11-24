//! Implementation for Linux / Android with `/dev/urandom` fallback
use super::use_file;
use crate::Error;
use core::{
    ffi::c_void,
    mem::{transmute, MaybeUninit},
    ptr::NonNull,
    sync::atomic::{AtomicPtr, Ordering},
};
use use_file::util_libc;

pub use crate::util::{inner_u32, inner_u64};

type GetRandomFn = unsafe extern "C" fn(*mut c_void, libc::size_t, libc::c_uint) -> libc::ssize_t;

/// Sentinel value which indicates that `libc::getrandom` either not available,
/// or not supported by kernel.
const NOT_AVAILABLE: NonNull<c_void> = unsafe { NonNull::new_unchecked(usize::MAX as *mut c_void) };

static GETRANDOM_FN: AtomicPtr<c_void> = AtomicPtr::new(core::ptr::null_mut());

#[cold]
#[inline(never)]
fn init() -> NonNull<c_void> {
    // Use static linking to `libc::getrandom` on MUSL targets and `dlsym` everywhere else
    #[cfg(not(target_env = "musl"))]
    let raw_ptr = {
        static NAME: &[u8] = b"getrandom\0";
        let name_ptr = NAME.as_ptr().cast::<libc::c_char>();
        unsafe { libc::dlsym(libc::RTLD_DEFAULT, name_ptr) }
    };
    #[cfg(target_env = "musl")]
    let raw_ptr = {
        let fptr: GetRandomFn = libc::getrandom;
        unsafe { transmute::<GetRandomFn, *mut c_void>(fptr) }
    };

    let res_ptr = match NonNull::new(raw_ptr) {
        Some(fptr) => {
            let getrandom_fn = unsafe { transmute::<NonNull<c_void>, GetRandomFn>(fptr) };
            let dangling_ptr = NonNull::dangling().as_ptr();
            // Check that `getrandom` syscall is supported by kernel
            let res = unsafe { getrandom_fn(dangling_ptr, 0, 0) };
            if cfg!(getrandom_test_linux_fallback) {
                NOT_AVAILABLE
            } else if res.is_negative() {
                match util_libc::last_os_error().raw_os_error() {
                    Some(libc::ENOSYS) => NOT_AVAILABLE, // No kernel support
                    // The fallback on EPERM is intentionally not done on Android since this workaround
                    // seems to be needed only for specific Linux-based products that aren't based
                    // on Android. See https://github.com/rust-random/getrandom/issues/229.
                    #[cfg(target_os = "linux")]
                    Some(libc::EPERM) => NOT_AVAILABLE, // Blocked by seccomp
                    _ => fptr,
                }
            } else {
                fptr
            }
        }
        None => NOT_AVAILABLE,
    };

    #[cfg(getrandom_test_linux_without_fallback)]
    if res_ptr == NOT_AVAILABLE {
        panic!("Fallback is triggered with enabled `getrandom_test_linux_without_fallback`")
    }

    GETRANDOM_FN.store(res_ptr.as_ptr(), Ordering::Release);
    res_ptr
}

// Prevent inlining of the fallback implementation
#[inline(never)]
fn use_file_fallback(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    use_file::fill_inner(dest)
}

#[inline]
pub fn fill_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    // Despite being only a single atomic variable, we still cannot always use
    // Ordering::Relaxed, as we need to make sure a successful call to `init`
    // is "ordered before" any data read through the returned pointer (which
    // occurs when the function is called). Our implementation mirrors that of
    // the one in libstd, meaning that the use of non-Relaxed operations is
    // probably unnecessary.
    let raw_ptr = GETRANDOM_FN.load(Ordering::Acquire);
    let fptr = match NonNull::new(raw_ptr) {
        Some(p) => p,
        None => init(),
    };

    if fptr == NOT_AVAILABLE {
        use_file_fallback(dest)
    } else {
        // note: `transmute` is currently the only way to convert a pointer into a function reference
        let getrandom_fn = unsafe { transmute::<NonNull<c_void>, GetRandomFn>(fptr) };
        util_libc::sys_fill_exact(dest, |buf| unsafe {
            getrandom_fn(buf.as_mut_ptr().cast(), buf.len(), 0)
        })
    }
}

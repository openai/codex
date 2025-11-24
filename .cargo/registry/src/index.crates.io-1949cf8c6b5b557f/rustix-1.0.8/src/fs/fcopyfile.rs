use crate::fs::CopyfileFlags;
use crate::{backend, io};
use backend::fd::AsFd;
use backend::fs::types::copyfile_state_t;

/// `fcopyfile(from, to, state, flags)`
///
/// # Safety
///
/// The `state` operand must be allocated with `copyfile_state_alloc` and not
/// yet freed with `copyfile_state_free`.
///
/// # References
///  - [Apple]
///
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/fcopyfile.3.html
#[inline]
pub unsafe fn fcopyfile<FromFd: AsFd, ToFd: AsFd>(
    from: FromFd,
    to: ToFd,
    state: copyfile_state_t,
    flags: CopyfileFlags,
) -> io::Result<()> {
    backend::fs::syscalls::fcopyfile(from.as_fd(), to.as_fd(), state, flags)
}

/// `copyfile_state_alloc()`
///
/// # References
///  - [Apple]
///
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/fcopyfile.3.html
#[inline]
pub fn copyfile_state_alloc() -> io::Result<copyfile_state_t> {
    backend::fs::syscalls::copyfile_state_alloc()
}

/// `copyfile_state_free(state)`
///
/// # Safety
///
/// The `state` operand must be allocated with `copyfile_state_alloc` and not
/// yet freed with `copyfile_state_free`.
///
/// # References
///  - [Apple]
///
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/fcopyfile.3.html
#[inline]
pub unsafe fn copyfile_state_free(state: copyfile_state_t) -> io::Result<()> {
    backend::fs::syscalls::copyfile_state_free(state)
}

/// `copyfile_state_get(state, COPYFILE_STATE_COPIED)`
///
/// # Safety
///
/// The `state` operand must be allocated with `copyfile_state_alloc` and not
/// yet freed with `copyfile_state_free`.
///
/// # References
///  - [Apple]
///
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/fcopyfile.3.html
#[inline]
pub unsafe fn copyfile_state_get_copied(state: copyfile_state_t) -> io::Result<u64> {
    backend::fs::syscalls::copyfile_state_get_copied(state)
}

/// `copyfile_state_get(state, flags, dst)`
///
/// # Safety
///
/// The `state` operand must be allocated with `copyfile_state_alloc` and not
/// yet freed with `copyfile_state_free`.
///
/// # References
///  - [Apple]
///
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/fcopyfile.3.html
#[inline]
pub unsafe fn copyfile_state_get(
    state: copyfile_state_t,
    flag: u32,
    dst: *mut core::ffi::c_void,
) -> io::Result<()> {
    backend::fs::syscalls::copyfile_state_get(state, flag, dst)
}

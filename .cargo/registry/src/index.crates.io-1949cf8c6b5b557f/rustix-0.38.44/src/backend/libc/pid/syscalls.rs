//! libc syscalls for PIDs

use crate::backend::c;
use crate::pid::Pid;

#[cfg(not(target_os = "wasi"))]
#[inline]
#[must_use]
pub(crate) fn getpid() -> Pid {
    unsafe {
        let pid = c::getpid();
        Pid::from_raw_unchecked(pid)
    }
}

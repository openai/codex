//! linux_raw syscalls supporting modules that use `prctl`.
//!
//! # Safety
//!
//! See the `rustix::backend` module documentation for details.
#![allow(unsafe_code, clippy::undocumented_unsafe_blocks)]

use crate::backend::c;
use crate::backend::conv::{c_int, ret_c_int};
use crate::io;

#[inline]
pub(crate) unsafe fn prctl(
    option: c::c_int,
    arg2: *mut c::c_void,
    arg3: *mut c::c_void,
    arg4: *mut c::c_void,
    arg5: *mut c::c_void,
) -> io::Result<c::c_int> {
    ret_c_int(syscall!(__NR_prctl, c_int(option), arg2, arg3, arg4, arg5))
}

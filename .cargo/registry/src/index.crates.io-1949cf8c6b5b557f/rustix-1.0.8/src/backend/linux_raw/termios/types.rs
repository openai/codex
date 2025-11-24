//! Types for the `termios` module.

#![allow(non_camel_case_types)]

use crate::ffi;

// We don't want to use `tcflag_t` directly so we don't expose linux_raw_sys
// publicly. It appears to be `c_ulong `on SPARC and `c_uint` everywhere else.

#[cfg(target_arch = "sparc")]
pub type tcflag_t = ffi::c_ulong;
#[cfg(not(target_arch = "sparc"))]
pub type tcflag_t = ffi::c_uint;

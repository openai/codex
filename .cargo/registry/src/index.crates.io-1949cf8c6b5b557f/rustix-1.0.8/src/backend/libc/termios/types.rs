//! Types for the `termios` module.

#![allow(non_camel_case_types)]

#[cfg(not(any(target_os = "espidf", target_os = "redox")))]
use crate::ffi;

// We don't want to use `tcflag_t` directly so we don't expose libc
// publicly. Redox uses `u32`, apple uses `c_ulong`, everything else
// seems to use `c_uint`.

#[cfg(apple)]
pub type tcflag_t = ffi::c_ulong;
#[cfg(target_os = "redox")]
pub type tcflag_t = u32;
#[cfg(not(any(apple, target_os = "espidf", target_os = "redox", target_os = "wasi")))]
pub type tcflag_t = ffi::c_uint;

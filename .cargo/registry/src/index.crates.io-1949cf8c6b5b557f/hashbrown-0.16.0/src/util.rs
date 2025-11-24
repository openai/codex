// FIXME: Replace with `core::hint::{likely, unlikely}` once they are stable.
#[cfg(feature = "nightly")]
pub(crate) use core::intrinsics::{likely, unlikely};

#[cfg(not(feature = "nightly"))]
#[inline(always)]
#[cold]
fn cold_path() {}

#[cfg(not(feature = "nightly"))]
#[inline(always)]
pub(crate) fn likely(b: bool) -> bool {
    if b {
        true
    } else {
        cold_path();
        false
    }
}

#[cfg(not(feature = "nightly"))]
#[inline(always)]
pub(crate) fn unlikely(b: bool) -> bool {
    if b {
        cold_path();
        true
    } else {
        false
    }
}

// FIXME: use strict provenance functions once they are stable.
// Implement it with a transmute for now.
#[inline(always)]
#[allow(clippy::useless_transmute)] // clippy is wrong, cast and transmute are different here
pub(crate) fn invalid_mut<T>(addr: usize) -> *mut T {
    unsafe { core::mem::transmute(addr) }
}

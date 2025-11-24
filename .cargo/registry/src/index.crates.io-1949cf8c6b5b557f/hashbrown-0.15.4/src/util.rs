// FIXME: Branch prediction hint. This is currently only available on nightly
// but it consistently improves performance by 10-15%.
#[cfg(not(feature = "nightly"))]
pub(crate) use core::convert::{identity as likely, identity as unlikely};
#[cfg(feature = "nightly")]
pub(crate) use core::intrinsics::{likely, unlikely};

// FIXME: use strict provenance functions once they are stable.
// Implement it with a transmute for now.
#[inline(always)]
#[allow(clippy::useless_transmute)] // clippy is wrong, cast and transmute are different here
pub(crate) fn invalid_mut<T>(addr: usize) -> *mut T {
    unsafe { core::mem::transmute(addr) }
}

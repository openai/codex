#[cfg(not(feature = "alloc"))]
pub use core::alloc;

#[cfg(feature = "alloc")]
pub use alloc_crate::{alloc, boxed, vec, collections};

//! An implementation which calls out to an externally defined function.
use crate::Error;
use core::mem::MaybeUninit;

pub use crate::util::{inner_u32, inner_u64};

#[inline]
pub fn fill_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    extern "Rust" {
        fn __getrandom_v03_custom(dest: *mut u8, len: usize) -> Result<(), Error>;
    }
    unsafe { __getrandom_v03_custom(dest.as_mut_ptr().cast(), dest.len()) }
}

//! Implementation for SOLID
use crate::Error;
use core::mem::MaybeUninit;

pub use crate::util::{inner_u32, inner_u64};

extern "C" {
    pub fn SOLID_RNG_SampleRandomBytes(buffer: *mut u8, length: usize) -> i32;
}

#[inline]
pub fn fill_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    let ret = unsafe { SOLID_RNG_SampleRandomBytes(dest.as_mut_ptr().cast::<u8>(), dest.len()) };
    if ret >= 0 {
        Ok(())
    } else {
        Err(Error::from_neg_error_code(ret))
    }
}

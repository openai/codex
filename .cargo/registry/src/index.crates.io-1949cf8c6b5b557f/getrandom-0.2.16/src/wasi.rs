//! Implementation for WASI
use crate::Error;
use core::{
    mem::MaybeUninit,
    num::{NonZeroU16, NonZeroU32},
};
use wasi::random_get;

pub fn getrandom_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    unsafe { random_get(dest.as_mut_ptr() as *mut u8, dest.len()) }.map_err(|e| {
        // The WASI errno will always be non-zero, but we check just in case.
        match NonZeroU16::new(e.raw()) {
            Some(r) => Error::from(NonZeroU32::from(r)),
            None => Error::ERRNO_NOT_POSITIVE,
        }
    })
}

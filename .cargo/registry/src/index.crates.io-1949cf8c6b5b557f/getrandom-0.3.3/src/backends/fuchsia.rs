//! Implementation for Fuchsia Zircon
use crate::Error;
use core::mem::MaybeUninit;

pub use crate::util::{inner_u32, inner_u64};

#[link(name = "zircon")]
extern "C" {
    fn zx_cprng_draw(buffer: *mut u8, length: usize);
}

#[inline]
pub fn fill_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    unsafe { zx_cprng_draw(dest.as_mut_ptr().cast::<u8>(), dest.len()) }
    Ok(())
}

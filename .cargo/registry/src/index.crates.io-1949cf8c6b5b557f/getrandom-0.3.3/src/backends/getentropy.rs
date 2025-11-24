//! Implementation using getentropy(2)
//!
//! Available since:
//!   - macOS 10.12
//!   - OpenBSD 5.6
//!   - Emscripten 2.0.5
//!   - vita newlib since Dec 2021
//!
//! For these targets, we use getentropy(2) because getrandom(2) doesn't exist.
use crate::Error;
use core::{ffi::c_void, mem::MaybeUninit};

pub use crate::util::{inner_u32, inner_u64};

#[path = "../util_libc.rs"]
mod util_libc;

#[inline]
pub fn fill_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    for chunk in dest.chunks_mut(256) {
        let ret = unsafe { libc::getentropy(chunk.as_mut_ptr().cast::<c_void>(), chunk.len()) };
        if ret != 0 {
            return Err(util_libc::last_os_error());
        }
    }
    Ok(())
}

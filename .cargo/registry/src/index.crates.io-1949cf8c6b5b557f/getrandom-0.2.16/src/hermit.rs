//! Implementation for Hermit
use crate::Error;
use core::{mem::MaybeUninit, num::NonZeroU32};

/// Minimum return value which we should get from syscalls in practice,
/// because Hermit uses positive `i32`s for error codes:
/// https://github.com/hermitcore/libhermit-rs/blob/main/src/errno.rs
const MIN_RET_CODE: isize = -(i32::MAX as isize);

extern "C" {
    fn sys_read_entropy(buffer: *mut u8, length: usize, flags: u32) -> isize;
}

pub fn getrandom_inner(mut dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    while !dest.is_empty() {
        let res = unsafe { sys_read_entropy(dest.as_mut_ptr() as *mut u8, dest.len(), 0) };
        // Positive `isize`s can be safely casted to `usize`
        if res > 0 && (res as usize) <= dest.len() {
            dest = &mut dest[res as usize..];
        } else {
            let err = match res {
                MIN_RET_CODE..=-1 => NonZeroU32::new(-res as u32).unwrap().into(),
                _ => Error::UNEXPECTED,
            };
            return Err(err);
        }
    }
    Ok(())
}

//! Generic array are commonly used as a return value for hash digests, so
//! it's a good idea to allow to hexlify them easily. This module implements
//! `std::fmt::LowerHex` and `std::fmt::UpperHex` traits.
//!
//! Example:
//!
//! ```rust
//! # #[macro_use]
//! # extern crate generic_array;
//! # extern crate typenum;
//! # fn main() {
//! let array = arr![u8; 10, 20, 30];
//! assert_eq!(format!("{:x}", array), "0a141e");
//! # }
//! ```
//!

use core::{fmt, str, ops::Add, cmp::min};

use typenum::*;

use crate::{ArrayLength, GenericArray};

static LOWER_CHARS: &'static [u8] = b"0123456789abcdef";
static UPPER_CHARS: &'static [u8] = b"0123456789ABCDEF";

impl<T: ArrayLength<u8>> fmt::LowerHex for GenericArray<u8, T>
where
    T: Add<T>,
    <T as Add<T>>::Output: ArrayLength<u8>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let max_digits = f.precision().unwrap_or_else(|| self.len() * 2);
        let max_hex = (max_digits >> 1) + (max_digits & 1);

        if T::USIZE < 1024 {
            // For small arrays use a stack allocated
            // buffer of 2x number of bytes
            let mut res = GenericArray::<u8, Sum<T, T>>::default();

            self.iter().take(max_hex).enumerate().for_each(|(i, c)| {
                res[i * 2] = LOWER_CHARS[(c >> 4) as usize];
                res[i * 2 + 1] = LOWER_CHARS[(c & 0xF) as usize];
            });

            f.write_str(unsafe { str::from_utf8_unchecked(&res[..max_digits]) })?;
        } else {
            // For large array use chunks of up to 1024 bytes (2048 hex chars)
            let mut buf = [0u8; 2048];
            let mut digits_left = max_digits;

            for chunk in self[..max_hex].chunks(1024) {
                chunk.iter().enumerate().for_each(|(i, c)| {
                    buf[i * 2] = LOWER_CHARS[(c >> 4) as usize];
                    buf[i * 2 + 1] = LOWER_CHARS[(c & 0xF) as usize];
                });

                let n = min(chunk.len() * 2, digits_left);
                f.write_str(unsafe { str::from_utf8_unchecked(&buf[..n]) })?;
                digits_left -= n;
            }
        }
        Ok(())
    }
}

impl<T: ArrayLength<u8>> fmt::UpperHex for GenericArray<u8, T>
where
    T: Add<T>,
    <T as Add<T>>::Output: ArrayLength<u8>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let max_digits = f.precision().unwrap_or_else(|| self.len() * 2);
        let max_hex = (max_digits >> 1) + (max_digits & 1);

        if T::USIZE < 1024 {
            // For small arrays use a stack allocated
            // buffer of 2x number of bytes
            let mut res = GenericArray::<u8, Sum<T, T>>::default();

            self.iter().take(max_hex).enumerate().for_each(|(i, c)| {
                res[i * 2] = UPPER_CHARS[(c >> 4) as usize];
                res[i * 2 + 1] = UPPER_CHARS[(c & 0xF) as usize];
            });

            f.write_str(unsafe { str::from_utf8_unchecked(&res[..max_digits]) })?;
        } else {
            // For large array use chunks of up to 1024 bytes (2048 hex chars)
            let mut buf = [0u8; 2048];
            let mut digits_left = max_digits;

            for chunk in self[..max_hex].chunks(1024) {
                chunk.iter().enumerate().for_each(|(i, c)| {
                    buf[i * 2] = UPPER_CHARS[(c >> 4) as usize];
                    buf[i * 2 + 1] = UPPER_CHARS[(c & 0xF) as usize];
                });

                let n = min(chunk.len() * 2, digits_left);
                f.write_str(unsafe { str::from_utf8_unchecked(&buf[..n]) })?;
                digits_left -= n;
            }
        }
        Ok(())
    }
}

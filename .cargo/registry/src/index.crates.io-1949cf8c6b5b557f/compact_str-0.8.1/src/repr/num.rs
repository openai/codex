//! Implementations for efficiently converting a number into a [`Repr`]
//!
//! Adapted from the implementation in the `std` library at
//! <https://github.com/rust-lang/rust/blob/b8214dc6c6fc20d0a660fb5700dca9ebf51ebe89/src/libcore/fmt/num.rs#L188-L266>

use core::{
    mem,
    num,
    ptr,
};

use super::traits::IntoRepr;
use super::Repr;
use crate::{
    ToCompactStringError,
    UnwrapWithMsg,
};

const DEC_DIGITS_LUT: &[u8] = b"\
      0001020304050607080910111213141516171819\
      2021222324252627282930313233343536373839\
      4041424344454647484950515253545556575859\
      6061626364656667686970717273747576777879\
      8081828384858687888990919293949596979899";

/// Defines the implementation of [`IntoRepr`] for integer types
macro_rules! impl_IntoRepr {
    ($t:ident, $conv_ty:ident) => {
        impl IntoRepr for $t {
            fn into_repr(self) -> Result<Repr, ToCompactStringError> {
                // Determine the number of digits in this value
                //
                // Note: this considers the `-` symbol
                let num_digits = NumChars::num_chars(self);
                let mut repr = Repr::with_capacity(num_digits).unwrap_with_msg();

                #[allow(unused_comparisons)]
                let is_nonnegative = self >= 0;
                let mut n = if is_nonnegative {
                    self as $conv_ty
                } else {
                    // convert the negative num to positive by summing 1 to it's 2 complement
                    (!(self as $conv_ty)).wrapping_add(1)
                };
                let mut curr = num_digits as isize;

                // our string will end up being num_digits long
                unsafe { repr.set_len(num_digits) };
                // get mutable pointer to our buffer
                let buf_ptr = unsafe { repr.as_mut_buf().as_mut_ptr() };

                let lut_ptr = DEC_DIGITS_LUT.as_ptr();

                unsafe {
                    // need at least 16 bits for the 4-characters-at-a-time to work.
                    if mem::size_of::<$t>() >= 2 {
                        // eagerly decode 4 characters at a time
                        while n >= 10000 {
                            let rem = (n % 10000) as isize;
                            n /= 10000;

                            let d1 = (rem / 100) << 1;
                            let d2 = (rem % 100) << 1;
                            curr -= 4;
                            ptr::copy_nonoverlapping(lut_ptr.offset(d1), buf_ptr.offset(curr), 2);
                            ptr::copy_nonoverlapping(
                                lut_ptr.offset(d2),
                                buf_ptr.offset(curr + 2),
                                2,
                            );
                        }
                    }

                    // if we reach here numbers are <= 9999, so at most 4 chars long
                    let mut n = n as isize; // possibly reduce 64bit math

                    // decode 2 more chars, if > 2 chars
                    if n >= 100 {
                        let d1 = (n % 100) << 1;
                        n /= 100;
                        curr -= 2;
                        ptr::copy_nonoverlapping(lut_ptr.offset(d1), buf_ptr.offset(curr), 2);
                    }

                    // decode last 1 or 2 chars
                    if n < 10 {
                        curr -= 1;
                        *buf_ptr.offset(curr) = (n as u8) + b'0';
                    } else {
                        let d1 = n << 1;
                        curr -= 2;
                        ptr::copy_nonoverlapping(lut_ptr.offset(d1), buf_ptr.offset(curr), 2);
                    }

                    if !is_nonnegative {
                        curr -= 1;
                        *buf_ptr.offset(curr) = b'-';
                    }
                }

                // we should have moved all the way down our buffer
                debug_assert_eq!(curr, 0);

                Ok(repr)
            }
        }
    };
}

impl_IntoRepr!(u8, u32);
impl_IntoRepr!(i8, u32);
impl_IntoRepr!(u16, u32);
impl_IntoRepr!(i16, u32);
impl_IntoRepr!(u32, u32);
impl_IntoRepr!(i32, u32);
impl_IntoRepr!(u64, u64);
impl_IntoRepr!(i64, u64);

#[cfg(target_pointer_width = "32")]
impl_IntoRepr!(usize, u32);
#[cfg(target_pointer_width = "32")]
impl_IntoRepr!(isize, u32);

#[cfg(target_pointer_width = "64")]
impl_IntoRepr!(usize, u64);
#[cfg(target_pointer_width = "64")]
impl_IntoRepr!(isize, u64);

/// For 128-bit integer types we use the [`itoa`] crate because writing into a buffer, and then
/// copying the amount of characters we've written, is faster than determining the number of
/// characters and then writing.
impl IntoRepr for u128 {
    #[inline]
    fn into_repr(self) -> Result<Repr, ToCompactStringError> {
        let mut buffer = itoa::Buffer::new();
        Ok(Repr::new(buffer.format(self))?)
    }
}

impl IntoRepr for i128 {
    #[inline]
    fn into_repr(self) -> Result<Repr, ToCompactStringError> {
        let mut buffer = itoa::Buffer::new();
        Ok(Repr::new(buffer.format(self))?)
    }
}

/// Defines the implementation of [`IntoRepr`] for NonZero integer types
macro_rules! impl_NonZero_IntoRepr {
    ($t:path) => {
        impl IntoRepr for $t {
            #[inline]
            fn into_repr(self) -> Result<Repr, ToCompactStringError> {
                self.get().into_repr()
            }
        }
    };
}

impl_NonZero_IntoRepr!(num::NonZeroU8);
impl_NonZero_IntoRepr!(num::NonZeroI8);
impl_NonZero_IntoRepr!(num::NonZeroU16);
impl_NonZero_IntoRepr!(num::NonZeroI16);
impl_NonZero_IntoRepr!(num::NonZeroU32);
impl_NonZero_IntoRepr!(num::NonZeroI32);
impl_NonZero_IntoRepr!(num::NonZeroU64);
impl_NonZero_IntoRepr!(num::NonZeroI64);
impl_NonZero_IntoRepr!(num::NonZeroUsize);
impl_NonZero_IntoRepr!(num::NonZeroIsize);
impl_NonZero_IntoRepr!(num::NonZeroU128);
impl_NonZero_IntoRepr!(num::NonZeroI128);

/// All of these `num_chars(...)` methods are kind of crazy, but they are necessary.
///
/// An alternate way to calculate the number of digits in a value is to do:
/// ```
/// let val = 42;
/// let num_digits = ((val as f32).log10().floor()) as usize + 1;
/// assert_eq!(num_digits, 2);
/// ```
/// But there are two problems with this approach:
/// 1. floating point math is slow
/// 2. results are dependent on floating point precision, which is too inaccurate for larger values
///
/// For example, consider this relatively large value...
///
/// ```
/// let val = 9999995;
/// let num_digits = ((val as f32).log10().floor()) as usize + 1;
///
/// // this is wrong! There are only 7 digits in this number!
/// assert_eq!(num_digits, 8);
/// ```
///
/// you can use `f64` to get better precision, e.g.
///
/// ```
/// let val = 9999995;
/// let num_digits = ((val as f64).log10().floor()) as usize + 1;
///
/// // the precision is enough to get the correct value
/// assert_eq!(num_digits, 7);
/// ```
///
/// ...but still not precise enough!
///
/// ```
/// let val: u64 = 9999999999999999999;
/// let num_digits = ((val as f64).log10().floor()) as usize + 1;
///
/// // this is wrong! the number is only 19 digits but the formula returns 20
/// assert_eq!(num_digits, 20);
/// ```
trait NumChars {
    fn num_chars(val: Self) -> usize;
}

impl NumChars for u8 {
    #[inline(always)]
    fn num_chars(val: u8) -> usize {
        match val {
            u8::MIN..=9 => 1,
            10..=99 => 2,
            100..=u8::MAX => 3,
        }
    }
}

impl NumChars for i8 {
    #[inline(always)]
    fn num_chars(val: i8) -> usize {
        match val {
            i8::MIN..=-100 => 4,
            -99..=-10 => 3,
            -9..=-1 => 2,
            0..=9 => 1,
            10..=99 => 2,
            100..=i8::MAX => 3,
        }
    }
}

impl NumChars for u16 {
    #[inline(always)]
    fn num_chars(val: u16) -> usize {
        match val {
            u16::MIN..=9 => 1,
            10..=99 => 2,
            100..=999 => 3,
            1000..=9999 => 4,
            10000..=u16::MAX => 5,
        }
    }
}

impl NumChars for i16 {
    #[inline(always)]
    fn num_chars(val: i16) -> usize {
        match val {
            i16::MIN..=-10000 => 6,
            -9999..=-1000 => 5,
            -999..=-100 => 4,
            -99..=-10 => 3,
            -9..=-1 => 2,
            0..=9 => 1,
            10..=99 => 2,
            100..=999 => 3,
            1000..=9999 => 4,
            10000..=i16::MAX => 5,
        }
    }
}

impl NumChars for u32 {
    #[inline(always)]
    fn num_chars(val: u32) -> usize {
        match val {
            u32::MIN..=9 => 1,
            10..=99 => 2,
            100..=999 => 3,
            1000..=9999 => 4,
            10000..=99999 => 5,
            100000..=999999 => 6,
            1000000..=9999999 => 7,
            10000000..=99999999 => 8,
            100000000..=999999999 => 9,
            1000000000..=u32::MAX => 10,
        }
    }
}

impl NumChars for i32 {
    #[inline(always)]
    fn num_chars(val: i32) -> usize {
        match val {
            i32::MIN..=-1000000000 => 11,
            -999999999..=-100000000 => 10,
            -99999999..=-10000000 => 9,
            -9999999..=-1000000 => 8,
            -999999..=-100000 => 7,
            -99999..=-10000 => 6,
            -9999..=-1000 => 5,
            -999..=-100 => 4,
            -99..=-10 => 3,
            -9..=-1 => 2,
            0..=9 => 1,
            10..=99 => 2,
            100..=999 => 3,
            1000..=9999 => 4,
            10000..=99999 => 5,
            100000..=999999 => 6,
            1000000..=9999999 => 7,
            10000000..=99999999 => 8,
            100000000..=999999999 => 9,
            1000000000..=i32::MAX => 10,
        }
    }
}

impl NumChars for u64 {
    #[inline(always)]
    fn num_chars(val: u64) -> usize {
        match val {
            u64::MIN..=9 => 1,
            10..=99 => 2,
            100..=999 => 3,
            1000..=9999 => 4,
            10000..=99999 => 5,
            100000..=999999 => 6,
            1000000..=9999999 => 7,
            10000000..=99999999 => 8,
            100000000..=999999999 => 9,
            1000000000..=9999999999 => 10,
            10000000000..=99999999999 => 11,
            100000000000..=999999999999 => 12,
            1000000000000..=9999999999999 => 13,
            10000000000000..=99999999999999 => 14,
            100000000000000..=999999999999999 => 15,
            1000000000000000..=9999999999999999 => 16,
            10000000000000000..=99999999999999999 => 17,
            100000000000000000..=999999999999999999 => 18,
            1000000000000000000..=9999999999999999999 => 19,
            10000000000000000000..=u64::MAX => 20,
        }
    }
}

impl NumChars for i64 {
    #[inline(always)]
    fn num_chars(val: i64) -> usize {
        match val {
            i64::MIN..=-1000000000000000000 => 20,
            -999999999999999999..=-100000000000000000 => 19,
            -99999999999999999..=-10000000000000000 => 18,
            -9999999999999999..=-1000000000000000 => 17,
            -999999999999999..=-100000000000000 => 16,
            -99999999999999..=-10000000000000 => 15,
            -9999999999999..=-1000000000000 => 14,
            -999999999999..=-100000000000 => 13,
            -99999999999..=-10000000000 => 12,
            -9999999999..=-1000000000 => 11,
            -999999999..=-100000000 => 10,
            -99999999..=-10000000 => 9,
            -9999999..=-1000000 => 8,
            -999999..=-100000 => 7,
            -99999..=-10000 => 6,
            -9999..=-1000 => 5,
            -999..=-100 => 4,
            -99..=-10 => 3,
            -9..=-1 => 2,
            0..=9 => 1,
            10..=99 => 2,
            100..=999 => 3,
            1000..=9999 => 4,
            10000..=99999 => 5,
            100000..=999999 => 6,
            1000000..=9999999 => 7,
            10000000..=99999999 => 8,
            100000000..=999999999 => 9,
            1000000000..=9999999999 => 10,
            10000000000..=99999999999 => 11,
            100000000000..=999999999999 => 12,
            1000000000000..=9999999999999 => 13,
            10000000000000..=99999999999999 => 14,
            100000000000000..=999999999999999 => 15,
            1000000000000000..=9999999999999999 => 16,
            10000000000000000..=99999999999999999 => 17,
            100000000000000000..=999999999999999999 => 18,
            1000000000000000000..=i64::MAX => 19,
        }
    }
}

impl NumChars for usize {
    fn num_chars(val: usize) -> usize {
        #[cfg(target_pointer_width = "32")]
        {
            u32::num_chars(val as u32)
        }

        #[cfg(target_pointer_width = "64")]
        {
            u64::num_chars(val as u64)
        }
    }
}

impl NumChars for isize {
    fn num_chars(val: isize) -> usize {
        #[cfg(target_pointer_width = "32")]
        {
            i32::num_chars(val as i32)
        }

        #[cfg(target_pointer_width = "64")]
        {
            i64::num_chars(val as i64)
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::IntoRepr;

    #[test]
    fn test_from_u8_sanity() {
        let vals = [u8::MIN, u8::MIN + 1, 0, 42, u8::MAX - 1, u8::MAX];

        for x in &vals {
            let repr = u8::into_repr(*x).unwrap();
            assert_eq!(repr.as_str(), x.to_string());
        }
    }

    #[test]
    fn test_from_i8_sanity() {
        let vals = [i8::MIN, i8::MIN + 1, 0, 42, i8::MAX - 1, i8::MAX];

        for x in &vals {
            let repr = i8::into_repr(*x).unwrap();
            assert_eq!(repr.as_str(), x.to_string());
        }
    }

    #[test]
    fn test_from_u16_sanity() {
        let vals = [u16::MIN, u16::MIN + 1, 0, 42, u16::MAX - 1, u16::MAX];

        for x in &vals {
            let repr = u16::into_repr(*x).unwrap();
            assert_eq!(repr.as_str(), x.to_string());
        }
    }

    #[test]
    fn test_from_i16_sanity() {
        let vals = [i16::MIN, i16::MIN + 1, 0, 42, i16::MAX - 1, i16::MAX];

        for x in &vals {
            let repr = i16::into_repr(*x).unwrap();
            assert_eq!(repr.as_str(), x.to_string());
        }
    }

    #[test]
    fn test_from_u32_sanity() {
        let vals = [u32::MIN, u32::MIN + 1, 0, 42, u32::MAX - 1, u32::MAX];

        for x in &vals {
            let repr = u32::into_repr(*x).unwrap();
            assert_eq!(repr.as_str(), x.to_string());
        }
    }

    #[test]
    fn test_from_i32_sanity() {
        let vals = [i32::MIN, i32::MIN + 1, 0, 42, i32::MAX - 1, i32::MAX];

        for x in &vals {
            let repr = i32::into_repr(*x).unwrap();
            assert_eq!(repr.as_str(), x.to_string());
        }
    }

    #[test]
    fn test_from_u64_sanity() {
        let vals = [u64::MIN, u64::MIN + 1, 0, 42, u64::MAX - 1, u64::MAX];

        for x in &vals {
            let repr = u64::into_repr(*x).unwrap();
            assert_eq!(repr.as_str(), x.to_string());
        }
    }

    #[test]
    fn test_from_i64_sanity() {
        let vals = [i64::MIN, i64::MIN + 1, 0, 42, i64::MAX - 1, i64::MAX];

        for x in &vals {
            let repr = i64::into_repr(*x).unwrap();
            assert_eq!(repr.as_str(), x.to_string());
        }
    }

    #[test]
    fn test_from_usize_sanity() {
        let vals = [
            usize::MIN,
            usize::MIN + 1,
            0,
            42,
            usize::MAX - 1,
            usize::MAX,
        ];

        for x in &vals {
            let repr = usize::into_repr(*x).unwrap();
            assert_eq!(repr.as_str(), x.to_string());
        }
    }

    #[test]
    fn test_from_isize_sanity() {
        let vals = [
            isize::MIN,
            isize::MIN + 1,
            0,
            42,
            isize::MAX - 1,
            isize::MAX,
        ];

        for x in &vals {
            let repr = isize::into_repr(*x).unwrap();
            assert_eq!(repr.as_str(), x.to_string());
        }
    }

    #[test]
    fn test_from_u128_sanity() {
        let vals = [u128::MIN, u128::MIN + 1, 0, 42, u128::MAX - 1, u128::MAX];

        for x in &vals {
            let repr = u128::into_repr(*x).unwrap();
            assert_eq!(repr.as_str(), x.to_string());
        }
    }

    #[test]
    fn test_from_i128_sanity() {
        let vals = [i128::MIN, i128::MIN + 1, 0, 42, i128::MAX - 1, i128::MAX];

        for x in &vals {
            let repr = i128::into_repr(*x).unwrap();
            assert_eq!(repr.as_str(), x.to_string());
        }
    }
}

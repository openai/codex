use num_conv::prelude::*;

/// A trait that indicates the formatted width of the value can be determined.
///
/// Note that this should not be implemented for any signed integers. This forces the caller to
/// write the sign if desired.
pub(crate) trait DigitCount {
    /// The number of digits in the stringified value.
    fn num_digits(self) -> u8;
}

/// A macro to generate implementations of `DigitCount` for unsigned integers.
macro_rules! impl_digit_count {
    ($($t:ty),* $(,)?) => {
        $(impl DigitCount for $t {
            #[inline]
            fn num_digits(self) -> u8 {
                match self.checked_ilog10() {
                    Some(n) => n.truncate::<u8>() + 1,
                    None => 1,
                }
            }
        })*
    };
}

impl_digit_count!(u8, u16, u32);

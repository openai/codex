use crate::Integer;
use core::ops::{BitAnd, BitOr, BitXor, Shr};

/// Provides methods to compute the average of two integers, without overflows.
pub trait Average: Integer {
    /// Returns the ceiling value of the average of `self` and `other`.
    /// -- `⌈(self + other)/2⌉`
    ///
    /// # Examples
    ///
    /// ```
    /// use num_integer::Average;
    ///
    /// assert_eq!(( 3).average_ceil(&10),  7);
    /// assert_eq!((-2).average_ceil(&-5), -3);
    /// assert_eq!(( 4).average_ceil(& 4),  4);
    ///
    /// assert_eq!(u8::max_value().average_ceil(&2), 129);
    /// assert_eq!(i8::min_value().average_ceil(&-1), -64);
    /// assert_eq!(i8::min_value().average_ceil(&i8::max_value()), 0);
    /// ```
    ///
    fn average_ceil(&self, other: &Self) -> Self;

    /// Returns the floor value of the average of `self` and `other`.
    /// -- `⌊(self + other)/2⌋`
    ///
    /// # Examples
    ///
    /// ```
    /// use num_integer::Average;
    ///
    /// assert_eq!(( 3).average_floor(&10),  6);
    /// assert_eq!((-2).average_floor(&-5), -4);
    /// assert_eq!(( 4).average_floor(& 4),  4);
    ///
    /// assert_eq!(u8::max_value().average_floor(&2), 128);
    /// assert_eq!(i8::min_value().average_floor(&-1), -65);
    /// assert_eq!(i8::min_value().average_floor(&i8::max_value()), -1);
    /// ```
    ///
    fn average_floor(&self, other: &Self) -> Self;
}

impl<I> Average for I
where
    I: Integer + Shr<usize, Output = I>,
    for<'a, 'b> &'a I:
        BitAnd<&'b I, Output = I> + BitOr<&'b I, Output = I> + BitXor<&'b I, Output = I>,
{
    // The Henry Gordon Dietz implementation as shown in the Hacker's Delight,
    // see http://aggregate.org/MAGIC/#Average%20of%20Integers

    /// Returns the floor value of the average of `self` and `other`.
    #[inline]
    fn average_floor(&self, other: &I) -> I {
        (self & other) + ((self ^ other) >> 1)
    }

    /// Returns the ceil value of the average of `self` and `other`.
    #[inline]
    fn average_ceil(&self, other: &I) -> I {
        (self | other) - ((self ^ other) >> 1)
    }
}

/// Returns the floor value of the average of `x` and `y` --
/// see [Average::average_floor](trait.Average.html#tymethod.average_floor).
#[inline]
pub fn average_floor<T: Average>(x: T, y: T) -> T {
    x.average_floor(&y)
}
/// Returns the ceiling value of the average of `x` and `y` --
/// see [Average::average_ceil](trait.Average.html#tymethod.average_ceil).
#[inline]
pub fn average_ceil<T: Average>(x: T, y: T) -> T {
    x.average_ceil(&y)
}

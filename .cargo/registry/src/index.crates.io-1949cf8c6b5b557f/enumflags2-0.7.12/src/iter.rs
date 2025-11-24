use crate::{BitFlag, BitFlags, BitFlagNum};
use core::iter::{FromIterator, FusedIterator};

impl<T> BitFlags<T>
where
    T: BitFlag,
{
    /// Iterate over the `BitFlags`.
    ///
    /// ```
    /// # use enumflags2::{bitflags, make_bitflags};
    /// # #[bitflags]
    /// # #[derive(Clone, Copy, PartialEq, Debug)]
    /// # #[repr(u8)]
    /// # enum MyFlag {
    /// #     A = 1 << 0,
    /// #     B = 1 << 1,
    /// #     C = 1 << 2,
    /// # }
    /// let flags = make_bitflags!(MyFlag::{A | C});
    ///
    /// flags.iter()
    ///     .for_each(|flag| println!("{:?}", flag));
    /// ```
    #[inline]
    pub fn iter(self) -> Iter<T> {
        Iter { rest: self }
    }
}

impl<T: BitFlag> IntoIterator for BitFlags<T> {
    type IntoIter = Iter<T>;
    type Item = T;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator that yields each flag set in a `BitFlags`.
#[derive(Clone, Debug)]
pub struct Iter<T: BitFlag> {
    rest: BitFlags<T>,
}

impl<T> Iterator for Iter<T>
where
    T: BitFlag,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.rest.is_empty() {
            None
        } else {
            // SAFETY: `flag` will be a single bit, because
            // x & -x = x & (~x + 1), and the increment causes only one 0 -> 1 transition.
            // The invariant of `from_bits_unchecked` is satisfied, because bits & x
            // is a subset of bits, which we know are the valid bits.
            unsafe {
                let bits = self.rest.bits();
                let flag: T::Numeric = bits & bits.wrapping_neg();
                let flag: T = core::mem::transmute_copy(&flag);
                self.rest = BitFlags::from_bits_unchecked(bits & (bits - BitFlagNum::ONE));
                Some(flag)
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let l = self.rest.len();
        (l, Some(l))
    }
}

impl<T> ExactSizeIterator for Iter<T>
where
    T: BitFlag,
{
    fn len(&self) -> usize {
        self.rest.len()
    }
}

impl<T: BitFlag> FusedIterator for Iter<T> {}

impl<T, B> FromIterator<B> for BitFlags<T>
where
    T: BitFlag,
    B: Into<BitFlags<T>>,
{
    #[inline]
    fn from_iter<I>(it: I) -> BitFlags<T>
    where
        I: IntoIterator<Item = B>,
    {
        it.into_iter()
            .fold(BitFlags::empty(), |acc, flag| acc | flag)
    }
}

impl<T, B> Extend<B> for BitFlags<T>
where
    T: BitFlag,
    B: Into<BitFlags<T>>,
{
    #[inline]
    fn extend<I>(&mut self, it: I)
    where
        I: IntoIterator<Item = B>,
    {
        *self = it.into_iter().fold(*self, |acc, flag| acc | flag)
    }
}

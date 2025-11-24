// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::LengthHint;

impl core::ops::Add<LengthHint> for LengthHint {
    type Output = Self;

    fn add(self, other: LengthHint) -> Self {
        LengthHint(
            self.0.saturating_add(other.0),
            match (self.1, other.1) {
                (Some(c), Some(d)) => c.checked_add(d),
                _ => None,
            },
        )
    }
}

impl core::ops::AddAssign<LengthHint> for LengthHint {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl core::iter::Sum<LengthHint> for LengthHint {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = LengthHint>,
    {
        iter.fold(LengthHint::exact(0), core::ops::Add::add)
    }
}

impl core::ops::Add<usize> for LengthHint {
    type Output = Self;

    fn add(self, other: usize) -> Self {
        Self(
            self.0.saturating_add(other),
            self.1.and_then(|upper| upper.checked_add(other)),
        )
    }
}

impl core::ops::AddAssign<usize> for LengthHint {
    fn add_assign(&mut self, other: usize) {
        *self = *self + other;
    }
}

impl core::ops::Mul<usize> for LengthHint {
    type Output = Self;

    fn mul(self, other: usize) -> Self {
        Self(
            self.0.saturating_mul(other),
            self.1.and_then(|upper| upper.checked_mul(other)),
        )
    }
}

impl core::ops::MulAssign<usize> for LengthHint {
    fn mul_assign(&mut self, other: usize) {
        *self = *self * other;
    }
}

impl core::ops::BitOr<LengthHint> for LengthHint {
    type Output = Self;

    /// Returns a new hint that is correct wherever `self` is correct, and wherever
    /// `other` is correct.
    ///
    /// Example:
    /// ```
    /// # use writeable::{LengthHint, Writeable};
    /// # use core::fmt;
    /// # fn coin_flip() -> bool { true }
    ///
    /// struct NonDeterministicWriteable(String, String);
    ///
    /// impl Writeable for NonDeterministicWriteable {
    ///     fn write_to<W: fmt::Write + ?Sized>(
    ///         &self,
    ///         sink: &mut W,
    ///     ) -> fmt::Result {
    ///         sink.write_str(if coin_flip() { &self.0 } else { &self.1 })
    ///     }
    ///
    ///     fn writeable_length_hint(&self) -> LengthHint {
    ///         LengthHint::exact(self.0.len()) | LengthHint::exact(self.1.len())
    ///     }
    /// }
    ///
    /// writeable::impl_display_with_writeable!(NonDeterministicWriteable);
    /// ```
    fn bitor(self, other: LengthHint) -> Self {
        LengthHint(
            Ord::min(self.0, other.0),
            match (self.1, other.1) {
                (Some(c), Some(d)) => Some(Ord::max(c, d)),
                _ => None,
            },
        )
    }
}

impl core::ops::BitOrAssign<LengthHint> for LengthHint {
    fn bitor_assign(&mut self, other: Self) {
        *self = *self | other;
    }
}

impl core::iter::Sum<usize> for LengthHint {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = usize>,
    {
        LengthHint::exact(iter.sum::<usize>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(LengthHint::exact(3) + 2, LengthHint::exact(5));
        assert_eq!(
            LengthHint::exact(3) + LengthHint::exact(2),
            LengthHint::exact(5)
        );
        assert_eq!(
            LengthHint::exact(3) + LengthHint::undefined(),
            LengthHint::at_least(3)
        );

        assert_eq!(LengthHint::undefined() + 2, LengthHint::at_least(2));
        assert_eq!(
            LengthHint::undefined() + LengthHint::exact(2),
            LengthHint::at_least(2)
        );
        assert_eq!(
            LengthHint::undefined() + LengthHint::undefined(),
            LengthHint::undefined()
        );

        assert_eq!(
            LengthHint::at_least(15) + LengthHint::exact(3),
            LengthHint::at_least(18)
        );

        assert_eq!(
            LengthHint::at_least(15) + LengthHint::at_most(3),
            LengthHint::at_least(15)
        );

        assert_eq!(LengthHint::between(48, 92) + 5, LengthHint::between(53, 97));

        let mut len = LengthHint::exact(5);
        len += LengthHint::exact(3);
        assert_eq!(len, LengthHint::exact(8));
        len += 2;
        assert_eq!(len, LengthHint::exact(10));
        len += LengthHint::undefined();
        assert_eq!(len, LengthHint::at_least(10));

        len += LengthHint::exact(3);
        assert_eq!(len, LengthHint::at_least(13));
        len += 2;
        assert_eq!(len, LengthHint::at_least(15));
        len += LengthHint::undefined();
        assert_eq!(len, LengthHint::at_least(15));

        assert_eq!(
            LengthHint::between(usize::MAX - 10, usize::MAX - 5) + LengthHint::exact(20),
            LengthHint::at_least(usize::MAX)
        );
    }

    #[test]
    fn test_sum() {
        let lens = [
            LengthHint::exact(4),
            LengthHint::exact(1),
            LengthHint::exact(1),
        ];
        assert_eq!(
            lens.iter().copied().sum::<LengthHint>(),
            LengthHint::exact(6)
        );

        let lens = [
            LengthHint::exact(4),
            LengthHint::undefined(),
            LengthHint::at_least(1),
        ];
        assert_eq!(
            lens.iter().copied().sum::<LengthHint>(),
            LengthHint::at_least(5)
        );

        let lens = [
            LengthHint::exact(4),
            LengthHint::undefined(),
            LengthHint::at_most(1),
        ];
        assert_eq!(
            lens.iter().copied().sum::<LengthHint>(),
            LengthHint::at_least(4)
        );

        let lens = [4, 1, 1];
        assert_eq!(
            lens.iter().copied().sum::<LengthHint>(),
            LengthHint::exact(6)
        );
    }

    #[test]
    fn test_mul() {
        assert_eq!(LengthHint::exact(3) * 2, LengthHint::exact(6));

        assert_eq!(LengthHint::undefined() * 2, LengthHint::undefined());

        assert_eq!(
            LengthHint::between(48, 92) * 2,
            LengthHint::between(96, 184)
        );

        let mut len = LengthHint::exact(5);
        len *= 2;
        assert_eq!(len, LengthHint::exact(10));

        assert_eq!(
            LengthHint::between(usize::MAX - 10, usize::MAX - 5) * 2,
            LengthHint::at_least(usize::MAX)
        );
    }

    #[test]
    fn test_bitor() {
        assert_eq!(
            LengthHint::exact(3) | LengthHint::exact(2),
            LengthHint::between(2, 3)
        );
        assert_eq!(
            LengthHint::exact(3) | LengthHint::undefined(),
            LengthHint::undefined()
        );

        assert_eq!(
            LengthHint::undefined() | LengthHint::undefined(),
            LengthHint::undefined()
        );

        assert_eq!(
            LengthHint::exact(10) | LengthHint::exact(10),
            LengthHint::exact(10)
        );

        assert_eq!(
            LengthHint::at_least(15) | LengthHint::exact(3),
            LengthHint::at_least(3)
        );

        assert_eq!(
            LengthHint::at_least(15) | LengthHint::at_most(18),
            LengthHint::undefined()
        );

        assert_eq!(
            LengthHint::at_least(15) | LengthHint::at_least(18),
            LengthHint::at_least(15)
        );

        assert_eq!(
            LengthHint::at_most(15) | LengthHint::at_most(18),
            LengthHint::at_most(18)
        );

        assert_eq!(
            LengthHint::between(5, 10) | LengthHint::at_most(3),
            LengthHint::at_most(10)
        );

        let mut len = LengthHint::exact(5);
        len |= LengthHint::exact(3);
        assert_eq!(len, LengthHint::between(5, 3));
    }
}

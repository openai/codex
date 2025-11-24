//! Type-level bits.
//!
//! These are rather simple and are used as the building blocks of the
//! other number types in this crate.
//!
//!
//! **Type operators** implemented:
//!
//! - From `core::ops`: `BitAnd`, `BitOr`, `BitXor`, and `Not`.
//! - From `typenum`: `Same` and `Cmp`.

use crate::{private::InternalMarker, Cmp, Equal, Greater, Less, NonZero, PowerOfTwo, Zero};
use core::ops::{BitAnd, BitOr, BitXor, Not};

pub use crate::marker_traits::Bit;

/// The type-level bit 0.
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, Debug, Default)]
#[cfg_attr(feature = "scale_info", derive(scale_info::TypeInfo))]
pub struct B0;

impl B0 {
    /// Instantiates a singleton representing this bit.
    #[inline]
    pub fn new() -> B0 {
        B0
    }
}

/// The type-level bit 1.
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, Debug, Default)]
#[cfg_attr(feature = "scale_info", derive(scale_info::TypeInfo))]
pub struct B1;

impl B1 {
    /// Instantiates a singleton representing this bit.
    #[inline]
    pub fn new() -> B1 {
        B1
    }
}

impl Bit for B0 {
    const U8: u8 = 0;
    const BOOL: bool = false;

    #[inline]
    fn new() -> Self {
        Self
    }
    #[inline]
    fn to_u8() -> u8 {
        0
    }
    #[inline]
    fn to_bool() -> bool {
        false
    }
}

impl Bit for B1 {
    const U8: u8 = 1;
    const BOOL: bool = true;

    #[inline]
    fn new() -> Self {
        Self
    }
    #[inline]
    fn to_u8() -> u8 {
        1
    }
    #[inline]
    fn to_bool() -> bool {
        true
    }
}

impl Zero for B0 {}
impl NonZero for B1 {}
impl PowerOfTwo for B1 {}

/// Not of 0 (!0 = 1)
impl Not for B0 {
    type Output = B1;
    #[inline]
    fn not(self) -> Self::Output {
        B1
    }
}
/// Not of 1 (!1 = 0)
impl Not for B1 {
    type Output = B0;
    #[inline]
    fn not(self) -> Self::Output {
        B0
    }
}

/// And with 0 ( 0 & B = 0)
impl<Rhs: Bit> BitAnd<Rhs> for B0 {
    type Output = B0;
    #[inline]
    fn bitand(self, _: Rhs) -> Self::Output {
        B0
    }
}

/// And with 1 ( 1 & 0 = 0)
impl BitAnd<B0> for B1 {
    type Output = B0;
    #[inline]
    fn bitand(self, _: B0) -> Self::Output {
        B0
    }
}

/// And with 1 ( 1 & 1 = 1)
impl BitAnd<B1> for B1 {
    type Output = B1;
    #[inline]
    fn bitand(self, _: B1) -> Self::Output {
        B1
    }
}

/// Or with 0 ( 0 | 0 = 0)
impl BitOr<B0> for B0 {
    type Output = B0;
    #[inline]
    fn bitor(self, _: B0) -> Self::Output {
        B0
    }
}

/// Or with 0 ( 0 | 1 = 1)
impl BitOr<B1> for B0 {
    type Output = B1;
    #[inline]
    fn bitor(self, _: B1) -> Self::Output {
        B1
    }
}

/// Or with 1 ( 1 | B = 1)
impl<Rhs: Bit> BitOr<Rhs> for B1 {
    type Output = B1;
    #[inline]
    fn bitor(self, _: Rhs) -> Self::Output {
        B1
    }
}

/// Xor between 0 and 0 ( 0 ^ 0 = 0)
impl BitXor<B0> for B0 {
    type Output = B0;
    #[inline]
    fn bitxor(self, _: B0) -> Self::Output {
        B0
    }
}
/// Xor between 1 and 0 ( 1 ^ 0 = 1)
impl BitXor<B0> for B1 {
    type Output = B1;
    #[inline]
    fn bitxor(self, _: B0) -> Self::Output {
        B1
    }
}
/// Xor between 0 and 1 ( 0 ^ 1 = 1)
impl BitXor<B1> for B0 {
    type Output = B1;
    #[inline]
    fn bitxor(self, _: B1) -> Self::Output {
        B1
    }
}
/// Xor between 1 and 1 ( 1 ^ 1 = 0)
impl BitXor<B1> for B1 {
    type Output = B0;
    #[inline]
    fn bitxor(self, _: B1) -> Self::Output {
        B0
    }
}

#[cfg(test)]
mod bit_op_tests {
    use core::ops::{BitAnd, BitOr, BitXor, Not};

    use crate::{B0, B1};

    // macro for testing operation results. Uses `Same` to ensure the types are equal and
    // not just the values they evaluate to.
    macro_rules! test_bit_op {
        ($op:ident $Lhs:ident = $Answer:ident) => {{
            type Test = <<$Lhs as $op>::Output as $crate::Same<$Answer>>::Output;
            assert_eq!(
                <$Answer as $crate::Bit>::to_u8(),
                <Test as $crate::Bit>::to_u8()
            );
        }};
        ($Lhs:ident $op:ident $Rhs:ident = $Answer:ident) => {{
            type Test = <<$Lhs as $op<$Rhs>>::Output as $crate::Same<$Answer>>::Output;
            assert_eq!(
                <$Answer as $crate::Bit>::to_u8(),
                <Test as $crate::Bit>::to_u8()
            );
        }};
    }

    #[test]
    fn bit_operations() {
        test_bit_op!(Not B0 = B1);
        test_bit_op!(Not B1 = B0);

        test_bit_op!(B0 BitAnd B0 = B0);
        test_bit_op!(B0 BitAnd B1 = B0);
        test_bit_op!(B1 BitAnd B0 = B0);
        test_bit_op!(B1 BitAnd B1 = B1);

        test_bit_op!(B0 BitOr B0 = B0);
        test_bit_op!(B0 BitOr B1 = B1);
        test_bit_op!(B1 BitOr B0 = B1);
        test_bit_op!(B1 BitOr B1 = B1);

        test_bit_op!(B0 BitXor B0 = B0);
        test_bit_op!(B0 BitXor B1 = B1);
        test_bit_op!(B1 BitXor B0 = B1);
        test_bit_op!(B1 BitXor B1 = B0);
    }
}

impl Cmp<B0> for B0 {
    type Output = Equal;

    #[inline]
    fn compare<P: InternalMarker>(&self, _: &B0) -> Self::Output {
        Equal
    }
}

impl Cmp<B1> for B0 {
    type Output = Less;

    #[inline]
    fn compare<P: InternalMarker>(&self, _: &B1) -> Self::Output {
        Less
    }
}

impl Cmp<B0> for B1 {
    type Output = Greater;

    #[inline]
    fn compare<P: InternalMarker>(&self, _: &B0) -> Self::Output {
        Greater
    }
}

impl Cmp<B1> for B1 {
    type Output = Equal;

    #[inline]
    fn compare<P: InternalMarker>(&self, _: &B1) -> Self::Output {
        Equal
    }
}

use crate::Min;
impl Min<B0> for B0 {
    type Output = B0;
    #[inline]
    fn min(self, _: B0) -> B0 {
        self
    }
}
impl Min<B1> for B0 {
    type Output = B0;
    #[inline]
    fn min(self, _: B1) -> B0 {
        self
    }
}
impl Min<B0> for B1 {
    type Output = B0;
    #[inline]
    fn min(self, rhs: B0) -> B0 {
        rhs
    }
}
impl Min<B1> for B1 {
    type Output = B1;
    #[inline]
    fn min(self, _: B1) -> B1 {
        self
    }
}

use crate::Max;
impl Max<B0> for B0 {
    type Output = B0;
    #[inline]
    fn max(self, _: B0) -> B0 {
        self
    }
}
impl Max<B1> for B0 {
    type Output = B1;
    #[inline]
    fn max(self, rhs: B1) -> B1 {
        rhs
    }
}
impl Max<B0> for B1 {
    type Output = B1;
    #[inline]
    fn max(self, _: B0) -> B1 {
        self
    }
}
impl Max<B1> for B1 {
    type Output = B1;
    #[inline]
    fn max(self, _: B1) -> B1 {
        self
    }
}

#[cfg(test)]
mod bit_creation_tests {
    #[test]
    fn bit_creation() {
        {
            use crate::{B0, B1};
            let _: B0 = B0::new();
            let _: B1 = B1::new();
        }

        {
            use crate::{Bit, B0, B1};

            let _: B0 = <B0 as Bit>::new();
            let _: B1 = <B1 as Bit>::new();
        }
    }
}

#![cfg(any(feature = "quickcheck", feature = "arbitrary"))]

use super::{BigInt, Sign};
use crate::BigUint;

#[cfg(feature = "quickcheck")]
use alloc::boxed::Box;

#[cfg(feature = "quickcheck")]
#[cfg_attr(docsrs, doc(cfg(feature = "quickcheck")))]
impl quickcheck::Arbitrary for BigInt {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        let positive = bool::arbitrary(g);
        let sign = if positive { Sign::Plus } else { Sign::Minus };
        Self::from_biguint(sign, BigUint::arbitrary(g))
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
        let sign = self.sign();
        let unsigned_shrink = self.data.shrink();
        Box::new(unsigned_shrink.map(move |x| BigInt::from_biguint(sign, x)))
    }
}

#[cfg(feature = "arbitrary")]
#[cfg_attr(docsrs, doc(cfg(feature = "arbitrary")))]
impl arbitrary::Arbitrary<'_> for BigInt {
    fn arbitrary(u: &mut arbitrary::Unstructured<'_>) -> arbitrary::Result<Self> {
        let positive = bool::arbitrary(u)?;
        let sign = if positive { Sign::Plus } else { Sign::Minus };
        Ok(Self::from_biguint(sign, BigUint::arbitrary(u)?))
    }

    fn arbitrary_take_rest(mut u: arbitrary::Unstructured<'_>) -> arbitrary::Result<Self> {
        let positive = bool::arbitrary(&mut u)?;
        let sign = if positive { Sign::Plus } else { Sign::Minus };
        Ok(Self::from_biguint(sign, BigUint::arbitrary_take_rest(u)?))
    }

    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        arbitrary::size_hint::and(bool::size_hint(depth), BigUint::size_hint(depth))
    }
}

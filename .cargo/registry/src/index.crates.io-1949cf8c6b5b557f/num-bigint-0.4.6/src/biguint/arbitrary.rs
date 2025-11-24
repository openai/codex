#![cfg(any(feature = "quickcheck", feature = "arbitrary"))]

use super::{biguint_from_vec, BigUint};

use crate::big_digit::BigDigit;
#[cfg(feature = "quickcheck")]
use alloc::boxed::Box;
use alloc::vec::Vec;

#[cfg(feature = "quickcheck")]
#[cfg_attr(docsrs, doc(cfg(feature = "quickcheck")))]
impl quickcheck::Arbitrary for BigUint {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        // Use arbitrary from Vec
        biguint_from_vec(Vec::<BigDigit>::arbitrary(g))
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
        // Use shrinker from Vec
        Box::new(self.data.shrink().map(biguint_from_vec))
    }
}

#[cfg(feature = "arbitrary")]
#[cfg_attr(docsrs, doc(cfg(feature = "arbitrary")))]
impl arbitrary::Arbitrary<'_> for BigUint {
    fn arbitrary(u: &mut arbitrary::Unstructured<'_>) -> arbitrary::Result<Self> {
        Ok(biguint_from_vec(Vec::<BigDigit>::arbitrary(u)?))
    }

    fn arbitrary_take_rest(u: arbitrary::Unstructured<'_>) -> arbitrary::Result<Self> {
        Ok(biguint_from_vec(Vec::<BigDigit>::arbitrary_take_rest(u)?))
    }

    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        Vec::<BigDigit>::size_hint(depth)
    }
}

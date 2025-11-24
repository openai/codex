//! Namespace containing the 2-word `Cow` implementation.

use crate::traits::Capacity;

/// Faster, 2-word `Cow`. This version is available only on 64-bit architecture,
/// and it puts both capacity and length together in a fat pointer. Both length and capacity
/// is limited to 32 bits.
///
/// # Panics
///
/// [`Cow::owned`](../generic/struct.Cow.html#method.owned) will panic if capacity is larger than `u32::max_size()`. Use the
/// top level `beef::Cow` if you wish to avoid this problem.
pub type Cow<'a, T> = crate::generic::Cow<'a, T, Lean>;

pub(crate) mod internal {
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct Lean;
}
use internal::Lean;

const MASK_LO: usize = u32::MAX as usize;
const MASK_HI: usize = !MASK_LO;

impl Lean {
    #[inline]
    pub const fn mask_len(len: usize) -> usize {
        len & MASK_LO
    }
}

impl Capacity for Lean {
    type Field = Lean;
    type NonZero = Lean;

    #[inline]
    fn len(fat: usize) -> usize {
        fat & MASK_LO
    }

    #[inline]
    fn empty(len: usize) -> (usize, Lean) {
        (len & MASK_LO, Lean)
    }

    #[inline]
    fn store(len: usize, capacity: usize) -> (usize, Lean) {
        if capacity & MASK_HI != 0 {
            panic!("beef::lean::Cow: Capacity out of bounds");
        }

        let fat = ((capacity & MASK_LO) << 32) | (len & MASK_LO);

        (fat, Lean)
    }

    #[inline]
    fn unpack(fat: usize, _: Lean) -> (usize, usize) {
        (fat & MASK_LO, (fat & MASK_HI) >> 32)
    }

    #[inline]
    fn maybe(fat: usize, _: Lean) -> Option<Lean> {
        if fat & MASK_HI != 0 {
            Some(Lean)
        } else {
            None
        }
    }
}

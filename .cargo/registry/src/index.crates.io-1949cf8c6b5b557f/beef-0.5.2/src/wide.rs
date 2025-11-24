use crate::traits::Capacity;
use core::num::NonZeroUsize;

/// Compact three word `Cow` that puts the ownership tag in capacity.
/// This is a type alias, for documentation see [`beef::generic::Cow`](./generic/struct.Cow.html).
pub type Cow<'a, T> = crate::generic::Cow<'a, T, Wide>;

pub(crate) mod internal {
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct Wide;
}
use internal::Wide;

impl Capacity for Wide {
    type Field = Option<NonZeroUsize>;
    type NonZero = NonZeroUsize;

    #[inline]
    fn len(fat: usize) -> usize {
        fat
    }

    #[inline]
    fn empty(len: usize) -> (usize, Self::Field) {
        (len, None)
    }

    #[inline]
    fn store(len: usize, capacity: usize) -> (usize, Self::Field) {
        (len, NonZeroUsize::new(capacity))
    }

    #[inline]
    fn unpack(fat: usize, capacity: NonZeroUsize) -> (usize, usize) {
        (fat, capacity.get())
    }

    #[inline]
    fn maybe(_: usize, capacity: Option<NonZeroUsize>) -> Option<NonZeroUsize> {
        capacity
    }
}

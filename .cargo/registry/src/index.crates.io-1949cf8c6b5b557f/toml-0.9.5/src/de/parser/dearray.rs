use serde_spanned::Spanned;

use crate::alloc_prelude::*;
use crate::de::DeValue;

/// Type representing a TOML array, payload of the `DeValue::Array` variant
#[derive(Clone)]
pub struct DeArray<'i> {
    items: Vec<Spanned<DeValue<'i>>>,
    array_of_tables: bool,
}

impl<'i> DeArray<'i> {
    /// Constructs a new, empty `DeArray`.
    ///
    /// This will not allocate until elements are pushed onto it.
    pub const fn new() -> Self {
        Self {
            items: Vec::new(),
            array_of_tables: false,
        }
    }

    /// Appends an element to the back of a collection.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity exceeds `isize::MAX` _bytes_.
    pub fn push(&mut self, value: Spanned<DeValue<'i>>) {
        self.items.push(value);
    }
}

impl DeArray<'_> {
    pub(crate) fn is_array_of_tables(&self) -> bool {
        self.array_of_tables
    }

    pub(crate) fn set_array_of_tables(&mut self, yes: bool) {
        self.array_of_tables = yes;
    }
}

impl<'i> core::ops::Deref for DeArray<'i> {
    type Target = [Spanned<DeValue<'i>>];

    #[inline]
    fn deref(&self) -> &[Spanned<DeValue<'i>>] {
        self.items.as_slice()
    }
}

impl<'i> core::ops::DerefMut for DeArray<'i> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [Spanned<DeValue<'i>>] {
        self.items.as_mut_slice()
    }
}

impl<'i> AsRef<[Spanned<DeValue<'i>>]> for DeArray<'i> {
    fn as_ref(&self) -> &[Spanned<DeValue<'i>>] {
        &self.items
    }
}

impl<'i> AsMut<[Spanned<DeValue<'i>>]> for DeArray<'i> {
    fn as_mut(&mut self) -> &mut [Spanned<DeValue<'i>>] {
        &mut self.items
    }
}

impl<'i> core::borrow::Borrow<[Spanned<DeValue<'i>>]> for DeArray<'i> {
    fn borrow(&self) -> &[Spanned<DeValue<'i>>] {
        &self.items[..]
    }
}

impl<'i> core::borrow::BorrowMut<[Spanned<DeValue<'i>>]> for DeArray<'i> {
    fn borrow_mut(&mut self) -> &mut [Spanned<DeValue<'i>>] {
        &mut self.items[..]
    }
}

impl<'i, I: core::slice::SliceIndex<[Spanned<DeValue<'i>>]>> core::ops::Index<I> for DeArray<'i> {
    type Output = I::Output;

    #[inline]
    fn index(&self, index: I) -> &Self::Output {
        self.items.index(index)
    }
}

impl<'a, 'i> IntoIterator for &'a DeArray<'i> {
    type Item = &'a Spanned<DeValue<'i>>;

    type IntoIter = core::slice::Iter<'a, Spanned<DeValue<'i>>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'i> IntoIterator for DeArray<'i> {
    type Item = Spanned<DeValue<'i>>;

    type IntoIter = alloc::vec::IntoIter<Spanned<DeValue<'i>>>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'i> FromIterator<Spanned<DeValue<'i>>> for DeArray<'i> {
    #[inline]
    #[track_caller]
    fn from_iter<I: IntoIterator<Item = Spanned<DeValue<'i>>>>(iter: I) -> Self {
        Self {
            items: iter.into_iter().collect(),
            array_of_tables: false,
        }
    }
}

impl Default for DeArray<'static> {
    #[inline]
    fn default() -> Self {
        Self {
            items: Default::default(),
            array_of_tables: false,
        }
    }
}

impl core::fmt::Debug for DeArray<'_> {
    #[inline]
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.items.fmt(formatter)
    }
}

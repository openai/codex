// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! This module includes variable-length data types that are const-constructible for single
//! values and overflow to the heap.
//!
//! # Why?
//!
//! This module is far from the first stack-or-heap vector in the Rust ecosystem. It was created
//! with the following value proposition:
//!
//! 1. Enable safe const construction of stack collections.
//! 2. Avoid stack size penalties common with stack-or-heap collections.
//!
//! As of this writing, `heapless` and `tinyvec` don't support const construction except
//! for empty vectors, and `smallvec` supports it on unstable.
//!
//! Additionally, [`ShortBoxSlice`] has a smaller stack size than any of these:
//!
//! ```ignore
//! use core::mem::size_of;
//!
//! // NonZeroU64 has a niche that this module utilizes
//! use core::num::NonZeroU64;
//!
//! // ShortBoxSlice is the same size as `Box<[]>` for small or nichey values
//! assert_eq!(16, size_of::<shortvec::ShortBoxSlice::<NonZeroU64>>());
//!
//! // Note: SmallVec supports pushing and therefore has a capacity field
//! assert_eq!(24, size_of::<smallvec::SmallVec::<[NonZeroU64; 1]>>());
//!
//! // Note: heapless doesn't support spilling to the heap
//! assert_eq!(16, size_of::<heapless::Vec::<NonZeroU64, 1>>());
//!
//! // Note: TinyVec only supports types that implement `Default`
//! assert_eq!(24, size_of::<tinyvec::TinyVec::<[u64; 1]>>());
//! ```
//!
//! The module is `no_std` with `alloc`.

mod litemap;

#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::vec;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::ops::Deref;
use core::ops::DerefMut;

/// A boxed slice that supports no-allocation, constant values if length 0 or 1.
/// Using ZeroOne(Option<T>) saves 8 bytes in ShortBoxSlice via niche optimization.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ShortBoxSliceInner<T> {
    ZeroOne(Option<T>),
    #[cfg(feature = "alloc")]
    Multi(Box<[T]>),
    #[cfg(not(feature = "alloc"))]
    Two([T; 2]),
}

impl<T> Default for ShortBoxSliceInner<T> {
    fn default() -> Self {
        use ShortBoxSliceInner::*;
        ZeroOne(None)
    }
}

/// A boxed slice that supports no-allocation, constant values if length 0 or 1.
///
/// Supports mutation but always reallocs when mutated.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ShortBoxSlice<T>(ShortBoxSliceInner<T>);

impl<T> Default for ShortBoxSlice<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T> ShortBoxSlice<T> {
    /// Creates a new, empty [`ShortBoxSlice`].
    #[inline]
    pub const fn new() -> Self {
        use ShortBoxSliceInner::*;
        Self(ZeroOne(None))
    }

    /// Creates a new [`ShortBoxSlice`] containing a single element.
    #[inline]
    pub const fn new_single(item: T) -> Self {
        use ShortBoxSliceInner::*;
        Self(ZeroOne(Some(item)))
    }

    pub fn new_double(first: T, second: T) -> Self {
        use ShortBoxSliceInner::*;
        #[cfg(feature = "alloc")]
        return Self(Multi(vec![first, second].into_boxed_slice()));
        #[cfg(not(feature = "alloc"))]
        return Self(Two([first, second]));
    }

    /// Pushes an element onto this [`ShortBoxSlice`].
    ///
    /// Reallocs if more than 1 item is already in the collection.
    #[cfg(feature = "alloc")]
    pub fn push(&mut self, item: T) {
        use ShortBoxSliceInner::*;
        self.0 = match core::mem::replace(&mut self.0, ZeroOne(None)) {
            ZeroOne(None) => ZeroOne(Some(item)),
            ZeroOne(Some(prev_item)) => Multi(vec![prev_item, item].into_boxed_slice()),
            Multi(items) => {
                let mut items = items.into_vec();
                items.push(item);
                Multi(items.into_boxed_slice())
            }
        };
    }

    /// Gets a single element from the [`ShortBoxSlice`].
    ///
    /// Returns `None` if empty or more than one element.
    #[inline]
    pub const fn single(&self) -> Option<&T> {
        use ShortBoxSliceInner::*;
        match self.0 {
            ZeroOne(Some(ref v)) => Some(v),
            _ => None,
        }
    }

    /// Destruct into a single element of the [`ShortBoxSlice`].
    ///
    /// Returns `None` if empty or more than one element.
    pub fn into_single(self) -> Option<T> {
        use ShortBoxSliceInner::*;
        match self.0 {
            ZeroOne(Some(v)) => Some(v),
            _ => None,
        }
    }

    /// Returns the number of elements in the collection.
    #[inline]
    pub fn len(&self) -> usize {
        use ShortBoxSliceInner::*;
        match self.0 {
            ZeroOne(None) => 0,
            ZeroOne(_) => 1,
            #[cfg(feature = "alloc")]
            Multi(ref v) => v.len(),
            #[cfg(not(feature = "alloc"))]
            Two(_) => 2,
        }
    }

    /// Returns whether the collection is empty.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        use ShortBoxSliceInner::*;
        matches!(self.0, ZeroOne(None))
    }

    /// Inserts an element at the specified index into the collection.
    ///
    /// Reallocs if more than 1 item is already in the collection.
    #[cfg(feature = "alloc")]
    pub fn insert(&mut self, index: usize, elt: T) {
        use ShortBoxSliceInner::*;
        assert!(
            index <= self.len(),
            "insertion index (is {}) should be <= len (is {})",
            index,
            self.len()
        );

        self.0 = match core::mem::replace(&mut self.0, ZeroOne(None)) {
            ZeroOne(None) => ZeroOne(Some(elt)),
            ZeroOne(Some(item)) => {
                let items = if index == 0 {
                    vec![elt, item].into_boxed_slice()
                } else {
                    vec![item, elt].into_boxed_slice()
                };
                Multi(items)
            }
            Multi(items) => {
                let mut items = items.into_vec();
                items.insert(index, elt);
                Multi(items.into_boxed_slice())
            }
        }
    }

    /// Removes the element at the specified index from the collection.
    ///
    /// Reallocs if more than 2 items are in the collection.
    pub fn remove(&mut self, index: usize) -> T {
        use ShortBoxSliceInner::*;
        assert!(
            index < self.len(),
            "removal index (is {}) should be < len (is {})",
            index,
            self.len()
        );

        let (replaced, removed_item) = match core::mem::replace(&mut self.0, ZeroOne(None)) {
            ZeroOne(None) => unreachable!(),
            ZeroOne(Some(v)) => (ZeroOne(None), v),
            #[cfg(feature = "alloc")]
            Multi(v) => {
                let mut v = v.into_vec();
                let removed_item = v.remove(index);
                match v.len() {
                    #[expect(clippy::unwrap_used)]
                    // we know that the vec has exactly one element left
                    1 => (ZeroOne(Some(v.pop().unwrap())), removed_item),
                    // v has at least 2 elements, create a Multi variant
                    _ => (Multi(v.into_boxed_slice()), removed_item),
                }
            }
            #[cfg(not(feature = "alloc"))]
            Two([f, s]) => (ZeroOne(Some(f)), s),
        };
        self.0 = replaced;
        removed_item
    }

    /// Removes all elements from the collection.
    #[inline]
    pub fn clear(&mut self) {
        use ShortBoxSliceInner::*;
        let _ = core::mem::replace(&mut self.0, ZeroOne(None));
    }

    /// Retains only the elements specified by the predicate.
    #[allow(dead_code)]
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> bool,
    {
        use ShortBoxSliceInner::*;
        match core::mem::take(&mut self.0) {
            ZeroOne(Some(one)) if f(&one) => self.0 = ZeroOne(Some(one)),
            ZeroOne(_) => self.0 = ZeroOne(None),
            #[cfg(feature = "alloc")]
            Multi(slice) => {
                let mut vec = slice.into_vec();
                vec.retain(f);
                *self = ShortBoxSlice::from(vec)
            }
            #[cfg(not(feature = "alloc"))]
            Two([first, second]) => {
                *self = match (Some(first).filter(&mut f), Some(second).filter(&mut f)) {
                    (None, None) => ShortBoxSlice::new(),
                    (None, Some(x)) | (Some(x), None) => ShortBoxSlice::new_single(x),
                    (Some(f), Some(s)) => ShortBoxSlice::new_double(f, s),
                }
            }
        };
    }
}

impl<T> Deref for ShortBoxSlice<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        use ShortBoxSliceInner::*;
        match self.0 {
            ZeroOne(None) => &[],
            ZeroOne(Some(ref v)) => core::slice::from_ref(v),
            #[cfg(feature = "alloc")]
            Multi(ref v) => v,
            #[cfg(not(feature = "alloc"))]
            Two(ref v) => v,
        }
    }
}

impl<T> DerefMut for ShortBoxSlice<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        use ShortBoxSliceInner::*;
        match self.0 {
            ZeroOne(None) => &mut [],
            ZeroOne(Some(ref mut v)) => core::slice::from_mut(v),
            #[cfg(feature = "alloc")]
            Multi(ref mut v) => v,
            #[cfg(not(feature = "alloc"))]
            Two(ref mut v) => v,
        }
    }
}

#[cfg(feature = "alloc")]
impl<T> From<Vec<T>> for ShortBoxSlice<T> {
    fn from(v: Vec<T>) -> Self {
        use ShortBoxSliceInner::*;
        match v.len() {
            0 => Self(ZeroOne(None)),
            #[expect(clippy::unwrap_used)] // we know that the vec is not empty
            1 => Self(ZeroOne(Some(v.into_iter().next().unwrap()))),
            _ => Self(Multi(v.into_boxed_slice())),
        }
    }
}

#[cfg(feature = "alloc")]
impl<T> FromIterator<T> for ShortBoxSlice<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        use ShortBoxSliceInner::*;
        let mut iter = iter.into_iter();
        match (iter.next(), iter.next()) {
            (Some(first), Some(second)) => {
                // Size hint behaviour same as `Vec::extend` + 2
                let mut vec = Vec::with_capacity(iter.size_hint().0.saturating_add(3));
                vec.push(first);
                vec.push(second);
                vec.extend(iter);
                Self(Multi(vec.into_boxed_slice()))
            }
            (first, _) => Self(ZeroOne(first)),
        }
    }
}

/// An iterator that yields elements from a [`ShortBoxSlice`].
#[derive(Debug)]
pub struct ShortBoxSliceIntoIter<T>(ShortBoxSliceIntoIterInner<T>);

#[derive(Debug)]
pub(crate) enum ShortBoxSliceIntoIterInner<T> {
    ZeroOne(Option<T>),
    #[cfg(feature = "alloc")]
    Multi(alloc::vec::IntoIter<T>),
    #[cfg(not(feature = "alloc"))]
    Two(core::array::IntoIter<T, 2>),
}

impl<T> Iterator for ShortBoxSliceIntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        use ShortBoxSliceIntoIterInner::*;
        match &mut self.0 {
            ZeroOne(option) => option.take(),
            #[cfg(feature = "alloc")]
            Multi(into_iter) => into_iter.next(),
            #[cfg(not(feature = "alloc"))]
            Two(into_iter) => into_iter.next(),
        }
    }
}

impl<T> IntoIterator for ShortBoxSlice<T> {
    type Item = T;
    type IntoIter = ShortBoxSliceIntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        match self.0 {
            ShortBoxSliceInner::ZeroOne(option) => {
                ShortBoxSliceIntoIter(ShortBoxSliceIntoIterInner::ZeroOne(option))
            }
            // TODO: Use a boxed slice IntoIter impl when available:
            // <https://github.com/rust-lang/rust/issues/59878>
            #[cfg(feature = "alloc")]
            ShortBoxSliceInner::Multi(boxed_slice) => ShortBoxSliceIntoIter(
                ShortBoxSliceIntoIterInner::Multi(boxed_slice.into_vec().into_iter()),
            ),
            #[cfg(not(feature = "alloc"))]
            ShortBoxSliceInner::Two(arr) => {
                ShortBoxSliceIntoIter(ShortBoxSliceIntoIterInner::Two(arr.into_iter()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[expect(clippy::get_first)]
    fn test_new_single_const() {
        const MY_CONST_SLICE: ShortBoxSlice<i32> = ShortBoxSlice::new_single(42);

        assert_eq!(MY_CONST_SLICE.len(), 1);
        assert_eq!(MY_CONST_SLICE.get(0), Some(&42));
    }

    #[test]
    #[expect(clippy::redundant_pattern_matching)]
    fn test_get_single() {
        let mut vec = ShortBoxSlice::new();
        assert!(matches!(vec.single(), None));

        vec.push(100);
        assert!(matches!(vec.single(), Some(_)));

        vec.push(200);
        assert!(matches!(vec.single(), None));
    }
}

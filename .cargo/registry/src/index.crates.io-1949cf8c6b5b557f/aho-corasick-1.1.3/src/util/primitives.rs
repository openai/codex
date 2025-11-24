/*!
Lower level primitive types that are useful in a variety of circumstances.

# Overview

This list represents the principle types in this module and briefly describes
when you might want to use them.

* [`PatternID`] - A type that represents the identifier of a regex pattern.
This is probably the most widely used type in this module (which is why it's
also re-exported in the crate root).
* [`StateID`] - A type the represents the identifier of a finite automaton
state. This is used for both NFAs and DFAs, with the notable exception of
the hybrid NFA/DFA. (The hybrid NFA/DFA uses a special purpose "lazy" state
identifier.)
* [`SmallIndex`] - The internal representation of both a `PatternID` and a
`StateID`. Its purpose is to serve as a type that can index memory without
being as big as a `usize` on 64-bit targets. The main idea behind this type
is that there are many things in regex engines that will, in practice, never
overflow a 32-bit integer. (For example, like the number of patterns in a regex
or the number of states in an NFA.) Thus, a `SmallIndex` can be used to index
memory without peppering `as` casts everywhere. Moreover, it forces callers
to handle errors in the case where, somehow, the value would otherwise overflow
either a 32-bit integer or a `usize` (e.g., on 16-bit targets).
*/

// The macro we use to define some types below adds methods that we don't
// use on some of the types. There isn't much, so we just squash the warning.
#![allow(dead_code)]

use alloc::vec::Vec;

use crate::util::int::{Usize, U16, U32, U64};

/// A type that represents a "small" index.
///
/// The main idea of this type is to provide something that can index memory,
/// but uses less memory than `usize` on 64-bit systems. Specifically, its
/// representation is always a `u32` and has `repr(transparent)` enabled. (So
/// it is safe to transmute between a `u32` and a `SmallIndex`.)
///
/// A small index is typically useful in cases where there is no practical way
/// that the index will overflow a 32-bit integer. A good example of this is
/// an NFA state. If you could somehow build an NFA with `2^30` states, its
/// memory usage would be exorbitant and its runtime execution would be so
/// slow as to be completely worthless. Therefore, this crate generally deems
/// it acceptable to return an error if it would otherwise build an NFA that
/// requires a slice longer than what a 32-bit integer can index. In exchange,
/// we can use 32-bit indices instead of 64-bit indices in various places.
///
/// This type ensures this by providing a constructor that will return an error
/// if its argument cannot fit into the type. This makes it much easier to
/// handle these sorts of boundary cases that are otherwise extremely subtle.
///
/// On all targets, this type guarantees that its value will fit in a `u32`,
/// `i32`, `usize` and an `isize`. This means that on 16-bit targets, for
/// example, this type's maximum value will never overflow an `isize`,
/// which means it will never overflow a `i16` even though its internal
/// representation is still a `u32`.
///
/// The purpose for making the type fit into even signed integer types like
/// `isize` is to guarantee that the difference between any two small indices
/// is itself also a small index. This is useful in certain contexts, e.g.,
/// for delta encoding.
///
/// # Other types
///
/// The following types wrap `SmallIndex` to provide a more focused use case:
///
/// * [`PatternID`] is for representing the identifiers of patterns.
/// * [`StateID`] is for representing the identifiers of states in finite
/// automata. It is used for both NFAs and DFAs.
///
/// # Representation
///
/// This type is always represented internally by a `u32` and is marked as
/// `repr(transparent)`. Thus, this type always has the same representation as
/// a `u32`. It is thus safe to transmute between a `u32` and a `SmallIndex`.
///
/// # Indexing
///
/// For convenience, callers may use a `SmallIndex` to index slices.
///
/// # Safety
///
/// While a `SmallIndex` is meant to guarantee that its value fits into `usize`
/// without using as much space as a `usize` on all targets, callers must
/// not rely on this property for safety. Callers may choose to rely on this
/// property for correctness however. For example, creating a `SmallIndex` with
/// an invalid value can be done in entirely safe code. This may in turn result
/// in panics or silent logical errors.
#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, PartialEq, PartialOrd, Ord,
)]
#[repr(transparent)]
pub(crate) struct SmallIndex(u32);

impl SmallIndex {
    /// The maximum index value.
    #[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
    pub const MAX: SmallIndex =
        // FIXME: Use as_usize() once const functions in traits are stable.
        SmallIndex::new_unchecked(core::i32::MAX as usize - 1);

    /// The maximum index value.
    #[cfg(target_pointer_width = "16")]
    pub const MAX: SmallIndex =
        SmallIndex::new_unchecked(core::isize::MAX - 1);

    /// The total number of values that can be represented as a small index.
    pub const LIMIT: usize = SmallIndex::MAX.as_usize() + 1;

    /// The zero index value.
    pub const ZERO: SmallIndex = SmallIndex::new_unchecked(0);

    /// The number of bytes that a single small index uses in memory.
    pub const SIZE: usize = core::mem::size_of::<SmallIndex>();

    /// Create a new small index.
    ///
    /// If the given index exceeds [`SmallIndex::MAX`], then this returns
    /// an error.
    #[inline]
    pub fn new(index: usize) -> Result<SmallIndex, SmallIndexError> {
        SmallIndex::try_from(index)
    }

    /// Create a new small index without checking whether the given value
    /// exceeds [`SmallIndex::MAX`].
    ///
    /// Using this routine with an invalid index value will result in
    /// unspecified behavior, but *not* undefined behavior. In particular, an
    /// invalid index value is likely to cause panics or possibly even silent
    /// logical errors.
    ///
    /// Callers must never rely on a `SmallIndex` to be within a certain range
    /// for memory safety.
    #[inline]
    pub const fn new_unchecked(index: usize) -> SmallIndex {
        // FIXME: Use as_u32() once const functions in traits are stable.
        SmallIndex::from_u32_unchecked(index as u32)
    }

    /// Create a new small index from a `u32` without checking whether the
    /// given value exceeds [`SmallIndex::MAX`].
    ///
    /// Using this routine with an invalid index value will result in
    /// unspecified behavior, but *not* undefined behavior. In particular, an
    /// invalid index value is likely to cause panics or possibly even silent
    /// logical errors.
    ///
    /// Callers must never rely on a `SmallIndex` to be within a certain range
    /// for memory safety.
    #[inline]
    pub const fn from_u32_unchecked(index: u32) -> SmallIndex {
        SmallIndex(index)
    }

    /// Like [`SmallIndex::new`], but panics if the given index is not valid.
    #[inline]
    pub fn must(index: usize) -> SmallIndex {
        SmallIndex::new(index).expect("invalid small index")
    }

    /// Return this small index as a `usize`. This is guaranteed to never
    /// overflow `usize`.
    #[inline]
    pub const fn as_usize(&self) -> usize {
        // FIXME: Use as_usize() once const functions in traits are stable.
        self.0 as usize
    }

    /// Return this small index as a `u64`. This is guaranteed to never
    /// overflow.
    #[inline]
    pub const fn as_u64(&self) -> u64 {
        // FIXME: Use u64::from() once const functions in traits are stable.
        self.0 as u64
    }

    /// Return the internal `u32` of this small index. This is guaranteed to
    /// never overflow `u32`.
    #[inline]
    pub const fn as_u32(&self) -> u32 {
        self.0
    }

    /// Return the internal `u32` of this small index represented as an `i32`.
    /// This is guaranteed to never overflow an `i32`.
    #[inline]
    pub const fn as_i32(&self) -> i32 {
        // This is OK because we guarantee that our max value is <= i32::MAX.
        self.0 as i32
    }

    /// Returns one more than this small index as a usize.
    ///
    /// Since a small index has constraints on its maximum value, adding `1` to
    /// it will always fit in a `usize`, `isize`, `u32` and a `i32`.
    #[inline]
    pub fn one_more(&self) -> usize {
        self.as_usize() + 1
    }

    /// Decode this small index from the bytes given using the native endian
    /// byte order for the current target.
    ///
    /// If the decoded integer is not representable as a small index for the
    /// current target, then this returns an error.
    #[inline]
    pub fn from_ne_bytes(
        bytes: [u8; 4],
    ) -> Result<SmallIndex, SmallIndexError> {
        let id = u32::from_ne_bytes(bytes);
        if id > SmallIndex::MAX.as_u32() {
            return Err(SmallIndexError { attempted: u64::from(id) });
        }
        Ok(SmallIndex::new_unchecked(id.as_usize()))
    }

    /// Decode this small index from the bytes given using the native endian
    /// byte order for the current target.
    ///
    /// This is analogous to [`SmallIndex::new_unchecked`] in that is does not
    /// check whether the decoded integer is representable as a small index.
    #[inline]
    pub fn from_ne_bytes_unchecked(bytes: [u8; 4]) -> SmallIndex {
        SmallIndex::new_unchecked(u32::from_ne_bytes(bytes).as_usize())
    }

    /// Return the underlying small index integer as raw bytes in native endian
    /// format.
    #[inline]
    pub fn to_ne_bytes(&self) -> [u8; 4] {
        self.0.to_ne_bytes()
    }
}

impl<T> core::ops::Index<SmallIndex> for [T] {
    type Output = T;

    #[inline]
    fn index(&self, index: SmallIndex) -> &T {
        &self[index.as_usize()]
    }
}

impl<T> core::ops::IndexMut<SmallIndex> for [T] {
    #[inline]
    fn index_mut(&mut self, index: SmallIndex) -> &mut T {
        &mut self[index.as_usize()]
    }
}

impl<T> core::ops::Index<SmallIndex> for Vec<T> {
    type Output = T;

    #[inline]
    fn index(&self, index: SmallIndex) -> &T {
        &self[index.as_usize()]
    }
}

impl<T> core::ops::IndexMut<SmallIndex> for Vec<T> {
    #[inline]
    fn index_mut(&mut self, index: SmallIndex) -> &mut T {
        &mut self[index.as_usize()]
    }
}

impl From<StateID> for SmallIndex {
    fn from(sid: StateID) -> SmallIndex {
        sid.0
    }
}

impl From<PatternID> for SmallIndex {
    fn from(pid: PatternID) -> SmallIndex {
        pid.0
    }
}

impl From<u8> for SmallIndex {
    fn from(index: u8) -> SmallIndex {
        SmallIndex::new_unchecked(usize::from(index))
    }
}

impl TryFrom<u16> for SmallIndex {
    type Error = SmallIndexError;

    fn try_from(index: u16) -> Result<SmallIndex, SmallIndexError> {
        if u32::from(index) > SmallIndex::MAX.as_u32() {
            return Err(SmallIndexError { attempted: u64::from(index) });
        }
        Ok(SmallIndex::new_unchecked(index.as_usize()))
    }
}

impl TryFrom<u32> for SmallIndex {
    type Error = SmallIndexError;

    fn try_from(index: u32) -> Result<SmallIndex, SmallIndexError> {
        if index > SmallIndex::MAX.as_u32() {
            return Err(SmallIndexError { attempted: u64::from(index) });
        }
        Ok(SmallIndex::new_unchecked(index.as_usize()))
    }
}

impl TryFrom<u64> for SmallIndex {
    type Error = SmallIndexError;

    fn try_from(index: u64) -> Result<SmallIndex, SmallIndexError> {
        if index > SmallIndex::MAX.as_u64() {
            return Err(SmallIndexError { attempted: index });
        }
        Ok(SmallIndex::new_unchecked(index.as_usize()))
    }
}

impl TryFrom<usize> for SmallIndex {
    type Error = SmallIndexError;

    fn try_from(index: usize) -> Result<SmallIndex, SmallIndexError> {
        if index > SmallIndex::MAX.as_usize() {
            return Err(SmallIndexError { attempted: index.as_u64() });
        }
        Ok(SmallIndex::new_unchecked(index))
    }
}

/// This error occurs when a small index could not be constructed.
///
/// This occurs when given an integer exceeding the maximum small index value.
///
/// When the `std` feature is enabled, this implements the `Error` trait.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmallIndexError {
    attempted: u64,
}

impl SmallIndexError {
    /// Returns the value that could not be converted to a small index.
    pub fn attempted(&self) -> u64 {
        self.attempted
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SmallIndexError {}

impl core::fmt::Display for SmallIndexError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "failed to create small index from {:?}, which exceeds {:?}",
            self.attempted(),
            SmallIndex::MAX,
        )
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SmallIndexIter {
    rng: core::ops::Range<usize>,
}

impl Iterator for SmallIndexIter {
    type Item = SmallIndex;

    fn next(&mut self) -> Option<SmallIndex> {
        if self.rng.start >= self.rng.end {
            return None;
        }
        let next_id = self.rng.start + 1;
        let id = core::mem::replace(&mut self.rng.start, next_id);
        // new_unchecked is OK since we asserted that the number of
        // elements in this iterator will fit in an ID at construction.
        Some(SmallIndex::new_unchecked(id))
    }
}

macro_rules! index_type_impls {
    ($name:ident, $err:ident, $iter:ident, $withiter:ident) => {
        impl $name {
            /// The maximum value.
            pub const MAX: $name = $name(SmallIndex::MAX);

            /// The total number of values that can be represented.
            pub const LIMIT: usize = SmallIndex::LIMIT;

            /// The zero value.
            pub const ZERO: $name = $name(SmallIndex::ZERO);

            /// The number of bytes that a single value uses in memory.
            pub const SIZE: usize = SmallIndex::SIZE;

            /// Create a new value that is represented by a "small index."
            ///
            /// If the given index exceeds the maximum allowed value, then this
            /// returns an error.
            #[inline]
            pub fn new(value: usize) -> Result<$name, $err> {
                SmallIndex::new(value).map($name).map_err($err)
            }

            /// Create a new value without checking whether the given argument
            /// exceeds the maximum.
            ///
            /// Using this routine with an invalid value will result in
            /// unspecified behavior, but *not* undefined behavior. In
            /// particular, an invalid ID value is likely to cause panics or
            /// possibly even silent logical errors.
            ///
            /// Callers must never rely on this type to be within a certain
            /// range for memory safety.
            #[inline]
            pub const fn new_unchecked(value: usize) -> $name {
                $name(SmallIndex::new_unchecked(value))
            }

            /// Create a new value from a `u32` without checking whether the
            /// given value exceeds the maximum.
            ///
            /// Using this routine with an invalid value will result in
            /// unspecified behavior, but *not* undefined behavior. In
            /// particular, an invalid ID value is likely to cause panics or
            /// possibly even silent logical errors.
            ///
            /// Callers must never rely on this type to be within a certain
            /// range for memory safety.
            #[inline]
            pub const fn from_u32_unchecked(index: u32) -> $name {
                $name(SmallIndex::from_u32_unchecked(index))
            }

            /// Like `new`, but panics if the given value is not valid.
            #[inline]
            pub fn must(value: usize) -> $name {
                $name::new(value).expect(concat!(
                    "invalid ",
                    stringify!($name),
                    " value"
                ))
            }

            /// Return the internal value as a `usize`. This is guaranteed to
            /// never overflow `usize`.
            #[inline]
            pub const fn as_usize(&self) -> usize {
                self.0.as_usize()
            }

            /// Return the internal value as a `u64`. This is guaranteed to
            /// never overflow.
            #[inline]
            pub const fn as_u64(&self) -> u64 {
                self.0.as_u64()
            }

            /// Return the internal value as a `u32`. This is guaranteed to
            /// never overflow `u32`.
            #[inline]
            pub const fn as_u32(&self) -> u32 {
                self.0.as_u32()
            }

            /// Return the internal value as a `i32`. This is guaranteed to
            /// never overflow an `i32`.
            #[inline]
            pub const fn as_i32(&self) -> i32 {
                self.0.as_i32()
            }

            /// Returns one more than this value as a usize.
            ///
            /// Since values represented by a "small index" have constraints
            /// on their maximum value, adding `1` to it will always fit in a
            /// `usize`, `u32` and a `i32`.
            #[inline]
            pub fn one_more(&self) -> usize {
                self.0.one_more()
            }

            /// Decode this value from the bytes given using the native endian
            /// byte order for the current target.
            ///
            /// If the decoded integer is not representable as a small index
            /// for the current target, then this returns an error.
            #[inline]
            pub fn from_ne_bytes(bytes: [u8; 4]) -> Result<$name, $err> {
                SmallIndex::from_ne_bytes(bytes).map($name).map_err($err)
            }

            /// Decode this value from the bytes given using the native endian
            /// byte order for the current target.
            ///
            /// This is analogous to `new_unchecked` in that is does not check
            /// whether the decoded integer is representable as a small index.
            #[inline]
            pub fn from_ne_bytes_unchecked(bytes: [u8; 4]) -> $name {
                $name(SmallIndex::from_ne_bytes_unchecked(bytes))
            }

            /// Return the underlying integer as raw bytes in native endian
            /// format.
            #[inline]
            pub fn to_ne_bytes(&self) -> [u8; 4] {
                self.0.to_ne_bytes()
            }

            /// Returns an iterator over all values from 0 up to and not
            /// including the given length.
            ///
            /// If the given length exceeds this type's limit, then this
            /// panics.
            pub(crate) fn iter(len: usize) -> $iter {
                $iter::new(len)
            }
        }

        // We write our own Debug impl so that we get things like PatternID(5)
        // instead of PatternID(SmallIndex(5)).
        impl core::fmt::Debug for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                f.debug_tuple(stringify!($name)).field(&self.as_u32()).finish()
            }
        }

        impl<T> core::ops::Index<$name> for [T] {
            type Output = T;

            #[inline]
            fn index(&self, index: $name) -> &T {
                &self[index.as_usize()]
            }
        }

        impl<T> core::ops::IndexMut<$name> for [T] {
            #[inline]
            fn index_mut(&mut self, index: $name) -> &mut T {
                &mut self[index.as_usize()]
            }
        }

        impl<T> core::ops::Index<$name> for Vec<T> {
            type Output = T;

            #[inline]
            fn index(&self, index: $name) -> &T {
                &self[index.as_usize()]
            }
        }

        impl<T> core::ops::IndexMut<$name> for Vec<T> {
            #[inline]
            fn index_mut(&mut self, index: $name) -> &mut T {
                &mut self[index.as_usize()]
            }
        }

        impl From<SmallIndex> for $name {
            fn from(index: SmallIndex) -> $name {
                $name(index)
            }
        }

        impl From<u8> for $name {
            fn from(value: u8) -> $name {
                $name(SmallIndex::from(value))
            }
        }

        impl TryFrom<u16> for $name {
            type Error = $err;

            fn try_from(value: u16) -> Result<$name, $err> {
                SmallIndex::try_from(value).map($name).map_err($err)
            }
        }

        impl TryFrom<u32> for $name {
            type Error = $err;

            fn try_from(value: u32) -> Result<$name, $err> {
                SmallIndex::try_from(value).map($name).map_err($err)
            }
        }

        impl TryFrom<u64> for $name {
            type Error = $err;

            fn try_from(value: u64) -> Result<$name, $err> {
                SmallIndex::try_from(value).map($name).map_err($err)
            }
        }

        impl TryFrom<usize> for $name {
            type Error = $err;

            fn try_from(value: usize) -> Result<$name, $err> {
                SmallIndex::try_from(value).map($name).map_err($err)
            }
        }

        /// This error occurs when an ID could not be constructed.
        ///
        /// This occurs when given an integer exceeding the maximum allowed
        /// value.
        ///
        /// When the `std` feature is enabled, this implements the `Error`
        /// trait.
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $err(SmallIndexError);

        impl $err {
            /// Returns the value that could not be converted to an ID.
            pub fn attempted(&self) -> u64 {
                self.0.attempted()
            }
        }

        #[cfg(feature = "std")]
        impl std::error::Error for $err {}

        impl core::fmt::Display for $err {
            fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                write!(
                    f,
                    "failed to create {} from {:?}, which exceeds {:?}",
                    stringify!($name),
                    self.attempted(),
                    $name::MAX,
                )
            }
        }

        #[derive(Clone, Debug)]
        pub(crate) struct $iter(SmallIndexIter);

        impl $iter {
            fn new(len: usize) -> $iter {
                assert!(
                    len <= $name::LIMIT,
                    "cannot create iterator for {} when number of \
                     elements exceed {:?}",
                    stringify!($name),
                    $name::LIMIT,
                );
                $iter(SmallIndexIter { rng: 0..len })
            }
        }

        impl Iterator for $iter {
            type Item = $name;

            fn next(&mut self) -> Option<$name> {
                self.0.next().map($name)
            }
        }

        /// An iterator adapter that is like std::iter::Enumerate, but attaches
        /// small index values instead. It requires `ExactSizeIterator`. At
        /// construction, it ensures that the index of each element in the
        /// iterator is representable in the corresponding small index type.
        #[derive(Clone, Debug)]
        pub(crate) struct $withiter<I> {
            it: I,
            ids: $iter,
        }

        impl<I: Iterator + ExactSizeIterator> $withiter<I> {
            fn new(it: I) -> $withiter<I> {
                let ids = $name::iter(it.len());
                $withiter { it, ids }
            }
        }

        impl<I: Iterator + ExactSizeIterator> Iterator for $withiter<I> {
            type Item = ($name, I::Item);

            fn next(&mut self) -> Option<($name, I::Item)> {
                let item = self.it.next()?;
                // Number of elements in this iterator must match, according
                // to contract of ExactSizeIterator.
                let id = self.ids.next().unwrap();
                Some((id, item))
            }
        }
    };
}

/// The identifier of a pattern in an Aho-Corasick automaton.
///
/// It is represented by a `u32` even on 64-bit systems in order to conserve
/// space. Namely, on all targets, this type guarantees that its value will
/// fit in a `u32`, `i32`, `usize` and an `isize`. This means that on 16-bit
/// targets, for example, this type's maximum value will never overflow an
/// `isize`, which means it will never overflow a `i16` even though its
/// internal representation is still a `u32`.
///
/// # Safety
///
/// While a `PatternID` is meant to guarantee that its value fits into `usize`
/// without using as much space as a `usize` on all targets, callers must
/// not rely on this property for safety. Callers may choose to rely on this
/// property for correctness however. For example, creating a `StateID` with an
/// invalid value can be done in entirely safe code. This may in turn result in
/// panics or silent logical errors.
#[derive(Clone, Copy, Default, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PatternID(SmallIndex);

/// The identifier of a finite automaton state.
///
/// It is represented by a `u32` even on 64-bit systems in order to conserve
/// space. Namely, on all targets, this type guarantees that its value will
/// fit in a `u32`, `i32`, `usize` and an `isize`. This means that on 16-bit
/// targets, for example, this type's maximum value will never overflow an
/// `isize`, which means it will never overflow a `i16` even though its
/// internal representation is still a `u32`.
///
/// # Safety
///
/// While a `StateID` is meant to guarantee that its value fits into `usize`
/// without using as much space as a `usize` on all targets, callers must
/// not rely on this property for safety. Callers may choose to rely on this
/// property for correctness however. For example, creating a `StateID` with an
/// invalid value can be done in entirely safe code. This may in turn result in
/// panics or silent logical errors.
#[derive(Clone, Copy, Default, Eq, Hash, PartialEq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct StateID(SmallIndex);

index_type_impls!(PatternID, PatternIDError, PatternIDIter, WithPatternIDIter);
index_type_impls!(StateID, StateIDError, StateIDIter, WithStateIDIter);

/// A utility trait that defines a couple of adapters for making it convenient
/// to access indices as "small index" types. We require ExactSizeIterator so
/// that iterator construction can do a single check to make sure the index of
/// each element is representable by its small index type.
pub(crate) trait IteratorIndexExt: Iterator {
    fn with_pattern_ids(self) -> WithPatternIDIter<Self>
    where
        Self: Sized + ExactSizeIterator,
    {
        WithPatternIDIter::new(self)
    }

    fn with_state_ids(self) -> WithStateIDIter<Self>
    where
        Self: Sized + ExactSizeIterator,
    {
        WithStateIDIter::new(self)
    }
}

impl<I: Iterator> IteratorIndexExt for I {}

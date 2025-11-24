// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::reader;

use core::borrow::Borrow;

#[cfg(feature = "alloc")]
use crate::{
    builder::bytestr::ByteStr, builder::nonconst::ZeroTrieBuilder, error::ZeroTrieBuildError,
};
#[cfg(feature = "alloc")]
use alloc::{boxed::Box, collections::BTreeMap, collections::VecDeque, string::String, vec::Vec};
#[cfg(feature = "litemap")]
use litemap::LiteMap;

/// A data structure that compactly maps from byte sequences to integers.
///
/// There are several variants of `ZeroTrie` which are very similar but are optimized
/// for different use cases:
///
/// - [`ZeroTrieSimpleAscii`] is the most compact structure. Very fast for small data.
///   Only stores ASCII-encoded strings. Can be const-constructed!
/// - [`ZeroTriePerfectHash`] is also compact, but it also supports arbitrary binary
///   strings. It also scales better to large data. Cannot be const-constructed.
/// - [`ZeroTrieExtendedCapacity`] can be used if more than 2^32 bytes are required.
///
/// You can create a `ZeroTrie` directly, in which case the most appropriate
/// backing implementation will be chosen.
///
/// # Backing Store
///
/// The data structure has a flexible backing data store. The only requirement for most
/// functionality is that it implement `AsRef<[u8]>`. All of the following are valid
/// ZeroTrie types:
///
/// - `ZeroTrie<[u8]>` (dynamically sized type: must be stored in a reference or Box)
/// - `ZeroTrie<&[u8]>` (borrows its data from a u8 buffer)
/// - `ZeroTrie<Vec<u8>>` (fully owned data)
/// - `ZeroTrie<ZeroVec<u8>>` (the recommended borrowed-or-owned signature)
/// - `Cow<ZeroTrie<[u8]>>` (another borrowed-or-owned signature)
/// - `ZeroTrie<Cow<[u8]>>` (another borrowed-or-owned signature)
///
/// # Examples
///
/// ```
/// use litemap::LiteMap;
/// use zerotrie::ZeroTrie;
///
/// let mut map = LiteMap::<&[u8], usize>::new_vec();
/// map.insert("foo".as_bytes(), 1);
/// map.insert("bar".as_bytes(), 2);
/// map.insert("bazzoo".as_bytes(), 3);
///
/// let trie = ZeroTrie::try_from(&map)?;
///
/// assert_eq!(trie.get("foo"), Some(1));
/// assert_eq!(trie.get("bar"), Some(2));
/// assert_eq!(trie.get("bazzoo"), Some(3));
/// assert_eq!(trie.get("unknown"), None);
///
/// # Ok::<_, zerotrie::ZeroTrieBuildError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// Note: The absence of the following derive does not cause any test failures in this crate
#[cfg_attr(feature = "yoke", derive(yoke::Yokeable))]
pub struct ZeroTrie<Store>(pub(crate) ZeroTrieFlavor<Store>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ZeroTrieFlavor<Store> {
    SimpleAscii(ZeroTrieSimpleAscii<Store>),
    PerfectHash(ZeroTriePerfectHash<Store>),
    ExtendedCapacity(ZeroTrieExtendedCapacity<Store>),
}

/// A data structure that compactly maps from ASCII strings to integers.
///
/// For more information, see [`ZeroTrie`].
///
/// # Examples
///
/// ```
/// use litemap::LiteMap;
/// use zerotrie::ZeroTrieSimpleAscii;
///
/// let mut map = LiteMap::new_vec();
/// map.insert(&b"foo"[..], 1);
/// map.insert(b"bar", 2);
/// map.insert(b"bazzoo", 3);
///
/// let trie = ZeroTrieSimpleAscii::try_from(&map)?;
///
/// assert_eq!(trie.get(b"foo"), Some(1));
/// assert_eq!(trie.get(b"bar"), Some(2));
/// assert_eq!(trie.get(b"bazzoo"), Some(3));
/// assert_eq!(trie.get(b"unknown"), None);
///
/// # Ok::<_, zerotrie::ZeroTrieBuildError>(())
/// ```
///
/// The trie can only store ASCII bytes; a string with non-ASCII always returns None:
///
/// ```
/// use zerotrie::ZeroTrieSimpleAscii;
///
/// // A trie with two values: "abc" and "abcdef"
/// let trie = ZeroTrieSimpleAscii::from_bytes(b"abc\x80def\x81");
///
/// assert!(matches!(trie.get(b"ab\xFF"), None));
/// ```
#[repr(transparent)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "databake", derive(databake::Bake))]
#[cfg_attr(feature = "databake", databake(path = zerotrie))]
#[allow(clippy::exhaustive_structs)] // databake hidden fields
pub struct ZeroTrieSimpleAscii<Store: ?Sized> {
    #[doc(hidden)] // for databake, but there are no invariants
    pub store: Store,
}

impl<Store: ?Sized> ZeroTrieSimpleAscii<Store> {
    fn transparent_ref_from_store(s: &Store) -> &Self {
        unsafe {
            // Safety: Self is transparent over Store
            core::mem::transmute(s)
        }
    }
}

impl<Store> ZeroTrieSimpleAscii<Store> {
    /// Wrap this specific ZeroTrie variant into a ZeroTrie.
    #[inline]
    pub const fn into_zerotrie(self) -> ZeroTrie<Store> {
        ZeroTrie(ZeroTrieFlavor::SimpleAscii(self))
    }
}

/// A data structure that compactly maps from ASCII strings to integers
/// in a case-insensitive way.
///
/// # Examples
///
/// ```
/// use litemap::LiteMap;
/// use zerotrie::ZeroAsciiIgnoreCaseTrie;
///
/// let mut map = LiteMap::new_vec();
/// map.insert(&b"foo"[..], 1);
/// map.insert(b"Bar", 2);
/// map.insert(b"Bazzoo", 3);
///
/// let trie = ZeroAsciiIgnoreCaseTrie::try_from(&map)?;
///
/// assert_eq!(trie.get(b"foo"), Some(1));
/// assert_eq!(trie.get(b"bar"), Some(2));
/// assert_eq!(trie.get(b"BAR"), Some(2));
/// assert_eq!(trie.get(b"bazzoo"), Some(3));
/// assert_eq!(trie.get(b"unknown"), None);
///
/// # Ok::<_, zerotrie::ZeroTrieBuildError>(())
/// ```
///
/// Strings with different cases of the same character at the same offset are not allowed:
///
/// ```
/// use litemap::LiteMap;
/// use zerotrie::ZeroAsciiIgnoreCaseTrie;
///
/// let mut map = LiteMap::new_vec();
/// map.insert(&b"bar"[..], 1);
/// // OK: 'r' and 'Z' are different letters
/// map.insert(b"baZ", 2);
/// // Bad: we already inserted 'r' so we cannot also insert 'R' at the same position
/// map.insert(b"baR", 2);
///
/// ZeroAsciiIgnoreCaseTrie::try_from(&map).expect_err("mixed-case strings!");
/// ```
#[repr(transparent)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "databake", derive(databake::Bake))]
#[cfg_attr(feature = "databake", databake(path = zerotrie))]
#[allow(clippy::exhaustive_structs)] // databake hidden fields
pub struct ZeroAsciiIgnoreCaseTrie<Store: ?Sized> {
    #[doc(hidden)] // for databake, but there are no invariants
    pub store: Store,
}

impl<Store: ?Sized> ZeroAsciiIgnoreCaseTrie<Store> {
    fn transparent_ref_from_store(s: &Store) -> &Self {
        unsafe {
            // Safety: Self is transparent over Store
            core::mem::transmute(s)
        }
    }
}

// Note: ZeroAsciiIgnoreCaseTrie is not a variant of ZeroTrie so there is no `into_zerotrie`

/// A data structure that compactly maps from byte strings to integers.
///
/// For more information, see [`ZeroTrie`].
///
/// # Examples
///
/// ```
/// use litemap::LiteMap;
/// use zerotrie::ZeroTriePerfectHash;
///
/// let mut map = LiteMap::<&[u8], usize>::new_vec();
/// map.insert("foo".as_bytes(), 1);
/// map.insert("bår".as_bytes(), 2);
/// map.insert("båzzøø".as_bytes(), 3);
///
/// let trie = ZeroTriePerfectHash::try_from(&map)?;
///
/// assert_eq!(trie.get("foo".as_bytes()), Some(1));
/// assert_eq!(trie.get("bår".as_bytes()), Some(2));
/// assert_eq!(trie.get("båzzøø".as_bytes()), Some(3));
/// assert_eq!(trie.get("bazzoo".as_bytes()), None);
///
/// # Ok::<_, zerotrie::ZeroTrieBuildError>(())
/// ```
#[repr(transparent)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "databake", derive(databake::Bake))]
#[cfg_attr(feature = "databake", databake(path = zerotrie))]
#[allow(clippy::exhaustive_structs)] // databake hidden fields
pub struct ZeroTriePerfectHash<Store: ?Sized> {
    #[doc(hidden)] // for databake, but there are no invariants
    pub store: Store,
}

impl<Store: ?Sized> ZeroTriePerfectHash<Store> {
    fn transparent_ref_from_store(s: &Store) -> &Self {
        unsafe {
            // Safety: Self is transparent over Store
            core::mem::transmute(s)
        }
    }
}

impl<Store> ZeroTriePerfectHash<Store> {
    /// Wrap this specific ZeroTrie variant into a ZeroTrie.
    #[inline]
    pub const fn into_zerotrie(self) -> ZeroTrie<Store> {
        ZeroTrie(ZeroTrieFlavor::PerfectHash(self))
    }
}

/// A data structure that maps from a large number of byte strings to integers.
///
/// For more information, see [`ZeroTrie`].
#[repr(transparent)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "databake", derive(databake::Bake))]
#[cfg_attr(feature = "databake", databake(path = zerotrie))]
#[allow(clippy::exhaustive_structs)] // databake hidden fields
pub struct ZeroTrieExtendedCapacity<Store: ?Sized> {
    #[doc(hidden)] // for databake, but there are no invariants
    pub store: Store,
}

impl<Store: ?Sized> ZeroTrieExtendedCapacity<Store> {
    fn transparent_ref_from_store(s: &Store) -> &Self {
        unsafe {
            // Safety: Self is transparent over Store
            core::mem::transmute(s)
        }
    }
}

impl<Store> ZeroTrieExtendedCapacity<Store> {
    /// Wrap this specific ZeroTrie variant into a ZeroTrie.
    #[inline]
    pub const fn into_zerotrie(self) -> ZeroTrie<Store> {
        ZeroTrie(ZeroTrieFlavor::ExtendedCapacity(self))
    }
}

macro_rules! impl_zerotrie_subtype {
    ($name:ident, $iter_element:ty, $iter_fn:path, $iter_ty:ty, $cnv_fn:path) => {
        impl<Store> $name<Store> {
            /// Create a trie directly from a store.
            ///
            /// If the store does not contain valid bytes, unexpected behavior may occur.
            #[inline]
            pub const fn from_store(store: Store) -> Self {
                Self { store }
            }
            /// Takes the byte store from this trie.
            #[inline]
            pub fn into_store(self) -> Store {
                self.store
            }
            /// Converts this trie's store to a different store implementing the `From` trait.
            ///
            #[doc = concat!("For example, use this to change `", stringify!($name), "<Vec<u8>>` to `", stringify!($name), "<Cow<[u8]>>`.")]
            ///
            /// # Examples
            ///
            /// ```
            /// use std::borrow::Cow;
            #[doc = concat!("use zerotrie::", stringify!($name), ";")]
            ///
            #[doc = concat!("let trie: ", stringify!($name), "<Vec<u8>> = ", stringify!($name), "::from_bytes(b\"abc\\x85\").to_owned();")]
            #[doc = concat!("let cow: ", stringify!($name), "<Cow<[u8]>> = trie.convert_store();")]
            ///
            /// assert_eq!(cow.get(b"abc"), Some(5));
            /// ```
            pub fn convert_store<X: From<Store>>(self) -> $name<X> {
                $name::<X>::from_store(X::from(self.store))
            }
        }
        impl<Store> $name<Store>
        where
        Store: AsRef<[u8]> + ?Sized,
        {
            /// Queries the trie for a string.
            pub fn get<K>(&self, key: K) -> Option<usize> where K: AsRef<[u8]> {
                // TODO: Should this be AsRef or Borrow?
                reader::get_parameterized::<Self>(self.store.as_ref(), key.as_ref())
            }
            /// Returns `true` if the trie is empty.
            #[inline]
            pub fn is_empty(&self) -> bool {
                self.store.as_ref().is_empty()
            }
            /// Returns the size of the trie in number of bytes.
            ///
            /// To get the number of keys in the trie, use `.iter().count()`:
            ///
            /// ```
            #[doc = concat!("use zerotrie::", stringify!($name), ";")]
            ///
            /// // A trie with two values: "abc" and "abcdef"
            #[doc = concat!("let trie: &", stringify!($name), "<[u8]> = ", stringify!($name), "::from_bytes(b\"abc\\x80def\\x81\");")]
            ///
            /// assert_eq!(8, trie.byte_len());
            /// assert_eq!(2, trie.iter().count());
            /// ```
            #[inline]
            pub fn byte_len(&self) -> usize {
                self.store.as_ref().len()
            }
            /// Returns the bytes contained in the underlying store.
            #[inline]
            pub fn as_bytes(&self) -> &[u8] {
                self.store.as_ref()
            }
            /// Returns this trie as a reference transparent over a byte slice.
            #[inline]
            pub fn as_borrowed(&self) -> &$name<[u8]> {
                $name::from_bytes(self.store.as_ref())
            }
            /// Returns a trie with a store borrowing from this trie.
            #[inline]
            pub fn as_borrowed_slice(&self) -> $name<&[u8]> {
                $name::from_store(self.store.as_ref())
            }
        }
        impl<Store> AsRef<$name<[u8]>> for $name<Store>
        where
        Store: AsRef<[u8]> + ?Sized,
        {
            #[inline]
            fn as_ref(&self) -> &$name<[u8]> {
                self.as_borrowed()
            }
        }
        #[cfg(feature = "alloc")]
        impl<Store> $name<Store>
        where
        Store: AsRef<[u8]> + ?Sized,
        {
            /// Converts a possibly-borrowed $name to an owned one.
            ///
            /// ✨ *Enabled with the `alloc` Cargo feature.*
            ///
            /// # Examples
            ///
            /// ```
            #[doc = concat!("use zerotrie::", stringify!($name), ";")]
            ///
            #[doc = concat!("let trie: &", stringify!($name), "<[u8]> = ", stringify!($name), "::from_bytes(b\"abc\\x85\");")]
            #[doc = concat!("let owned: ", stringify!($name), "<Vec<u8>> = trie.to_owned();")]
            ///
            /// assert_eq!(trie.get(b"abc"), Some(5));
            /// assert_eq!(owned.get(b"abc"), Some(5));
            /// ```
            #[inline]
            pub fn to_owned(&self) -> $name<Vec<u8>> {
                $name::from_store(
                    Vec::from(self.store.as_ref()),
                )
            }
            /// Returns an iterator over the key/value pairs in this trie.
            ///
            /// ✨ *Enabled with the `alloc` Cargo feature.*
            ///
            /// # Examples
            ///
            /// ```
            #[doc = concat!("use zerotrie::", stringify!($name), ";")]
            ///
            /// // A trie with two values: "abc" and "abcdef"
            #[doc = concat!("let trie: &", stringify!($name), "<[u8]> = ", stringify!($name), "::from_bytes(b\"abc\\x80def\\x81\");")]
            ///
            /// let mut it = trie.iter();
            /// assert_eq!(it.next(), Some(("abc".into(), 0)));
            /// assert_eq!(it.next(), Some(("abcdef".into(), 1)));
            /// assert_eq!(it.next(), None);
            /// ```
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn iter(&self) -> $iter_ty {
                 $iter_fn(self.as_bytes())
            }
        }
        impl $name<[u8]> {
            /// Casts from a byte slice to a reference to a trie with the same lifetime.
            ///
            /// If the bytes are not a valid trie, unexpected behavior may occur.
            #[inline]
            pub fn from_bytes(trie: &[u8]) -> &Self {
                Self::transparent_ref_from_store(trie)
            }
        }
        #[cfg(feature = "alloc")]
        impl $name<Vec<u8>> {
            pub(crate) fn try_from_tuple_slice(items: &[(&ByteStr, usize)]) -> Result<Self, ZeroTrieBuildError> {
                use crate::options::ZeroTrieWithOptions;
                ZeroTrieBuilder::<VecDeque<u8>>::from_sorted_tuple_slice(
                    items,
                    Self::OPTIONS,
                )
                .map(|s| Self {
                    store: s.to_bytes(),
                })
            }
        }
        #[cfg(feature = "alloc")]
        impl<'a, K> FromIterator<(K, usize)> for $name<Vec<u8>>
        where
            K: AsRef<[u8]>
        {
            fn from_iter<T: IntoIterator<Item = (K, usize)>>(iter: T) -> Self {
                use crate::options::ZeroTrieWithOptions;
                use crate::builder::nonconst::ZeroTrieBuilder;
                ZeroTrieBuilder::<VecDeque<u8>>::from_bytes_iter(
                    iter,
                    Self::OPTIONS
                )
                .map(|s| Self {
                    store: s.to_bytes(),
                })
                .unwrap()
            }
        }
        #[cfg(feature = "alloc")]
        impl<'a, K> TryFrom<&'a BTreeMap<K, usize>> for $name<Vec<u8>>
        where
            K: Borrow<[u8]>
        {
            type Error = crate::error::ZeroTrieBuildError;
            fn try_from(map: &'a BTreeMap<K, usize>) -> Result<Self, Self::Error> {
                let tuples: Vec<(&[u8], usize)> = map
                    .iter()
                    .map(|(k, v)| (k.borrow(), *v))
                    .collect();
                let byte_str_slice = ByteStr::from_byte_slice_with_value(&tuples);
                Self::try_from_tuple_slice(byte_str_slice)
            }
        }
        #[cfg(feature = "alloc")]
        impl<Store> $name<Store>
        where
            Store: AsRef<[u8]> + ?Sized
        {
            /// Exports the data from this ZeroTrie type into a BTreeMap.
            ///
            /// ✨ *Enabled with the `alloc` Cargo feature.*
            ///
            /// # Examples
            ///
            /// ```
            #[doc = concat!("use zerotrie::", stringify!($name), ";")]
            /// use std::collections::BTreeMap;
            ///
            #[doc = concat!("let trie = ", stringify!($name), "::from_bytes(b\"abc\\x81def\\x82\");")]
            /// let items = trie.to_btreemap();
            ///
            /// assert_eq!(items.len(), 2);
            ///
            #[doc = concat!("let recovered_trie: ", stringify!($name), "<Vec<u8>> = items")]
            ///     .into_iter()
            ///     .collect();
            /// assert_eq!(trie.as_bytes(), recovered_trie.as_bytes());
            /// ```
            pub fn to_btreemap(&self) -> BTreeMap<$iter_element, usize> {
                self.iter().collect()
            }
            #[allow(dead_code)] // not needed for ZeroAsciiIgnoreCaseTrie
            pub(crate) fn to_btreemap_bytes(&self) -> BTreeMap<Box<[u8]>, usize> {
                self.iter().map(|(k, v)| ($cnv_fn(k), v)).collect()
            }
        }
        #[cfg(feature = "alloc")]
        impl<Store> From<&$name<Store>> for BTreeMap<$iter_element, usize>
        where
            Store: AsRef<[u8]> + ?Sized,
        {
            #[inline]
            fn from(other: &$name<Store>) -> Self {
                other.to_btreemap()
            }
        }
        #[cfg(feature = "litemap")]
        impl<'a, K, S> TryFrom<&'a LiteMap<K, usize, S>> for $name<Vec<u8>>
        where
            K: Borrow<[u8]>,
            S: litemap::store::StoreIterable<'a, K, usize>,
        {
            type Error = crate::error::ZeroTrieBuildError;
            fn try_from(map: &'a LiteMap<K, usize, S>) -> Result<Self, Self::Error> {
                let tuples: Vec<(&[u8], usize)> = map
                    .iter()
                    .map(|(k, v)| (k.borrow(), *v))
                    .collect();
                let byte_str_slice = ByteStr::from_byte_slice_with_value(&tuples);
                Self::try_from_tuple_slice(byte_str_slice)
            }
        }
        #[cfg(feature = "litemap")]
        impl<Store> $name<Store>
        where
            Store: AsRef<[u8]> + ?Sized,
        {
            /// Exports the data from this ZeroTrie type into a LiteMap.
            ///
            /// ✨ *Enabled with the `litemap` Cargo feature.*
            ///
            /// # Examples
            ///
            /// ```
            #[doc = concat!("use zerotrie::", stringify!($name), ";")]
            /// use litemap::LiteMap;
            ///
            #[doc = concat!("let trie = ", stringify!($name), "::from_bytes(b\"abc\\x81def\\x82\");")]
            ///
            /// let items = trie.to_litemap();
            /// assert_eq!(items.len(), 2);
            ///
            #[doc = concat!("let recovered_trie: ", stringify!($name), "<Vec<u8>> = items")]
            ///     .iter()
            ///     .map(|(k, v)| (k, *v))
            ///     .collect();
            /// assert_eq!(trie.as_bytes(), recovered_trie.as_bytes());
            /// ```
            pub fn to_litemap(&self) -> LiteMap<$iter_element, usize> {
                self.iter().collect()
            }
            #[allow(dead_code)] // not needed for ZeroAsciiIgnoreCaseTrie
            pub(crate) fn to_litemap_bytes(&self) -> LiteMap<Box<[u8]>, usize> {
                self.iter().map(|(k, v)| ($cnv_fn(k), v)).collect()
            }
        }
        #[cfg(feature = "litemap")]
        impl<Store> From<&$name<Store>> for LiteMap<$iter_element, usize>
        where
            Store: AsRef<[u8]> + ?Sized,
        {
            #[inline]
            fn from(other: &$name<Store>) -> Self {
                other.to_litemap()
            }
        }
        #[cfg(feature = "litemap")]
        impl $name<Vec<u8>>
        {
            #[cfg(feature = "serde")]
            pub(crate) fn try_from_serde_litemap(items: &LiteMap<Box<ByteStr>, usize>) -> Result<Self, ZeroTrieBuildError> {
                let lm_borrowed: LiteMap<&ByteStr, usize> = items.to_borrowed_keys();
                Self::try_from_tuple_slice(lm_borrowed.as_slice())
            }
        }
        // Note: Can't generalize this impl due to the `core::borrow::Borrow` blanket impl.
        impl Borrow<$name<[u8]>> for $name<&[u8]> {
            #[inline]
            fn borrow(&self) -> &$name<[u8]> {
                self.as_borrowed()
            }
        }
        // Note: Can't generalize this impl due to the `core::borrow::Borrow` blanket impl.
        #[cfg(feature = "alloc")]
        impl Borrow<$name<[u8]>> for $name<Box<[u8]>> {
            #[inline]
            fn borrow(&self) -> &$name<[u8]> {
                self.as_borrowed()
            }
        }
        // Note: Can't generalize this impl due to the `core::borrow::Borrow` blanket impl.
        #[cfg(feature = "alloc")]
        impl Borrow<$name<[u8]>> for $name<Vec<u8>> {
            #[inline]
            fn borrow(&self) -> &$name<[u8]> {
                self.as_borrowed()
            }
        }
        #[cfg(feature = "alloc")]
        impl alloc::borrow::ToOwned for $name<[u8]> {
            type Owned = $name<Box<[u8]>>;
            #[doc = concat!("This impl allows [`", stringify!($name), "`] to be used inside of a [`Cow`](alloc::borrow::Cow).")]
            ///
            #[doc = concat!("Note that it is also possible to use `", stringify!($name), "<ZeroVec<u8>>` for a similar result.")]
            ///
            /// ✨ *Enabled with the `alloc` Cargo feature.*
            ///
            /// # Examples
            ///
            /// ```
            /// use std::borrow::Cow;
            #[doc = concat!("use zerotrie::", stringify!($name), ";")]
            ///
            #[doc = concat!("let trie: Cow<", stringify!($name), "<[u8]>> = Cow::Borrowed(", stringify!($name), "::from_bytes(b\"abc\\x85\"));")]
            /// assert_eq!(trie.get(b"abc"), Some(5));
            /// ```
            fn to_owned(&self) -> Self::Owned {
                let bytes: &[u8] = self.store.as_ref();
                $name::from_store(
                    Vec::from(bytes).into_boxed_slice(),
                )
            }
        }
        // TODO(#2778): Auto-derive these impls based on the repr(transparent).
        //
        // Safety (based on the safety checklist on the VarULE trait):
        //  1. `$name` does not include any uninitialized or padding bytes as it is `repr(transparent)`
        //     over a `VarULE` type, `Store`, as evidenced by the existence of `transparent_ref_from_store()`
        //  2. `$name` is aligned to 1 byte for the same reason
        //  3. The impl of `validate_bytes()` returns an error if any byte is not valid (passed down to `VarULE` impl of `Store`)
        //  4. The impl of `validate_bytes()` returns an error if the slice cannot be used in its entirety (passed down to `VarULE` impl of `Store`)
        //  5. The impl of `from_bytes_unchecked()` returns a reference to the same data.
        //  6. `parse_bytes()` is left to its default impl
        //  7. byte equality is semantic equality
        #[cfg(feature = "zerovec")]
        unsafe impl<Store> zerovec::ule::VarULE for $name<Store>
        where
            Store: zerovec::ule::VarULE,
        {
            #[inline]
            fn validate_bytes(bytes: &[u8]) -> Result<(), zerovec::ule::UleError> {
                Store::validate_bytes(bytes)
            }
            #[inline]
            unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
                // Safety: we can pass down the validity invariant to Store
                Self::transparent_ref_from_store(Store::from_bytes_unchecked(bytes))
            }
        }
        #[cfg(feature = "zerofrom")]
        impl<'zf, Store1, Store2> zerofrom::ZeroFrom<'zf, $name<Store1>> for $name<Store2>
        where
            Store2: zerofrom::ZeroFrom<'zf, Store1>,
        {
            #[inline]
            fn zero_from(other: &'zf $name<Store1>) -> Self {
                $name::from_store(zerofrom::ZeroFrom::zero_from(&other.store))
            }
        }
    };
}

#[cfg(feature = "alloc")]
fn string_to_box_u8(input: String) -> Box<[u8]> {
    input.into_boxed_str().into_boxed_bytes()
}

#[doc(hidden)] // subject to change
#[cfg(feature = "alloc")]
pub type ZeroTrieStringIterator<'a> =
    core::iter::Map<reader::ZeroTrieIterator<'a>, fn((Vec<u8>, usize)) -> (String, usize)>;

impl_zerotrie_subtype!(
    ZeroTrieSimpleAscii,
    String,
    reader::get_iter_ascii_or_panic,
    ZeroTrieStringIterator,
    string_to_box_u8
);
impl_zerotrie_subtype!(
    ZeroAsciiIgnoreCaseTrie,
    String,
    reader::get_iter_ascii_or_panic,
    ZeroTrieStringIterator,
    string_to_box_u8
);
impl_zerotrie_subtype!(
    ZeroTriePerfectHash,
    Vec<u8>,
    reader::get_iter_phf,
    reader::ZeroTrieIterator<'_>,
    Vec::into_boxed_slice
);
impl_zerotrie_subtype!(
    ZeroTrieExtendedCapacity,
    Vec<u8>,
    reader::get_iter_phf,
    reader::ZeroTrieIterator<'_>,
    Vec::into_boxed_slice
);

macro_rules! impl_dispatch {
    ($self:ident, $inner_fn:ident()) => {
        match $self.0 {
            ZeroTrieFlavor::SimpleAscii(subtype) => subtype.$inner_fn(),
            ZeroTrieFlavor::PerfectHash(subtype) => subtype.$inner_fn(),
            ZeroTrieFlavor::ExtendedCapacity(subtype) => subtype.$inner_fn(),
        }
    };
    ($self:ident, $inner_fn:ident().into_zerotrie()) => {
        match $self.0 {
            ZeroTrieFlavor::SimpleAscii(subtype) => subtype.$inner_fn().into_zerotrie(),
            ZeroTrieFlavor::PerfectHash(subtype) => subtype.$inner_fn().into_zerotrie(),
            ZeroTrieFlavor::ExtendedCapacity(subtype) => subtype.$inner_fn().into_zerotrie(),
        }
    };
    (&$self:ident, $inner_fn:ident()) => {
        match &$self.0 {
            ZeroTrieFlavor::SimpleAscii(subtype) => subtype.$inner_fn(),
            ZeroTrieFlavor::PerfectHash(subtype) => subtype.$inner_fn(),
            ZeroTrieFlavor::ExtendedCapacity(subtype) => subtype.$inner_fn(),
        }
    };
    ($self:ident, $inner_fn:ident($arg:ident)) => {
        match $self.0 {
            ZeroTrieFlavor::SimpleAscii(subtype) => subtype.$inner_fn($arg),
            ZeroTrieFlavor::PerfectHash(subtype) => subtype.$inner_fn($arg),
            ZeroTrieFlavor::ExtendedCapacity(subtype) => subtype.$inner_fn($arg),
        }
    };
    (&$self:ident, $inner_fn:ident($arg:ident)) => {
        match &$self.0 {
            ZeroTrieFlavor::SimpleAscii(subtype) => subtype.$inner_fn($arg),
            ZeroTrieFlavor::PerfectHash(subtype) => subtype.$inner_fn($arg),
            ZeroTrieFlavor::ExtendedCapacity(subtype) => subtype.$inner_fn($arg),
        }
    };
    (&$self:ident, $trait:ident::$inner_fn:ident()) => {
        match &$self.0 {
            ZeroTrieFlavor::SimpleAscii(subtype) => {
                ZeroTrie(ZeroTrieFlavor::SimpleAscii($trait::$inner_fn(subtype)))
            }
            ZeroTrieFlavor::PerfectHash(subtype) => {
                ZeroTrie(ZeroTrieFlavor::PerfectHash($trait::$inner_fn(subtype)))
            }
            ZeroTrieFlavor::ExtendedCapacity(subtype) => {
                ZeroTrie(ZeroTrieFlavor::ExtendedCapacity($trait::$inner_fn(subtype)))
            }
        }
    };
}

impl<Store> ZeroTrie<Store> {
    /// Takes the byte store from this trie.
    pub fn into_store(self) -> Store {
        impl_dispatch!(self, into_store())
    }
    /// Converts this trie's store to a different store implementing the `From` trait.
    ///
    /// For example, use this to change `ZeroTrie<Vec<u8>>` to `ZeroTrie<Cow<[u8]>>`.
    pub fn convert_store<NewStore>(self) -> ZeroTrie<NewStore>
    where
        NewStore: From<Store>,
    {
        impl_dispatch!(self, convert_store().into_zerotrie())
    }
}

impl<Store> ZeroTrie<Store>
where
    Store: AsRef<[u8]>,
{
    /// Queries the trie for a string.
    pub fn get<K>(&self, key: K) -> Option<usize>
    where
        K: AsRef<[u8]>,
    {
        impl_dispatch!(&self, get(key))
    }
    /// Returns `true` if the trie is empty.
    pub fn is_empty(&self) -> bool {
        impl_dispatch!(&self, is_empty())
    }
    /// Returns the size of the trie in number of bytes.
    ///
    /// To get the number of keys in the trie, use `.iter().count()`.
    pub fn byte_len(&self) -> usize {
        impl_dispatch!(&self, byte_len())
    }
}

#[cfg(feature = "alloc")]
impl<Store> ZeroTrie<Store>
where
    Store: AsRef<[u8]>,
{
    /// Exports the data from this ZeroTrie into a BTreeMap.
    pub fn to_btreemap(&self) -> BTreeMap<Box<[u8]>, usize> {
        impl_dispatch!(&self, to_btreemap_bytes())
    }
}

#[cfg(feature = "litemap")]
impl<Store> ZeroTrie<Store>
where
    Store: AsRef<[u8]>,
{
    /// Exports the data from this ZeroTrie into a LiteMap.
    pub fn to_litemap(&self) -> LiteMap<Box<[u8]>, usize> {
        impl_dispatch!(&self, to_litemap_bytes())
    }
}

#[cfg(feature = "alloc")]
impl ZeroTrie<Vec<u8>> {
    pub(crate) fn try_from_tuple_slice(
        items: &[(&ByteStr, usize)],
    ) -> Result<Self, ZeroTrieBuildError> {
        let is_all_ascii = items.iter().all(|(s, _)| s.is_all_ascii());
        if is_all_ascii && items.len() < 512 {
            ZeroTrieSimpleAscii::try_from_tuple_slice(items).map(|x| x.into_zerotrie())
        } else {
            ZeroTriePerfectHash::try_from_tuple_slice(items).map(|x| x.into_zerotrie())
        }
    }
}

#[cfg(feature = "alloc")]
impl<K> FromIterator<(K, usize)> for ZeroTrie<Vec<u8>>
where
    K: AsRef<[u8]>,
{
    fn from_iter<T: IntoIterator<Item = (K, usize)>>(iter: T) -> Self {
        // We need two Vecs because the first one anchors the `K`s that the second one borrows.
        let items = Vec::from_iter(iter);
        let mut items: Vec<(&[u8], usize)> = items.iter().map(|(k, v)| (k.as_ref(), *v)).collect();
        items.sort();
        let byte_str_slice = ByteStr::from_byte_slice_with_value(&items);
        #[allow(clippy::unwrap_used)] // FromIterator is panicky
        Self::try_from_tuple_slice(byte_str_slice).unwrap()
    }
}

#[cfg(feature = "databake")]
impl<Store> databake::Bake for ZeroTrie<Store>
where
    Store: databake::Bake,
{
    fn bake(&self, env: &databake::CrateEnv) -> databake::TokenStream {
        use databake::*;
        let inner = impl_dispatch!(&self, bake(env));
        quote! { #inner.into_zerotrie() }
    }
}

#[cfg(feature = "databake")]
impl<Store> databake::BakeSize for ZeroTrie<Store>
where
    Store: databake::BakeSize,
{
    fn borrows_size(&self) -> usize {
        impl_dispatch!(&self, borrows_size())
    }
}

#[cfg(feature = "zerofrom")]
impl<'zf, Store1, Store2> zerofrom::ZeroFrom<'zf, ZeroTrie<Store1>> for ZeroTrie<Store2>
where
    Store2: zerofrom::ZeroFrom<'zf, Store1>,
{
    fn zero_from(other: &'zf ZeroTrie<Store1>) -> Self {
        use zerofrom::ZeroFrom;
        impl_dispatch!(&other, ZeroFrom::zero_from())
    }
}

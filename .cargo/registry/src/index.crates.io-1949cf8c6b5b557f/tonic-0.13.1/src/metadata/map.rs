use http::HeaderName;

pub(crate) use self::as_encoding_agnostic_metadata_key::AsEncodingAgnosticMetadataKey;
pub(crate) use self::as_metadata_key::AsMetadataKey;
pub(crate) use self::into_metadata_key::IntoMetadataKey;

use super::encoding::{Ascii, Binary, ValueEncoding};
use super::key::{InvalidMetadataKey, MetadataKey};
use super::value::MetadataValue;

use std::marker::PhantomData;

/// A set of gRPC custom metadata entries.
///
/// # Examples
///
/// Basic usage
///
/// ```
/// # use tonic::metadata::*;
/// let mut map = MetadataMap::new();
///
/// map.insert("x-host", "example.com".parse().unwrap());
/// map.insert("x-number", "123".parse().unwrap());
/// map.insert_bin("trace-proto-bin", MetadataValue::from_bytes(b"[binary data]"));
///
/// assert!(map.contains_key("x-host"));
/// assert!(!map.contains_key("x-location"));
///
/// assert_eq!(map.get("x-host").unwrap(), "example.com");
///
/// map.remove("x-host");
///
/// assert!(!map.contains_key("x-host"));
/// ```
#[derive(Clone, Debug, Default)]
pub struct MetadataMap {
    headers: http::HeaderMap,
}

impl AsRef<http::HeaderMap> for MetadataMap {
    fn as_ref(&self) -> &http::HeaderMap {
        &self.headers
    }
}

impl AsMut<http::HeaderMap> for MetadataMap {
    fn as_mut(&mut self) -> &mut http::HeaderMap {
        &mut self.headers
    }
}

/// `MetadataMap` entry iterator.
///
/// Yields `KeyAndValueRef` values. The same header name may be yielded
/// more than once if it has more than one associated value.
#[derive(Debug)]
pub struct Iter<'a> {
    inner: http::header::Iter<'a, http::header::HeaderValue>,
}

/// Reference to a key and an associated value in a `MetadataMap`. It can point
/// to either an ascii or a binary ("*-bin") key.
#[derive(Debug)]
pub enum KeyAndValueRef<'a> {
    /// An ascii metadata key and value.
    Ascii(&'a MetadataKey<Ascii>, &'a MetadataValue<Ascii>),
    /// A binary metadata key and value.
    Binary(&'a MetadataKey<Binary>, &'a MetadataValue<Binary>),
}

/// Reference to a key and an associated value in a `MetadataMap`. It can point
/// to either an ascii or a binary ("*-bin") key.
#[derive(Debug)]
pub enum KeyAndMutValueRef<'a> {
    /// An ascii metadata key and value.
    Ascii(&'a MetadataKey<Ascii>, &'a mut MetadataValue<Ascii>),
    /// A binary metadata key and value.
    Binary(&'a MetadataKey<Binary>, &'a mut MetadataValue<Binary>),
}

/// `MetadataMap` entry iterator.
///
/// Yields `(&MetadataKey, &mut value)` tuples. The same header name may be yielded
/// more than once if it has more than one associated value.
#[derive(Debug)]
pub struct IterMut<'a> {
    inner: http::header::IterMut<'a, http::header::HeaderValue>,
}

/// A drain iterator of all values associated with a single metadata key.
#[derive(Debug)]
pub struct ValueDrain<'a, VE: ValueEncoding> {
    inner: http::header::ValueDrain<'a, http::header::HeaderValue>,
    phantom: PhantomData<VE>,
}

/// An iterator over `MetadataMap` keys.
///
/// Yields `KeyRef` values. Each header name is yielded only once, even if it
/// has more than one associated value.
#[derive(Debug)]
pub struct Keys<'a> {
    inner: http::header::Keys<'a, http::header::HeaderValue>,
}

/// Reference to a key in a `MetadataMap`. It can point
/// to either an ascii or a binary ("*-bin") key.
#[derive(Debug)]
pub enum KeyRef<'a> {
    /// An ascii metadata key and value.
    Ascii(&'a MetadataKey<Ascii>),
    /// A binary metadata key and value.
    Binary(&'a MetadataKey<Binary>),
}

/// `MetadataMap` value iterator.
///
/// Yields `ValueRef` values. Each value contained in the `MetadataMap` will be
/// yielded.
#[derive(Debug)]
pub struct Values<'a> {
    // Need to use http::header::Iter and not http::header::Values to be able
    // to know if a value is binary or not.
    inner: http::header::Iter<'a, http::header::HeaderValue>,
}

/// Reference to a value in a `MetadataMap`. It can point
/// to either an ascii or a binary ("*-bin" key) value.
#[derive(Debug)]
pub enum ValueRef<'a> {
    /// An ascii metadata key and value.
    Ascii(&'a MetadataValue<Ascii>),
    /// A binary metadata key and value.
    Binary(&'a MetadataValue<Binary>),
}

/// `MetadataMap` value iterator.
///
/// Each value contained in the `MetadataMap` will be yielded.
#[derive(Debug)]
pub struct ValuesMut<'a> {
    // Need to use http::header::IterMut and not http::header::ValuesMut to be
    // able to know if a value is binary or not.
    inner: http::header::IterMut<'a, http::header::HeaderValue>,
}

/// Reference to a value in a `MetadataMap`. It can point
/// to either an ascii or a binary ("*-bin" key) value.
#[derive(Debug)]
pub enum ValueRefMut<'a> {
    /// An ascii metadata key and value.
    Ascii(&'a mut MetadataValue<Ascii>),
    /// A binary metadata key and value.
    Binary(&'a mut MetadataValue<Binary>),
}

/// An iterator of all values associated with a single metadata key.
#[derive(Debug)]
pub struct ValueIter<'a, VE: ValueEncoding> {
    inner: Option<http::header::ValueIter<'a, http::header::HeaderValue>>,
    phantom: PhantomData<VE>,
}

/// An iterator of all values associated with a single metadata key.
#[derive(Debug)]
pub struct ValueIterMut<'a, VE: ValueEncoding> {
    inner: http::header::ValueIterMut<'a, http::header::HeaderValue>,
    phantom: PhantomData<VE>,
}

/// A view to all values stored in a single entry.
///
/// This struct is returned by `MetadataMap::get_all` and
/// `MetadataMap::get_all_bin`.
#[derive(Debug)]
pub struct GetAll<'a, VE: ValueEncoding> {
    inner: Option<http::header::GetAll<'a, http::header::HeaderValue>>,
    phantom: PhantomData<VE>,
}

/// A view into a single location in a `MetadataMap`, which may be vacant or
/// occupied.
#[derive(Debug)]
pub enum Entry<'a, VE: ValueEncoding> {
    /// An occupied entry
    Occupied(OccupiedEntry<'a, VE>),

    /// A vacant entry
    Vacant(VacantEntry<'a, VE>),
}

/// A view into a single empty location in a `MetadataMap`.
///
/// This struct is returned as part of the `Entry` enum.
#[derive(Debug)]
pub struct VacantEntry<'a, VE: ValueEncoding> {
    inner: http::header::VacantEntry<'a, http::header::HeaderValue>,
    phantom: PhantomData<VE>,
}

/// A view into a single occupied location in a `MetadataMap`.
///
/// This struct is returned as part of the `Entry` enum.
#[derive(Debug)]
pub struct OccupiedEntry<'a, VE: ValueEncoding> {
    inner: http::header::OccupiedEntry<'a, http::header::HeaderValue>,
    phantom: PhantomData<VE>,
}

pub(crate) const GRPC_TIMEOUT_HEADER: &str = "grpc-timeout";

// ===== impl MetadataMap =====

impl MetadataMap {
    // Headers reserved by the gRPC protocol.
    pub(crate) const GRPC_RESERVED_HEADERS: [HeaderName; 6] = [
        HeaderName::from_static("te"),
        HeaderName::from_static("user-agent"),
        HeaderName::from_static("content-type"),
        HeaderName::from_static("grpc-message"),
        HeaderName::from_static("grpc-message-type"),
        HeaderName::from_static("grpc-status"),
    ];

    /// Create an empty `MetadataMap`.
    ///
    /// The map will be created without any capacity. This function will not
    /// allocate.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let map = MetadataMap::new();
    ///
    /// assert!(map.is_empty());
    /// assert_eq!(0, map.capacity());
    /// ```
    pub fn new() -> Self {
        MetadataMap::with_capacity(0)
    }

    /// Convert an HTTP HeaderMap to a MetadataMap
    pub fn from_headers(headers: http::HeaderMap) -> Self {
        MetadataMap { headers }
    }

    /// Convert a MetadataMap into a HTTP HeaderMap
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// map.insert("x-host", "example.com".parse().unwrap());
    ///
    /// let http_map = map.into_headers();
    ///
    /// assert_eq!(http_map.get("x-host").unwrap(), "example.com");
    /// ```
    pub fn into_headers(self) -> http::HeaderMap {
        self.headers
    }

    pub(crate) fn into_sanitized_headers(mut self) -> http::HeaderMap {
        for r in &Self::GRPC_RESERVED_HEADERS {
            self.headers.remove(r);
        }
        self.headers
    }

    /// Create an empty `MetadataMap` with the specified capacity.
    ///
    /// The returned map will allocate internal storage in order to hold about
    /// `capacity` elements without reallocating. However, this is a "best
    /// effort" as there are usage patterns that could cause additional
    /// allocations before `capacity` metadata entries are stored in the map.
    ///
    /// More capacity than requested may be allocated.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let map: MetadataMap = MetadataMap::with_capacity(10);
    ///
    /// assert!(map.is_empty());
    /// assert!(map.capacity() >= 10);
    /// ```
    pub fn with_capacity(capacity: usize) -> MetadataMap {
        MetadataMap {
            headers: http::HeaderMap::with_capacity(capacity),
        }
    }

    /// Returns the number of metadata entries (ascii and binary) stored in the
    /// map.
    ///
    /// This number represents the total number of **values** stored in the map.
    /// This number can be greater than or equal to the number of **keys**
    /// stored given that a single key may have more than one associated value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    ///
    /// assert_eq!(0, map.len());
    ///
    /// map.insert("x-host-ip", "127.0.0.1".parse().unwrap());
    /// map.insert_bin("x-host-name-bin", MetadataValue::from_bytes(b"localhost"));
    ///
    /// assert_eq!(2, map.len());
    ///
    /// map.append("x-host-ip", "text/html".parse().unwrap());
    ///
    /// assert_eq!(3, map.len());
    /// ```
    pub fn len(&self) -> usize {
        self.headers.len()
    }

    /// Returns the number of keys (ascii and binary) stored in the map.
    ///
    /// This number will be less than or equal to `len()` as each key may have
    /// more than one associated value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    ///
    /// assert_eq!(0, map.keys_len());
    ///
    /// map.insert("x-host-ip", "127.0.0.1".parse().unwrap());
    /// map.insert_bin("x-host-name-bin", MetadataValue::from_bytes(b"localhost"));
    ///
    /// assert_eq!(2, map.keys_len());
    ///
    /// map.append("x-host-ip", "text/html".parse().unwrap());
    ///
    /// assert_eq!(2, map.keys_len());
    /// ```
    pub fn keys_len(&self) -> usize {
        self.headers.keys_len()
    }

    /// Returns true if the map contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    ///
    /// assert!(map.is_empty());
    ///
    /// map.insert("x-host", "hello.world".parse().unwrap());
    ///
    /// assert!(!map.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.headers.is_empty()
    }

    /// Clears the map, removing all key-value pairs. Keeps the allocated memory
    /// for reuse.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// map.insert("x-host", "hello.world".parse().unwrap());
    ///
    /// map.clear();
    /// assert!(map.is_empty());
    /// assert!(map.capacity() > 0);
    /// ```
    pub fn clear(&mut self) {
        self.headers.clear();
    }

    /// Returns the number of custom metadata entries the map can hold without
    /// reallocating.
    ///
    /// This number is an approximation as certain usage patterns could cause
    /// additional allocations before the returned capacity is filled.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    ///
    /// assert_eq!(0, map.capacity());
    ///
    /// map.insert("x-host", "hello.world".parse().unwrap());
    /// assert_eq!(6, map.capacity());
    /// ```
    pub fn capacity(&self) -> usize {
        self.headers.capacity()
    }

    /// Reserves capacity for at least `additional` more custom metadata to be
    /// inserted into the `MetadataMap`.
    ///
    /// The metadata map may reserve more space to avoid frequent reallocations.
    /// Like with `with_capacity`, this will be a "best effort" to avoid
    /// allocations until `additional` more custom metadata is inserted. Certain
    /// usage patterns could cause additional allocations before the number is
    /// reached.
    ///
    /// # Panics
    ///
    /// Panics if the new allocation size overflows `usize`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// map.reserve(10);
    /// # map.insert("x-host", "bar".parse().unwrap());
    /// ```
    pub fn reserve(&mut self, additional: usize) {
        self.headers.reserve(additional);
    }

    /// Returns a reference to the value associated with the key. This method
    /// is for ascii metadata entries (those whose names don't end with
    /// "-bin"). For binary entries, use get_bin.
    ///
    /// If there are multiple values associated with the key, then the first one
    /// is returned. Use `get_all` to get all values associated with a given
    /// key. Returns `None` if there are no values associated with the key.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// assert!(map.get("x-host").is_none());
    ///
    /// map.insert("x-host", "hello".parse().unwrap());
    /// assert_eq!(map.get("x-host").unwrap(), &"hello");
    /// assert_eq!(map.get("x-host").unwrap(), &"hello");
    ///
    /// map.append("x-host", "world".parse().unwrap());
    /// assert_eq!(map.get("x-host").unwrap(), &"hello");
    ///
    /// // Attempting to read a key of the wrong type fails by not
    /// // finding anything.
    /// map.append_bin("host-bin", MetadataValue::from_bytes(b"world"));
    /// assert!(map.get("host-bin").is_none());
    /// assert!(map.get("host-bin".to_string()).is_none());
    /// assert!(map.get(&("host-bin".to_string())).is_none());
    ///
    /// // Attempting to read an invalid key string fails by not
    /// // finding anything.
    /// assert!(map.get("host{}bin").is_none());
    /// assert!(map.get("host{}bin".to_string()).is_none());
    /// assert!(map.get(&("host{}bin".to_string())).is_none());
    /// ```
    pub fn get<K>(&self, key: K) -> Option<&MetadataValue<Ascii>>
    where
        K: AsMetadataKey<Ascii>,
    {
        key.get(self)
    }

    /// Like get, but for Binary keys (for example "trace-proto-bin").
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// assert!(map.get_bin("trace-proto-bin").is_none());
    ///
    /// map.insert_bin("trace-proto-bin", MetadataValue::from_bytes(b"hello"));
    /// assert_eq!(map.get_bin("trace-proto-bin").unwrap(), &"hello");
    /// assert_eq!(map.get_bin("trace-proto-bin").unwrap(), &"hello");
    ///
    /// map.append_bin("trace-proto-bin", MetadataValue::from_bytes(b"world"));
    /// assert_eq!(map.get_bin("trace-proto-bin").unwrap(), &"hello");
    ///
    /// // Attempting to read a key of the wrong type fails by not
    /// // finding anything.
    /// map.append("host", "world".parse().unwrap());
    /// assert!(map.get_bin("host").is_none());
    /// assert!(map.get_bin("host".to_string()).is_none());
    /// assert!(map.get_bin(&("host".to_string())).is_none());
    ///
    /// // Attempting to read an invalid key string fails by not
    /// // finding anything.
    /// assert!(map.get_bin("host{}-bin").is_none());
    /// assert!(map.get_bin("host{}-bin".to_string()).is_none());
    /// assert!(map.get_bin(&("host{}-bin".to_string())).is_none());
    /// ```
    pub fn get_bin<K>(&self, key: K) -> Option<&MetadataValue<Binary>>
    where
        K: AsMetadataKey<Binary>,
    {
        key.get(self)
    }

    /// Returns a mutable reference to the value associated with the key. This
    /// method is for ascii metadata entries (those whose names don't end with
    /// "-bin"). For binary entries, use get_mut_bin.
    ///
    /// If there are multiple values associated with the key, then the first one
    /// is returned. Use `entry` to get all values associated with a given
    /// key. Returns `None` if there are no values associated with the key.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::default();
    /// map.insert("x-host", "hello".parse().unwrap());
    /// map.get_mut("x-host").unwrap().set_sensitive(true);
    ///
    /// assert!(map.get("x-host").unwrap().is_sensitive());
    ///
    /// // Attempting to read a key of the wrong type fails by not
    /// // finding anything.
    /// map.append_bin("host-bin", MetadataValue::from_bytes(b"world"));
    /// assert!(map.get_mut("host-bin").is_none());
    /// assert!(map.get_mut("host-bin".to_string()).is_none());
    /// assert!(map.get_mut(&("host-bin".to_string())).is_none());
    ///
    /// // Attempting to read an invalid key string fails by not
    /// // finding anything.
    /// assert!(map.get_mut("host{}").is_none());
    /// assert!(map.get_mut("host{}".to_string()).is_none());
    /// assert!(map.get_mut(&("host{}".to_string())).is_none());
    /// ```
    pub fn get_mut<K>(&mut self, key: K) -> Option<&mut MetadataValue<Ascii>>
    where
        K: AsMetadataKey<Ascii>,
    {
        key.get_mut(self)
    }

    /// Like get_mut, but for Binary keys (for example "trace-proto-bin").
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::default();
    /// map.insert_bin("trace-proto-bin", MetadataValue::from_bytes(b"hello"));
    /// map.get_bin_mut("trace-proto-bin").unwrap().set_sensitive(true);
    ///
    /// assert!(map.get_bin("trace-proto-bin").unwrap().is_sensitive());
    ///
    /// // Attempting to read a key of the wrong type fails by not
    /// // finding anything.
    /// map.append("host", "world".parse().unwrap());
    /// assert!(map.get_bin_mut("host").is_none());
    /// assert!(map.get_bin_mut("host".to_string()).is_none());
    /// assert!(map.get_bin_mut(&("host".to_string())).is_none());
    ///
    /// // Attempting to read an invalid key string fails by not
    /// // finding anything.
    /// assert!(map.get_bin_mut("host{}-bin").is_none());
    /// assert!(map.get_bin_mut("host{}-bin".to_string()).is_none());
    /// assert!(map.get_bin_mut(&("host{}-bin".to_string())).is_none());
    /// ```
    pub fn get_bin_mut<K>(&mut self, key: K) -> Option<&mut MetadataValue<Binary>>
    where
        K: AsMetadataKey<Binary>,
    {
        key.get_mut(self)
    }

    /// Returns a view of all values associated with a key. This method is for
    /// ascii metadata entries (those whose names don't end with "-bin"). For
    /// binary entries, use get_all_bin.
    ///
    /// The returned view does not incur any allocations and allows iterating
    /// the values associated with the key.  See [`GetAll`] for more details.
    /// Returns `None` if there are no values associated with the key.
    ///
    /// [`GetAll`]: struct.GetAll.html
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    ///
    /// map.insert("x-host", "hello".parse().unwrap());
    /// map.append("x-host", "goodbye".parse().unwrap());
    ///
    /// {
    ///     let view = map.get_all("x-host");
    ///
    ///     let mut iter = view.iter();
    ///     assert_eq!(&"hello", iter.next().unwrap());
    ///     assert_eq!(&"goodbye", iter.next().unwrap());
    ///     assert!(iter.next().is_none());
    /// }
    ///
    /// // Attempting to read a key of the wrong type fails by not
    /// // finding anything.
    /// map.append_bin("host-bin", MetadataValue::from_bytes(b"world"));
    /// assert!(map.get_all("host-bin").iter().next().is_none());
    /// assert!(map.get_all("host-bin".to_string()).iter().next().is_none());
    /// assert!(map.get_all(&("host-bin".to_string())).iter().next().is_none());
    ///
    /// // Attempting to read an invalid key string fails by not
    /// // finding anything.
    /// assert!(map.get_all("host{}").iter().next().is_none());
    /// assert!(map.get_all("host{}".to_string()).iter().next().is_none());
    /// assert!(map.get_all(&("host{}".to_string())).iter().next().is_none());
    /// ```
    pub fn get_all<K>(&self, key: K) -> GetAll<'_, Ascii>
    where
        K: AsMetadataKey<Ascii>,
    {
        GetAll {
            inner: key.get_all(self),
            phantom: PhantomData,
        }
    }

    /// Like get_all, but for Binary keys (for example "trace-proto-bin").
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    ///
    /// map.insert_bin("trace-proto-bin", MetadataValue::from_bytes(b"hello"));
    /// map.append_bin("trace-proto-bin", MetadataValue::from_bytes(b"goodbye"));
    ///
    /// {
    ///     let view = map.get_all_bin("trace-proto-bin");
    ///
    ///     let mut iter = view.iter();
    ///     assert_eq!(&"hello", iter.next().unwrap());
    ///     assert_eq!(&"goodbye", iter.next().unwrap());
    ///     assert!(iter.next().is_none());
    /// }
    ///
    /// // Attempting to read a key of the wrong type fails by not
    /// // finding anything.
    /// map.append("host", "world".parse().unwrap());
    /// assert!(map.get_all_bin("host").iter().next().is_none());
    /// assert!(map.get_all_bin("host".to_string()).iter().next().is_none());
    /// assert!(map.get_all_bin(&("host".to_string())).iter().next().is_none());
    ///
    /// // Attempting to read an invalid key string fails by not
    /// // finding anything.
    /// assert!(map.get_all_bin("host{}-bin").iter().next().is_none());
    /// assert!(map.get_all_bin("host{}-bin".to_string()).iter().next().is_none());
    /// assert!(map.get_all_bin(&("host{}-bin".to_string())).iter().next().is_none());
    /// ```
    pub fn get_all_bin<K>(&self, key: K) -> GetAll<'_, Binary>
    where
        K: AsMetadataKey<Binary>,
    {
        GetAll {
            inner: key.get_all(self),
            phantom: PhantomData,
        }
    }

    /// Returns true if the map contains a value for the specified key. This
    /// method works for both ascii and binary entries.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// assert!(!map.contains_key("x-host"));
    ///
    /// map.append_bin("host-bin", MetadataValue::from_bytes(b"world"));
    /// map.insert("x-host", "world".parse().unwrap());
    ///
    /// // contains_key works for both Binary and Ascii keys:
    /// assert!(map.contains_key("x-host"));
    /// assert!(map.contains_key("host-bin"));
    ///
    /// // contains_key returns false for invalid keys:
    /// assert!(!map.contains_key("x{}host"));
    /// ```
    pub fn contains_key<K>(&self, key: K) -> bool
    where
        K: AsEncodingAgnosticMetadataKey,
    {
        key.contains_key(self)
    }

    /// An iterator visiting all key-value pairs (both ascii and binary).
    ///
    /// The iteration order is arbitrary, but consistent across platforms for
    /// the same crate version. Each key will be yielded once per associated
    /// value. So, if a key has 3 associated values, it will be yielded 3 times.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    ///
    /// map.insert("x-word", "hello".parse().unwrap());
    /// map.append("x-word", "goodbye".parse().unwrap());
    /// map.insert("x-number", "123".parse().unwrap());
    ///
    /// for key_and_value in map.iter() {
    ///     match key_and_value {
    ///         KeyAndValueRef::Ascii(ref key, ref value) =>
    ///             println!("Ascii: {:?}: {:?}", key, value),
    ///         KeyAndValueRef::Binary(ref key, ref value) =>
    ///             println!("Binary: {:?}: {:?}", key, value),
    ///     }
    /// }
    /// ```
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            inner: self.headers.iter(),
        }
    }

    /// An iterator visiting all key-value pairs, with mutable value references.
    ///
    /// The iterator order is arbitrary, but consistent across platforms for the
    /// same crate version. Each key will be yielded once per associated value,
    /// so if a key has 3 associated values, it will be yielded 3 times.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    ///
    /// map.insert("x-word", "hello".parse().unwrap());
    /// map.append("x-word", "goodbye".parse().unwrap());
    /// map.insert("x-number", "123".parse().unwrap());
    ///
    /// for key_and_value in map.iter_mut() {
    ///     match key_and_value {
    ///         KeyAndMutValueRef::Ascii(key, mut value) =>
    ///             value.set_sensitive(true),
    ///         KeyAndMutValueRef::Binary(key, mut value) =>
    ///             value.set_sensitive(false),
    ///     }
    /// }
    /// ```
    pub fn iter_mut(&mut self) -> IterMut<'_> {
        IterMut {
            inner: self.headers.iter_mut(),
        }
    }

    /// An iterator visiting all keys.
    ///
    /// The iteration order is arbitrary, but consistent across platforms for
    /// the same crate version. Each key will be yielded only once even if it
    /// has multiple associated values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    ///
    /// map.insert("x-word", "hello".parse().unwrap());
    /// map.append("x-word", "goodbye".parse().unwrap());
    /// map.insert_bin("x-number-bin", MetadataValue::from_bytes(b"123"));
    ///
    /// for key in map.keys() {
    ///     match key {
    ///         KeyRef::Ascii(ref key) =>
    ///             println!("Ascii key: {:?}", key),
    ///         KeyRef::Binary(ref key) =>
    ///             println!("Binary key: {:?}", key),
    ///     }
    ///     println!("{:?}", key);
    /// }
    /// ```
    pub fn keys(&self) -> Keys<'_> {
        Keys {
            inner: self.headers.keys(),
        }
    }

    /// An iterator visiting all values (both ascii and binary).
    ///
    /// The iteration order is arbitrary, but consistent across platforms for
    /// the same crate version.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    ///
    /// map.insert("x-word", "hello".parse().unwrap());
    /// map.append("x-word", "goodbye".parse().unwrap());
    /// map.insert_bin("x-number-bin", MetadataValue::from_bytes(b"123"));
    ///
    /// for value in map.values() {
    ///     match value {
    ///         ValueRef::Ascii(ref value) =>
    ///             println!("Ascii value: {:?}", value),
    ///         ValueRef::Binary(ref value) =>
    ///             println!("Binary value: {:?}", value),
    ///     }
    ///     println!("{:?}", value);
    /// }
    /// ```
    pub fn values(&self) -> Values<'_> {
        Values {
            inner: self.headers.iter(),
        }
    }

    /// An iterator visiting all values mutably.
    ///
    /// The iteration order is arbitrary, but consistent across platforms for
    /// the same crate version.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::default();
    ///
    /// map.insert("x-word", "hello".parse().unwrap());
    /// map.append("x-word", "goodbye".parse().unwrap());
    /// map.insert("x-number", "123".parse().unwrap());
    ///
    /// for value in map.values_mut() {
    ///     match value {
    ///         ValueRefMut::Ascii(mut value) =>
    ///             value.set_sensitive(true),
    ///         ValueRefMut::Binary(mut value) =>
    ///             value.set_sensitive(false),
    ///     }
    /// }
    /// ```
    pub fn values_mut(&mut self) -> ValuesMut<'_> {
        ValuesMut {
            inner: self.headers.iter_mut(),
        }
    }

    /// Gets the given ascii key's corresponding entry in the map for in-place
    /// manipulation. For binary keys, use `entry_bin`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::default();
    ///
    /// let headers = &[
    ///     "content-length",
    ///     "x-hello",
    ///     "Content-Length",
    ///     "x-world",
    /// ];
    ///
    /// for &header in headers {
    ///     let counter = map.entry(header).unwrap().or_insert("".parse().unwrap());
    ///     *counter = format!("{}{}", counter.to_str().unwrap(), "1").parse().unwrap();
    /// }
    ///
    /// assert_eq!(map.get("content-length").unwrap(), "11");
    /// assert_eq!(map.get("x-hello").unwrap(), "1");
    ///
    /// // Gracefully handles parting invalid key strings
    /// assert!(!map.entry("a{}b").is_ok());
    ///
    /// // Attempting to read a key of the wrong type fails by not
    /// // finding anything.
    /// map.append_bin("host-bin", MetadataValue::from_bytes(b"world"));
    /// assert!(!map.entry("host-bin").is_ok());
    /// assert!(!map.entry("host-bin".to_string()).is_ok());
    /// assert!(!map.entry(&("host-bin".to_string())).is_ok());
    ///
    /// // Attempting to read an invalid key string fails by not
    /// // finding anything.
    /// assert!(!map.entry("host{}").is_ok());
    /// assert!(!map.entry("host{}".to_string()).is_ok());
    /// assert!(!map.entry(&("host{}".to_string())).is_ok());
    /// ```
    pub fn entry<K>(&mut self, key: K) -> Result<Entry<'_, Ascii>, InvalidMetadataKey>
    where
        K: AsMetadataKey<Ascii>,
    {
        self.generic_entry::<Ascii, K>(key)
    }

    /// Gets the given Binary key's corresponding entry in the map for in-place
    /// manipulation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// # use std::str;
    /// let mut map = MetadataMap::default();
    ///
    /// let headers = &[
    ///     "content-length-bin",
    ///     "x-hello-bin",
    ///     "Content-Length-bin",
    ///     "x-world-bin",
    /// ];
    ///
    /// for &header in headers {
    ///     let counter = map.entry_bin(header).unwrap().or_insert(MetadataValue::from_bytes(b""));
    ///     *counter = MetadataValue::from_bytes(format!("{}{}", str::from_utf8(counter.to_bytes().unwrap().as_ref()).unwrap(), "1").as_bytes());
    /// }
    ///
    /// assert_eq!(map.get_bin("content-length-bin").unwrap(), "11");
    /// assert_eq!(map.get_bin("x-hello-bin").unwrap(), "1");
    ///
    /// // Attempting to read a key of the wrong type fails by not
    /// // finding anything.
    /// map.append("host", "world".parse().unwrap());
    /// assert!(!map.entry_bin("host").is_ok());
    /// assert!(!map.entry_bin("host".to_string()).is_ok());
    /// assert!(!map.entry_bin(&("host".to_string())).is_ok());
    ///
    /// // Attempting to read an invalid key string fails by not
    /// // finding anything.
    /// assert!(!map.entry_bin("host{}-bin").is_ok());
    /// assert!(!map.entry_bin("host{}-bin".to_string()).is_ok());
    /// assert!(!map.entry_bin(&("host{}-bin".to_string())).is_ok());
    /// ```
    pub fn entry_bin<K>(&mut self, key: K) -> Result<Entry<'_, Binary>, InvalidMetadataKey>
    where
        K: AsMetadataKey<Binary>,
    {
        self.generic_entry::<Binary, K>(key)
    }

    fn generic_entry<VE: ValueEncoding, K>(
        &mut self,
        key: K,
    ) -> Result<Entry<'_, VE>, InvalidMetadataKey>
    where
        K: AsMetadataKey<VE>,
    {
        match key.entry(self) {
            Ok(entry) => Ok(match entry {
                http::header::Entry::Occupied(e) => Entry::Occupied(OccupiedEntry {
                    inner: e,
                    phantom: PhantomData,
                }),
                http::header::Entry::Vacant(e) => Entry::Vacant(VacantEntry {
                    inner: e,
                    phantom: PhantomData,
                }),
            }),
            Err(err) => Err(err),
        }
    }

    /// Inserts an ascii key-value pair into the map. To insert a binary entry,
    /// use `insert_bin`.
    ///
    /// This method panics when the given key is a string and it cannot be
    /// converted to a `MetadataKey<Ascii>`.
    ///
    /// If the map did not previously have this key present, then `None` is
    /// returned.
    ///
    /// If the map did have this key present, the new value is associated with
    /// the key and all previous values are removed. **Note** that only a single
    /// one of the previous values is returned. If there are multiple values
    /// that have been previously associated with the key, then the first one is
    /// returned. See `insert_mult` on `OccupiedEntry` for an API that returns
    /// all values.
    ///
    /// The key is not updated, though; this matters for types that can be `==`
    /// without being identical.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// assert!(map.insert("x-host", "world".parse().unwrap()).is_none());
    /// assert!(!map.is_empty());
    ///
    /// let mut prev = map.insert("x-host", "earth".parse().unwrap()).unwrap();
    /// assert_eq!("world", prev);
    /// ```
    ///
    /// ```should_panic
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// // Trying to insert a key that is not valid panics.
    /// map.insert("x{}host", "world".parse().unwrap());
    /// ```
    ///
    /// ```should_panic
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// // Trying to insert a key that is binary panics (use insert_bin).
    /// map.insert("x-host-bin", "world".parse().unwrap());
    /// ```
    pub fn insert<K>(&mut self, key: K, val: MetadataValue<Ascii>) -> Option<MetadataValue<Ascii>>
    where
        K: IntoMetadataKey<Ascii>,
    {
        key.insert(self, val)
    }

    /// Like insert, but for Binary keys (for example "trace-proto-bin").
    ///
    /// This method panics when the given key is a string and it cannot be
    /// converted to a `MetadataKey<Binary>`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// assert!(map.insert_bin("trace-proto-bin", MetadataValue::from_bytes(b"world")).is_none());
    /// assert!(!map.is_empty());
    ///
    /// let mut prev = map.insert_bin("trace-proto-bin", MetadataValue::from_bytes(b"earth")).unwrap();
    /// assert_eq!("world", prev);
    /// ```
    ///
    /// ```should_panic
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::default();
    /// // Attempting to add a binary metadata entry with an invalid name
    /// map.insert_bin("trace-proto", MetadataValue::from_bytes(b"hello")); // This line panics!
    /// ```
    ///
    /// ```should_panic
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// // Trying to insert a key that is not valid panics.
    /// map.insert_bin("x{}host-bin", MetadataValue::from_bytes(b"world")); // This line panics!
    /// ```
    pub fn insert_bin<K>(
        &mut self,
        key: K,
        val: MetadataValue<Binary>,
    ) -> Option<MetadataValue<Binary>>
    where
        K: IntoMetadataKey<Binary>,
    {
        key.insert(self, val)
    }

    /// Inserts an ascii key-value pair into the map. To insert a binary entry,
    /// use `append_bin`.
    ///
    /// This method panics when the given key is a string and it cannot be
    /// converted to a `MetadataKey<Ascii>`.
    ///
    /// If the map did not previously have this key present, then `false` is
    /// returned.
    ///
    /// If the map did have this key present, the new value is pushed to the end
    /// of the list of values currently associated with the key. The key is not
    /// updated, though; this matters for types that can be `==` without being
    /// identical.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// assert!(map.insert("x-host", "world".parse().unwrap()).is_none());
    /// assert!(!map.is_empty());
    ///
    /// map.append("x-host", "earth".parse().unwrap());
    ///
    /// let values = map.get_all("x-host");
    /// let mut i = values.iter();
    /// assert_eq!("world", *i.next().unwrap());
    /// assert_eq!("earth", *i.next().unwrap());
    /// ```
    ///
    /// ```should_panic
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// // Trying to append a key that is not valid panics.
    /// map.append("x{}host", "world".parse().unwrap()); // This line panics!
    /// ```
    ///
    /// ```should_panic
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// // Trying to append a key that is binary panics (use append_bin).
    /// map.append("x-host-bin", "world".parse().unwrap()); // This line panics!
    /// ```
    pub fn append<K>(&mut self, key: K, value: MetadataValue<Ascii>) -> bool
    where
        K: IntoMetadataKey<Ascii>,
    {
        key.append(self, value)
    }

    /// Like append, but for binary keys (for example "trace-proto-bin").
    ///
    /// This method panics when the given key is a string and it cannot be
    /// converted to a `MetadataKey<Binary>`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// assert!(map.insert_bin("trace-proto-bin", MetadataValue::from_bytes(b"world")).is_none());
    /// assert!(!map.is_empty());
    ///
    /// map.append_bin("trace-proto-bin", MetadataValue::from_bytes(b"earth"));
    ///
    /// let values = map.get_all_bin("trace-proto-bin");
    /// let mut i = values.iter();
    /// assert_eq!("world", *i.next().unwrap());
    /// assert_eq!("earth", *i.next().unwrap());
    /// ```
    ///
    /// ```should_panic
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// // Trying to append a key that is not valid panics.
    /// map.append_bin("x{}host-bin", MetadataValue::from_bytes(b"world")); // This line panics!
    /// ```
    ///
    /// ```should_panic
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// // Trying to append a key that is ascii panics (use append).
    /// map.append_bin("x-host", MetadataValue::from_bytes(b"world")); // This line panics!
    /// ```
    pub fn append_bin<K>(&mut self, key: K, value: MetadataValue<Binary>) -> bool
    where
        K: IntoMetadataKey<Binary>,
    {
        key.append(self, value)
    }

    /// Removes an ascii key from the map, returning the value associated with
    /// the key. To remove a binary key, use `remove_bin`.
    ///
    /// Returns `None` if the map does not contain the key. If there are
    /// multiple values associated with the key, then the first one is returned.
    /// See `remove_entry_mult` on `OccupiedEntry` for an API that yields all
    /// values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// map.insert("x-host", "hello.world".parse().unwrap());
    ///
    /// let prev = map.remove("x-host").unwrap();
    /// assert_eq!("hello.world", prev);
    ///
    /// assert!(map.remove("x-host").is_none());
    ///
    /// // Attempting to remove a key of the wrong type fails by not
    /// // finding anything.
    /// map.append_bin("host-bin", MetadataValue::from_bytes(b"world"));
    /// assert!(map.remove("host-bin").is_none());
    /// assert!(map.remove("host-bin".to_string()).is_none());
    /// assert!(map.remove(&("host-bin".to_string())).is_none());
    ///
    /// // Attempting to remove an invalid key string fails by not
    /// // finding anything.
    /// assert!(map.remove("host{}").is_none());
    /// assert!(map.remove("host{}".to_string()).is_none());
    /// assert!(map.remove(&("host{}".to_string())).is_none());
    /// ```
    pub fn remove<K>(&mut self, key: K) -> Option<MetadataValue<Ascii>>
    where
        K: AsMetadataKey<Ascii>,
    {
        key.remove(self)
    }

    /// Like remove, but for Binary keys (for example "trace-proto-bin").
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// map.insert_bin("trace-proto-bin", MetadataValue::from_bytes(b"hello.world"));
    ///
    /// let prev = map.remove_bin("trace-proto-bin").unwrap();
    /// assert_eq!("hello.world", prev);
    ///
    /// assert!(map.remove_bin("trace-proto-bin").is_none());
    ///
    /// // Attempting to remove a key of the wrong type fails by not
    /// // finding anything.
    /// map.append("host", "world".parse().unwrap());
    /// assert!(map.remove_bin("host").is_none());
    /// assert!(map.remove_bin("host".to_string()).is_none());
    /// assert!(map.remove_bin(&("host".to_string())).is_none());
    ///
    /// // Attempting to remove an invalid key string fails by not
    /// // finding anything.
    /// assert!(map.remove_bin("host{}-bin").is_none());
    /// assert!(map.remove_bin("host{}-bin".to_string()).is_none());
    /// assert!(map.remove_bin(&("host{}-bin".to_string())).is_none());
    /// ```
    pub fn remove_bin<K>(&mut self, key: K) -> Option<MetadataValue<Binary>>
    where
        K: AsMetadataKey<Binary>,
    {
        key.remove(self)
    }

    pub(crate) fn merge(&mut self, other: MetadataMap) {
        self.headers.extend(other.headers);
    }
}

// ===== impl Iter =====

impl<'a> Iterator for Iter<'a> {
    type Item = KeyAndValueRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|item| {
            let (name, value) = item;
            if Ascii::is_valid_key(name.as_str()) {
                KeyAndValueRef::Ascii(
                    MetadataKey::unchecked_from_header_name_ref(name),
                    MetadataValue::unchecked_from_header_value_ref(value),
                )
            } else {
                KeyAndValueRef::Binary(
                    MetadataKey::unchecked_from_header_name_ref(name),
                    MetadataValue::unchecked_from_header_value_ref(value),
                )
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

// ===== impl IterMut =====

impl<'a> Iterator for IterMut<'a> {
    type Item = KeyAndMutValueRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|item| {
            let (name, value) = item;
            if Ascii::is_valid_key(name.as_str()) {
                KeyAndMutValueRef::Ascii(
                    MetadataKey::unchecked_from_header_name_ref(name),
                    MetadataValue::unchecked_from_mut_header_value_ref(value),
                )
            } else {
                KeyAndMutValueRef::Binary(
                    MetadataKey::unchecked_from_header_name_ref(name),
                    MetadataValue::unchecked_from_mut_header_value_ref(value),
                )
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

// ===== impl ValueDrain =====

impl<VE: ValueEncoding> Iterator for ValueDrain<'_, VE> {
    type Item = MetadataValue<VE>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(MetadataValue::unchecked_from_header_value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

// ===== impl Keys =====

impl<'a> Iterator for Keys<'a> {
    type Item = KeyRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|key| {
            if Ascii::is_valid_key(key.as_str()) {
                KeyRef::Ascii(MetadataKey::unchecked_from_header_name_ref(key))
            } else {
                KeyRef::Binary(MetadataKey::unchecked_from_header_name_ref(key))
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for Keys<'_> {}

// ===== impl Values ====

impl<'a> Iterator for Values<'a> {
    type Item = ValueRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|item| {
            let (name, value) = item;
            if Ascii::is_valid_key(name.as_str()) {
                ValueRef::Ascii(MetadataValue::unchecked_from_header_value_ref(value))
            } else {
                ValueRef::Binary(MetadataValue::unchecked_from_header_value_ref(value))
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

// ===== impl Values ====

impl<'a> Iterator for ValuesMut<'a> {
    type Item = ValueRefMut<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|item| {
            let (name, value) = item;
            if Ascii::is_valid_key(name.as_str()) {
                ValueRefMut::Ascii(MetadataValue::unchecked_from_mut_header_value_ref(value))
            } else {
                ValueRefMut::Binary(MetadataValue::unchecked_from_mut_header_value_ref(value))
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

// ===== impl ValueIter =====

impl<'a, VE: ValueEncoding> Iterator for ValueIter<'a, VE>
where
    VE: 'a,
{
    type Item = &'a MetadataValue<VE>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner {
            Some(ref mut inner) => inner
                .next()
                .map(MetadataValue::unchecked_from_header_value_ref),
            None => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.inner {
            Some(ref inner) => inner.size_hint(),
            None => (0, Some(0)),
        }
    }
}

impl<'a, VE: ValueEncoding> DoubleEndedIterator for ValueIter<'a, VE>
where
    VE: 'a,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        match self.inner {
            Some(ref mut inner) => inner
                .next_back()
                .map(MetadataValue::unchecked_from_header_value_ref),
            None => None,
        }
    }
}

// ===== impl ValueIterMut =====

impl<'a, VE: ValueEncoding> Iterator for ValueIterMut<'a, VE>
where
    VE: 'a,
{
    type Item = &'a mut MetadataValue<VE>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(MetadataValue::unchecked_from_mut_header_value_ref)
    }
}

impl<'a, VE: ValueEncoding> DoubleEndedIterator for ValueIterMut<'a, VE>
where
    VE: 'a,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner
            .next_back()
            .map(MetadataValue::unchecked_from_mut_header_value_ref)
    }
}

// ===== impl Entry =====

impl<'a, VE: ValueEncoding> Entry<'a, VE> {
    /// Ensures a value is in the entry by inserting the default if empty.
    ///
    /// Returns a mutable reference to the **first** value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map: MetadataMap = MetadataMap::default();
    ///
    /// let keys = &[
    ///     "content-length",
    ///     "x-hello",
    ///     "Content-Length",
    ///     "x-world",
    /// ];
    ///
    /// for &key in keys {
    ///     let counter = map.entry(key)
    ///         .expect("valid key names")
    ///         .or_insert("".parse().unwrap());
    ///     *counter = format!("{}{}", counter.to_str().unwrap(), "1").parse().unwrap();
    /// }
    ///
    /// assert_eq!(map.get("content-length").unwrap(), "11");
    /// assert_eq!(map.get("x-hello").unwrap(), "1");
    /// ```
    pub fn or_insert(self, default: MetadataValue<VE>) -> &'a mut MetadataValue<VE> {
        use self::Entry::*;

        match self {
            Occupied(e) => e.into_mut(),
            Vacant(e) => e.insert(default),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default
    /// function if empty.
    ///
    /// The default function is not called if the entry exists in the map.
    /// Returns a mutable reference to the **first** value in the entry.
    ///
    /// # Examples
    ///
    /// Basic usage.
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    ///
    /// let res = map.entry("x-hello").unwrap()
    ///     .or_insert_with(|| "world".parse().unwrap());
    ///
    /// assert_eq!(res, "world");
    /// ```
    ///
    /// The default function is not called if the entry exists in the map.
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// map.insert("host", "world".parse().unwrap());
    ///
    /// let res = map.entry("host")
    ///     .expect("host is a valid string")
    ///     .or_insert_with(|| unreachable!());
    ///
    ///
    /// assert_eq!(res, "world");
    /// ```
    pub fn or_insert_with<F: FnOnce() -> MetadataValue<VE>>(
        self,
        default: F,
    ) -> &'a mut MetadataValue<VE> {
        use self::Entry::*;

        match self {
            Occupied(e) => e.into_mut(),
            Vacant(e) => e.insert(default()),
        }
    }

    /// Returns a reference to the entry's key
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    ///
    /// assert_eq!(map.entry("x-hello").unwrap().key(), "x-hello");
    /// ```
    pub fn key(&self) -> &MetadataKey<VE> {
        use self::Entry::*;

        MetadataKey::unchecked_from_header_name_ref(match *self {
            Vacant(ref e) => e.inner.key(),
            Occupied(ref e) => e.inner.key(),
        })
    }
}

// ===== impl VacantEntry =====

impl<'a, VE: ValueEncoding> VacantEntry<'a, VE> {
    /// Returns a reference to the entry's key
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    ///
    /// assert_eq!(map.entry("x-hello").unwrap().key(), "x-hello");
    /// ```
    pub fn key(&self) -> &MetadataKey<VE> {
        MetadataKey::unchecked_from_header_name_ref(self.inner.key())
    }

    /// Take ownership of the key
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    ///
    /// if let Entry::Vacant(v) = map.entry("x-hello").unwrap() {
    ///     assert_eq!(v.into_key().as_str(), "x-hello");
    /// }
    /// ```
    pub fn into_key(self) -> MetadataKey<VE> {
        MetadataKey::unchecked_from_header_name(self.inner.into_key())
    }

    /// Insert the value into the entry.
    ///
    /// The value will be associated with this entry's key. A mutable reference
    /// to the inserted value will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    ///
    /// if let Entry::Vacant(v) = map.entry("x-hello").unwrap() {
    ///     v.insert("world".parse().unwrap());
    /// }
    ///
    /// assert_eq!(map.get("x-hello").unwrap(), "world");
    /// ```
    pub fn insert(self, value: MetadataValue<VE>) -> &'a mut MetadataValue<VE> {
        MetadataValue::unchecked_from_mut_header_value_ref(self.inner.insert(value.inner))
    }

    /// Insert the value into the entry.
    ///
    /// The value will be associated with this entry's key. The new
    /// `OccupiedEntry` is returned, allowing for further manipulation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    ///
    /// if let Entry::Vacant(v) = map.entry("x-hello").unwrap() {
    ///     let mut e = v.insert_entry("world".parse().unwrap());
    ///     e.insert("world2".parse().unwrap());
    /// }
    ///
    /// assert_eq!(map.get("x-hello").unwrap(), "world2");
    /// ```
    pub fn insert_entry(self, value: MetadataValue<VE>) -> OccupiedEntry<'a, Ascii> {
        OccupiedEntry {
            inner: self.inner.insert_entry(value.inner),
            phantom: PhantomData,
        }
    }
}

// ===== impl OccupiedEntry =====

impl<'a, VE: ValueEncoding> OccupiedEntry<'a, VE> {
    /// Returns a reference to the entry's key.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// map.insert("host", "world".parse().unwrap());
    ///
    /// if let Entry::Occupied(e) = map.entry("host").unwrap() {
    ///     assert_eq!("host", e.key());
    /// }
    /// ```
    pub fn key(&self) -> &MetadataKey<VE> {
        MetadataKey::unchecked_from_header_name_ref(self.inner.key())
    }

    /// Get a reference to the first value in the entry.
    ///
    /// Values are stored in insertion order.
    ///
    /// # Panics
    ///
    /// `get` panics if there are no values associated with the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// map.insert("host", "hello.world".parse().unwrap());
    ///
    /// if let Entry::Occupied(mut e) = map.entry("host").unwrap() {
    ///     assert_eq!(e.get(), &"hello.world");
    ///
    ///     e.append("hello.earth".parse().unwrap());
    ///
    ///     assert_eq!(e.get(), &"hello.world");
    /// }
    /// ```
    pub fn get(&self) -> &MetadataValue<VE> {
        MetadataValue::unchecked_from_header_value_ref(self.inner.get())
    }

    /// Get a mutable reference to the first value in the entry.
    ///
    /// Values are stored in insertion order.
    ///
    /// # Panics
    ///
    /// `get_mut` panics if there are no values associated with the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::default();
    /// map.insert("host", "hello.world".parse().unwrap());
    ///
    /// if let Entry::Occupied(mut e) = map.entry("host").unwrap() {
    ///     e.get_mut().set_sensitive(true);
    ///     assert_eq!(e.get(), &"hello.world");
    ///     assert!(e.get().is_sensitive());
    /// }
    /// ```
    pub fn get_mut(&mut self) -> &mut MetadataValue<VE> {
        MetadataValue::unchecked_from_mut_header_value_ref(self.inner.get_mut())
    }

    /// Converts the `OccupiedEntry` into a mutable reference to the **first**
    /// value.
    ///
    /// The lifetime of the returned reference is bound to the original map.
    ///
    /// # Panics
    ///
    /// `into_mut` panics if there are no values associated with the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::default();
    /// map.insert("host", "hello.world".parse().unwrap());
    /// map.append("host", "hello.earth".parse().unwrap());
    ///
    /// if let Entry::Occupied(e) = map.entry("host").unwrap() {
    ///     e.into_mut().set_sensitive(true);
    /// }
    ///
    /// assert!(map.get("host").unwrap().is_sensitive());
    /// ```
    pub fn into_mut(self) -> &'a mut MetadataValue<VE> {
        MetadataValue::unchecked_from_mut_header_value_ref(self.inner.into_mut())
    }

    /// Sets the value of the entry.
    ///
    /// All previous values associated with the entry are removed and the first
    /// one is returned. See `insert_mult` for an API that returns all values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// map.insert("host", "hello.world".parse().unwrap());
    ///
    /// if let Entry::Occupied(mut e) = map.entry("host").unwrap() {
    ///     let mut prev = e.insert("earth".parse().unwrap());
    ///     assert_eq!("hello.world", prev);
    /// }
    ///
    /// assert_eq!("earth", map.get("host").unwrap());
    /// ```
    pub fn insert(&mut self, value: MetadataValue<VE>) -> MetadataValue<VE> {
        let header_value = self.inner.insert(value.inner);
        MetadataValue::unchecked_from_header_value(header_value)
    }

    /// Sets the value of the entry.
    ///
    /// This function does the same as `insert` except it returns an iterator
    /// that yields all values previously associated with the key.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// map.insert("host", "world".parse().unwrap());
    /// map.append("host", "world2".parse().unwrap());
    ///
    /// if let Entry::Occupied(mut e) = map.entry("host").unwrap() {
    ///     let mut prev = e.insert_mult("earth".parse().unwrap());
    ///     assert_eq!("world", prev.next().unwrap());
    ///     assert_eq!("world2", prev.next().unwrap());
    ///     assert!(prev.next().is_none());
    /// }
    ///
    /// assert_eq!("earth", map.get("host").unwrap());
    /// ```
    pub fn insert_mult(&mut self, value: MetadataValue<VE>) -> ValueDrain<'_, VE> {
        ValueDrain {
            inner: self.inner.insert_mult(value.inner),
            phantom: PhantomData,
        }
    }

    /// Insert the value into the entry.
    ///
    /// The new value is appended to the end of the entry's value list. All
    /// previous values associated with the entry are retained.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// map.insert("host", "world".parse().unwrap());
    ///
    /// if let Entry::Occupied(mut e) = map.entry("host").unwrap() {
    ///     e.append("earth".parse().unwrap());
    /// }
    ///
    /// let values = map.get_all("host");
    /// let mut i = values.iter();
    /// assert_eq!("world", *i.next().unwrap());
    /// assert_eq!("earth", *i.next().unwrap());
    /// ```
    pub fn append(&mut self, value: MetadataValue<VE>) {
        self.inner.append(value.inner)
    }

    /// Remove the entry from the map.
    ///
    /// All values associated with the entry are removed and the first one is
    /// returned. See `remove_entry_mult` for an API that returns all values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// map.insert("host", "world".parse().unwrap());
    ///
    /// if let Entry::Occupied(e) = map.entry("host").unwrap() {
    ///     let mut prev = e.remove();
    ///     assert_eq!("world", prev);
    /// }
    ///
    /// assert!(!map.contains_key("host"));
    /// ```
    pub fn remove(self) -> MetadataValue<VE> {
        let value = self.inner.remove();
        MetadataValue::unchecked_from_header_value(value)
    }

    /// Remove the entry from the map.
    ///
    /// The key and all values associated with the entry are removed and the
    /// first one is returned. See `remove_entry_mult` for an API that returns
    /// all values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// map.insert("host", "world".parse().unwrap());
    ///
    /// if let Entry::Occupied(e) = map.entry("host").unwrap() {
    ///     let (key, mut prev) = e.remove_entry();
    ///     assert_eq!("host", key.as_str());
    ///     assert_eq!("world", prev);
    /// }
    ///
    /// assert!(!map.contains_key("host"));
    /// ```
    pub fn remove_entry(self) -> (MetadataKey<VE>, MetadataValue<VE>) {
        let (name, value) = self.inner.remove_entry();
        (
            MetadataKey::unchecked_from_header_name(name),
            MetadataValue::unchecked_from_header_value(value),
        )
    }

    /// Remove the entry from the map.
    ///
    /// The key and all values associated with the entry are removed and
    /// returned.
    pub fn remove_entry_mult(self) -> (MetadataKey<VE>, ValueDrain<'a, VE>) {
        let (name, value_drain) = self.inner.remove_entry_mult();
        (
            MetadataKey::unchecked_from_header_name(name),
            ValueDrain {
                inner: value_drain,
                phantom: PhantomData,
            },
        )
    }

    /// Returns an iterator visiting all values associated with the entry.
    ///
    /// Values are iterated in insertion order.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// map.insert("host", "world".parse().unwrap());
    /// map.append("host", "earth".parse().unwrap());
    ///
    /// if let Entry::Occupied(e) = map.entry("host").unwrap() {
    ///     let mut iter = e.iter();
    ///     assert_eq!(&"world", iter.next().unwrap());
    ///     assert_eq!(&"earth", iter.next().unwrap());
    ///     assert!(iter.next().is_none());
    /// }
    /// ```
    pub fn iter(&self) -> ValueIter<'_, VE> {
        ValueIter {
            inner: Some(self.inner.iter()),
            phantom: PhantomData,
        }
    }

    /// Returns an iterator mutably visiting all values associated with the
    /// entry.
    ///
    /// Values are iterated in insertion order.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::default();
    /// map.insert("host", "world".parse().unwrap());
    /// map.append("host", "earth".parse().unwrap());
    ///
    /// if let Entry::Occupied(mut e) = map.entry("host").unwrap() {
    ///     for e in e.iter_mut() {
    ///         e.set_sensitive(true);
    ///     }
    /// }
    ///
    /// let mut values = map.get_all("host");
    /// let mut i = values.iter();
    /// assert!(i.next().unwrap().is_sensitive());
    /// assert!(i.next().unwrap().is_sensitive());
    /// ```
    pub fn iter_mut(&mut self) -> ValueIterMut<'_, VE> {
        ValueIterMut {
            inner: self.inner.iter_mut(),
            phantom: PhantomData,
        }
    }
}

impl<'a, VE: ValueEncoding> IntoIterator for OccupiedEntry<'a, VE>
where
    VE: 'a,
{
    type Item = &'a mut MetadataValue<VE>;
    type IntoIter = ValueIterMut<'a, VE>;

    fn into_iter(self) -> ValueIterMut<'a, VE> {
        ValueIterMut {
            inner: self.inner.into_iter(),
            phantom: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, VE: ValueEncoding> IntoIterator for &'b OccupiedEntry<'a, VE> {
    type Item = &'a MetadataValue<VE>;
    type IntoIter = ValueIter<'a, VE>;

    fn into_iter(self) -> ValueIter<'a, VE> {
        self.iter()
    }
}

impl<'a, 'b: 'a, VE: ValueEncoding> IntoIterator for &'b mut OccupiedEntry<'a, VE> {
    type Item = &'a mut MetadataValue<VE>;
    type IntoIter = ValueIterMut<'a, VE>;

    fn into_iter(self) -> ValueIterMut<'a, VE> {
        self.iter_mut()
    }
}

// ===== impl GetAll =====

impl<'a, VE: ValueEncoding> GetAll<'a, VE> {
    /// Returns an iterator visiting all values associated with the entry.
    ///
    /// Values are iterated in insertion order.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut map = MetadataMap::new();
    /// map.insert("x-host", "hello.world".parse().unwrap());
    /// map.append("x-host", "hello.earth".parse().unwrap());
    ///
    /// let values = map.get_all("x-host");
    /// let mut iter = values.iter();
    /// assert_eq!(&"hello.world", iter.next().unwrap());
    /// assert_eq!(&"hello.earth", iter.next().unwrap());
    /// assert!(iter.next().is_none());
    /// ```
    pub fn iter(&self) -> ValueIter<'a, VE> {
        ValueIter {
            inner: self.inner.as_ref().map(|inner| inner.iter()),
            phantom: PhantomData,
        }
    }
}

impl<VE: ValueEncoding> PartialEq for GetAll<'_, VE> {
    fn eq(&self, other: &Self) -> bool {
        self.inner.iter().eq(other.inner.iter())
    }
}

impl<'a, VE: ValueEncoding> IntoIterator for GetAll<'a, VE>
where
    VE: 'a,
{
    type Item = &'a MetadataValue<VE>;
    type IntoIter = ValueIter<'a, VE>;

    fn into_iter(self) -> ValueIter<'a, VE> {
        ValueIter {
            inner: self.inner.map(|inner| inner.into_iter()),
            phantom: PhantomData,
        }
    }
}

impl<'a, 'b: 'a, VE: ValueEncoding> IntoIterator for &'b GetAll<'a, VE> {
    type Item = &'a MetadataValue<VE>;
    type IntoIter = ValueIter<'a, VE>;

    fn into_iter(self) -> ValueIter<'a, VE> {
        ValueIter {
            inner: self.inner.as_ref().map(|inner| inner.into_iter()),
            phantom: PhantomData,
        }
    }
}

// ===== impl IntoMetadataKey / AsMetadataKey =====

mod into_metadata_key {
    use super::{MetadataMap, MetadataValue, ValueEncoding};
    use crate::metadata::key::MetadataKey;

    /// A marker trait used to identify values that can be used as insert keys
    /// to a `MetadataMap`.
    pub trait IntoMetadataKey<VE: ValueEncoding>: Sealed<VE> {}

    // All methods are on this pub(super) trait, instead of `IntoMetadataKey`,
    // so that they aren't publicly exposed to the world.
    //
    // Being on the `IntoMetadataKey` trait would mean users could call
    // `"host".insert(&mut map, "localhost")`.
    //
    // Ultimately, this allows us to adjust the signatures of these methods
    // without breaking any external crate.
    pub trait Sealed<VE: ValueEncoding> {
        #[doc(hidden)]
        fn insert(self, map: &mut MetadataMap, val: MetadataValue<VE>)
            -> Option<MetadataValue<VE>>;

        #[doc(hidden)]
        fn append(self, map: &mut MetadataMap, val: MetadataValue<VE>) -> bool;
    }

    // ==== impls ====

    impl<VE: ValueEncoding> Sealed<VE> for MetadataKey<VE> {
        #[doc(hidden)]
        #[inline]
        fn insert(
            self,
            map: &mut MetadataMap,
            val: MetadataValue<VE>,
        ) -> Option<MetadataValue<VE>> {
            map.headers
                .insert(self.inner, val.inner)
                .map(MetadataValue::unchecked_from_header_value)
        }

        #[doc(hidden)]
        #[inline]
        fn append(self, map: &mut MetadataMap, val: MetadataValue<VE>) -> bool {
            map.headers.append(self.inner, val.inner)
        }
    }

    impl<VE: ValueEncoding> IntoMetadataKey<VE> for MetadataKey<VE> {}

    impl<VE: ValueEncoding> Sealed<VE> for &MetadataKey<VE> {
        #[doc(hidden)]
        #[inline]
        fn insert(
            self,
            map: &mut MetadataMap,
            val: MetadataValue<VE>,
        ) -> Option<MetadataValue<VE>> {
            map.headers
                .insert(&self.inner, val.inner)
                .map(MetadataValue::unchecked_from_header_value)
        }
        #[doc(hidden)]
        #[inline]
        fn append(self, map: &mut MetadataMap, val: MetadataValue<VE>) -> bool {
            map.headers.append(&self.inner, val.inner)
        }
    }

    impl<VE: ValueEncoding> IntoMetadataKey<VE> for &MetadataKey<VE> {}

    impl<VE: ValueEncoding> Sealed<VE> for &'static str {
        #[doc(hidden)]
        #[inline]
        fn insert(
            self,
            map: &mut MetadataMap,
            val: MetadataValue<VE>,
        ) -> Option<MetadataValue<VE>> {
            // Perform name validation
            let key = MetadataKey::<VE>::from_static(self);

            map.headers
                .insert(key.inner, val.inner)
                .map(MetadataValue::unchecked_from_header_value)
        }
        #[doc(hidden)]
        #[inline]
        fn append(self, map: &mut MetadataMap, val: MetadataValue<VE>) -> bool {
            // Perform name validation
            let key = MetadataKey::<VE>::from_static(self);

            map.headers.append(key.inner, val.inner)
        }
    }

    impl<VE: ValueEncoding> IntoMetadataKey<VE> for &'static str {}
}

mod as_metadata_key {
    use super::{MetadataMap, MetadataValue, ValueEncoding};
    use crate::metadata::key::{InvalidMetadataKey, MetadataKey};
    use http::header::{Entry, GetAll, HeaderValue};

    /// A marker trait used to identify values that can be used as search keys
    /// to a `MetadataMap`.
    pub trait AsMetadataKey<VE: ValueEncoding>: Sealed<VE> {}

    // All methods are on this pub(super) trait, instead of `AsMetadataKey`,
    // so that they aren't publicly exposed to the world.
    //
    // Being on the `AsMetadataKey` trait would mean users could call
    // `"host".find(&map)`.
    //
    // Ultimately, this allows us to adjust the signatures of these methods
    // without breaking any external crate.
    pub trait Sealed<VE: ValueEncoding> {
        #[doc(hidden)]
        fn get(self, map: &MetadataMap) -> Option<&MetadataValue<VE>>;

        #[doc(hidden)]
        fn get_mut(self, map: &mut MetadataMap) -> Option<&mut MetadataValue<VE>>;

        #[doc(hidden)]
        fn get_all(self, map: &MetadataMap) -> Option<GetAll<'_, HeaderValue>>;

        #[doc(hidden)]
        fn entry(self, map: &mut MetadataMap)
            -> Result<Entry<'_, HeaderValue>, InvalidMetadataKey>;

        #[doc(hidden)]
        fn remove(self, map: &mut MetadataMap) -> Option<MetadataValue<VE>>;
    }

    // ==== impls ====

    impl<VE: ValueEncoding> Sealed<VE> for MetadataKey<VE> {
        #[doc(hidden)]
        #[inline]
        fn get(self, map: &MetadataMap) -> Option<&MetadataValue<VE>> {
            map.headers
                .get(self.inner)
                .map(MetadataValue::unchecked_from_header_value_ref)
        }

        #[doc(hidden)]
        #[inline]
        fn get_mut(self, map: &mut MetadataMap) -> Option<&mut MetadataValue<VE>> {
            map.headers
                .get_mut(self.inner)
                .map(MetadataValue::unchecked_from_mut_header_value_ref)
        }

        #[doc(hidden)]
        #[inline]
        fn get_all(self, map: &MetadataMap) -> Option<GetAll<'_, HeaderValue>> {
            Some(map.headers.get_all(self.inner))
        }

        #[doc(hidden)]
        #[inline]
        fn entry(
            self,
            map: &mut MetadataMap,
        ) -> Result<Entry<'_, HeaderValue>, InvalidMetadataKey> {
            Ok(map.headers.entry(self.inner))
        }

        #[doc(hidden)]
        #[inline]
        fn remove(self, map: &mut MetadataMap) -> Option<MetadataValue<VE>> {
            map.headers
                .remove(self.inner)
                .map(MetadataValue::unchecked_from_header_value)
        }
    }

    impl<VE: ValueEncoding> AsMetadataKey<VE> for MetadataKey<VE> {}

    impl<VE: ValueEncoding> Sealed<VE> for &MetadataKey<VE> {
        #[doc(hidden)]
        #[inline]
        fn get(self, map: &MetadataMap) -> Option<&MetadataValue<VE>> {
            map.headers
                .get(&self.inner)
                .map(MetadataValue::unchecked_from_header_value_ref)
        }

        #[doc(hidden)]
        #[inline]
        fn get_mut(self, map: &mut MetadataMap) -> Option<&mut MetadataValue<VE>> {
            map.headers
                .get_mut(&self.inner)
                .map(MetadataValue::unchecked_from_mut_header_value_ref)
        }

        #[doc(hidden)]
        #[inline]
        fn get_all(self, map: &MetadataMap) -> Option<GetAll<'_, HeaderValue>> {
            Some(map.headers.get_all(&self.inner))
        }

        #[doc(hidden)]
        #[inline]
        fn entry(
            self,
            map: &mut MetadataMap,
        ) -> Result<Entry<'_, HeaderValue>, InvalidMetadataKey> {
            Ok(map.headers.entry(&self.inner))
        }

        #[doc(hidden)]
        #[inline]
        fn remove(self, map: &mut MetadataMap) -> Option<MetadataValue<VE>> {
            map.headers
                .remove(&self.inner)
                .map(MetadataValue::unchecked_from_header_value)
        }
    }

    impl<VE: ValueEncoding> AsMetadataKey<VE> for &MetadataKey<VE> {}

    impl<VE: ValueEncoding> Sealed<VE> for &str {
        #[doc(hidden)]
        #[inline]
        fn get(self, map: &MetadataMap) -> Option<&MetadataValue<VE>> {
            if !VE::is_valid_key(self) {
                return None;
            }
            map.headers
                .get(self)
                .map(MetadataValue::unchecked_from_header_value_ref)
        }

        #[doc(hidden)]
        #[inline]
        fn get_mut(self, map: &mut MetadataMap) -> Option<&mut MetadataValue<VE>> {
            if !VE::is_valid_key(self) {
                return None;
            }
            map.headers
                .get_mut(self)
                .map(MetadataValue::unchecked_from_mut_header_value_ref)
        }

        #[doc(hidden)]
        #[inline]
        fn get_all(self, map: &MetadataMap) -> Option<GetAll<'_, HeaderValue>> {
            if !VE::is_valid_key(self) {
                return None;
            }
            Some(map.headers.get_all(self))
        }

        #[doc(hidden)]
        #[inline]
        fn entry(
            self,
            map: &mut MetadataMap,
        ) -> Result<Entry<'_, HeaderValue>, InvalidMetadataKey> {
            if !VE::is_valid_key(self) {
                return Err(InvalidMetadataKey::new());
            }

            let key = http::header::HeaderName::from_bytes(self.as_bytes())
                .map_err(|_| InvalidMetadataKey::new())?;
            let entry = map.headers.entry(key);
            Ok(entry)
        }

        #[doc(hidden)]
        #[inline]
        fn remove(self, map: &mut MetadataMap) -> Option<MetadataValue<VE>> {
            if !VE::is_valid_key(self) {
                return None;
            }
            map.headers
                .remove(self)
                .map(MetadataValue::unchecked_from_header_value)
        }
    }

    impl<VE: ValueEncoding> AsMetadataKey<VE> for &str {}

    impl<VE: ValueEncoding> Sealed<VE> for String {
        #[doc(hidden)]
        #[inline]
        fn get(self, map: &MetadataMap) -> Option<&MetadataValue<VE>> {
            if !VE::is_valid_key(self.as_str()) {
                return None;
            }
            map.headers
                .get(self.as_str())
                .map(MetadataValue::unchecked_from_header_value_ref)
        }

        #[doc(hidden)]
        #[inline]
        fn get_mut(self, map: &mut MetadataMap) -> Option<&mut MetadataValue<VE>> {
            if !VE::is_valid_key(self.as_str()) {
                return None;
            }
            map.headers
                .get_mut(self.as_str())
                .map(MetadataValue::unchecked_from_mut_header_value_ref)
        }

        #[doc(hidden)]
        #[inline]
        fn get_all(self, map: &MetadataMap) -> Option<GetAll<'_, HeaderValue>> {
            if !VE::is_valid_key(self.as_str()) {
                return None;
            }
            Some(map.headers.get_all(self.as_str()))
        }

        #[doc(hidden)]
        #[inline]
        fn entry(
            self,
            map: &mut MetadataMap,
        ) -> Result<Entry<'_, HeaderValue>, InvalidMetadataKey> {
            if !VE::is_valid_key(self.as_str()) {
                return Err(InvalidMetadataKey::new());
            }

            let key = http::header::HeaderName::from_bytes(self.as_bytes())
                .map_err(|_| InvalidMetadataKey::new())?;
            Ok(map.headers.entry(key))
        }

        #[doc(hidden)]
        #[inline]
        fn remove(self, map: &mut MetadataMap) -> Option<MetadataValue<VE>> {
            if !VE::is_valid_key(self.as_str()) {
                return None;
            }
            map.headers
                .remove(self.as_str())
                .map(MetadataValue::unchecked_from_header_value)
        }
    }

    impl<VE: ValueEncoding> AsMetadataKey<VE> for String {}

    impl<VE: ValueEncoding> Sealed<VE> for &String {
        #[doc(hidden)]
        #[inline]
        fn get(self, map: &MetadataMap) -> Option<&MetadataValue<VE>> {
            if !VE::is_valid_key(self) {
                return None;
            }
            map.headers
                .get(self.as_str())
                .map(MetadataValue::unchecked_from_header_value_ref)
        }

        #[doc(hidden)]
        #[inline]
        fn get_mut(self, map: &mut MetadataMap) -> Option<&mut MetadataValue<VE>> {
            if !VE::is_valid_key(self) {
                return None;
            }
            map.headers
                .get_mut(self.as_str())
                .map(MetadataValue::unchecked_from_mut_header_value_ref)
        }

        #[doc(hidden)]
        #[inline]
        fn get_all(self, map: &MetadataMap) -> Option<GetAll<'_, HeaderValue>> {
            if !VE::is_valid_key(self) {
                return None;
            }
            Some(map.headers.get_all(self.as_str()))
        }

        #[doc(hidden)]
        #[inline]
        fn entry(
            self,
            map: &mut MetadataMap,
        ) -> Result<Entry<'_, HeaderValue>, InvalidMetadataKey> {
            if !VE::is_valid_key(self) {
                return Err(InvalidMetadataKey::new());
            }

            let key = http::header::HeaderName::from_bytes(self.as_bytes())
                .map_err(|_| InvalidMetadataKey::new())?;
            Ok(map.headers.entry(key))
        }

        #[doc(hidden)]
        #[inline]
        fn remove(self, map: &mut MetadataMap) -> Option<MetadataValue<VE>> {
            if !VE::is_valid_key(self) {
                return None;
            }
            map.headers
                .remove(self.as_str())
                .map(MetadataValue::unchecked_from_header_value)
        }
    }

    impl<VE: ValueEncoding> AsMetadataKey<VE> for &String {}
}

mod as_encoding_agnostic_metadata_key {
    use super::{MetadataMap, ValueEncoding};
    use crate::metadata::key::MetadataKey;

    /// A marker trait used to identify values that can be used as search keys
    /// to a `MetadataMap`, for operations that don't expose the actual value.
    pub trait AsEncodingAgnosticMetadataKey: Sealed {}

    // All methods are on this pub(super) trait, instead of
    // `AsEncodingAgnosticMetadataKey`, so that they aren't publicly exposed to
    // the world.
    //
    // Being on the `AsEncodingAgnosticMetadataKey` trait would mean users could
    // call `"host".contains_key(&map)`.
    //
    // Ultimately, this allows us to adjust the signatures of these methods
    // without breaking any external crate.
    pub trait Sealed {
        #[doc(hidden)]
        fn contains_key(&self, map: &MetadataMap) -> bool;
    }

    // ==== impls ====

    impl<VE: ValueEncoding> Sealed for MetadataKey<VE> {
        #[doc(hidden)]
        #[inline]
        fn contains_key(&self, map: &MetadataMap) -> bool {
            map.headers.contains_key(&self.inner)
        }
    }

    impl<VE: ValueEncoding> AsEncodingAgnosticMetadataKey for MetadataKey<VE> {}

    impl<VE: ValueEncoding> Sealed for &MetadataKey<VE> {
        #[doc(hidden)]
        #[inline]
        fn contains_key(&self, map: &MetadataMap) -> bool {
            map.headers.contains_key(&self.inner)
        }
    }

    impl<VE: ValueEncoding> AsEncodingAgnosticMetadataKey for &MetadataKey<VE> {}

    impl Sealed for &str {
        #[doc(hidden)]
        #[inline]
        fn contains_key(&self, map: &MetadataMap) -> bool {
            map.headers.contains_key(*self)
        }
    }

    impl AsEncodingAgnosticMetadataKey for &str {}

    impl Sealed for String {
        #[doc(hidden)]
        #[inline]
        fn contains_key(&self, map: &MetadataMap) -> bool {
            map.headers.contains_key(self.as_str())
        }
    }

    impl AsEncodingAgnosticMetadataKey for String {}

    impl Sealed for &String {
        #[doc(hidden)]
        #[inline]
        fn contains_key(&self, map: &MetadataMap) -> bool {
            map.headers.contains_key(self.as_str())
        }
    }

    impl AsEncodingAgnosticMetadataKey for &String {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_headers_takes_http_headers() {
        let mut http_map = http::HeaderMap::new();
        http_map.insert("x-host", "example.com".parse().unwrap());

        let map = MetadataMap::from_headers(http_map);

        assert_eq!(map.get("x-host").unwrap(), "example.com");
    }

    #[test]
    fn test_to_headers_encoding() {
        use crate::Status;
        let special_char_message = "Beyond 100% ascii \t\n\r🌶️💉💧🐮🍺";
        let s1 = Status::unknown(special_char_message);

        assert_eq!(s1.message(), special_char_message);

        let s1_map = s1.to_header_map().unwrap();
        let s2 = Status::from_header_map(&s1_map).unwrap();

        assert_eq!(s1.message(), s2.message());

        assert!(
            s1_map
                .get("grpc-message")
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("Beyond%20100%25%20ascii"),
            "Percent sign or other character isn't encoded as desired: {:?}",
            s1_map.get("grpc-message")
        );
    }

    #[test]
    fn test_iter_categorizes_ascii_entries() {
        let mut map = MetadataMap::new();

        map.insert("x-word", "hello".parse().unwrap());
        map.append_bin("x-word-bin", MetadataValue::from_bytes(b"goodbye"));
        map.insert_bin("x-number-bin", MetadataValue::from_bytes(b"123"));

        let mut found_x_word = false;
        for key_and_value in map.iter() {
            if let KeyAndValueRef::Ascii(key, _value) = key_and_value {
                if key.as_str() == "x-word" {
                    found_x_word = true;
                } else {
                    panic!("Unexpected key");
                }
            }
        }
        assert!(found_x_word);
    }

    #[test]
    fn test_iter_categorizes_binary_entries() {
        let mut map = MetadataMap::new();

        map.insert("x-word", "hello".parse().unwrap());
        map.append_bin("x-word-bin", MetadataValue::from_bytes(b"goodbye"));

        let mut found_x_word_bin = false;
        for key_and_value in map.iter() {
            if let KeyAndValueRef::Binary(key, _value) = key_and_value {
                if key.as_str() == "x-word-bin" {
                    found_x_word_bin = true;
                } else {
                    panic!("Unexpected key");
                }
            }
        }
        assert!(found_x_word_bin);
    }

    #[test]
    fn test_iter_mut_categorizes_ascii_entries() {
        let mut map = MetadataMap::new();

        map.insert("x-word", "hello".parse().unwrap());
        map.append_bin("x-word-bin", MetadataValue::from_bytes(b"goodbye"));
        map.insert_bin("x-number-bin", MetadataValue::from_bytes(b"123"));

        let mut found_x_word = false;
        for key_and_value in map.iter_mut() {
            if let KeyAndMutValueRef::Ascii(key, _value) = key_and_value {
                if key.as_str() == "x-word" {
                    found_x_word = true;
                } else {
                    panic!("Unexpected key");
                }
            }
        }
        assert!(found_x_word);
    }

    #[test]
    fn test_iter_mut_categorizes_binary_entries() {
        let mut map = MetadataMap::new();

        map.insert("x-word", "hello".parse().unwrap());
        map.append_bin("x-word-bin", MetadataValue::from_bytes(b"goodbye"));

        let mut found_x_word_bin = false;
        for key_and_value in map.iter_mut() {
            if let KeyAndMutValueRef::Binary(key, _value) = key_and_value {
                if key.as_str() == "x-word-bin" {
                    found_x_word_bin = true;
                } else {
                    panic!("Unexpected key");
                }
            }
        }
        assert!(found_x_word_bin);
    }

    #[test]
    fn test_keys_categorizes_ascii_entries() {
        let mut map = MetadataMap::new();

        map.insert("x-word", "hello".parse().unwrap());
        map.append_bin("x-word-bin", MetadataValue::from_bytes(b"goodbye"));
        map.insert_bin("x-number-bin", MetadataValue::from_bytes(b"123"));

        let mut found_x_word = false;
        for key in map.keys() {
            if let KeyRef::Ascii(key) = key {
                if key.as_str() == "x-word" {
                    found_x_word = true;
                } else {
                    panic!("Unexpected key");
                }
            }
        }
        assert!(found_x_word);
    }

    #[test]
    fn test_keys_categorizes_binary_entries() {
        let mut map = MetadataMap::new();

        map.insert("x-word", "hello".parse().unwrap());
        map.insert_bin("x-number-bin", MetadataValue::from_bytes(b"123"));

        let mut found_x_number_bin = false;
        for key in map.keys() {
            if let KeyRef::Binary(key) = key {
                if key.as_str() == "x-number-bin" {
                    found_x_number_bin = true;
                } else {
                    panic!("Unexpected key");
                }
            }
        }
        assert!(found_x_number_bin);
    }

    #[test]
    fn test_values_categorizes_ascii_entries() {
        let mut map = MetadataMap::new();

        map.insert("x-word", "hello".parse().unwrap());
        map.append_bin("x-word-bin", MetadataValue::from_bytes(b"goodbye"));
        map.insert_bin("x-number-bin", MetadataValue::from_bytes(b"123"));

        let mut found_x_word = false;
        for value in map.values() {
            if let ValueRef::Ascii(value) = value {
                if *value == "hello" {
                    found_x_word = true;
                } else {
                    panic!("Unexpected key");
                }
            }
        }
        assert!(found_x_word);
    }

    #[test]
    fn test_values_categorizes_binary_entries() {
        let mut map = MetadataMap::new();

        map.insert("x-word", "hello".parse().unwrap());
        map.append_bin("x-word-bin", MetadataValue::from_bytes(b"goodbye"));

        let mut found_x_word_bin = false;
        for value_ref in map.values() {
            if let ValueRef::Binary(value) = value_ref {
                assert_eq!(*value, "goodbye");
                found_x_word_bin = true;
            }
        }
        assert!(found_x_word_bin);
    }

    #[test]
    fn test_values_mut_categorizes_ascii_entries() {
        let mut map = MetadataMap::new();

        map.insert("x-word", "hello".parse().unwrap());
        map.append_bin("x-word-bin", MetadataValue::from_bytes(b"goodbye"));
        map.insert_bin("x-number-bin", MetadataValue::from_bytes(b"123"));

        let mut found_x_word = false;
        for value_ref in map.values_mut() {
            if let ValueRefMut::Ascii(value) = value_ref {
                assert_eq!(*value, "hello");
                found_x_word = true;
            }
        }
        assert!(found_x_word);
    }

    #[test]
    fn test_values_mut_categorizes_binary_entries() {
        let mut map = MetadataMap::new();

        map.insert("x-word", "hello".parse().unwrap());
        map.append_bin("x-word-bin", MetadataValue::from_bytes(b"goodbye"));

        let mut found_x_word_bin = false;
        for value in map.values_mut() {
            if let ValueRefMut::Binary(value) = value {
                assert_eq!(*value, "goodbye");
                found_x_word_bin = true;
            }
        }
        assert!(found_x_word_bin);
    }

    #[allow(dead_code)]
    fn value_drain_is_send_sync() {
        fn is_send_sync<T: Send + Sync>() {}

        is_send_sync::<Iter<'_>>();
        is_send_sync::<IterMut<'_>>();

        is_send_sync::<ValueDrain<'_, Ascii>>();
        is_send_sync::<ValueDrain<'_, Binary>>();

        is_send_sync::<ValueIterMut<'_, Ascii>>();
        is_send_sync::<ValueIterMut<'_, Binary>>();
    }
}

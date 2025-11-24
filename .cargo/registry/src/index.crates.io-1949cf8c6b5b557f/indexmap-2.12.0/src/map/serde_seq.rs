//! Functions to serialize and deserialize an [`IndexMap`] as an ordered sequence.
//!
//! The default `serde` implementation serializes `IndexMap` as a normal map,
//! but there is no guarantee that serialization formats will preserve the order
//! of the key-value pairs. This module serializes `IndexMap` as a sequence of
//! `(key, value)` elements instead, in order.
//!
//! This module may be used in a field attribute for derived implementations:
//!
//! ```
//! # use indexmap::IndexMap;
//! # use serde::{Deserialize, Serialize};
//! #[derive(Deserialize, Serialize)]
//! struct Data {
//!     #[serde(with = "indexmap::map::serde_seq")]
//!     map: IndexMap<i32, u64>,
//!     // ...
//! }
//! ```

use serde_core::de::{Deserialize, Deserializer, SeqAccess, Visitor};
use serde_core::ser::{Serialize, Serializer};

use core::fmt::{self, Formatter};
use core::hash::{BuildHasher, Hash};
use core::marker::PhantomData;

use crate::map::Slice as MapSlice;
use crate::serde::cautious_capacity;
use crate::set::Slice as SetSlice;
use crate::IndexMap;

/// Serializes a [`map::Slice`][MapSlice] as an ordered sequence.
///
/// This behaves like [`crate::map::serde_seq`] for `IndexMap`, serializing a sequence
/// of `(key, value)` pairs, rather than as a map that might not preserve order.
impl<K, V> Serialize for MapSlice<K, V>
where
    K: Serialize,
    V: Serialize,
{
    fn serialize<T>(&self, serializer: T) -> Result<T::Ok, T::Error>
    where
        T: Serializer,
    {
        serializer.collect_seq(self)
    }
}

/// Serializes a [`set::Slice`][SetSlice] as an ordered sequence.
impl<T> Serialize for SetSlice<T>
where
    T: Serialize,
{
    fn serialize<Se>(&self, serializer: Se) -> Result<Se::Ok, Se::Error>
    where
        Se: Serializer,
    {
        serializer.collect_seq(self)
    }
}

/// Serializes an [`IndexMap`] as an ordered sequence.
///
/// This function may be used in a field attribute for deriving [`Serialize`]:
///
/// ```
/// # use indexmap::IndexMap;
/// # use serde::Serialize;
/// #[derive(Serialize)]
/// struct Data {
///     #[serde(serialize_with = "indexmap::map::serde_seq::serialize")]
///     map: IndexMap<i32, u64>,
///     // ...
/// }
/// ```
pub fn serialize<K, V, S, T>(map: &IndexMap<K, V, S>, serializer: T) -> Result<T::Ok, T::Error>
where
    K: Serialize,
    V: Serialize,
    T: Serializer,
{
    serializer.collect_seq(map)
}

/// Visitor to deserialize a *sequenced* `IndexMap`
struct SeqVisitor<K, V, S>(PhantomData<(K, V, S)>);

impl<'de, K, V, S> Visitor<'de> for SeqVisitor<K, V, S>
where
    K: Deserialize<'de> + Eq + Hash,
    V: Deserialize<'de>,
    S: Default + BuildHasher,
{
    type Value = IndexMap<K, V, S>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "a sequenced map")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let capacity = cautious_capacity::<K, V>(seq.size_hint());
        let mut map = IndexMap::with_capacity_and_hasher(capacity, S::default());

        while let Some((key, value)) = seq.next_element()? {
            map.insert(key, value);
        }

        Ok(map)
    }
}

/// Deserializes an [`IndexMap`] from an ordered sequence.
///
/// This function may be used in a field attribute for deriving [`Deserialize`]:
///
/// ```
/// # use indexmap::IndexMap;
/// # use serde::Deserialize;
/// #[derive(Deserialize)]
/// struct Data {
///     #[serde(deserialize_with = "indexmap::map::serde_seq::deserialize")]
///     map: IndexMap<i32, u64>,
///     // ...
/// }
/// ```
pub fn deserialize<'de, D, K, V, S>(deserializer: D) -> Result<IndexMap<K, V, S>, D::Error>
where
    D: Deserializer<'de>,
    K: Deserialize<'de> + Eq + Hash,
    V: Deserialize<'de>,
    S: Default + BuildHasher,
{
    deserializer.deserialize_seq(SeqVisitor(PhantomData))
}

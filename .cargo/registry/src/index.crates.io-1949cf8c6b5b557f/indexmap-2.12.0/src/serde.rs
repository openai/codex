#![cfg_attr(docsrs, doc(cfg(feature = "serde")))]

use serde_core::de::value::{MapDeserializer, SeqDeserializer};
use serde_core::de::{
    Deserialize, Deserializer, Error, IntoDeserializer, MapAccess, SeqAccess, Visitor,
};
use serde_core::ser::{Serialize, Serializer};

use core::fmt::{self, Formatter};
use core::hash::{BuildHasher, Hash};
use core::marker::PhantomData;

use crate::{Bucket, IndexMap, IndexSet};

/// Limit our preallocated capacity from a deserializer `size_hint()`.
///
/// We do account for the `Bucket` overhead from its saved `hash` field, but we don't count the
/// `RawTable` allocation or the fact that its raw capacity will be rounded up to a power of two.
/// The "max" is an arbitrary choice anyway, not something that needs precise adherence.
///
/// This is based on the internal `serde::de::size_hint::cautious(hint)` function.
pub(crate) fn cautious_capacity<K, V>(hint: Option<usize>) -> usize {
    const MAX_PREALLOC_BYTES: usize = 1024 * 1024;

    Ord::min(
        hint.unwrap_or(0),
        MAX_PREALLOC_BYTES / size_of::<Bucket<K, V>>(),
    )
}

impl<K, V, S> Serialize for IndexMap<K, V, S>
where
    K: Serialize,
    V: Serialize,
{
    fn serialize<T>(&self, serializer: T) -> Result<T::Ok, T::Error>
    where
        T: Serializer,
    {
        serializer.collect_map(self)
    }
}

struct IndexMapVisitor<K, V, S>(PhantomData<(K, V, S)>);

impl<'de, K, V, S> Visitor<'de> for IndexMapVisitor<K, V, S>
where
    K: Deserialize<'de> + Eq + Hash,
    V: Deserialize<'de>,
    S: Default + BuildHasher,
{
    type Value = IndexMap<K, V, S>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "a map")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let capacity = cautious_capacity::<K, V>(map.size_hint());
        let mut values = IndexMap::with_capacity_and_hasher(capacity, S::default());

        while let Some((key, value)) = map.next_entry()? {
            values.insert(key, value);
        }

        Ok(values)
    }
}

impl<'de, K, V, S> Deserialize<'de> for IndexMap<K, V, S>
where
    K: Deserialize<'de> + Eq + Hash,
    V: Deserialize<'de>,
    S: Default + BuildHasher,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(IndexMapVisitor(PhantomData))
    }
}

impl<'de, K, V, S, E> IntoDeserializer<'de, E> for IndexMap<K, V, S>
where
    K: IntoDeserializer<'de, E> + Eq + Hash,
    V: IntoDeserializer<'de, E>,
    S: BuildHasher,
    E: Error,
{
    type Deserializer = MapDeserializer<'de, <Self as IntoIterator>::IntoIter, E>;

    fn into_deserializer(self) -> Self::Deserializer {
        MapDeserializer::new(self.into_iter())
    }
}

impl<T, S> Serialize for IndexSet<T, S>
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

struct IndexSetVisitor<T, S>(PhantomData<(T, S)>);

impl<'de, T, S> Visitor<'de> for IndexSetVisitor<T, S>
where
    T: Deserialize<'de> + Eq + Hash,
    S: Default + BuildHasher,
{
    type Value = IndexSet<T, S>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "a set")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let capacity = cautious_capacity::<T, ()>(seq.size_hint());
        let mut values = IndexSet::with_capacity_and_hasher(capacity, S::default());

        while let Some(value) = seq.next_element()? {
            values.insert(value);
        }

        Ok(values)
    }
}

impl<'de, T, S> Deserialize<'de> for IndexSet<T, S>
where
    T: Deserialize<'de> + Eq + Hash,
    S: Default + BuildHasher,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(IndexSetVisitor(PhantomData))
    }
}

impl<'de, T, S, E> IntoDeserializer<'de, E> for IndexSet<T, S>
where
    T: IntoDeserializer<'de, E> + Eq + Hash,
    S: BuildHasher,
    E: Error,
{
    type Deserializer = SeqDeserializer<<Self as IntoIterator>::IntoIter, E>;

    fn into_deserializer(self) -> Self::Deserializer {
        SeqDeserializer::new(self.into_iter())
    }
}

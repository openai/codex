// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::LiteMap;
use crate::store::*;
use alloc::vec::Vec;
use core::fmt;
use core::marker::PhantomData;
use serde::{
    de::{MapAccess, SeqAccess, Visitor},
    ser::{SerializeMap, SerializeSeq},
    Deserialize, Deserializer, Serialize, Serializer,
};

impl<K, V, R> Serialize for LiteMap<K, V, R>
where
    K: Serialize,
    V: Serialize,
    R: Store<K, V>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Many human-readable formats don't support values other
        // than numbers and strings as map keys. For them, we can serialize
        // as a sequence of tuples instead
        if serializer.is_human_readable() {
            let k_is_num_or_string = self
                .values
                .lm_get(0)
                .is_some_and(|(k, _)| super::serde_helpers::is_num_or_string(k));
            if !k_is_num_or_string {
                let mut seq = serializer.serialize_seq(Some(self.len()))?;
                // Note that we can't require StoreIterable for R, see below.
                for index in 0..self.len() {
                    #[allow(clippy::unwrap_used)] // looping over 0..len
                    seq.serialize_element(&self.get_indexed(index).unwrap())?;
                }
                return seq.end();
            }
            // continue to regular serialization
        }

        // Note that we can't require StoreIterable for R, because the Higher-Rank Trait Bounds (HRTBs)
        // `R: for<'a> StoreIterable<'a, K, V>` would end up being too strict for some use cases
        // as it would require Self, K and V to be 'static. See https://github.com/rust-lang/types-team/blob/master/minutes/2022-07-08-implied-bounds-and-wf-checking.md#problem-fora-shouldnt-really-mean-for-all-a
        // Instead, we require only R: Store and manually iterate over the items.
        // For some R types this is equivalent to StoreIterable after compiler optimizations but retains
        // flexibility for other types.
        let mut map = serializer.serialize_map(Some(self.len()))?;
        for index in 0..self.len() {
            #[allow(clippy::unwrap_used)] // looping over 0..len
            let (k, v) = self.get_indexed(index).unwrap();
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

/// Modified example from https://serde.rs/deserialize-map.html
#[allow(clippy::type_complexity)]
struct LiteMapVisitor<K, V, R> {
    marker: PhantomData<fn() -> LiteMap<K, V, R>>,
}

impl<K, V, R> LiteMapVisitor<K, V, R> {
    fn new() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}

impl<'de, K, V, R> Visitor<'de> for LiteMapVisitor<K, V, R>
where
    K: Deserialize<'de> + Ord,
    V: Deserialize<'de>,
    R: StoreBulkMut<K, V>,
{
    type Value = LiteMap<K, V, R>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map produced by LiteMap")
    }

    fn visit_seq<S>(self, mut access: S) -> Result<Self::Value, S::Error>
    where
        S: SeqAccess<'de>,
    {
        // See visit_map for an explanation of the fast-path and out-of-order handling
        let mut map = LiteMap::with_capacity(access.size_hint().unwrap_or(0));
        let mut out_of_order = Vec::new();

        while let Some((key, value)) = access.next_element()? {
            if let Some((key, value)) = map.try_append(key, value) {
                out_of_order.push((key, value));
            }
        }

        if !out_of_order.is_empty() {
            map.extend(out_of_order);
        }

        Ok(map)
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut map = LiteMap::with_capacity(access.size_hint().unwrap_or(0));
        let mut out_of_order = Vec::new();

        // While there are entries remaining in the input, add them
        // into our map.
        while let Some((key, value)) = access.next_entry()? {
            // Try to append it at the end, hoping for a sorted map. Otherwise, collect
            // out of order items and extend the map with them later. This way, we give
            // the implementation an opportunity to avoid quadratic costs from calling
            // insert() with out of order items.
            // Handling ordered inputs first allows for arbitrary maps (e.g. from user JSON)
            // to be deserialized into LiteMap without impacting performance in the case of
            // deserializing a serialized map that came from another LiteMap.
            if let Some((key, value)) = map.try_append(key, value) {
                out_of_order.push((key, value));
            }
        }

        if !out_of_order.is_empty() {
            map.extend(out_of_order);
        }

        Ok(map)
    }
}

impl<'de, K, V, R> Deserialize<'de> for LiteMap<K, V, R>
where
    K: Ord + Deserialize<'de>,
    V: Deserialize<'de>,
    R: StoreBulkMut<K, V>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            // deserialize_any only works on self-describing (human-readable)
            // formats
            deserializer.deserialize_any(LiteMapVisitor::new())
        } else {
            deserializer.deserialize_map(LiteMapVisitor::new())
        }
    }
}

#[cfg(test)]
mod test {
    use crate::LiteMap;
    use alloc::borrow::ToOwned;
    use alloc::string::String;

    fn get_simple_map() -> LiteMap<u32, String> {
        [
            (1, "one".to_owned()),
            (2, "two".to_owned()),
            (4, "four".to_owned()),
            (5, "five".to_owned()),
        ]
        .into_iter()
        .collect()
    }

    fn get_tuple_map() -> LiteMap<(u32, String), String> {
        [
            ((1, "en".to_owned()), "one".to_owned()),
            ((1, "zh".to_owned()), "一".to_owned()),
            ((2, "en".to_owned()), "two".to_owned()),
            ((2, "zh".to_owned()), "二".to_owned()),
            ((4, "en".to_owned()), "four".to_owned()),
            ((5, "en".to_owned()), "five".to_owned()),
            ((5, "zh".to_owned()), "五".to_owned()),
            ((7, "zh".to_owned()), "七".to_owned()),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn test_roundtrip_json() {
        let map = get_simple_map();
        let json = serde_json::to_string(&map).unwrap();
        assert_eq!(
            json,
            "{\"1\":\"one\",\"2\":\"two\",\"4\":\"four\",\"5\":\"five\"}"
        );
        let deserialized: LiteMap<u32, String> = serde_json::from_str(&json).unwrap();
        assert_eq!(map, deserialized);

        let map = get_tuple_map();
        let json = serde_json::to_string(&map).unwrap();
        assert_eq!(
            json,
            "[[[1,\"en\"],\"one\"],[[1,\"zh\"],\"一\"],[[2,\"en\"],\"two\"],\
                          [[2,\"zh\"],\"二\"],[[4,\"en\"],\"four\"],[[5,\"en\"],\"five\"],\
                          [[5,\"zh\"],\"五\"],[[7,\"zh\"],\"七\"]]"
        );
        let deserialized: LiteMap<(u32, String), String> = serde_json::from_str(&json).unwrap();
        assert_eq!(map, deserialized);
    }

    #[test]
    fn test_roundtrip_postcard() {
        let map = get_simple_map();
        let postcard = postcard::to_stdvec(&map).unwrap();
        let deserialized: LiteMap<u32, String> = postcard::from_bytes(&postcard).unwrap();
        assert_eq!(map, deserialized);

        let map = get_tuple_map();
        let postcard = postcard::to_stdvec(&map).unwrap();
        let deserialized: LiteMap<(u32, String), String> = postcard::from_bytes(&postcard).unwrap();
        assert_eq!(map, deserialized);
    }

    /// Test that a LiteMap<_, _, Vec> is deserialized with an exact capacity
    /// if the deserializer provides a size hint information, like postcard here.
    #[test]
    fn test_deserialize_capacity() {
        for len in 0..50 {
            let mut map = (0..len).map(|i| (i, i.to_string())).collect::<Vec<_>>();
            let postcard = postcard::to_stdvec(&map).unwrap();
            let deserialized: LiteMap<u32, String> = postcard::from_bytes(&postcard).unwrap();
            assert_eq!(deserialized.values.capacity(), len);
            assert_eq!(deserialized.values.len(), len);
            // again, but with a shuffled map
            rand::seq::SliceRandom::shuffle(&mut map[..], &mut rand::rng());
            let postcard = postcard::to_stdvec(&map).unwrap();
            let deserialized: LiteMap<u32, String> = postcard::from_bytes(&postcard).unwrap();
            assert_eq!(deserialized.values.capacity(), len);
            assert_eq!(deserialized.values.len(), len);
        }
    }
}

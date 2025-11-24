// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::{ZeroMap2d, ZeroMap2dBorrowed, ZeroMap2dCursor};
use crate::map::{MutableZeroVecLike, ZeroMapKV, ZeroVecLike};
use crate::ZeroVec;
use alloc::vec::Vec;
use core::fmt;
use core::marker::PhantomData;
use serde::de::{self, Deserialize, Deserializer, MapAccess, Visitor};
#[cfg(feature = "serde")]
use serde::ser::{Serialize, SerializeMap, Serializer};

/// This impl requires enabling the optional `serde` Cargo feature of the `zerovec` crate
#[cfg(feature = "serde")]
impl<'a, K0, K1, V> Serialize for ZeroMap2d<'a, K0, K1, V>
where
    K0: ZeroMapKV<'a> + Serialize + ?Sized + Ord,
    K1: ZeroMapKV<'a> + Serialize + ?Sized + Ord,
    V: ZeroMapKV<'a> + Serialize + ?Sized,
    K0::Container: Serialize,
    K1::Container: Serialize,
    V::Container: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            let mut serde_map = serializer.serialize_map(None)?;
            for cursor in self.iter0() {
                K0::Container::zvl_get_as_t(cursor.key0(), |k| serde_map.serialize_key(k))?;
                let inner_map = ZeroMap2dInnerMapSerialize { cursor };
                serde_map.serialize_value(&inner_map)?;
            }
            serde_map.end()
        } else {
            (&self.keys0, &self.joiner, &self.keys1, &self.values).serialize(serializer)
        }
    }
}

/// Helper struct for human-serializing the inner map of a ZeroMap2d
#[cfg(feature = "serde")]
struct ZeroMap2dInnerMapSerialize<'a, 'l, K0, K1, V>
where
    K0: ZeroMapKV<'a> + ?Sized + Ord,
    K1: ZeroMapKV<'a> + ?Sized + Ord,
    V: ZeroMapKV<'a> + ?Sized,
{
    pub cursor: ZeroMap2dCursor<'l, 'a, K0, K1, V>,
}

#[cfg(feature = "serde")]
impl<'a, 'l, K0, K1, V> Serialize for ZeroMap2dInnerMapSerialize<'a, 'l, K0, K1, V>
where
    K0: ZeroMapKV<'a> + Serialize + ?Sized + Ord,
    K1: ZeroMapKV<'a> + Serialize + ?Sized + Ord,
    V: ZeroMapKV<'a> + Serialize + ?Sized,
    K0::Container: Serialize,
    K1::Container: Serialize,
    V::Container: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut serde_map = serializer.serialize_map(None)?;
        for (key1, v) in self.cursor.iter1() {
            K1::Container::zvl_get_as_t(key1, |k| serde_map.serialize_key(k))?;
            V::Container::zvl_get_as_t(v, |v| serde_map.serialize_value(v))?;
        }
        serde_map.end()
    }
}

/// This impl requires enabling the optional `serde` Cargo feature of the `zerovec` crate
#[cfg(feature = "serde")]
impl<'a, K0, K1, V> Serialize for ZeroMap2dBorrowed<'a, K0, K1, V>
where
    K0: ZeroMapKV<'a> + Serialize + ?Sized + Ord,
    K1: ZeroMapKV<'a> + Serialize + ?Sized + Ord,
    V: ZeroMapKV<'a> + Serialize + ?Sized,
    K0::Container: Serialize,
    K1::Container: Serialize,
    V::Container: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ZeroMap2d::<K0, K1, V>::from(*self).serialize(serializer)
    }
}

/// Modified example from https://serde.rs/deserialize-map.html
struct ZeroMap2dMapVisitor<'a, K0, K1, V>
where
    K0: ZeroMapKV<'a> + ?Sized + Ord,
    K1: ZeroMapKV<'a> + ?Sized + Ord,
    V: ZeroMapKV<'a> + ?Sized,
{
    #[expect(clippy::type_complexity)] // it's a marker type, complexity doesn't matter
    marker: PhantomData<fn() -> (&'a K0::OwnedType, &'a K1::OwnedType, &'a V::OwnedType)>,
}

impl<'a, K0, K1, V> ZeroMap2dMapVisitor<'a, K0, K1, V>
where
    K0: ZeroMapKV<'a> + ?Sized + Ord,
    K1: ZeroMapKV<'a> + ?Sized + Ord,
    V: ZeroMapKV<'a> + ?Sized,
{
    fn new() -> Self {
        ZeroMap2dMapVisitor {
            marker: PhantomData,
        }
    }
}

impl<'a, 'de, K0, K1, V> Visitor<'de> for ZeroMap2dMapVisitor<'a, K0, K1, V>
where
    K0: ZeroMapKV<'a> + Ord + ?Sized + Ord,
    K1: ZeroMapKV<'a> + Ord + ?Sized + Ord,
    V: ZeroMapKV<'a> + ?Sized,
    K1::Container: Deserialize<'de>,
    V::Container: Deserialize<'de>,
    K0::OwnedType: Deserialize<'de>,
    K1::OwnedType: Deserialize<'de>,
    V::OwnedType: Deserialize<'de>,
{
    type Value = ZeroMap2d<'a, K0, K1, V>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map produced by ZeroMap2d")
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut map = ZeroMap2d::with_capacity(access.size_hint().unwrap_or(0));

        // On the first level, pull out the K0s and a TupleVecMap of the
        // K1s and Vs, and then collect them into a ZeroMap2d
        while let Some((key0, inner_map)) =
            access.next_entry::<K0::OwnedType, TupleVecMap<K1::OwnedType, V::OwnedType>>()?
        {
            for (key1, value) in inner_map.entries.iter() {
                if map
                    .try_append(
                        K0::Container::owned_as_t(&key0),
                        K1::Container::owned_as_t(key1),
                        V::Container::owned_as_t(value),
                    )
                    .is_some()
                {
                    return Err(de::Error::custom(
                        "ZeroMap2d's keys must be sorted while deserializing",
                    ));
                }
            }
        }

        Ok(map)
    }
}

/// Helper struct for human-deserializing the inner map of a ZeroMap2d
struct TupleVecMap<K1, V> {
    pub entries: Vec<(K1, V)>,
}

struct TupleVecMapVisitor<K1, V> {
    marker: PhantomData<fn() -> (K1, V)>,
}

impl<K1, V> TupleVecMapVisitor<K1, V> {
    fn new() -> Self {
        TupleVecMapVisitor {
            marker: PhantomData,
        }
    }
}

impl<'de, K1, V> Visitor<'de> for TupleVecMapVisitor<K1, V>
where
    K1: Deserialize<'de>,
    V: Deserialize<'de>,
{
    type Value = TupleVecMap<K1, V>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an inner map produced by ZeroMap2d")
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut result = Vec::with_capacity(access.size_hint().unwrap_or(0));
        while let Some((key1, value)) = access.next_entry::<K1, V>()? {
            result.push((key1, value));
        }
        Ok(TupleVecMap { entries: result })
    }
}

impl<'de, K1, V> Deserialize<'de> for TupleVecMap<K1, V>
where
    K1: Deserialize<'de>,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(TupleVecMapVisitor::<K1, V>::new())
    }
}

/// This impl requires enabling the optional `serde` Cargo feature of the `zerovec` crate
impl<'de, 'a, K0, K1, V> Deserialize<'de> for ZeroMap2d<'a, K0, K1, V>
where
    K0: ZeroMapKV<'a> + Ord + ?Sized,
    K1: ZeroMapKV<'a> + Ord + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
    K0::Container: Deserialize<'de>,
    K1::Container: Deserialize<'de>,
    V::Container: Deserialize<'de>,
    K0::OwnedType: Deserialize<'de>,
    K1::OwnedType: Deserialize<'de>,
    V::OwnedType: Deserialize<'de>,
    'de: 'a,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            deserializer.deserialize_map(ZeroMap2dMapVisitor::<'a, K0, K1, V>::new())
        } else {
            let (keys0, joiner, keys1, values): (
                K0::Container,
                ZeroVec<u32>,
                K1::Container,
                V::Container,
            ) = Deserialize::deserialize(deserializer)?;
            // Invariant 1: len(keys0) == len(joiner)
            if keys0.zvl_len() != joiner.len() {
                return Err(de::Error::custom(
                    "Mismatched keys0 and joiner sizes in ZeroMap2d",
                ));
            }
            // Invariant 2: len(keys1) == len(values)
            if keys1.zvl_len() != values.zvl_len() {
                return Err(de::Error::custom(
                    "Mismatched keys1 and value sizes in ZeroMap2d",
                ));
            }
            // Invariant 3: joiner is sorted
            if !joiner.zvl_is_ascending() {
                return Err(de::Error::custom(
                    "ZeroMap2d deserializing joiner array out of order",
                ));
            }
            // Invariant 4: the last element of joiner is the length of keys1
            if let Some(last_joiner0) = joiner.last() {
                if keys1.zvl_len() != last_joiner0 as usize {
                    return Err(de::Error::custom(
                        "ZeroMap2d deserializing joiner array malformed",
                    ));
                }
            }
            let result = Self {
                keys0,
                joiner,
                keys1,
                values,
            };
            // In debug mode, check the optional invariants, too
            #[cfg(debug_assertions)]
            result.check_invariants();
            Ok(result)
        }
    }
}

/// This impl requires enabling the optional `serde` Cargo feature of the `zerovec` crate
impl<'de, 'a, K0, K1, V> Deserialize<'de> for ZeroMap2dBorrowed<'a, K0, K1, V>
where
    K0: ZeroMapKV<'a> + Ord + ?Sized,
    K1: ZeroMapKV<'a> + Ord + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
    K0::Container: Deserialize<'de>,
    K1::Container: Deserialize<'de>,
    V::Container: Deserialize<'de>,
    K0::OwnedType: Deserialize<'de>,
    K1::OwnedType: Deserialize<'de>,
    V::OwnedType: Deserialize<'de>,
    'de: 'a,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            Err(de::Error::custom(
                "ZeroMap2dBorrowed cannot be deserialized from human-readable formats",
            ))
        } else {
            let deserialized: ZeroMap2d<'a, K0, K1, V> = ZeroMap2d::deserialize(deserializer)?;
            let keys0 = if let Some(keys0) = deserialized.keys0.zvl_as_borrowed_inner() {
                keys0
            } else {
                return Err(de::Error::custom(
                    "ZeroMap2dBorrowed can only deserialize in zero-copy ways",
                ));
            };
            let joiner = if let Some(joiner) = deserialized.joiner.zvl_as_borrowed_inner() {
                joiner
            } else {
                return Err(de::Error::custom(
                    "ZeroMap2dBorrowed can only deserialize in zero-copy ways",
                ));
            };
            let keys1 = if let Some(keys1) = deserialized.keys1.zvl_as_borrowed_inner() {
                keys1
            } else {
                return Err(de::Error::custom(
                    "ZeroMap2dBorrowed can only deserialize in zero-copy ways",
                ));
            };
            let values = if let Some(values) = deserialized.values.zvl_as_borrowed_inner() {
                values
            } else {
                return Err(de::Error::custom(
                    "ZeroMap2dBorrowed can only deserialize in zero-copy ways",
                ));
            };
            Ok(Self {
                keys0,
                joiner,
                keys1,
                values,
            })
        }
    }
}

#[cfg(test)]
#[allow(non_camel_case_types)]
mod test {
    use crate::map2d::{ZeroMap2d, ZeroMap2dBorrowed};

    #[derive(serde::Serialize, serde::Deserialize)]
    #[allow(
        dead_code,
        reason = "We are testing that these types can be deserialized, and Tests compatibility of custom impl with Serde derive."
    )]
    struct DeriveTest_ZeroMap2d<'data> {
        #[serde(borrow)]
        _data: ZeroMap2d<'data, u16, str, [u8]>,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    #[allow(
        dead_code,
        reason = "We are testing that these types can be deserialized, and Tests compatibility of custom impl with Serde derive."
    )]
    struct DeriveTest_ZeroMap2dBorrowed<'data> {
        #[serde(borrow)]
        _data: ZeroMap2dBorrowed<'data, u16, str, [u8]>,
    }

    const JSON_STR: &str = "{\"1\":{\"1\":\"uno\"},\"2\":{\"2\":\"dos\",\"3\":\"tres\"}}";
    const BINCODE_BYTES: &[u8] = &[
        8, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 3, 0,
        0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 1, 0, 2, 0, 3, 0, 16, 0, 0, 0, 0, 0, 0, 0, 3, 0, 3, 0, 6, 0,
        117, 110, 111, 100, 111, 115, 116, 114, 101, 115,
    ];

    fn make_map() -> ZeroMap2d<'static, u32, u16, str> {
        let mut map = ZeroMap2d::new();
        map.insert(&1, &1, "uno");
        map.insert(&2, &2, "dos");
        map.insert(&2, &3, "tres");
        map
    }

    #[test]
    fn test_serde_json() {
        let map = make_map();
        let json_str = serde_json::to_string(&map).expect("serialize");
        assert_eq!(JSON_STR, json_str);
        let new_map: ZeroMap2d<u32, u16, str> =
            serde_json::from_str(&json_str).expect("deserialize");
        assert_eq!(format!("{new_map:?}"), format!("{map:?}"));
    }

    #[test]
    fn test_bincode() {
        let map = make_map();
        let bincode_bytes = bincode::serialize(&map).expect("serialize");
        assert_eq!(BINCODE_BYTES, bincode_bytes);
        let new_map: ZeroMap2d<u32, u16, str> =
            bincode::deserialize(&bincode_bytes).expect("deserialize");
        assert_eq!(
            format!("{new_map:?}"),
            format!("{map:?}").replace("Owned", "Borrowed"),
        );

        let new_map: ZeroMap2dBorrowed<u32, u16, str> =
            bincode::deserialize(&bincode_bytes).expect("deserialize");
        assert_eq!(
            format!("{new_map:?}"),
            format!("{map:?}")
                .replace("Owned", "Borrowed")
                .replace("ZeroMap2d", "ZeroMap2dBorrowed")
        );
    }

    #[test]
    fn test_serde_rmp() {
        let map = make_map();
        let rmp_buf = rmp_serde::to_vec(&map).expect("serialize");
        let new_map: ZeroMap2d<u32, u16, str> = rmp_serde::from_slice(&rmp_buf).unwrap();
        assert_eq!(map, new_map);
    }

    #[test]
    fn test_sample_bincode() {
        // This is the map from the main docs page for ZeroMap2d
        let mut map: ZeroMap2d<u16, u16, str> = ZeroMap2d::new();
        map.insert(&1, &2, "three");
        let bincode_bytes: Vec<u8> = bincode::serialize(&map).expect("serialize");
        assert_eq!(
            bincode_bytes.as_slice(),
            &[
                2, 0, 0, 0, 0, 0, 0, 0, 1, 0, 4, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 0, 0,
                0, 0, 2, 0, 7, 0, 0, 0, 0, 0, 0, 0, 1, 0, 116, 104, 114, 101, 101
            ]
        );
    }
}

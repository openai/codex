// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::{MutableZeroVecLike, ZeroMap, ZeroMapBorrowed, ZeroMapKV, ZeroVecLike};
use core::fmt;
use core::marker::PhantomData;
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
#[cfg(feature = "serde")]
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};

/// This impl requires enabling the optional `serde` Cargo feature of the `zerovec` crate
#[cfg(feature = "serde")]
impl<'a, K, V> Serialize for ZeroMap<'a, K, V>
where
    K: ZeroMapKV<'a> + Serialize + ?Sized + Ord,
    V: ZeroMapKV<'a> + Serialize + ?Sized,
    K::Container: Serialize,
    V::Container: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            // Many human-readable formats don't support values other
            // than numbers and strings as map keys. For them, we can serialize
            // as a vec of tuples instead
            if let Some(k) = self.iter_keys().next() {
                if !K::Container::zvl_get_as_t(k, super::serde_helpers::is_num_or_string) {
                    let mut seq = serializer.serialize_seq(Some(self.len()))?;
                    for (k, v) in self.iter() {
                        K::Container::zvl_get_as_t(k, |k| {
                            V::Container::zvl_get_as_t(v, |v| seq.serialize_element(&(k, v)))
                        })?;
                    }
                    return seq.end();
                }
            }
            let mut map = serializer.serialize_map(Some(self.len()))?;
            for (k, v) in self.iter() {
                K::Container::zvl_get_as_t(k, |k| map.serialize_key(k))?;
                V::Container::zvl_get_as_t(v, |v| map.serialize_value(v))?;
            }
            map.end()
        } else {
            (&self.keys, &self.values).serialize(serializer)
        }
    }
}

/// This impl requires enabling the optional `serde` Cargo feature of the `zerovec` crate
#[cfg(feature = "serde")]
impl<'a, K, V> Serialize for ZeroMapBorrowed<'a, K, V>
where
    K: ZeroMapKV<'a> + Serialize + ?Sized + Ord,
    V: ZeroMapKV<'a> + Serialize + ?Sized,
    K::Container: Serialize,
    V::Container: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ZeroMap::<K, V>::from(*self).serialize(serializer)
    }
}

/// Modified example from https://serde.rs/deserialize-map.html
struct ZeroMapMapVisitor<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized + Ord,
    V: ZeroMapKV<'a> + ?Sized,
{
    #[expect(clippy::type_complexity)] // it's a marker type, complexity doesn't matter
    marker: PhantomData<fn() -> (&'a K::OwnedType, &'a V::OwnedType)>,
}

impl<'a, K, V> ZeroMapMapVisitor<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized + Ord,
    V: ZeroMapKV<'a> + ?Sized,
{
    fn new() -> Self {
        ZeroMapMapVisitor {
            marker: PhantomData,
        }
    }
}

impl<'a, 'de, K, V> Visitor<'de> for ZeroMapMapVisitor<'a, K, V>
where
    K: ZeroMapKV<'a> + Ord + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
    K::OwnedType: Deserialize<'de>,
    V::OwnedType: Deserialize<'de>,
{
    type Value = ZeroMap<'a, K, V>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map produced by ZeroMap")
    }

    fn visit_seq<S>(self, mut access: S) -> Result<Self::Value, S::Error>
    where
        S: SeqAccess<'de>,
    {
        let mut map = ZeroMap::with_capacity(access.size_hint().unwrap_or(0));

        // While there are entries remaining in the input, add them
        // into our map.
        while let Some((key, value)) = access.next_element::<(K::OwnedType, V::OwnedType)>()? {
            // Try to append it at the end, hoping for a sorted map.
            // If not sorted, return an error
            // a serialized map that came from another ZeroMap
            if map
                .try_append(
                    K::Container::owned_as_t(&key),
                    V::Container::owned_as_t(&value),
                )
                .is_some()
            {
                return Err(de::Error::custom(
                    "ZeroMap's keys must be sorted while deserializing",
                ));
            }
        }

        Ok(map)
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut map = ZeroMap::with_capacity(access.size_hint().unwrap_or(0));

        // While there are entries remaining in the input, add them
        // into our map.
        while let Some((key, value)) = access.next_entry::<K::OwnedType, V::OwnedType>()? {
            // Try to append it at the end, hoping for a sorted map.
            // If not sorted, return an error
            // a serialized map that came from another ZeroMap
            if map
                .try_append(
                    K::Container::owned_as_t(&key),
                    V::Container::owned_as_t(&value),
                )
                .is_some()
            {
                return Err(de::Error::custom(
                    "ZeroMap's keys must be sorted while deserializing",
                ));
            }
        }

        Ok(map)
    }
}

/// This impl requires enabling the optional `serde` Cargo feature of the `zerovec` crate
impl<'de, 'a, K, V> Deserialize<'de> for ZeroMap<'a, K, V>
where
    K: ZeroMapKV<'a> + Ord + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
    K::Container: Deserialize<'de>,
    V::Container: Deserialize<'de>,
    K::OwnedType: Deserialize<'de>,
    V::OwnedType: Deserialize<'de>,
    'de: 'a,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            deserializer.deserialize_any(ZeroMapMapVisitor::<'a, K, V>::new())
        } else {
            let (keys, values): (K::Container, V::Container) =
                Deserialize::deserialize(deserializer)?;
            if keys.zvl_len() != values.zvl_len() {
                return Err(de::Error::custom(
                    "Mismatched key and value sizes in ZeroMap",
                ));
            }
            // #1433: If keys are out of order, treat it as GIGO.
            debug_assert!(keys.zvl_is_ascending());
            Ok(Self { keys, values })
        }
    }
}

// /// This impl requires enabling the optional `serde` Cargo feature of the `zerovec` crate
impl<'de, 'a, K, V> Deserialize<'de> for ZeroMapBorrowed<'a, K, V>
where
    K: ZeroMapKV<'a> + Ord + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
    K::Container: Deserialize<'de>,
    V::Container: Deserialize<'de>,
    K::OwnedType: Deserialize<'de>,
    V::OwnedType: Deserialize<'de>,
    'de: 'a,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            Err(de::Error::custom(
                "ZeroMapBorrowed cannot be deserialized from human-readable formats",
            ))
        } else {
            let deserialized: ZeroMap<'a, K, V> = ZeroMap::deserialize(deserializer)?;
            let keys = if let Some(keys) = deserialized.keys.zvl_as_borrowed_inner() {
                keys
            } else {
                return Err(de::Error::custom(
                    "ZeroMapBorrowed can only deserialize in zero-copy ways",
                ));
            };
            let values = if let Some(values) = deserialized.values.zvl_as_borrowed_inner() {
                values
            } else {
                return Err(de::Error::custom(
                    "ZeroMapBorrowed can only deserialize in zero-copy ways",
                ));
            };
            Ok(Self { keys, values })
        }
    }
}

#[cfg(test)]
#[allow(non_camel_case_types)]
mod test {
    use crate::{map::ZeroMapBorrowed, ZeroMap};

    #[derive(serde::Serialize, serde::Deserialize)]
    #[expect(
        dead_code,
        reason = "Tests compatibility of custom impl with Serde derive."
    )]
    struct DeriveTest_ZeroMap<'data> {
        #[serde(borrow)]
        _data: ZeroMap<'data, str, [u8]>,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    #[expect(
        dead_code,
        reason = "Tests compatibility of custom impl with Serde derive."
    )]
    struct DeriveTest_ZeroMapBorrowed<'data> {
        #[serde(borrow)]
        _data: ZeroMapBorrowed<'data, str, [u8]>,
    }

    const JSON_STR: &str = "{\"1\":\"uno\",\"2\":\"dos\",\"3\":\"tres\"}";
    const BINCODE_BYTES: &[u8] = &[
        12, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 3, 0,
        3, 0, 6, 0, 117, 110, 111, 100, 111, 115, 116, 114, 101, 115,
    ];

    fn make_map() -> ZeroMap<'static, u32, str> {
        let mut map = ZeroMap::new();
        map.insert(&1, "uno");
        map.insert(&2, "dos");
        map.insert(&3, "tres");
        map
    }

    #[test]
    fn test_serde_json() {
        let map = make_map();
        let json_str = serde_json::to_string(&map).expect("serialize");
        assert_eq!(JSON_STR, json_str);
        let new_map: ZeroMap<u32, str> = serde_json::from_str(&json_str).expect("deserialize");
        assert_eq!(
            new_map.iter().collect::<Vec<_>>(),
            map.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_serde_json_complex_key() {
        let mut map = ZeroMap::new();
        map.insert(&(1, 1), "uno");
        map.insert(&(2, 2), "dos");
        map.insert(&(3, 3), "tres");
        let json_str = serde_json::to_string(&map).expect("serialize");
        assert_eq!(
            json_str,
            "[[[1,1],\"uno\"],[[2,2],\"dos\"],[[3,3],\"tres\"]]"
        );
        let new_map: ZeroMap<(u32, u32), str> =
            serde_json::from_str(&json_str).expect("deserialize");
        assert_eq!(
            new_map.iter().collect::<Vec<_>>(),
            map.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_bincode() {
        let map = make_map();
        let bincode_bytes = bincode::serialize(&map).expect("serialize");
        assert_eq!(BINCODE_BYTES, bincode_bytes);
        let new_map: ZeroMap<u32, str> = bincode::deserialize(&bincode_bytes).expect("deserialize");
        assert_eq!(
            new_map.iter().collect::<Vec<_>>(),
            map.iter().collect::<Vec<_>>()
        );

        let new_map: ZeroMapBorrowed<u32, str> =
            bincode::deserialize(&bincode_bytes).expect("deserialize");
        assert_eq!(
            new_map.iter().collect::<Vec<_>>(),
            map.iter().collect::<Vec<_>>()
        );
    }
}

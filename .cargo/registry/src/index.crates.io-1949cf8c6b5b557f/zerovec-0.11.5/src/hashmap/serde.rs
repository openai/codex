// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::ZeroHashMap;
use crate::{
    map::{ZeroMapKV, ZeroVecLike},
    ZeroVec,
};

use serde::{de, Deserialize, Serialize};

impl<'a, K, V> Serialize for ZeroHashMap<'a, K, V>
where
    K: ZeroMapKV<'a> + Serialize + ?Sized,
    V: ZeroMapKV<'a> + Serialize + ?Sized,
    K::Container: Serialize,
    V::Container: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        (&self.displacements, &self.keys, &self.values).serialize(serializer)
    }
}

impl<'de, 'a, K, V> Deserialize<'de> for ZeroHashMap<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
    K::Container: Deserialize<'de>,
    V::Container: Deserialize<'de>,
    'de: 'a,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let (displacements, keys, values): (ZeroVec<(u32, u32)>, K::Container, V::Container) =
            Deserialize::deserialize(deserializer)?;
        if keys.zvl_len() != values.zvl_len() {
            return Err(de::Error::custom(
                "Mismatched key and value sizes in ZeroHashMap",
            ));
        }
        if displacements.zvl_len() != keys.zvl_len() {
            return Err(de::Error::custom(
                "Mismatched displacements and key, value sizes in ZeroHashMap",
            ));
        }
        Ok(Self {
            displacements,
            keys,
            values,
        })
    }
}

#[cfg(test)]
mod test {
    use crate::{VarZeroVec, ZeroHashMap, ZeroVec};
    use serde::{Deserialize, Serialize};

    const JSON_STR: &str = "[[[0,0],[0,1],[0,1]],[1,2,0],[\"b\",\"c\",\"a\"]]";

    const BINCODE_BYTES: &[u8] = &[
        24, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0,
        0, 0, 12, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0,
        3, 0, 1, 0, 2, 0, 98, 99, 97,
    ];

    #[derive(Serialize, Deserialize)]
    #[allow(
        dead_code,
        reason = "Tests compatibility of custom impl with Serde derive."
    )]
    struct DeriveTestZeroHashMap<'data> {
        #[serde(borrow)]
        _data: ZeroHashMap<'data, str, [u8]>,
    }

    fn make_zerohashmap() -> ZeroHashMap<'static, u32, str> {
        ZeroHashMap::from_iter([(0, "a"), (1, "b"), (2, "c")])
    }

    fn build_invalid_hashmap_str(
        displacements: Vec<(u32, u32)>,
        keys: Vec<u32>,
        values: Vec<&str>,
    ) -> String {
        let invalid_hm: ZeroHashMap<u32, str> = ZeroHashMap {
            displacements: ZeroVec::alloc_from_slice(&displacements),
            keys: ZeroVec::alloc_from_slice(&keys),
            values: VarZeroVec::<str>::from(&values),
        };
        serde_json::to_string(&invalid_hm).expect("serialize")
    }

    #[test]
    fn test_invalid_deser_zhm() {
        // Invalid hashmap |keys| != |values|
        let mut invalid_hm_str =
            build_invalid_hashmap_str(vec![(0, 1), (0, 0)], vec![1, 2], vec!["a", "b", "c"]);

        assert_eq!(
            serde_json::from_str::<ZeroHashMap<u32, str>>(&invalid_hm_str)
                .unwrap_err()
                .to_string(),
            "Mismatched key and value sizes in ZeroHashMap"
        );

        // Invalid hashmap |displacements| != |keys| == |values|
        // |displacements| = 2, |keys| = 3, |values| = 3
        invalid_hm_str =
            build_invalid_hashmap_str(vec![(0, 1), (0, 0)], vec![2, 1, 0], vec!["a", "b", "c"]);

        assert_eq!(
            serde_json::from_str::<ZeroHashMap<u32, str>>(&invalid_hm_str)
                .unwrap_err()
                .to_string(),
            "Mismatched displacements and key, value sizes in ZeroHashMap"
        );
    }

    // TODO(#6588): Fix sensitivity to host endianness.
    #[cfg(target_endian = "little")]
    #[test]
    fn test_serde_valid_deser_zhm() {
        let hm = make_zerohashmap();
        let json_str = serde_json::to_string(&hm).expect("serialize");
        assert_eq!(json_str, JSON_STR);
        let deserialized_hm: ZeroHashMap<u32, str> =
            serde_json::from_str(JSON_STR).expect("deserialize");
        assert_eq!(
            hm.iter().collect::<Vec<_>>(),
            deserialized_hm.iter().collect::<Vec<_>>()
        );
    }

    // TODO(#6588): Fix sensitivity to host endianness.
    #[cfg(target_endian = "little")]
    #[test]
    fn test_bincode_zhm() {
        let hm = make_zerohashmap();
        let bincode_bytes = bincode::serialize(&hm).expect("serialize");
        assert_eq!(bincode_bytes, BINCODE_BYTES);
        let deserialized_hm: ZeroHashMap<u32, str> =
            bincode::deserialize(BINCODE_BYTES).expect("deserialize");
        assert_eq!(
            hm.iter().collect::<Vec<_>>(),
            deserialized_hm.iter().collect::<Vec<_>>()
        );
    }
}

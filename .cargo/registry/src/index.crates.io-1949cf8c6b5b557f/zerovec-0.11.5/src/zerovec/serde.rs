// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::{ZeroSlice, ZeroVec};
use crate::ule::*;
use core::fmt;
use core::marker::PhantomData;
use serde::de::{self, Deserialize, Deserializer, Visitor};
#[cfg(feature = "serde")]
use serde::ser::{Serialize, SerializeSeq, Serializer};

struct ZeroVecVisitor<T> {
    marker: PhantomData<fn() -> T>,
}

impl<T> Default for ZeroVecVisitor<T> {
    fn default() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}

impl<'de, T> Visitor<'de> for ZeroVecVisitor<T>
where
    T: 'de + Deserialize<'de> + AsULE,
{
    type Value = ZeroVec<'de, T>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a sequence or borrowed buffer of fixed-width elements")
    }

    fn visit_borrowed_bytes<E>(self, bytes: &'de [u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        ZeroVec::parse_bytes(bytes).map_err(de::Error::custom)
    }

    #[cfg(feature = "alloc")]
    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut vec: alloc::vec::Vec<T::ULE> = if let Some(capacity) = seq.size_hint() {
            alloc::vec::Vec::with_capacity(capacity)
        } else {
            alloc::vec::Vec::new()
        };
        while let Some(value) = seq.next_element::<T>()? {
            vec.push(T::to_unaligned(value));
        }
        Ok(ZeroVec::new_owned(vec))
    }
}

/// This impl requires enabling the optional `serde` Cargo feature of the `zerovec` crate
impl<'de, 'a, T> Deserialize<'de> for ZeroVec<'a, T>
where
    T: 'de + Deserialize<'de> + AsULE,
    'de: 'a,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let visitor = ZeroVecVisitor::default();
        if deserializer.is_human_readable() {
            deserializer.deserialize_seq(visitor)
        } else {
            deserializer.deserialize_bytes(visitor)
        }
    }
}

/// This impl requires enabling the optional `serde` Cargo feature of the `zerovec` crate
impl<T> Serialize for ZeroVec<'_, T>
where
    T: Serialize + AsULE,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            let mut seq = serializer.serialize_seq(Some(self.len()))?;
            for value in self.iter() {
                seq.serialize_element(&value)?;
            }
            seq.end()
        } else {
            serializer.serialize_bytes(self.as_bytes())
        }
    }
}

/// This impl requires enabling the optional `serde` Cargo feature of the `zerovec` crate
#[cfg(feature = "alloc")]
impl<'de, T> Deserialize<'de> for alloc::boxed::Box<ZeroSlice<T>>
where
    T: Deserialize<'de> + AsULE + 'static,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut zv = ZeroVec::<T>::deserialize(deserializer)?;
        let vec = zv.with_mut(core::mem::take);
        Ok(ZeroSlice::from_boxed_slice(vec.into_boxed_slice()))
    }
}

/// This impl requires enabling the optional `serde` Cargo feature of the `zerovec` crate
impl<'de, 'a, T> Deserialize<'de> for &'a ZeroSlice<T>
where
    T: Deserialize<'de> + AsULE + 'static,
    'de: 'a,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            Err(de::Error::custom(
                "&ZeroSlice cannot be deserialized from human-readable formats",
            ))
        } else {
            let deserialized: ZeroVec<'a, T> = ZeroVec::deserialize(deserializer)?;
            let borrowed = if let Some(b) = deserialized.as_maybe_borrowed() {
                b
            } else {
                return Err(de::Error::custom(
                    "&ZeroSlice can only deserialize in zero-copy ways",
                ));
            };
            Ok(borrowed)
        }
    }
}

/// This impl requires enabling the optional `serde` Cargo feature of the `zerovec` crate
impl<T> Serialize for ZeroSlice<T>
where
    T: Serialize + AsULE,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.as_zerovec().serialize(serializer)
    }
}

#[cfg(test)]
#[allow(non_camel_case_types)]
mod test {
    use crate::samples::*;
    use crate::ZeroVec;

    #[derive(serde::Serialize, serde::Deserialize)]
    #[allow(
        dead_code,
        reason = "Tests compatibility of custom impl with Serde derive."
    )]
    struct DeriveTest_ZeroVec<'data> {
        #[serde(borrow)]
        _data: ZeroVec<'data, u16>,
    }

    #[test]
    fn test_serde_json() {
        let zerovec_orig = ZeroVec::from_slice_or_alloc(TEST_SLICE);
        let json_str = serde_json::to_string(&zerovec_orig).expect("serialize");
        assert_eq!(JSON_STR, json_str);
        // ZeroVec should deserialize from JSON to either Vec or ZeroVec
        let vec_new: Vec<u32> =
            serde_json::from_str(&json_str).expect("deserialize from buffer to Vec");
        assert_eq!(
            zerovec_orig,
            ZeroVec::<u32>::from_slice_or_alloc(vec_new.as_slice())
        );
        let zerovec_new: ZeroVec<u32> =
            serde_json::from_str(&json_str).expect("deserialize from buffer to ZeroVec");
        assert_eq!(zerovec_orig, zerovec_new);
        assert!(zerovec_new.is_owned());
    }

    #[test]
    fn test_serde_bincode() {
        let zerovec_orig = ZeroVec::from_slice_or_alloc(TEST_SLICE);
        let bincode_buf = bincode::serialize(&zerovec_orig).expect("serialize");
        assert_eq!(BINCODE_BUF, bincode_buf);
        // ZeroVec should deserialize from Bincode to ZeroVec but not Vec
        bincode::deserialize::<Vec<u32>>(&bincode_buf).expect_err("deserialize from buffer to Vec");
        let zerovec_new: ZeroVec<u32> =
            bincode::deserialize(&bincode_buf).expect("deserialize from buffer to ZeroVec");
        assert_eq!(zerovec_orig, zerovec_new);

        assert!(!zerovec_new.is_owned());
    }

    #[test]
    fn test_serde_rmp() {
        let zerovec_orig = ZeroVec::from_slice_or_alloc(TEST_SLICE);
        let rmp_buf = rmp_serde::to_vec(&zerovec_orig).expect("serialize");
        // ZeroVec should deserialize from Bincode to ZeroVec but not Vec
        bincode::deserialize::<Vec<u32>>(&rmp_buf).expect_err("deserialize from buffer to Vec");
        let zerovec_new: ZeroVec<u32> =
            rmp_serde::from_slice(&rmp_buf).expect("deserialize from buffer to ZeroVec");
        assert_eq!(zerovec_orig, zerovec_new);

        assert!(!zerovec_new.is_owned());
    }

    #[test]
    fn test_chars_valid() {
        // 1-byte, 2-byte, 3-byte, and 4-byte character in UTF-8 (not as relevant in UTF-32)
        let zerovec_orig = ZeroVec::alloc_from_slice(&['w', 'ω', '文', '𑄃']);
        let bincode_buf = bincode::serialize(&zerovec_orig).expect("serialize");
        let zerovec_new: ZeroVec<char> =
            bincode::deserialize(&bincode_buf).expect("deserialize from buffer to ZeroVec");
        assert_eq!(zerovec_orig, zerovec_new);

        assert!(!zerovec_new.is_owned());
    }

    #[test]
    fn test_chars_invalid() {
        // 119 and 120 are valid, but not 0xD800 (high surrogate)
        let zerovec_orig: ZeroVec<u32> = ZeroVec::from_slice_or_alloc(&[119, 0xD800, 120]);
        let bincode_buf = bincode::serialize(&zerovec_orig).expect("serialize");
        let zerovec_result = bincode::deserialize::<ZeroVec<char>>(&bincode_buf);
        assert!(zerovec_result.is_err());
    }
}

// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::{VarZeroSlice, VarZeroVec, VarZeroVecFormat};
use crate::ule::*;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::fmt;
use core::marker::PhantomData;
#[cfg(feature = "alloc")]
use serde::de::SeqAccess;
use serde::de::{self, Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, SerializeSeq, Serializer};

struct VarZeroVecVisitor<T: ?Sized, F: VarZeroVecFormat> {
    marker: PhantomData<(fn() -> T, F)>,
}

#[cfg(feature = "alloc")]
struct VarZeroVecHumanVisitor<T: ?Sized, F: VarZeroVecFormat> {
    marker: PhantomData<(fn() -> T, F)>,
}

impl<T: ?Sized, F: VarZeroVecFormat> Default for VarZeroVecVisitor<T, F> {
    fn default() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}

#[cfg(feature = "alloc")]
impl<T: ?Sized, F: VarZeroVecFormat> Default for VarZeroVecHumanVisitor<T, F> {
    fn default() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}

impl<'de, T, F> Visitor<'de> for VarZeroVecVisitor<T, F>
where
    T: VarULE + ?Sized,
    F: VarZeroVecFormat,
{
    type Value = VarZeroVec<'de, T, F>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a sequence or borrowed buffer of bytes")
    }

    fn visit_borrowed_bytes<E>(self, bytes: &'de [u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        VarZeroVec::parse_bytes(bytes).map_err(de::Error::custom)
    }
}

#[cfg(feature = "alloc")]
impl<'de, T, F> Visitor<'de> for VarZeroVecHumanVisitor<T, F>
where
    T: VarULE + ?Sized,
    Box<T>: Deserialize<'de>,
    F: VarZeroVecFormat,
{
    type Value = VarZeroVec<'de, T, F>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a sequence or borrowed buffer of bytes")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut vec: Vec<Box<T>> = if let Some(capacity) = seq.size_hint() {
            Vec::with_capacity(capacity)
        } else {
            Vec::new()
        };
        while let Some(value) = seq.next_element::<Box<T>>()? {
            vec.push(value);
        }
        Ok(VarZeroVec::from(&vec))
    }
}

/// This impl requires enabling the optional `serde` Cargo feature of the `zerovec` crate
#[cfg(feature = "alloc")]
impl<'de, 'a, T, F> Deserialize<'de> for VarZeroVec<'a, T, F>
where
    T: VarULE + ?Sized,
    Box<T>: Deserialize<'de>,
    F: VarZeroVecFormat,
    'de: 'a,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            deserializer.deserialize_seq(VarZeroVecHumanVisitor::<T, F>::default())
        } else {
            deserializer.deserialize_bytes(VarZeroVecVisitor::<T, F>::default())
        }
    }
}

/// This impl requires enabling the optional `serde` Cargo feature of the `zerovec` crate
#[cfg(not(feature = "alloc"))]
impl<'de, 'a, T, F> Deserialize<'de> for VarZeroVec<'a, T, F>
where
    T: VarULE + ?Sized,
    F: VarZeroVecFormat,
    'de: 'a,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_bytes(VarZeroVecVisitor::<T, F>::default())
    }
}

/// This impl requires enabling the optional `serde` Cargo feature of the `zerovec` crate
impl<'de, 'a, T, F> Deserialize<'de> for &'a VarZeroSlice<T, F>
where
    T: VarULE + ?Sized,
    F: VarZeroVecFormat,
    'de: 'a,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            Err(de::Error::custom(
                "&VarZeroSlice cannot be deserialized from human-readable formats",
            ))
        } else {
            let bytes = <&[u8]>::deserialize(deserializer)?;
            VarZeroSlice::<T, F>::parse_bytes(bytes).map_err(de::Error::custom)
        }
    }
}

/// This impl requires enabling the optional `serde` Cargo feature of the `zerovec` crate
#[cfg(feature = "alloc")]
impl<'de, T, F> Deserialize<'de> for Box<VarZeroSlice<T, F>>
where
    T: VarULE + ?Sized,
    Box<T>: Deserialize<'de>,
    F: VarZeroVecFormat,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let deserialized = VarZeroVec::<T, F>::deserialize(deserializer)?;
        Ok(deserialized.to_boxed())
    }
}

/// This impl requires enabling the optional `serde` Cargo feature of the `zerovec` crate
#[cfg(feature = "serde")]
impl<T, F> Serialize for VarZeroVec<'_, T, F>
where
    T: Serialize + VarULE + ?Sized,
    F: VarZeroVecFormat,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            let mut seq = serializer.serialize_seq(Some(self.len()))?;
            for value in self.iter() {
                seq.serialize_element(value)?;
            }
            seq.end()
        } else {
            serializer.serialize_bytes(self.as_bytes())
        }
    }
}

/// This impl requires enabling the optional `serde` Cargo feature of the `zerovec` crate
#[cfg(feature = "serde")]
impl<T, F> Serialize for VarZeroSlice<T, F>
where
    T: Serialize + VarULE + ?Sized,
    F: VarZeroVecFormat,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.as_varzerovec().serialize(serializer)
    }
}

#[cfg(test)]
#[allow(non_camel_case_types)]
mod test {
    use crate::{VarZeroSlice, VarZeroVec};

    #[derive(serde::Serialize, serde::Deserialize)]
    #[allow(
        dead_code,
        reason = "Tests compatibility of custom impl with Serde derive."
    )]
    struct DeriveTest_VarZeroVec<'data> {
        #[serde(borrow)]
        _data: VarZeroVec<'data, str>,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    #[allow(
        dead_code,
        reason = "Tests compatibility of custom impl with Serde derive."
    )]
    struct DeriveTest_VarZeroSlice<'data> {
        #[serde(borrow)]
        _data: &'data VarZeroSlice<str>,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    #[allow(
        dead_code,
        reason = "Tests compatibility of custom impl with Serde derive."
    )]
    struct DeriveTest_VarZeroVec_of_VarZeroSlice<'data> {
        #[serde(borrow)]
        _data: VarZeroVec<'data, VarZeroSlice<str>>,
    }

    // ["foo", "bar", "baz", "dolor", "quux", "lorem ipsum"];
    const BYTES: &[u8] = &[
        6, 0, 3, 0, 6, 0, 9, 0, 14, 0, 18, 0, 102, 111, 111, 98, 97, 114, 98, 97, 122, 100, 111,
        108, 111, 114, 113, 117, 117, 120, 108, 111, 114, 101, 109, 32, 105, 112, 115, 117, 109,
    ];
    const JSON_STR: &str = "[\"foo\",\"bar\",\"baz\",\"dolor\",\"quux\",\"lorem ipsum\"]";
    const BINCODE_BUF: &[u8] = &[
        41, 0, 0, 0, 0, 0, 0, 0, 6, 0, 3, 0, 6, 0, 9, 0, 14, 0, 18, 0, 102, 111, 111, 98, 97, 114,
        98, 97, 122, 100, 111, 108, 111, 114, 113, 117, 117, 120, 108, 111, 114, 101, 109, 32, 105,
        112, 115, 117, 109,
    ];

    // ["w", "ω", "文", "𑄃"]
    const NONASCII_STR: &[&str] = &["w", "ω", "文", "𑄃"];
    const NONASCII_BYTES: &[u8] = &[
        4, 0, 1, 0, 3, 0, 6, 0, 119, 207, 137, 230, 150, 135, 240, 145, 132, 131,
    ];
    #[test]
    fn test_serde_json() {
        let zerovec_orig: VarZeroVec<str> = VarZeroVec::parse_bytes(BYTES).expect("parse");
        let json_str = serde_json::to_string(&zerovec_orig).expect("serialize");
        assert_eq!(JSON_STR, json_str);
        // VarZeroVec should deserialize from JSON to either Vec or VarZeroVec
        let vec_new: Vec<Box<str>> =
            serde_json::from_str(&json_str).expect("deserialize from buffer to Vec");
        assert_eq!(zerovec_orig.to_vec(), vec_new);
        let zerovec_new: VarZeroVec<str> =
            serde_json::from_str(&json_str).expect("deserialize from buffer to VarZeroVec");
        assert_eq!(zerovec_orig.to_vec(), zerovec_new.to_vec());
        assert!(zerovec_new.is_owned());
    }

    #[test]
    fn test_serde_bincode() {
        let zerovec_orig: VarZeroVec<str> = VarZeroVec::parse_bytes(BYTES).expect("parse");
        let bincode_buf = bincode::serialize(&zerovec_orig).expect("serialize");
        assert_eq!(BINCODE_BUF, bincode_buf);
        let zerovec_new: VarZeroVec<str> =
            bincode::deserialize(&bincode_buf).expect("deserialize from buffer to VarZeroVec");
        assert_eq!(zerovec_orig.to_vec(), zerovec_new.to_vec());
        assert!(!zerovec_new.is_owned());
    }

    #[test]
    fn test_vzv_borrowed() {
        let zerovec_orig: &VarZeroSlice<str> = VarZeroSlice::parse_bytes(BYTES).expect("parse");
        let bincode_buf = bincode::serialize(&zerovec_orig).expect("serialize");
        assert_eq!(BINCODE_BUF, bincode_buf);
        let zerovec_new: &VarZeroSlice<str> =
            bincode::deserialize(&bincode_buf).expect("deserialize from buffer to VarZeroSlice");
        assert_eq!(zerovec_orig.to_vec(), zerovec_new.to_vec());
    }

    #[test]
    fn test_nonascii_bincode() {
        let src_vec = NONASCII_STR
            .iter()
            .copied()
            .map(Box::<str>::from)
            .collect::<Vec<_>>();
        let mut zerovec: VarZeroVec<str> = VarZeroVec::parse_bytes(NONASCII_BYTES).expect("parse");
        assert_eq!(zerovec.to_vec(), src_vec);
        let bincode_buf = bincode::serialize(&zerovec).expect("serialize");
        let zerovec_result =
            bincode::deserialize::<VarZeroVec<str>>(&bincode_buf).expect("deserialize");
        assert_eq!(zerovec_result.to_vec(), src_vec);

        // try again with owned zerovec
        zerovec.make_mut();
        let bincode_buf = bincode::serialize(&zerovec).expect("serialize");
        let zerovec_result =
            bincode::deserialize::<VarZeroVec<str>>(&bincode_buf).expect("deserialize");
        assert_eq!(zerovec_result.to_vec(), src_vec);
    }
}

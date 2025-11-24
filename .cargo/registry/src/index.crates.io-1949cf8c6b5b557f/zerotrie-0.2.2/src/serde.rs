// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::builder::bytestr::ByteStr;
use crate::options::ZeroTrieWithOptions;
use crate::zerotrie::ZeroTrieFlavor;
use crate::ZeroAsciiIgnoreCaseTrie;
use crate::ZeroTrie;
use crate::ZeroTrieExtendedCapacity;
use crate::ZeroTriePerfectHash;
use crate::ZeroTrieSimpleAscii;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;
use litemap::LiteMap;
use serde::de::Error;
use serde::de::Visitor;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;

struct ByteStrVisitor;
impl<'de> Visitor<'de> for ByteStrVisitor {
    type Value = Box<[u8]>;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a slice of borrowed bytes or a string")
    }
    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E> {
        Ok(Box::from(v))
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
        Ok(Box::from(v.as_bytes()))
    }
    fn visit_seq<A>(self, mut v: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut result = Vec::with_capacity(v.size_hint().unwrap_or(0));
        while let Some(x) = v.next_element::<u8>()? {
            result.push(x);
        }
        Ok(Box::from(result))
    }
}

impl<'data, 'de: 'data> Deserialize<'de> for &'data ByteStr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = <&'data [u8]>::deserialize(deserializer)?;
        Ok(ByteStr::from_bytes(s))
    }
}

impl<'de> Deserialize<'de> for Box<ByteStr> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let s = deserializer.deserialize_any(ByteStrVisitor)?;
            Ok(ByteStr::from_boxed_bytes(s))
        } else {
            let s = Vec::<u8>::deserialize(deserializer)?;
            Ok(ByteStr::from_boxed_bytes(s.into_boxed_slice()))
        }
    }
}

impl Serialize for &ByteStr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bytes = self.as_bytes();
        if serializer.is_human_readable() {
            match core::str::from_utf8(bytes) {
                Ok(s) => serializer.serialize_str(s),
                Err(_) => serializer.serialize_bytes(bytes),
            }
        } else {
            serializer.serialize_bytes(bytes)
        }
    }
}

impl<'data, 'de: 'data, Store> Deserialize<'de> for ZeroTrieSimpleAscii<Store>
where
    // DISCUSS: There are several possibilities for the bounds here that would
    // get the job done. I could look for Deserialize, but this would require
    // creating a custom Deserializer for the map case. I also considered
    // introducing a new trait instead of relying on From.
    Store: From<&'data [u8]> + From<Vec<u8>> + 'data,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let lm = LiteMap::<Box<ByteStr>, usize>::deserialize(deserializer)?;
            ZeroTrieSimpleAscii::try_from_serde_litemap(&lm)
                .map_err(D::Error::custom)
                .map(|trie| trie.convert_store())
        } else {
            // Note: `impl Deserialize for &[u8]` uses visit_borrowed_bytes
            let (flags, trie_bytes) = <(u8, &[u8])>::deserialize(deserializer)?;
            if Self::OPTIONS.to_u8_flags() != flags {
                return Err(D::Error::custom("invalid ZeroTrie tag"));
            };
            Ok(ZeroTrieSimpleAscii::from_store(Store::from(trie_bytes)))
        }
    }
}

impl<Store> Serialize for ZeroTrieSimpleAscii<Store>
where
    Store: AsRef<[u8]>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            let lm = self.to_litemap();
            lm.serialize(serializer)
        } else {
            // Note: `impl Serialize for ByteStr` uses `serialize_bytes`
            (Self::FLAGS, ByteStr::from_bytes(self.as_bytes())).serialize(serializer)
        }
    }
}

impl<'de, 'data, Store> Deserialize<'de> for ZeroAsciiIgnoreCaseTrie<Store>
where
    'de: 'data,
    // DISCUSS: There are several possibilities for the bounds here that would
    // get the job done. I could look for Deserialize, but this would require
    // creating a custom Deserializer for the map case. I also considered
    // introducing a new trait instead of relying on From.
    Store: From<&'data [u8]> + From<Vec<u8>> + 'data,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let lm = LiteMap::<Box<ByteStr>, usize>::deserialize(deserializer)?;
            ZeroAsciiIgnoreCaseTrie::try_from_serde_litemap(&lm)
                .map_err(D::Error::custom)
                .map(|trie| trie.convert_store())
        } else {
            // Note: `impl Deserialize for &[u8]` uses visit_borrowed_bytes
            let (flags, trie_bytes) = <(u8, &[u8])>::deserialize(deserializer)?;
            if Self::OPTIONS.to_u8_flags() != flags {
                return Err(D::Error::custom("invalid ZeroTrie tag"));
            }
            Ok(ZeroAsciiIgnoreCaseTrie::from_store(Store::from(trie_bytes)))
        }
    }
}

impl<Store> Serialize for ZeroAsciiIgnoreCaseTrie<Store>
where
    Store: AsRef<[u8]>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            let lm = self.to_litemap();
            lm.serialize(serializer)
        } else {
            // Note: `impl Serialize for ByteStr` uses `serialize_bytes`
            (
                Self::OPTIONS.to_u8_flags(),
                ByteStr::from_bytes(self.as_bytes()),
            )
                .serialize(serializer)
        }
    }
}

impl<'de, 'data, Store> Deserialize<'de> for ZeroTriePerfectHash<Store>
where
    'de: 'data,
    Store: From<&'data [u8]> + From<Vec<u8>> + 'data,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let lm = LiteMap::<Box<ByteStr>, usize>::deserialize(deserializer)?;
            ZeroTriePerfectHash::try_from_serde_litemap(&lm)
                .map_err(D::Error::custom)
                .map(|trie| trie.convert_store())
        } else {
            // Note: `impl Deserialize for &[u8]` uses visit_borrowed_bytes
            let (flags, trie_bytes) = <(u8, &[u8])>::deserialize(deserializer)?;
            if Self::OPTIONS.to_u8_flags() != flags {
                return Err(D::Error::custom("invalid ZeroTrie tag"));
            }
            Ok(ZeroTriePerfectHash::from_store(Store::from(trie_bytes)))
        }
    }
}

impl<Store> Serialize for ZeroTriePerfectHash<Store>
where
    Store: AsRef<[u8]>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            let lm = self.to_litemap();
            let lm = lm
                .iter()
                .map(|(k, v)| (ByteStr::from_bytes(k), v))
                .collect::<LiteMap<_, _>>();
            lm.serialize(serializer)
        } else {
            // Note: `impl Serialize for ByteStr` uses `serialize_bytes`
            (
                Self::OPTIONS.to_u8_flags(),
                ByteStr::from_bytes(self.as_bytes()),
            )
                .serialize(serializer)
        }
    }
}

impl<'de, 'data, Store> Deserialize<'de> for ZeroTrieExtendedCapacity<Store>
where
    'de: 'data,
    Store: From<&'data [u8]> + From<Vec<u8>> + 'data,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let lm = LiteMap::<Box<ByteStr>, usize>::deserialize(deserializer)?;
            ZeroTrieExtendedCapacity::try_from_serde_litemap(&lm)
                .map_err(D::Error::custom)
                .map(|trie| trie.convert_store())
        } else {
            // Note: `impl Deserialize for &[u8]` uses visit_borrowed_bytes
            let (flags, trie_bytes) = <(u8, &[u8])>::deserialize(deserializer)?;
            if Self::OPTIONS.to_u8_flags() != flags {
                return Err(D::Error::custom("invalid ZeroTrie tag"));
            }
            Ok(ZeroTrieExtendedCapacity::from_store(Store::from(
                trie_bytes,
            )))
        }
    }
}

impl<Store> Serialize for ZeroTrieExtendedCapacity<Store>
where
    Store: AsRef<[u8]>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            let lm = self.to_litemap();
            let lm = lm
                .iter()
                .map(|(k, v)| (ByteStr::from_bytes(k), v))
                .collect::<LiteMap<_, _>>();
            lm.serialize(serializer)
        } else {
            // Note: `impl Serialize for ByteStr` uses `serialize_bytes`
            (
                Self::OPTIONS.to_u8_flags(),
                ByteStr::from_bytes(self.as_bytes()),
            )
                .serialize(serializer)
        }
    }
}

impl<'de, 'data, Store> Deserialize<'de> for ZeroTrie<Store>
where
    'de: 'data,
    Store: From<&'data [u8]> + From<Vec<u8>> + 'data,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let lm = LiteMap::<Box<ByteStr>, usize>::deserialize(deserializer)?;
            ZeroTrie::<Vec<u8>>::try_from(&lm)
                .map_err(D::Error::custom)
                .map(|trie| trie.convert_store())
        } else {
            // Note: `impl Deserialize for &[u8]` uses visit_borrowed_bytes
            let bytes = <&[u8]>::deserialize(deserializer)?;
            let (tag, trie_bytes) = bytes
                .split_first()
                .ok_or(D::Error::custom("expected at least 1 byte for ZeroTrie"))?;
            let store = Store::from(trie_bytes);
            let zerotrie = if *tag == ZeroTrieSimpleAscii::<u8>::OPTIONS.to_u8_flags() {
                ZeroTrieSimpleAscii::from_store(store).into_zerotrie()
            } else if *tag == ZeroTriePerfectHash::<u8>::OPTIONS.to_u8_flags() {
                ZeroTriePerfectHash::from_store(store).into_zerotrie()
            } else if *tag == ZeroTrieExtendedCapacity::<u8>::OPTIONS.to_u8_flags() {
                ZeroTrieExtendedCapacity::from_store(store).into_zerotrie()
            } else {
                return Err(D::Error::custom("invalid ZeroTrie tag"));
            };
            Ok(zerotrie)
        }
    }
}

impl<Store> Serialize for ZeroTrie<Store>
where
    Store: AsRef<[u8]>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            let lm = self.to_litemap();
            let lm = lm
                .iter()
                .map(|(k, v)| (ByteStr::from_bytes(k), v))
                .collect::<LiteMap<_, _>>();
            lm.serialize(serializer)
        } else {
            let (tag, bytes) = match &self.0 {
                ZeroTrieFlavor::SimpleAscii(t) => (
                    ZeroTrieSimpleAscii::<u8>::OPTIONS.to_u8_flags(),
                    t.as_bytes(),
                ),
                ZeroTrieFlavor::PerfectHash(t) => (
                    ZeroTriePerfectHash::<u8>::OPTIONS.to_u8_flags(),
                    t.as_bytes(),
                ),
                ZeroTrieFlavor::ExtendedCapacity(t) => (
                    ZeroTrieExtendedCapacity::<u8>::OPTIONS.to_u8_flags(),
                    t.as_bytes(),
                ),
            };
            let mut all_in_one_vec = Vec::with_capacity(bytes.len() + 1);
            all_in_one_vec.push(tag);
            all_in_one_vec.extend(bytes);
            serializer.serialize_bytes(&all_in_one_vec)
        }
    }
}

#[cfg(test)]
mod testdata {
    include!("../tests/data/data.rs");
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::borrow::Cow;

    #[derive(Serialize, Deserialize)]
    pub struct ZeroTrieSimpleAsciiCow<'a> {
        #[serde(borrow)]
        trie: ZeroTrieSimpleAscii<Cow<'a, [u8]>>,
    }

    #[test]
    pub fn test_serde_simpleascii_cow() {
        let trie = ZeroTrieSimpleAscii::from_store(Cow::from(testdata::basic::TRIE_ASCII));
        let original = ZeroTrieSimpleAsciiCow { trie };
        let json_str = serde_json::to_string(&original).unwrap();
        let bincode_bytes = bincode::serialize(&original).unwrap();
        let rmp_bytes = rmp_serde::to_vec(&original).unwrap();

        assert_eq!(json_str, testdata::basic::JSON_STR_ASCII);
        assert_eq!(&bincode_bytes[0..9], &[0, 26, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&bincode_bytes[9..], testdata::basic::BINCODE_BYTES_ASCII);
        assert_eq!(&rmp_bytes[0..5], &[145, 146, 0, 196, 26]);
        assert_eq!(&rmp_bytes[5..], testdata::basic::BINCODE_BYTES_ASCII);

        let json_recovered: ZeroTrieSimpleAsciiCow = serde_json::from_str(&json_str).unwrap();
        let bincode_recovered: ZeroTrieSimpleAsciiCow =
            bincode::deserialize(&bincode_bytes).unwrap();
        let rmp_recovered: ZeroTrieSimpleAsciiCow = rmp_serde::from_slice(&rmp_bytes).unwrap();

        assert_eq!(original.trie, json_recovered.trie);
        assert_eq!(original.trie, bincode_recovered.trie);
        assert_eq!(original.trie, rmp_recovered.trie);

        assert!(matches!(json_recovered.trie.into_store(), Cow::Owned(_)));
        assert!(matches!(
            bincode_recovered.trie.into_store(),
            Cow::Borrowed(_)
        ));
    }

    #[derive(Serialize, Deserialize)]
    pub struct ZeroAsciiIgnoreCaseTrieCow<'a> {
        #[serde(borrow)]
        trie: ZeroAsciiIgnoreCaseTrie<Cow<'a, [u8]>>,
    }

    #[test]
    pub fn test_serde_asciiignorecase_cow() {
        let trie = ZeroAsciiIgnoreCaseTrie::from_store(Cow::from(testdata::basic::TRIE_ASCII));
        let original = ZeroAsciiIgnoreCaseTrieCow { trie };
        let json_str = serde_json::to_string(&original).unwrap();
        let bincode_bytes = bincode::serialize(&original).unwrap();

        assert_eq!(json_str, testdata::basic::JSON_STR_ASCII);
        assert_eq!(&bincode_bytes[0..9], &[8, 26, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&bincode_bytes[9..], testdata::basic::BINCODE_BYTES_ASCII);

        let json_recovered: ZeroAsciiIgnoreCaseTrieCow = serde_json::from_str(&json_str).unwrap();
        let bincode_recovered: ZeroAsciiIgnoreCaseTrieCow =
            bincode::deserialize(&bincode_bytes).unwrap();

        assert_eq!(original.trie, json_recovered.trie);
        assert_eq!(original.trie, bincode_recovered.trie);

        assert!(matches!(json_recovered.trie.into_store(), Cow::Owned(_)));
        assert!(matches!(
            bincode_recovered.trie.into_store(),
            Cow::Borrowed(_)
        ));
    }

    #[derive(Serialize, Deserialize)]
    pub struct ZeroTriePerfectHashCow<'a> {
        #[serde(borrow)]
        trie: ZeroTriePerfectHash<Cow<'a, [u8]>>,
    }

    #[test]
    pub fn test_serde_perfecthash_cow() {
        let trie = ZeroTriePerfectHash::from_store(Cow::from(testdata::basic::TRIE_ASCII));
        let original = ZeroTriePerfectHashCow { trie };
        let json_str = serde_json::to_string(&original).unwrap();
        let bincode_bytes = bincode::serialize(&original).unwrap();

        assert_eq!(json_str, testdata::basic::JSON_STR_ASCII);
        assert_eq!(&bincode_bytes[0..9], &[3, 26, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&bincode_bytes[9..], testdata::basic::BINCODE_BYTES_ASCII);

        let json_recovered: ZeroTriePerfectHashCow = serde_json::from_str(&json_str).unwrap();
        let bincode_recovered: ZeroTriePerfectHashCow =
            bincode::deserialize(&bincode_bytes).unwrap();

        assert_eq!(original.trie, json_recovered.trie);
        assert_eq!(original.trie, bincode_recovered.trie);

        assert!(matches!(json_recovered.trie.into_store(), Cow::Owned(_)));
        assert!(matches!(
            bincode_recovered.trie.into_store(),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    pub fn test_serde_perfecthash_cow_u() {
        let trie = ZeroTriePerfectHash::from_store(Cow::from(testdata::basic::TRIE_UNICODE));
        let original = ZeroTriePerfectHashCow { trie };
        let json_str = serde_json::to_string(&original).unwrap();
        let bincode_bytes = bincode::serialize(&original).unwrap();

        assert_eq!(json_str, testdata::basic::JSON_STR_UNICODE);
        assert_eq!(&bincode_bytes[0..9], &[3, 39, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&bincode_bytes[9..], testdata::basic::BINCODE_BYTES_UNICODE);

        let json_recovered: ZeroTriePerfectHashCow = serde_json::from_str(&json_str).unwrap();
        let bincode_recovered: ZeroTriePerfectHashCow =
            bincode::deserialize(&bincode_bytes).unwrap();

        assert_eq!(original.trie, json_recovered.trie);
        assert_eq!(original.trie, bincode_recovered.trie);

        assert!(matches!(json_recovered.trie.into_store(), Cow::Owned(_)));
        assert!(matches!(
            bincode_recovered.trie.into_store(),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    pub fn test_serde_perfecthash_cow_bin() {
        let trie = ZeroTriePerfectHash::from_store(Cow::from(testdata::basic::TRIE_BINARY));
        let original = ZeroTriePerfectHashCow { trie };
        let json_str = serde_json::to_string(&original).unwrap();
        let bincode_bytes = bincode::serialize(&original).unwrap();

        assert_eq!(json_str, testdata::basic::JSON_STR_BINARY);
        assert_eq!(&bincode_bytes[0..9], &[3, 26, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&bincode_bytes[9..], testdata::basic::BINCODE_BYTES_BINARY);

        let json_recovered: ZeroTriePerfectHashCow = serde_json::from_str(&json_str).unwrap();
        let bincode_recovered: ZeroTriePerfectHashCow =
            bincode::deserialize(&bincode_bytes).unwrap();

        assert_eq!(original.trie, json_recovered.trie);
        assert_eq!(original.trie, bincode_recovered.trie);

        assert!(matches!(json_recovered.trie.into_store(), Cow::Owned(_)));
        assert!(matches!(
            bincode_recovered.trie.into_store(),
            Cow::Borrowed(_)
        ));
    }

    #[derive(Serialize, Deserialize)]
    pub struct ZeroTrieAnyCow<'a> {
        #[serde(borrow)]
        trie: ZeroTrie<Cow<'a, [u8]>>,
    }

    #[test]
    pub fn test_serde_any_cow() {
        let trie =
            ZeroTrieSimpleAscii::from_store(Cow::from(testdata::basic::TRIE_ASCII)).into_zerotrie();
        let original = ZeroTrieAnyCow { trie };
        let json_str = serde_json::to_string(&original).unwrap();
        let bincode_bytes = bincode::serialize(&original).unwrap();

        assert_eq!(json_str, testdata::basic::JSON_STR_ASCII);
        assert_eq!(&bincode_bytes[0..9], &[27, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&bincode_bytes[9..], testdata::basic::BINCODE_BYTES_ASCII);

        let json_recovered: ZeroTrieAnyCow = serde_json::from_str(&json_str).unwrap();
        let bincode_recovered: ZeroTrieAnyCow = bincode::deserialize(&bincode_bytes).unwrap();

        assert_eq!(original.trie, json_recovered.trie);
        assert_eq!(original.trie, bincode_recovered.trie);

        assert!(matches!(json_recovered.trie.into_store(), Cow::Owned(_)));
        assert!(matches!(
            bincode_recovered.trie.into_store(),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    pub fn test_serde_any_cow_u() {
        let trie = ZeroTriePerfectHash::from_store(Cow::from(testdata::basic::TRIE_UNICODE))
            .into_zerotrie();
        let original = ZeroTrieAnyCow { trie };
        let json_str = serde_json::to_string(&original).unwrap();
        let bincode_bytes = bincode::serialize(&original).unwrap();

        assert_eq!(json_str, testdata::basic::JSON_STR_UNICODE);
        assert_eq!(&bincode_bytes[0..9], &[40, 0, 0, 0, 0, 0, 0, 0, 3]);
        assert_eq!(&bincode_bytes[9..], testdata::basic::BINCODE_BYTES_UNICODE);

        let json_recovered: ZeroTrieAnyCow = serde_json::from_str(&json_str).unwrap();
        let bincode_recovered: ZeroTrieAnyCow = bincode::deserialize(&bincode_bytes).unwrap();

        assert_eq!(original.trie, json_recovered.trie);
        assert_eq!(original.trie, bincode_recovered.trie);

        assert!(matches!(json_recovered.trie.into_store(), Cow::Owned(_)));
        assert!(matches!(
            bincode_recovered.trie.into_store(),
            Cow::Borrowed(_)
        ));
    }
}

#[cfg(test)]
#[cfg(feature = "zerovec")]
mod tests_zerovec {
    use super::*;
    use zerovec::ZeroVec;

    #[derive(Serialize, Deserialize)]
    pub struct ZeroTrieSimpleAsciiZeroVec<'a> {
        #[serde(borrow)]
        trie: ZeroTrieSimpleAscii<ZeroVec<'a, u8>>,
    }

    #[test]
    pub fn test_serde_simpleascii_zerovec() {
        let trie =
            ZeroTrieSimpleAscii::from_store(ZeroVec::new_borrowed(testdata::basic::TRIE_ASCII));
        let original = ZeroTrieSimpleAsciiZeroVec { trie };
        let json_str = serde_json::to_string(&original).unwrap();
        let bincode_bytes = bincode::serialize(&original).unwrap();

        assert_eq!(json_str, testdata::basic::JSON_STR_ASCII);
        assert_eq!(&bincode_bytes[0..9], &[0, 26, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&bincode_bytes[9..], testdata::basic::BINCODE_BYTES_ASCII);

        let json_recovered: ZeroTrieSimpleAsciiZeroVec = serde_json::from_str(&json_str).unwrap();
        let bincode_recovered: ZeroTrieSimpleAsciiZeroVec =
            bincode::deserialize(&bincode_bytes).unwrap();

        assert_eq!(original.trie, json_recovered.trie);
        assert_eq!(original.trie, bincode_recovered.trie);

        assert!(json_recovered.trie.into_store().is_owned());
        assert!(!bincode_recovered.trie.into_store().is_owned());
    }

    #[derive(Serialize, Deserialize)]
    pub struct ZeroTriePerfectHashZeroVec<'a> {
        #[serde(borrow)]
        trie: ZeroTriePerfectHash<ZeroVec<'a, u8>>,
    }

    #[test]
    pub fn test_serde_perfecthash_zerovec() {
        let trie =
            ZeroTriePerfectHash::from_store(ZeroVec::new_borrowed(testdata::basic::TRIE_ASCII));
        let original = ZeroTriePerfectHashZeroVec { trie };
        let json_str = serde_json::to_string(&original).unwrap();
        let bincode_bytes = bincode::serialize(&original).unwrap();

        assert_eq!(json_str, testdata::basic::JSON_STR_ASCII);
        assert_eq!(&bincode_bytes[0..9], &[3, 26, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&bincode_bytes[9..], testdata::basic::BINCODE_BYTES_ASCII);

        let json_recovered: ZeroTriePerfectHashZeroVec = serde_json::from_str(&json_str).unwrap();
        let bincode_recovered: ZeroTriePerfectHashZeroVec =
            bincode::deserialize(&bincode_bytes).unwrap();

        assert_eq!(original.trie, json_recovered.trie);
        assert_eq!(original.trie, bincode_recovered.trie);

        assert!(json_recovered.trie.into_store().is_owned());
        assert!(!bincode_recovered.trie.into_store().is_owned());
    }
}

use alloc::string::String;
use alloc::vec::Vec;

use serde::de::{
    Deserializer,
    Error,
    Unexpected,
    Visitor,
};

use crate::CompactString;

fn compact_string<'de: 'a, 'a, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<CompactString, D::Error> {
    struct CompactStringVisitor;

    impl<'a> Visitor<'a> for CompactStringVisitor {
        type Value = CompactString;

        fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
            formatter.write_str("a string")
        }

        fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(CompactString::from(v))
        }

        fn visit_borrowed_str<E: Error>(self, v: &'a str) -> Result<Self::Value, E> {
            Ok(CompactString::from(v))
        }

        fn visit_string<E: Error>(self, v: String) -> Result<Self::Value, E> {
            Ok(CompactString::from(v))
        }

        fn visit_bytes<E: Error>(self, v: &[u8]) -> Result<Self::Value, E> {
            match core::str::from_utf8(v) {
                Ok(s) => Ok(CompactString::from(s)),
                Err(_) => Err(Error::invalid_value(Unexpected::Bytes(v), &self)),
            }
        }

        fn visit_borrowed_bytes<E: Error>(self, v: &'a [u8]) -> Result<Self::Value, E> {
            match core::str::from_utf8(v) {
                Ok(s) => Ok(CompactString::from(s)),
                Err(_) => Err(Error::invalid_value(Unexpected::Bytes(v), &self)),
            }
        }

        fn visit_byte_buf<E: Error>(self, v: Vec<u8>) -> Result<Self::Value, E> {
            match String::from_utf8(v) {
                Ok(s) => Ok(CompactString::from(s)),
                Err(e) => Err(Error::invalid_value(
                    Unexpected::Bytes(&e.into_bytes()),
                    &self,
                )),
            }
        }
    }

    deserializer.deserialize_str(CompactStringVisitor)
}

#[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
impl serde::Serialize for CompactString {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_str().serialize(serializer)
    }
}

#[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
impl<'de> serde::Deserialize<'de> for CompactString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        compact_string(deserializer)
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::{
        String,
        ToString,
    };
    use alloc::vec::Vec;

    use serde::{
        Deserialize,
        Serialize,
    };
    use test_strategy::proptest;

    use crate::CompactString;

    #[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
    struct PersonString {
        name: String,
        phones: Vec<String>,
        address: Option<String>,
    }

    #[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
    struct PersonCompactString {
        name: CompactString,
        phones: Vec<CompactString>,
        address: Option<CompactString>,
    }

    #[test]
    fn test_roundtrip() {
        let name = "Ferris the Crab";
        let phones = vec!["1-800-111-1111", "2-222-222-2222"];
        let address = Some("123 Sesame Street");

        let std = PersonString {
            name: name.to_string(),
            phones: phones.iter().map(|s| s.to_string()).collect(),
            address: address.as_ref().map(|s| s.to_string()),
        };
        let compact = PersonCompactString {
            name: name.into(),
            phones: phones.iter().map(|s| CompactString::from(*s)).collect(),
            address: address.as_ref().map(|s| CompactString::from(*s)),
        };

        let std_json = serde_json::to_string(&std).unwrap();
        let compact_json = serde_json::to_string(&compact).unwrap();

        // the serialized forms should be the same
        assert_eq!(std_json, compact_json);

        let std_de_compact: PersonString = serde_json::from_str(&compact_json).unwrap();
        let compact_de_std: PersonCompactString = serde_json::from_str(&std_json).unwrap();

        // we should be able to deserailze from the opposite, serialized, source
        assert_eq!(std_de_compact, std);
        assert_eq!(compact_de_std, compact);
    }

    #[cfg_attr(miri, ignore)]
    #[proptest]
    fn proptest_roundtrip(name: String, phones: Vec<String>, address: Option<String>) {
        let std = PersonString {
            name: name.clone(),
            phones: phones.iter().map(|s| s.clone()).collect(),
            address: address.clone(),
        };
        let compact = PersonCompactString {
            name: name.into(),
            phones: phones.iter().map(|s| CompactString::from(s)).collect(),
            address: address.map(|s| CompactString::from(s)),
        };

        let std_json = serde_json::to_string(&std).unwrap();
        let compact_json = serde_json::to_string(&compact).unwrap();

        // the serialized forms should be the same
        assert_eq!(std_json, compact_json);

        let std_de_compact: PersonString = serde_json::from_str(&compact_json).unwrap();
        let compact_de_std: PersonCompactString = serde_json::from_str(&std_json).unwrap();

        // we should be able to deserailze from the opposite, serialized, source
        assert_eq!(std_de_compact, std);
        assert_eq!(compact_de_std, compact);
    }
}

use std::fmt;

use serde::de::{Error, Unexpected, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use ascii_str::AsciiStr;

impl Serialize for AsciiStr {
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

struct AsciiStrVisitor;

impl<'a> Visitor<'a> for AsciiStrVisitor {
    type Value = &'a AsciiStr;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a borrowed ascii string")
    }

    fn visit_borrowed_str<E: Error>(self, v: &'a str) -> Result<Self::Value, E> {
        AsciiStr::from_ascii(v.as_bytes())
            .map_err(|_| Error::invalid_value(Unexpected::Str(v), &self))
    }

    fn visit_borrowed_bytes<E: Error>(self, v: &'a [u8]) -> Result<Self::Value, E> {
        AsciiStr::from_ascii(v).map_err(|_| Error::invalid_value(Unexpected::Bytes(v), &self))
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for &'a AsciiStr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(AsciiStrVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "serde_test")]
    const ASCII: &str = "Francais";
    #[cfg(feature = "serde_test")]
    const UNICODE: &str = "Français";

    #[test]
    fn basic() {
        fn assert_serialize<T: Serialize>() {}
        fn assert_deserialize<'de, T: Deserialize<'de>>() {}
        assert_serialize::<&AsciiStr>();
        assert_deserialize::<&AsciiStr>();
    }

    #[test]
    #[cfg(feature = "serde_test")]
    fn serialize() {
        use serde_test::{assert_tokens, Token};
        let ascii_str = AsciiStr::from_ascii(ASCII).unwrap();
        assert_tokens(&ascii_str, &[Token::BorrowedStr(ASCII)]);
    }

    #[test]
    #[cfg(feature = "serde_test")]
    fn deserialize() {
        use serde_test::{assert_de_tokens, assert_de_tokens_error, Token};
        let ascii_str = AsciiStr::from_ascii(ASCII).unwrap();
        assert_de_tokens(&ascii_str, &[Token::BorrowedBytes(ASCII.as_bytes())]);
        assert_de_tokens_error::<&AsciiStr>(
            &[Token::BorrowedStr(UNICODE)],
            "invalid value: string \"Français\", expected a borrowed ascii string",
        );
    }
}

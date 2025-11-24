use std::fmt;

use serde::de::{Error, Unexpected, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use ascii_str::AsciiStr;
use ascii_string::AsciiString;

impl Serialize for AsciiString {
    #[inline]
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

struct AsciiStringVisitor;

impl<'de> Visitor<'de> for AsciiStringVisitor {
    type Value = AsciiString;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an ascii string")
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        AsciiString::from_ascii(v).map_err(|_| Error::invalid_value(Unexpected::Str(v), &self))
    }

    fn visit_string<E: Error>(self, v: String) -> Result<Self::Value, E> {
        AsciiString::from_ascii(v.as_bytes())
            .map_err(|_| Error::invalid_value(Unexpected::Str(&v), &self))
    }

    fn visit_bytes<E: Error>(self, v: &[u8]) -> Result<Self::Value, E> {
        AsciiString::from_ascii(v).map_err(|_| Error::invalid_value(Unexpected::Bytes(v), &self))
    }

    fn visit_byte_buf<E: Error>(self, v: Vec<u8>) -> Result<Self::Value, E> {
        AsciiString::from_ascii(v.as_slice())
            .map_err(|_| Error::invalid_value(Unexpected::Bytes(&v), &self))
    }
}

struct AsciiStringInPlaceVisitor<'a>(&'a mut AsciiString);

impl<'a, 'de> Visitor<'de> for AsciiStringInPlaceVisitor<'a> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an ascii string")
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        let ascii_str = match AsciiStr::from_ascii(v.as_bytes()) {
            Ok(ascii_str) => ascii_str,
            Err(_) => return Err(Error::invalid_value(Unexpected::Str(v), &self)),
        };
        self.0.clear();
        self.0.push_str(ascii_str);
        Ok(())
    }

    fn visit_string<E: Error>(self, v: String) -> Result<Self::Value, E> {
        let ascii_string = match AsciiString::from_ascii(v.as_bytes()) {
            Ok(ascii_string) => ascii_string,
            Err(_) => return Err(Error::invalid_value(Unexpected::Str(&v), &self)),
        };
        *self.0 = ascii_string;
        Ok(())
    }

    fn visit_bytes<E: Error>(self, v: &[u8]) -> Result<Self::Value, E> {
        let ascii_str = match AsciiStr::from_ascii(v) {
            Ok(ascii_str) => ascii_str,
            Err(_) => return Err(Error::invalid_value(Unexpected::Bytes(v), &self)),
        };
        self.0.clear();
        self.0.push_str(ascii_str);
        Ok(())
    }

    fn visit_byte_buf<E: Error>(self, v: Vec<u8>) -> Result<Self::Value, E> {
        let ascii_string = match AsciiString::from_ascii(v.as_slice()) {
            Ok(ascii_string) => ascii_string,
            Err(_) => return Err(Error::invalid_value(Unexpected::Bytes(&v), &self)),
        };
        *self.0 = ascii_string;
        Ok(())
    }
}

impl<'de> Deserialize<'de> for AsciiString {
    fn deserialize<D>(deserializer: D) -> Result<AsciiString, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(AsciiStringVisitor)
    }

    fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(AsciiStringInPlaceVisitor(place))
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
        assert_serialize::<AsciiString>();
        assert_deserialize::<AsciiString>();
    }

    #[test]
    #[cfg(feature = "serde_test")]
    fn serialize() {
        use serde_test::{assert_tokens, Token};

        let ascii_string = AsciiString::from_ascii(ASCII).unwrap();
        assert_tokens(&ascii_string, &[Token::String(ASCII)]);
        assert_tokens(&ascii_string, &[Token::Str(ASCII)]);
        assert_tokens(&ascii_string, &[Token::BorrowedStr(ASCII)]);
    }

    #[test]
    #[cfg(feature = "serde_test")]
    fn deserialize() {
        use serde_test::{assert_de_tokens, assert_de_tokens_error, Token};
        let ascii_string = AsciiString::from_ascii(ASCII).unwrap();
        assert_de_tokens(&ascii_string, &[Token::Bytes(ASCII.as_bytes())]);
        assert_de_tokens(&ascii_string, &[Token::BorrowedBytes(ASCII.as_bytes())]);
        assert_de_tokens(&ascii_string, &[Token::ByteBuf(ASCII.as_bytes())]);
        assert_de_tokens_error::<AsciiString>(
            &[Token::String(UNICODE)],
            "invalid value: string \"Français\", expected an ascii string",
        );
    }
}

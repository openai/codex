use crate::_alloc_prelude::*;
use alloc::borrow::Cow;
use core::fmt::Write as _;

/// Encodes a string for insertion into a JSON Pointer in URI fragment representation.
pub fn encode_ref_name(name: &str) -> Cow<str> {
    fn needs_encoding(byte: u8) -> bool {
        match byte {
            // `~` and `/` need encoding for JSON Pointer
            // See https://datatracker.ietf.org/doc/html/rfc6901#section-3
            b'~' | b'/' => true,
            // These chars (and `~`) are valid in URL fragment
            // See https://datatracker.ietf.org/doc/html/rfc3986/#section-3.5
            b'!' | b'$' | b'&'..=b';' | b'=' | b'?'..=b'Z' | b'_' | b'a'..=b'z' => false,
            // Everything else needs percent-encoding
            _ => true,
        }
    }

    if name.bytes().any(needs_encoding) {
        let mut buf = String::new();

        for byte in name.bytes() {
            if byte == b'~' {
                buf.push_str("~0");
            } else if byte == b'/' {
                buf.push_str("~1");
            } else if needs_encoding(byte) {
                write!(buf, "%{byte:2X}").unwrap();
            } else {
                buf.push(byte as char);
            }
        }

        Cow::Owned(buf)
    } else {
        Cow::Borrowed(name)
    }
}

/// Percent-decodes the given string, returning `None` if it results in invalid UTF-8.
/// A `%` that is not followed by two hex digits is treated as a literal `%`.
pub fn percent_decode(s: &str) -> Option<Cow<str>> {
    if s.contains('%') {
        let mut buf = Vec::<u8>::new();

        let mut segments = s.split('%');
        buf.extend(segments.next().unwrap_or_default().as_bytes());

        for segment in segments {
            if let Some(decoded_byte) = segment
                .get(0..2)
                .and_then(|p| u8::from_str_radix(p, 16).ok())
            {
                buf.push(decoded_byte);
                buf.extend(&segment.as_bytes()[2..]);
            } else {
                buf.push(b'%');
                buf.extend(segment.as_bytes());
            }
        }

        String::from_utf8(buf).ok().map(Cow::Owned)
    } else {
        Some(Cow::Borrowed(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_ref_name() {
        assert_eq!(encode_ref_name("Simple!"), "Simple!");
        assert_eq!(
            encode_ref_name("Needs %-encoding 🚀"),
            "Needs%20%25-encoding%20%F0%9F%9A%80"
        );
        assert_eq!(
            encode_ref_name("aA0-._!$&'()*+,;=:@?"),
            "aA0-._!$&'()*+,;=:@?",
        );
        assert_eq!(encode_ref_name("\"£%^\\~/"), "%22%C2%A3%25%5E%5C~0~1",);
    }

    #[test]
    fn test_percent_decode() {
        assert_eq!(percent_decode("Simple!"), Some("Simple!".into()));
        assert_eq!(
            percent_decode("Needs %-encoding 🚀"),
            Some("Needs %-encoding 🚀".into())
        );
        assert_eq!(
            percent_decode("Needs%20%25-encoding%20%F0%9F%9A%80"),
            Some("Needs %-encoding 🚀".into())
        );
        assert_eq!(
            percent_decode("aA0-._!$&'()*+,;=:@?"),
            Some("aA0-._!$&'()*+,;=:@?".into())
        );
        assert_eq!(percent_decode("\"£%^\\~/"), Some("\"£%^\\~/".into()));
        assert_eq!(
            percent_decode("%22%C2%A3%25%5E%5C~0~1"),
            Some("\"£%^\\~0~1".into())
        );
        assert_eq!(percent_decode("%%%2020%%%"), Some("%% 20%%%".into()));
        assert_eq!(percent_decode("%f0%9F%9a%80"), Some("🚀".into()));
        assert_eq!(percent_decode("%F0%9F%9A"), None);
    }
}

//! JSON serialization helpers for output that must remain parseable as JSON
//! while staying safe for ASCII-only transports.

use std::io;

use serde::Serialize;

struct AsciiJsonFormatter;

impl serde_json::ser::Formatter for AsciiJsonFormatter {
    // serde_json has no ensure_ascii flag; this formatter keeps its serializer
    // in charge and only escapes non-ASCII string fragments.
    fn write_string_fragment<W>(&mut self, writer: &mut W, fragment: &str) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        let mut start = 0;
        for (index, ch) in fragment.char_indices() {
            if ch.is_ascii() {
                continue;
            }

            if start < index {
                writer.write_all(&fragment.as_bytes()[start..index])?;
            }

            let mut utf16 = [0; 2];
            for code_unit in ch.encode_utf16(&mut utf16) {
                write!(writer, "\\u{code_unit:04x}")?;
            }
            start = index + ch.len_utf8();
        }

        if start < fragment.len() {
            writer.write_all(&fragment.as_bytes()[start..])?;
        }

        Ok(())
    }
}

/// Serialize JSON while escaping non-ASCII string content as `\uXXXX`.
///
/// This is useful when JSON needs to remain parseable as JSON but must be
/// carried through ASCII-safe transports such as HTTP headers.
pub fn to_ascii_json_string<T>(value: &T) -> serde_json::Result<String>
where
    T: Serialize + ?Sized,
{
    let mut bytes = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(&mut bytes, AsciiJsonFormatter);
    value.serialize(&mut serializer)?;
    String::from_utf8(bytes)
        .map_err(|err| serde_json::Error::io(io::Error::new(io::ErrorKind::InvalidData, err)))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::Value;
    use serde_json::json;

    use super::to_ascii_json_string;

    #[test]
    fn to_ascii_json_string_escapes_non_ascii_strings() {
        let value = json!({
            "workspaces": {
                "/tmp/東京": {
                    "label": "Agentlarım",
                    "emoji": "🚀"
                }
            }
        });

        let serialized = to_ascii_json_string(&value).expect("serialize ascii json");

        assert!(serialized.is_ascii());
        assert!(!serialized.contains("東京"));
        assert!(!serialized.contains("Agentlarım"));
        assert!(!serialized.contains("🚀"));
        let parsed: Value = serde_json::from_str(&serialized).expect("serialized json");
        assert_eq!(parsed, value);
    }
}

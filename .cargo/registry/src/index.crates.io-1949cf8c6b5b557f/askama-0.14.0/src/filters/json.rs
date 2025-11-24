use std::convert::Infallible;
use std::{fmt, io, str};

use serde::Serialize;
use serde_json::ser::{CompactFormatter, PrettyFormatter, Serializer};

use super::AsIndent;
use crate::ascii_str::{AsciiChar, AsciiStr};
use crate::{FastWritable, NO_VALUES, Values};

/// Serialize to JSON (requires `json` feature)
///
/// The generated string does not contain ampersands `&`, chevrons `< >`, or apostrophes `'`.
/// To use it in a `<script>` you can combine it with the safe filter:
///
/// ``` html
/// <script>
/// var data = {{data|json|safe}};
/// </script>
/// ```
///
/// To use it in HTML attributes, you can either use it in quotation marks `"{{data|json}}"` as is,
/// or in apostrophes with the (optional) safe filter `'{{data|json|safe}}'`.
/// In HTML texts the output of e.g. `<pre>{{data|json|safe}}</pre>` is safe, too.
///
/// ```
/// # #[cfg(feature = "code-in-doc")] {
/// # use askama::Template;
/// /// ```jinja
/// /// <div><li data-extra='{{data|json|safe}}'>Example</li></div>
/// /// ```
///
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example<'a> {
///     data: Vec<&'a str>,
/// }
///
/// assert_eq!(
///     Example { data: vec!["foo", "bar"] }.to_string(),
///     "<div><li data-extra='[\"foo\",\"bar\"]'>Example</li></div>"
/// );
/// # }
/// ```
#[inline]
pub fn json(value: impl Serialize) -> Result<impl fmt::Display, Infallible> {
    Ok(ToJson { value })
}

/// Serialize to formatted/prettified JSON (requires `json` feature)
///
/// This filter works the same as [`json()`], but it formats the data for human readability.
/// It has an additional "indent" argument, which can either be an integer how many spaces to use
/// for indentation (capped to 16 characters), or a string (e.g. `"\u{A0}\u{A0}"` for two
/// non-breaking spaces).
///
/// ### Note
///
/// In askama's template language, this filter is called `|json`, too. The right function is
/// automatically selected depending on whether an `indent` argument was provided or not.
///
/// ```
/// # #[cfg(feature = "code-in-doc")] {
/// # use askama::Template;
/// /// ```jinja
/// /// <div>{{data|json(4)|safe}}</div>
/// /// ```
///
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example<'a> {
///     data: Vec<&'a str>,
/// }
///
/// assert_eq!(
///     Example { data: vec!["foo", "bar"] }.to_string(),
///     "<div>[
///     \"foo\",
///     \"bar\"
/// ]</div>"
/// );
/// # }
/// ```
#[inline]
pub fn json_pretty(
    value: impl Serialize,
    indent: impl AsIndent,
) -> Result<impl fmt::Display, Infallible> {
    Ok(ToJsonPretty { value, indent })
}

#[derive(Debug, Clone)]
struct ToJson<S> {
    value: S,
}

#[derive(Debug, Clone)]
struct ToJsonPretty<S, I> {
    value: S,
    indent: I,
}

impl<S: Serialize> FastWritable for ToJson<S> {
    #[inline]
    fn write_into<W: fmt::Write + ?Sized>(&self, f: &mut W, _: &dyn Values) -> crate::Result<()> {
        serialize(f, &self.value, CompactFormatter)
    }
}

impl<S: Serialize> fmt::Display for ToJson<S> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(self.write_into(f, NO_VALUES)?)
    }
}

impl<S: Serialize, I: AsIndent> FastWritable for ToJsonPretty<S, I> {
    #[inline]
    fn write_into<W: fmt::Write + ?Sized>(&self, f: &mut W, _: &dyn Values) -> crate::Result<()> {
        serialize(
            f,
            &self.value,
            PrettyFormatter::with_indent(self.indent.as_indent().as_bytes()),
        )
    }
}

impl<S: Serialize, I: AsIndent> fmt::Display for ToJsonPretty<S, I> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(self.write_into(f, NO_VALUES)?)
    }
}

fn serialize<S, W, F>(dest: &mut W, value: &S, formatter: F) -> Result<(), crate::Error>
where
    S: Serialize + ?Sized,
    W: fmt::Write + ?Sized,
    F: serde_json::ser::Formatter,
{
    /// The struct must only ever be used with the output of `serde_json`.
    /// `serde_json` only produces UTF-8 strings in its `io::Write::write()` calls,
    /// and `<JsonWriter as io::Write>` depends on this invariant.
    struct JsonWriter<'a, W: fmt::Write + ?Sized>(&'a mut W);

    impl<W: fmt::Write + ?Sized> io::Write for JsonWriter<'_, W> {
        /// Invariant: must be passed valid UTF-8 slices
        #[inline]
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.write_all(bytes)?;
            Ok(bytes.len())
        }

        /// Invariant: must be passed valid UTF-8 slices
        fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
            // SAFETY: `serde_json` only writes valid strings
            let string = unsafe { std::str::from_utf8_unchecked(bytes) };
            write_escaped_str(&mut *self.0, string)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
        }

        #[inline]
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// Invariant: no character that needs escaping is multi-byte character when encoded in UTF-8;
    /// that is true for characters in ASCII range.
    #[inline]
    fn write_escaped_str(dest: &mut (impl fmt::Write + ?Sized), src: &str) -> fmt::Result {
        // This implementation reads one byte after another.
        // It's not very fast, but should work well enough until portable SIMD gets stabilized.

        let mut escaped_buf = ESCAPED_BUF_INIT;
        let mut last = 0;

        for (index, byte) in src.bytes().enumerate() {
            if let Some(escaped) = get_escaped(byte) {
                [escaped_buf[4], escaped_buf[5]] = escaped;
                write_str_if_nonempty(dest, &src[last..index])?;
                dest.write_str(AsciiStr::from_slice(&escaped_buf[..ESCAPED_BUF_LEN]))?;
                last = index + 1;
            }
        }
        write_str_if_nonempty(dest, &src[last..])
    }

    let mut serializer = Serializer::with_formatter(JsonWriter(dest), formatter);
    Ok(value.serialize(&mut serializer)?)
}

/// Returns the decimal representation of the codepoint if the character needs HTML escaping.
#[inline]
fn get_escaped(byte: u8) -> Option<[AsciiChar; 2]> {
    const _: () = assert!(CHAR_RANGE < 32);

    if let MIN_CHAR..=MAX_CHAR = byte {
        if (1u32 << (byte - MIN_CHAR)) & BITS != 0 {
            return Some(TABLE.0[(byte - MIN_CHAR) as usize]);
        }
    }
    None
}

#[inline(always)]
fn write_str_if_nonempty(output: &mut (impl fmt::Write + ?Sized), input: &str) -> fmt::Result {
    if !input.is_empty() {
        output.write_str(input)
    } else {
        Ok(())
    }
}

/// List of characters that need HTML escaping, not necessarily in ordinal order.
const CHARS: &[u8] = br#"&'<>"#;

/// The character with the lowest codepoint that needs HTML escaping.
const MIN_CHAR: u8 = {
    let mut v = u8::MAX;
    let mut i = 0;
    while i < CHARS.len() {
        if v > CHARS[i] {
            v = CHARS[i];
        }
        i += 1;
    }
    v
};

/// The character with the highest codepoint that needs HTML escaping.
const MAX_CHAR: u8 = {
    let mut v = u8::MIN;
    let mut i = 0;
    while i < CHARS.len() {
        if v < CHARS[i] {
            v = CHARS[i];
        }
        i += 1;
    }
    v
};

const BITS: u32 = {
    let mut bits = 0;
    let mut i = 0;
    while i < CHARS.len() {
        bits |= 1 << (CHARS[i] - MIN_CHAR);
        i += 1;
    }
    bits
};

/// Number of codepoints between the lowest and highest character that needs escaping, incl.
const CHAR_RANGE: usize = (MAX_CHAR - MIN_CHAR + 1) as usize;

#[repr(align(64))]
struct Table([[AsciiChar; 2]; CHAR_RANGE]);

/// For characters that need HTML escaping, the codepoint is formatted as decimal digits,
/// otherwise `b"\0\0"`. Starting at [`MIN_CHAR`].
const TABLE: &Table = &{
    let mut table = Table([UNESCAPED; CHAR_RANGE]);
    let mut i = 0;
    while i < CHARS.len() {
        let c = CHARS[i];
        table.0[c as u32 as usize - MIN_CHAR as usize] = AsciiChar::two_hex_digits(c as u32);
        i += 1;
    }
    table
};

const UNESCAPED: [AsciiChar; 2] = AsciiStr::new_sized("");

const ESCAPED_BUF_INIT_UNPADDED: &str = "\\u00__";
// RATIONALE: llvm generates better code if the buffer is register sized
const ESCAPED_BUF_INIT: [AsciiChar; 8] = AsciiStr::new_sized(ESCAPED_BUF_INIT_UNPADDED);
const ESCAPED_BUF_LEN: usize = ESCAPED_BUF_INIT_UNPADDED.len();

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use alloc::string::ToString;
    use alloc::vec;

    use super::*;

    #[test]
    fn test_ugly() {
        assert_eq!(json(true).unwrap().to_string(), "true");
        assert_eq!(json("foo").unwrap().to_string(), r#""foo""#);
        assert_eq!(json(true).unwrap().to_string(), "true");
        assert_eq!(json("foo").unwrap().to_string(), r#""foo""#);
        assert_eq!(
            json("<script>").unwrap().to_string(),
            r#""\u003cscript\u003e""#
        );
        assert_eq!(
            json(vec!["foo", "bar"]).unwrap().to_string(),
            r#"["foo","bar"]"#
        );
    }

    #[test]
    fn test_pretty() {
        assert_eq!(json_pretty(true, "").unwrap().to_string(), "true");
        assert_eq!(
            json_pretty("<script>", "").unwrap().to_string(),
            r#""\u003cscript\u003e""#
        );
        assert_eq!(
            json_pretty(vec!["foo", "bar"], "").unwrap().to_string(),
            r#"[
"foo",
"bar"
]"#
        );
        assert_eq!(
            json_pretty(vec!["foo", "bar"], 2).unwrap().to_string(),
            r#"[
  "foo",
  "bar"
]"#
        );
        assert_eq!(
            json_pretty(vec!["foo", "bar"], "————").unwrap().to_string(),
            r#"[
————"foo",
————"bar"
]"#
        );
    }
}

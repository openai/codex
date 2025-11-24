use std::borrow::Cow;
use std::fmt;
use std::io;
use std::str;

/// Wrapper type that implements `Display`. Encodes on the fly, without allocating.
/// Percent-encodes every byte except alphanumerics and `-`, `_`, `.`, `~`. Assumes UTF-8 encoding.
///
/// ```rust
/// use urlencoding::Encoded;
/// format!("{}", Encoded("hello!"));
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[repr(transparent)]
pub struct Encoded<Str>(pub Str);

impl<Str: AsRef<[u8]>> Encoded<Str> {
    /// Long way of writing `Encoded(data)`
    ///
    /// Takes any string-like type or a slice of bytes, either owned or borrowed.
    #[inline(always)]
    pub fn new(string: Str) -> Self {
        Self(string)
    }

    #[inline(always)]
    pub fn to_str(&self) -> Cow<str> {
        encode_binary(self.0.as_ref())
    }

    /// Perform urlencoding to a string
    #[inline]
    #[allow(clippy::inherent_to_string_shadow_display)]
    pub fn to_string(&self) -> String {
        self.to_str().into_owned()
    }

    /// Perform urlencoding into a writer
    #[inline]
    pub fn write<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        encode_into(self.0.as_ref(), false, |s| writer.write_all(s.as_bytes()))?;
        Ok(())
    }

    /// Perform urlencoding into a string
    #[inline]
    pub fn append_to(&self, string: &mut String) {
        append_string(self.0.as_ref(), string, false);
    }
}

impl<'a> Encoded<&'a str> {
    /// Same as new, but hints a more specific type, so you can avoid errors about `AsRef<[u8]>` not implemented
    /// on references-to-references.
    #[inline(always)]
    pub fn str(string: &'a str) -> Self {
        Self(string)
    }
}

impl<String: AsRef<[u8]>> fmt::Display for Encoded<String> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        encode_into(self.0.as_ref(), false, |s| f.write_str(s))?;
        Ok(())
    }
}

/// Percent-encodes every byte except alphanumerics and `-`, `_`, `.`, `~`. Assumes UTF-8 encoding.
///
/// Call `.into_owned()` if you need a `String`
#[inline(always)]
pub fn encode(data: &str) -> Cow<str> {
    encode_binary(data.as_bytes())
}

/// Percent-encodes every byte except alphanumerics and `-`, `_`, `.`, `~`.
#[inline]
pub fn encode_binary(data: &[u8]) -> Cow<str> {
    // add maybe extra capacity, but try not to exceed allocator's bucket size
    let mut escaped = String::with_capacity(data.len() | 15);
    let unmodified = append_string(data, &mut escaped, true);
    if unmodified {
        return Cow::Borrowed(unsafe {
            // encode_into has checked it's ASCII
            str::from_utf8_unchecked(data)
        });
    }
    Cow::Owned(escaped)
}

fn append_string(data: &[u8], escaped: &mut String, may_skip: bool) -> bool {
    encode_into(data, may_skip, |s| {
        escaped.push_str(s);
        Ok::<_, std::convert::Infallible>(())
    }).unwrap()
}

fn encode_into<E>(mut data: &[u8], may_skip_write: bool, mut push_str: impl FnMut(&str) -> Result<(), E>) -> Result<bool, E> {
    let mut pushed = false;
    loop {
        // Fast path to skip over safe chars at the beginning of the remaining string
        let ascii_len = data.iter()
            .take_while(|&&c| matches!(c, b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' |  b'-' | b'.' | b'_' | b'~')).count();

        let (safe, rest) = if ascii_len >= data.len() {
            if !pushed && may_skip_write {
                return Ok(true);
            }
            (data, &[][..]) // redundatnt to optimize out a panic in split_at
        } else {
            data.split_at(ascii_len)
        };
        pushed = true;
        if !safe.is_empty() {
            push_str(unsafe { str::from_utf8_unchecked(safe) })?;
        }
        if rest.is_empty() {
            break;
        }

        match rest.split_first() {
            Some((byte, rest)) => {
                let enc = &[b'%', to_hex_digit(byte >> 4), to_hex_digit(byte & 15)];
                push_str(unsafe { str::from_utf8_unchecked(enc) })?;
                data = rest;
            }
            None => break,
        };
    }
    Ok(false)
}

#[inline]
fn to_hex_digit(digit: u8) -> u8 {
    match digit {
        0..=9 => b'0' + digit,
        10..=255 => b'A' - 10 + digit,
    }
}

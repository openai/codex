/// An iterator of `char` values that represent an escaping of arbitrary bytes.
///
/// The lifetime parameter `'a` refers to the lifetime of the bytes being
/// escaped.
///
/// This iterator is created by the
/// [`ByteSlice::escape_bytes`](crate::ByteSlice::escape_bytes) method.
#[derive(Clone, Debug)]
pub struct EscapeBytes<'a> {
    remaining: &'a [u8],
    state: EscapeState,
}

impl<'a> EscapeBytes<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> EscapeBytes<'a> {
        EscapeBytes { remaining: bytes, state: EscapeState::Start }
    }
}

impl<'a> Iterator for EscapeBytes<'a> {
    type Item = char;

    #[inline]
    fn next(&mut self) -> Option<char> {
        use self::EscapeState::*;

        match self.state {
            Start => {
                let byte = match crate::decode_utf8(self.remaining) {
                    (None, 0) => return None,
                    // If we see invalid UTF-8 or ASCII, then we always just
                    // peel one byte off. If it's printable ASCII, we'll pass
                    // it through as-is below. Otherwise, below, it will get
                    // escaped in some way.
                    (None, _) | (Some(_), 1) => {
                        let byte = self.remaining[0];
                        self.remaining = &self.remaining[1..];
                        byte
                    }
                    // For any valid UTF-8 that is not ASCII, we pass it
                    // through as-is. We don't do any Unicode escaping.
                    (Some(ch), size) => {
                        self.remaining = &self.remaining[size..];
                        return Some(ch);
                    }
                };
                self.state = match byte {
                    0x21..=0x5B | 0x5D..=0x7E => {
                        return Some(char::from(byte))
                    }
                    b'\0' => SpecialEscape('0'),
                    b'\n' => SpecialEscape('n'),
                    b'\r' => SpecialEscape('r'),
                    b'\t' => SpecialEscape('t'),
                    b'\\' => SpecialEscape('\\'),
                    _ => HexEscapeX(byte),
                };
                Some('\\')
            }
            SpecialEscape(ch) => {
                self.state = Start;
                Some(ch)
            }
            HexEscapeX(byte) => {
                self.state = HexEscapeHighNybble(byte);
                Some('x')
            }
            HexEscapeHighNybble(byte) => {
                self.state = HexEscapeLowNybble(byte);
                let nybble = byte >> 4;
                Some(hexdigit_to_char(nybble))
            }
            HexEscapeLowNybble(byte) => {
                self.state = Start;
                let nybble = byte & 0xF;
                Some(hexdigit_to_char(nybble))
            }
        }
    }
}

impl<'a> core::fmt::Display for EscapeBytes<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use core::fmt::Write;
        for ch in self.clone() {
            f.write_char(ch)?;
        }
        Ok(())
    }
}

/// The state used by the FSM in the escaping iterator.
#[derive(Clone, Debug)]
enum EscapeState {
    /// Read and remove the next byte from 'remaining'. If 'remaining' is
    /// empty, then return None. Otherwise, escape the byte according to the
    /// following rules or emit it as-is.
    ///
    /// If it's \n, \r, \t, \\ or \0, then emit a '\' and set the current
    /// state to 'SpecialEscape(n | r | t | \ | 0)'. Otherwise, if the 'byte'
    /// is not in [\x21-\x5B\x5D-\x7E], then emit a '\' and set the state to
    /// to 'HexEscapeX(byte)'.
    Start,
    /// Emit the given codepoint as is. This assumes '\' has just been emitted.
    /// Then set the state to 'Start'.
    SpecialEscape(char),
    /// Emit the 'x' part of a hex escape. This assumes '\' has just been
    /// emitted. Then set the state to 'HexEscapeHighNybble(byte)'.
    HexEscapeX(u8),
    /// Emit the high nybble of the byte as a hexadecimal digit. This
    /// assumes '\x' has just been emitted. Then set the state to
    /// 'HexEscapeLowNybble(byte)'.
    HexEscapeHighNybble(u8),
    /// Emit the low nybble of the byte as a hexadecimal digit. This assume
    /// '\xZ' has just been emitted, where 'Z' is the high nybble of this byte.
    /// Then set the state to 'Start'.
    HexEscapeLowNybble(u8),
}

/// An iterator of `u8` values that represent an unescaping of a sequence of
/// codepoints.
///
/// The type parameter `I` refers to the iterator of codepoints that is
/// unescaped.
///
/// Currently this iterator is not exposed in the crate API, and instead all
/// we expose is a `ByteVec::unescape` method. Which of course requires an
/// alloc. That's the most convenient form of this, but in theory, we could
/// expose this for core-only use cases too. I'm just not quite sure what the
/// API should be.
#[derive(Clone, Debug)]
#[cfg(feature = "alloc")]
pub(crate) struct UnescapeBytes<I> {
    it: I,
    state: UnescapeState,
}

#[cfg(feature = "alloc")]
impl<I: Iterator<Item = char>> UnescapeBytes<I> {
    pub(crate) fn new<T: IntoIterator<IntoIter = I>>(
        t: T,
    ) -> UnescapeBytes<I> {
        UnescapeBytes { it: t.into_iter(), state: UnescapeState::Start }
    }
}

#[cfg(feature = "alloc")]
impl<I: Iterator<Item = char>> Iterator for UnescapeBytes<I> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        use self::UnescapeState::*;

        loop {
            match self.state {
                Start => {
                    let ch = self.it.next()?;
                    match ch {
                        '\\' => {
                            self.state = Escape;
                        }
                        ch => {
                            self.state = UnescapeState::bytes(&[], ch);
                        }
                    }
                }
                Bytes { buf, mut cur, len } => {
                    let byte = buf[cur];
                    cur += 1;
                    if cur >= len {
                        self.state = Start;
                    } else {
                        self.state = Bytes { buf, cur, len };
                    }
                    return Some(byte);
                }
                Escape => {
                    let ch = match self.it.next() {
                        Some(ch) => ch,
                        None => {
                            self.state = Start;
                            // Incomplete escape sequences unescape as
                            // themselves.
                            return Some(b'\\');
                        }
                    };
                    match ch {
                        '0' => {
                            self.state = Start;
                            return Some(b'\x00');
                        }
                        '\\' => {
                            self.state = Start;
                            return Some(b'\\');
                        }
                        'r' => {
                            self.state = Start;
                            return Some(b'\r');
                        }
                        'n' => {
                            self.state = Start;
                            return Some(b'\n');
                        }
                        't' => {
                            self.state = Start;
                            return Some(b'\t');
                        }
                        'x' => {
                            self.state = HexFirst;
                        }
                        ch => {
                            // An invalid escape sequence unescapes as itself.
                            self.state = UnescapeState::bytes(&[b'\\'], ch);
                        }
                    }
                }
                HexFirst => {
                    let ch = match self.it.next() {
                        Some(ch) => ch,
                        None => {
                            // An incomplete escape sequence unescapes as
                            // itself.
                            self.state = UnescapeState::bytes_raw(&[b'x']);
                            return Some(b'\\');
                        }
                    };
                    match ch {
                        '0'..='9' | 'A'..='F' | 'a'..='f' => {
                            self.state = HexSecond(ch);
                        }
                        ch => {
                            // An invalid escape sequence unescapes as itself.
                            self.state = UnescapeState::bytes(&[b'x'], ch);
                            return Some(b'\\');
                        }
                    }
                }
                HexSecond(first) => {
                    let second = match self.it.next() {
                        Some(ch) => ch,
                        None => {
                            // An incomplete escape sequence unescapes as
                            // itself.
                            self.state = UnescapeState::bytes(&[b'x'], first);
                            return Some(b'\\');
                        }
                    };
                    match second {
                        '0'..='9' | 'A'..='F' | 'a'..='f' => {
                            self.state = Start;
                            let hinybble = char_to_hexdigit(first);
                            let lonybble = char_to_hexdigit(second);
                            let byte = hinybble << 4 | lonybble;
                            return Some(byte);
                        }
                        ch => {
                            // An invalid escape sequence unescapes as itself.
                            self.state =
                                UnescapeState::bytes2(&[b'x'], first, ch);
                            return Some(b'\\');
                        }
                    }
                }
            }
        }
    }
}

/// The state used by the FSM in the unescaping iterator.
#[derive(Clone, Debug)]
#[cfg(feature = "alloc")]
enum UnescapeState {
    /// The start state. Look for an escape sequence, otherwise emit the next
    /// codepoint as-is.
    Start,
    /// Emit the byte at `buf[cur]`.
    ///
    /// This state should never be created when `cur >= len`. That is, when
    /// this state is visited, it is assumed that `cur < len`.
    Bytes { buf: [u8; 11], cur: usize, len: usize },
    /// This state is entered after a `\` is seen.
    Escape,
    /// This state is entered after a `\x` is seen.
    HexFirst,
    /// This state is entered after a `\xN` is seen, where `N` is in
    /// `[0-9A-Fa-f]`. The given codepoint corresponds to `N`.
    HexSecond(char),
}

#[cfg(feature = "alloc")]
impl UnescapeState {
    /// Create a new `Bytes` variant with the given slice.
    ///
    /// # Panics
    ///
    /// Panics if `bytes.len() > 11`.
    fn bytes_raw(bytes: &[u8]) -> UnescapeState {
        // This can be increased, you just need to make sure 'buf' in the
        // 'Bytes' state has enough room.
        assert!(bytes.len() <= 11, "no more than 11 bytes allowed");
        let mut buf = [0; 11];
        buf[..bytes.len()].copy_from_slice(bytes);
        UnescapeState::Bytes { buf, cur: 0, len: bytes.len() }
    }

    /// Create a new `Bytes` variant with the prefix byte slice, followed by
    /// the UTF-8 encoding of the given char.
    ///
    /// # Panics
    ///
    /// Panics if `prefix.len() > 3`.
    fn bytes(prefix: &[u8], ch: char) -> UnescapeState {
        // This can be increased, you just need to make sure 'buf' in the
        // 'Bytes' state has enough room.
        assert!(prefix.len() <= 3, "no more than 3 bytes allowed");
        let mut buf = [0; 11];
        buf[..prefix.len()].copy_from_slice(prefix);
        let chlen = ch.encode_utf8(&mut buf[prefix.len()..]).len();
        UnescapeState::Bytes { buf, cur: 0, len: prefix.len() + chlen }
    }

    /// Create a new `Bytes` variant with the prefix byte slice, followed by
    /// the UTF-8 encoding of `ch1` and then `ch2`.
    ///
    /// # Panics
    ///
    /// Panics if `prefix.len() > 3`.
    fn bytes2(prefix: &[u8], ch1: char, ch2: char) -> UnescapeState {
        // This can be increased, you just need to make sure 'buf' in the
        // 'Bytes' state has enough room.
        assert!(prefix.len() <= 3, "no more than 3 bytes allowed");
        let mut buf = [0; 11];
        buf[..prefix.len()].copy_from_slice(prefix);
        let len1 = ch1.encode_utf8(&mut buf[prefix.len()..]).len();
        let len2 = ch2.encode_utf8(&mut buf[prefix.len() + len1..]).len();
        UnescapeState::Bytes { buf, cur: 0, len: prefix.len() + len1 + len2 }
    }
}

/// Convert the given codepoint to its corresponding hexadecimal digit.
///
/// # Panics
///
/// This panics if `ch` is not in `[0-9A-Fa-f]`.
#[cfg(feature = "alloc")]
fn char_to_hexdigit(ch: char) -> u8 {
    u8::try_from(ch.to_digit(16).unwrap()).unwrap()
}

/// Convert the given hexadecimal digit to its corresponding codepoint.
///
/// # Panics
///
/// This panics when `digit > 15`.
fn hexdigit_to_char(digit: u8) -> char {
    char::from_digit(u32::from(digit), 16).unwrap().to_ascii_uppercase()
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use alloc::string::{String, ToString};

    use crate::BString;

    use super::*;

    #[allow(non_snake_case)]
    fn B<B: AsRef<[u8]>>(bytes: B) -> BString {
        BString::from(bytes.as_ref())
    }

    fn e<B: AsRef<[u8]>>(bytes: B) -> String {
        EscapeBytes::new(bytes.as_ref()).to_string()
    }

    fn u(string: &str) -> BString {
        UnescapeBytes::new(string.chars()).collect()
    }

    #[test]
    fn escape() {
        assert_eq!(r"a", e(br"a"));
        assert_eq!(r"\\x61", e(br"\x61"));
        assert_eq!(r"a", e(b"\x61"));
        assert_eq!(r"~", e(b"\x7E"));
        assert_eq!(r"\x7F", e(b"\x7F"));

        assert_eq!(r"\n", e(b"\n"));
        assert_eq!(r"\r", e(b"\r"));
        assert_eq!(r"\t", e(b"\t"));
        assert_eq!(r"\\", e(b"\\"));
        assert_eq!(r"\0", e(b"\0"));
        assert_eq!(r"\0", e(b"\x00"));

        assert_eq!(r"\x88", e(b"\x88"));
        assert_eq!(r"\x8F", e(b"\x8F"));
        assert_eq!(r"\xF8", e(b"\xF8"));
        assert_eq!(r"\xFF", e(b"\xFF"));

        assert_eq!(r"\xE2", e(b"\xE2"));
        assert_eq!(r"\xE2\x98", e(b"\xE2\x98"));
        assert_eq!(r"☃", e(b"\xE2\x98\x83"));

        assert_eq!(r"\xF0", e(b"\xF0"));
        assert_eq!(r"\xF0\x9F", e(b"\xF0\x9F"));
        assert_eq!(r"\xF0\x9F\x92", e(b"\xF0\x9F\x92"));
        assert_eq!(r"💩", e(b"\xF0\x9F\x92\xA9"));
    }

    #[test]
    fn unescape() {
        assert_eq!(B(r"a"), u(r"a"));
        assert_eq!(B(r"\x61"), u(r"\\x61"));
        assert_eq!(B(r"a"), u(r"\x61"));
        assert_eq!(B(r"~"), u(r"\x7E"));
        assert_eq!(B(b"\x7F"), u(r"\x7F"));

        assert_eq!(B(b"\n"), u(r"\n"));
        assert_eq!(B(b"\r"), u(r"\r"));
        assert_eq!(B(b"\t"), u(r"\t"));
        assert_eq!(B(b"\\"), u(r"\\"));
        assert_eq!(B(b"\0"), u(r"\0"));
        assert_eq!(B(b"\0"), u(r"\x00"));

        assert_eq!(B(b"\x88"), u(r"\x88"));
        assert_eq!(B(b"\x8F"), u(r"\x8F"));
        assert_eq!(B(b"\xF8"), u(r"\xF8"));
        assert_eq!(B(b"\xFF"), u(r"\xFF"));

        assert_eq!(B(b"\xE2"), u(r"\xE2"));
        assert_eq!(B(b"\xE2\x98"), u(r"\xE2\x98"));
        assert_eq!(B("☃"), u(r"\xE2\x98\x83"));

        assert_eq!(B(b"\xF0"), u(r"\xf0"));
        assert_eq!(B(b"\xF0\x9F"), u(r"\xf0\x9f"));
        assert_eq!(B(b"\xF0\x9F\x92"), u(r"\xf0\x9f\x92"));
        assert_eq!(B("💩"), u(r"\xf0\x9f\x92\xa9"));
    }

    #[test]
    fn unescape_weird() {
        assert_eq!(B(b"\\"), u(r"\"));
        assert_eq!(B(b"\\"), u(r"\\"));
        assert_eq!(B(b"\\x"), u(r"\x"));
        assert_eq!(B(b"\\xA"), u(r"\xA"));

        assert_eq!(B(b"\\xZ"), u(r"\xZ"));
        assert_eq!(B(b"\\xZZ"), u(r"\xZZ"));
        assert_eq!(B(b"\\i"), u(r"\i"));
        assert_eq!(B(b"\\u"), u(r"\u"));
        assert_eq!(B(b"\\u{2603}"), u(r"\u{2603}"));
    }
}

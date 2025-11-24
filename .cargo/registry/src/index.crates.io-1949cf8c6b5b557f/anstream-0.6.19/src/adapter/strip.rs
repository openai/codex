use anstyle_parse::state::state_change;
use anstyle_parse::state::Action;
use anstyle_parse::state::State;

/// Strip ANSI escapes from a `&str`, returning the printable content
///
/// This can be used to take output from a program that includes escape sequences and write it
/// somewhere that does not easily support them, such as a log file.
///
/// For non-contiguous data, see [`StripStr`].
///
/// # Example
///
/// ```rust
/// use std::io::Write as _;
///
/// let styled_text = "\x1b[32mfoo\x1b[m bar";
/// let plain_str = anstream::adapter::strip_str(&styled_text).to_string();
/// assert_eq!(plain_str, "foo bar");
/// ```
#[inline]
pub fn strip_str(data: &str) -> StrippedStr<'_> {
    StrippedStr::new(data)
}

/// See [`strip_str`]
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct StrippedStr<'s> {
    bytes: &'s [u8],
    state: State,
}

impl<'s> StrippedStr<'s> {
    #[inline]
    fn new(data: &'s str) -> Self {
        Self {
            bytes: data.as_bytes(),
            state: State::Ground,
        }
    }

    /// Create a [`String`] of the printable content
    #[inline]
    #[allow(clippy::inherent_to_string_shadow_display)] // Single-allocation implementation
    pub fn to_string(&self) -> String {
        use std::fmt::Write as _;
        let mut stripped = String::with_capacity(self.bytes.len());
        let _ = write!(&mut stripped, "{self}");
        stripped
    }
}

impl std::fmt::Display for StrippedStr<'_> {
    /// **Note:** this does *not* exhaust the [`Iterator`]
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let iter = Self {
            bytes: self.bytes,
            state: self.state,
        };
        for printable in iter {
            printable.fmt(f)?;
        }
        Ok(())
    }
}

impl<'s> Iterator for StrippedStr<'s> {
    type Item = &'s str;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        next_str(&mut self.bytes, &mut self.state)
    }
}

/// Incrementally strip non-contiguous data
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct StripStr {
    state: State,
}

impl StripStr {
    /// Initial state
    pub fn new() -> Self {
        Default::default()
    }

    /// Strip the next segment of data
    pub fn strip_next<'s>(&'s mut self, data: &'s str) -> StripStrIter<'s> {
        StripStrIter {
            bytes: data.as_bytes(),
            state: &mut self.state,
        }
    }
}

/// See [`StripStr`]
#[derive(Debug, PartialEq, Eq)]
pub struct StripStrIter<'s> {
    bytes: &'s [u8],
    state: &'s mut State,
}

impl<'s> Iterator for StripStrIter<'s> {
    type Item = &'s str;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        next_str(&mut self.bytes, self.state)
    }
}

#[inline]
fn next_str<'s>(bytes: &mut &'s [u8], state: &mut State) -> Option<&'s str> {
    let offset = bytes.iter().copied().position(|b| {
        let (next_state, action) = state_change(*state, b);
        if next_state != State::Anywhere {
            *state = next_state;
        }
        is_printable_bytes(action, b)
    });
    let (_, next) = bytes.split_at(offset.unwrap_or(bytes.len()));
    *bytes = next;
    *state = State::Ground;

    let offset = bytes.iter().copied().position(|b| {
        let (_next_state, action) = state_change(State::Ground, b);
        !(is_printable_bytes(action, b) || is_utf8_continuation(b))
    });
    let (printable, next) = bytes.split_at(offset.unwrap_or(bytes.len()));
    *bytes = next;
    if printable.is_empty() {
        None
    } else {
        let printable = unsafe {
            from_utf8_unchecked(
                printable,
                "`bytes` was validated as UTF-8, the parser preserves UTF-8 continuations",
            )
        };
        Some(printable)
    }
}

#[inline]
unsafe fn from_utf8_unchecked<'b>(bytes: &'b [u8], safety_justification: &'static str) -> &'b str {
    unsafe {
        if cfg!(debug_assertions) {
            // Catch problems more quickly when testing
            std::str::from_utf8(bytes).expect(safety_justification)
        } else {
            std::str::from_utf8_unchecked(bytes)
        }
    }
}

#[inline]
fn is_utf8_continuation(b: u8) -> bool {
    matches!(b, 0x80..=0xbf)
}

/// Strip ANSI escapes from bytes, returning the printable content
///
/// This can be used to take output from a program that includes escape sequences and write it
/// somewhere that does not easily support them, such as a log file.
///
/// # Example
///
/// ```rust
/// use std::io::Write as _;
///
/// let styled_text = "\x1b[32mfoo\x1b[m bar";
/// let plain_str = anstream::adapter::strip_bytes(styled_text.as_bytes()).into_vec();
/// assert_eq!(plain_str.as_slice(), &b"foo bar"[..]);
/// ```
#[inline]
pub fn strip_bytes(data: &[u8]) -> StrippedBytes<'_> {
    StrippedBytes::new(data)
}

/// See [`strip_bytes`]
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct StrippedBytes<'s> {
    bytes: &'s [u8],
    state: State,
    utf8parser: Utf8Parser,
}

impl<'s> StrippedBytes<'s> {
    /// See [`strip_bytes`]
    #[inline]
    pub fn new(bytes: &'s [u8]) -> Self {
        Self {
            bytes,
            state: State::Ground,
            utf8parser: Default::default(),
        }
    }

    /// Strip the next slice of bytes
    ///
    /// Used when the content is in several non-contiguous slices
    ///
    /// # Panic
    ///
    /// May panic if it is not exhausted / empty
    #[inline]
    pub fn extend(&mut self, bytes: &'s [u8]) {
        debug_assert!(
            self.is_empty(),
            "current bytes must be processed to ensure we end at the right state"
        );
        self.bytes = bytes;
    }

    /// Report the bytes has been exhausted
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Create a [`Vec`] of the printable content
    #[inline]
    pub fn into_vec(self) -> Vec<u8> {
        let mut stripped = Vec::with_capacity(self.bytes.len());
        for printable in self {
            stripped.extend(printable);
        }
        stripped
    }
}

impl<'s> Iterator for StrippedBytes<'s> {
    type Item = &'s [u8];

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        next_bytes(&mut self.bytes, &mut self.state, &mut self.utf8parser)
    }
}

/// Incrementally strip non-contiguous data
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct StripBytes {
    state: State,
    utf8parser: Utf8Parser,
}

impl StripBytes {
    /// Initial state
    pub fn new() -> Self {
        Default::default()
    }

    /// Strip the next segment of data
    pub fn strip_next<'s>(&'s mut self, bytes: &'s [u8]) -> StripBytesIter<'s> {
        StripBytesIter {
            bytes,
            state: &mut self.state,
            utf8parser: &mut self.utf8parser,
        }
    }
}

/// See [`StripBytes`]
#[derive(Debug, PartialEq, Eq)]
pub struct StripBytesIter<'s> {
    bytes: &'s [u8],
    state: &'s mut State,
    utf8parser: &'s mut Utf8Parser,
}

impl<'s> Iterator for StripBytesIter<'s> {
    type Item = &'s [u8];

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        next_bytes(&mut self.bytes, self.state, self.utf8parser)
    }
}

#[inline]
fn next_bytes<'s>(
    bytes: &mut &'s [u8],
    state: &mut State,
    utf8parser: &mut Utf8Parser,
) -> Option<&'s [u8]> {
    let offset = bytes.iter().copied().position(|b| {
        if *state == State::Utf8 {
            true
        } else {
            let (next_state, action) = state_change(*state, b);
            if next_state != State::Anywhere {
                *state = next_state;
            }
            is_printable_bytes(action, b)
        }
    });
    let (_, next) = bytes.split_at(offset.unwrap_or(bytes.len()));
    *bytes = next;

    let offset = bytes.iter().copied().position(|b| {
        if *state == State::Utf8 {
            if utf8parser.add(b) {
                *state = State::Ground;
            }
            false
        } else {
            let (next_state, action) = state_change(State::Ground, b);
            if next_state != State::Anywhere {
                *state = next_state;
            }
            if *state == State::Utf8 {
                utf8parser.add(b);
                false
            } else {
                !is_printable_bytes(action, b)
            }
        }
    });
    let (printable, next) = bytes.split_at(offset.unwrap_or(bytes.len()));
    *bytes = next;
    if printable.is_empty() {
        None
    } else {
        Some(printable)
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Utf8Parser {
    utf8_parser: utf8parse::Parser,
}

impl Utf8Parser {
    fn add(&mut self, byte: u8) -> bool {
        let mut b = false;
        let mut receiver = VtUtf8Receiver(&mut b);
        self.utf8_parser.advance(&mut receiver, byte);
        b
    }
}

struct VtUtf8Receiver<'a>(&'a mut bool);

impl utf8parse::Receiver for VtUtf8Receiver<'_> {
    fn codepoint(&mut self, _: char) {
        *self.0 = true;
    }

    fn invalid_sequence(&mut self) {
        *self.0 = true;
    }
}

#[inline]
fn is_printable_bytes(action: Action, byte: u8) -> bool {
    // VT320 considered 0x7f to be `Print`able but we expect to be working in UTF-8 systems and not
    // ISO Latin-1, making it DEL and non-printable
    const DEL: u8 = 0x7f;

    // Continuations aren't included as they may also be control codes, requiring more context
    (action == Action::Print && byte != DEL)
        || action == Action::BeginUtf8
        || (action == Action::Execute && byte.is_ascii_whitespace())
}

#[cfg(test)]
mod test {
    use super::*;
    use proptest::prelude::*;

    /// Model based off full parser
    fn parser_strip(bytes: &[u8]) -> String {
        #[derive(Default)]
        struct Strip(String);
        impl Strip {
            fn with_capacity(capacity: usize) -> Self {
                Self(String::with_capacity(capacity))
            }
        }
        impl anstyle_parse::Perform for Strip {
            fn print(&mut self, c: char) {
                self.0.push(c);
            }

            fn execute(&mut self, byte: u8) {
                if byte.is_ascii_whitespace() {
                    self.0.push(byte as char);
                }
            }
        }

        let mut stripped = Strip::with_capacity(bytes.len());
        let mut parser = anstyle_parse::Parser::<anstyle_parse::DefaultCharAccumulator>::new();
        for byte in bytes {
            parser.advance(&mut stripped, *byte);
        }
        stripped.0
    }

    /// Model verifying incremental parsing
    fn strip_char(mut s: &str) -> String {
        let mut result = String::new();
        let mut state = StripStr::new();
        while !s.is_empty() {
            let mut indices = s.char_indices();
            indices.next(); // current
            let offset = indices.next().map(|(i, _)| i).unwrap_or_else(|| s.len());
            let (current, remainder) = s.split_at(offset);
            for printable in state.strip_next(current) {
                result.push_str(printable);
            }
            s = remainder;
        }
        result
    }

    /// Model verifying incremental parsing
    fn strip_byte(s: &[u8]) -> Vec<u8> {
        let mut result = Vec::new();
        let mut state = StripBytes::default();
        for start in 0..s.len() {
            let current = &s[start..=start];
            for printable in state.strip_next(current) {
                result.extend(printable);
            }
        }
        result
    }

    #[test]
    fn test_strip_bytes_multibyte() {
        let bytes = [240, 145, 141, 139];
        let expected = parser_strip(&bytes);
        let actual = String::from_utf8(strip_bytes(&bytes).into_vec()).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_strip_byte_multibyte() {
        let bytes = [240, 145, 141, 139];
        let expected = parser_strip(&bytes);
        let actual = String::from_utf8(strip_byte(&bytes).clone()).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_strip_str_del() {
        let input = std::str::from_utf8(&[0x7f]).unwrap();
        let expected = "";
        let actual = strip_str(input).to_string();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_strip_byte_del() {
        let bytes = [0x7f];
        let expected = "";
        let actual = String::from_utf8(strip_byte(&bytes).clone()).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_strip_str_handles_broken_sequence() {
        // valid utf8: \xc3\xb6 then \x1b then \xf0\x9f\x98\x80
        let s = "ö\x1b😀hello😀goodbye";
        let mut it = strip_str(s);
        assert_eq!("ö", it.next().unwrap());
        assert_eq!("ello😀goodbye", it.next().unwrap());
    }

    proptest! {
        #[test]
        #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
        fn strip_str_no_escapes(s in "\\PC*") {
            let expected = parser_strip(s.as_bytes());
            let actual = strip_str(&s).to_string();
            assert_eq!(expected, actual);
        }

        #[test]
        #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
        fn strip_char_no_escapes(s in "\\PC*") {
            let expected = parser_strip(s.as_bytes());
            let actual = strip_char(&s);
            assert_eq!(expected, actual);
        }

        #[test]
        #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
        fn strip_bytes_no_escapes(s in "\\PC*") {
            dbg!(&s);
            dbg!(s.as_bytes());
            let expected = parser_strip(s.as_bytes());
            let actual = String::from_utf8(strip_bytes(s.as_bytes()).into_vec()).unwrap();
            assert_eq!(expected, actual);
        }

        #[test]
        #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
        fn strip_byte_no_escapes(s in "\\PC*") {
            dbg!(&s);
            dbg!(s.as_bytes());
            let expected = parser_strip(s.as_bytes());
            let actual = String::from_utf8(strip_byte(s.as_bytes()).clone()).unwrap();
            assert_eq!(expected, actual);
        }
    }
}

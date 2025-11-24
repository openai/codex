use std::borrow::Cow;
use std::hash::Hash;
use std::ops::Range;

/// Reference to a [`DiffableStr`].
///
/// This type exists because while the library only really provides ways to
/// work with `&str` and `&[u8]` there are types that deref into those string
/// slices such as `String` and `Vec<u8>`.
///
/// This trait is used in the library whenever it's nice to be able to pass
/// strings of different types in.
///
/// Requires the `text` feature.
pub trait DiffableStrRef {
    /// The type of the resolved [`DiffableStr`].
    type Output: DiffableStr + ?Sized;

    /// Resolves the reference.
    fn as_diffable_str(&self) -> &Self::Output;
}

impl<T: DiffableStr + ?Sized> DiffableStrRef for T {
    type Output = T;

    fn as_diffable_str(&self) -> &T {
        self
    }
}

impl DiffableStrRef for String {
    type Output = str;

    fn as_diffable_str(&self) -> &str {
        self.as_str()
    }
}

impl<T: DiffableStr + ?Sized> DiffableStrRef for Cow<'_, T> {
    type Output = T;

    fn as_diffable_str(&self) -> &T {
        self
    }
}

/// All supported diffable strings.
///
/// The text module can work with different types of strings depending
/// on how the crate is compiled.  Out of the box `&str` is always supported
/// but with the `bytes` feature one can also work with `[u8]` slices for
/// as long as they are ASCII compatible.
///
/// Requires the `text` feature.
pub trait DiffableStr: Hash + PartialEq + PartialOrd + Ord + Eq + ToOwned {
    /// Splits the value into newlines with newlines attached.
    fn tokenize_lines(&self) -> Vec<&Self>;

    /// Splits the value into newlines with newlines separated.
    fn tokenize_lines_and_newlines(&self) -> Vec<&Self>;

    /// Tokenizes into words.
    fn tokenize_words(&self) -> Vec<&Self>;

    /// Tokenizes the input into characters.
    fn tokenize_chars(&self) -> Vec<&Self>;

    /// Tokenizes into unicode words.
    #[cfg(feature = "unicode")]
    fn tokenize_unicode_words(&self) -> Vec<&Self>;

    /// Tokenizes into unicode graphemes.
    #[cfg(feature = "unicode")]
    fn tokenize_graphemes(&self) -> Vec<&Self>;

    /// Decodes the string (potentially) lossy.
    fn as_str(&self) -> Option<&str>;

    /// Decodes the string (potentially) lossy.
    fn to_string_lossy(&self) -> Cow<'_, str>;

    /// Checks if the string ends in a newline.
    fn ends_with_newline(&self) -> bool;

    /// The length of the string.
    fn len(&self) -> usize;

    /// Slices the string.
    fn slice(&self, rng: Range<usize>) -> &Self;

    /// Returns the string as slice of raw bytes.
    fn as_bytes(&self) -> &[u8];

    /// Checks if the string is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl DiffableStr for str {
    fn tokenize_lines(&self) -> Vec<&Self> {
        let mut iter = self.char_indices().peekable();
        let mut last_pos = 0;
        let mut lines = vec![];

        while let Some((idx, c)) = iter.next() {
            if c == '\r' {
                if iter.peek().map_or(false, |x| x.1 == '\n') {
                    lines.push(&self[last_pos..=idx + 1]);
                    iter.next();
                    last_pos = idx + 2;
                } else {
                    lines.push(&self[last_pos..=idx]);
                    last_pos = idx + 1;
                }
            } else if c == '\n' {
                lines.push(&self[last_pos..=idx]);
                last_pos = idx + 1;
            }
        }

        if last_pos < self.len() {
            lines.push(&self[last_pos..]);
        }

        lines
    }

    fn tokenize_lines_and_newlines(&self) -> Vec<&Self> {
        let mut rv = vec![];
        let mut iter = self.char_indices().peekable();

        while let Some((idx, c)) = iter.next() {
            let is_newline = c == '\r' || c == '\n';
            let start = idx;
            let mut end = idx + c.len_utf8();
            while let Some(&(_, next_char)) = iter.peek() {
                if (next_char == '\r' || next_char == '\n') != is_newline {
                    break;
                }
                iter.next();
                end += next_char.len_utf8();
            }
            rv.push(&self[start..end]);
        }

        rv
    }

    fn tokenize_words(&self) -> Vec<&Self> {
        let mut iter = self.char_indices().peekable();
        let mut rv = vec![];

        while let Some((idx, c)) = iter.next() {
            let is_whitespace = c.is_whitespace();
            let start = idx;
            let mut end = idx + c.len_utf8();
            while let Some(&(_, next_char)) = iter.peek() {
                if next_char.is_whitespace() != is_whitespace {
                    break;
                }
                iter.next();
                end += next_char.len_utf8();
            }
            rv.push(&self[start..end]);
        }

        rv
    }

    fn tokenize_chars(&self) -> Vec<&Self> {
        self.char_indices()
            .map(move |(i, c)| &self[i..i + c.len_utf8()])
            .collect()
    }

    #[cfg(feature = "unicode")]
    fn tokenize_unicode_words(&self) -> Vec<&Self> {
        unicode_segmentation::UnicodeSegmentation::split_word_bounds(self).collect()
    }

    #[cfg(feature = "unicode")]
    fn tokenize_graphemes(&self) -> Vec<&Self> {
        unicode_segmentation::UnicodeSegmentation::graphemes(self, true).collect()
    }

    fn as_str(&self) -> Option<&str> {
        Some(self)
    }

    fn to_string_lossy(&self) -> Cow<'_, str> {
        Cow::Borrowed(self)
    }

    fn ends_with_newline(&self) -> bool {
        self.ends_with(&['\r', '\n'][..])
    }

    fn len(&self) -> usize {
        str::len(self)
    }

    fn slice(&self, rng: Range<usize>) -> &Self {
        &self[rng]
    }

    fn as_bytes(&self) -> &[u8] {
        str::as_bytes(self)
    }
}

#[cfg(feature = "bytes")]
mod bytes_support {
    use super::*;

    use bstr::ByteSlice;

    impl DiffableStrRef for Vec<u8> {
        type Output = [u8];

        fn as_diffable_str(&self) -> &[u8] {
            self.as_slice()
        }
    }

    /// Allows viewing ASCII compatible byte slices as strings.
    ///
    /// Requires the `bytes` feature.
    impl DiffableStr for [u8] {
        fn tokenize_lines(&self) -> Vec<&Self> {
            let mut iter = self.char_indices().peekable();
            let mut last_pos = 0;
            let mut lines = vec![];

            while let Some((_, end, c)) = iter.next() {
                if c == '\r' {
                    if iter.peek().map_or(false, |x| x.2 == '\n') {
                        lines.push(&self[last_pos..end + 1]);
                        iter.next();
                        last_pos = end + 1;
                    } else {
                        lines.push(&self[last_pos..end]);
                        last_pos = end;
                    }
                } else if c == '\n' {
                    lines.push(&self[last_pos..end]);
                    last_pos = end;
                }
            }

            if last_pos < self.len() {
                lines.push(&self[last_pos..]);
            }

            lines
        }

        fn tokenize_lines_and_newlines(&self) -> Vec<&Self> {
            let mut rv = vec![];
            let mut iter = self.char_indices().peekable();

            while let Some((start, mut end, c)) = iter.next() {
                let is_newline = c == '\r' || c == '\n';
                while let Some(&(_, new_end, next_char)) = iter.peek() {
                    if (next_char == '\r' || next_char == '\n') != is_newline {
                        break;
                    }
                    iter.next();
                    end = new_end;
                }
                rv.push(&self[start..end]);
            }

            rv
        }

        fn tokenize_words(&self) -> Vec<&Self> {
            let mut iter = self.char_indices().peekable();
            let mut rv = vec![];

            while let Some((start, mut end, c)) = iter.next() {
                let is_whitespace = c.is_whitespace();
                while let Some(&(_, new_end, next_char)) = iter.peek() {
                    if next_char.is_whitespace() != is_whitespace {
                        break;
                    }
                    iter.next();
                    end = new_end;
                }
                rv.push(&self[start..end]);
            }

            rv
        }

        #[cfg(feature = "unicode")]
        fn tokenize_unicode_words(&self) -> Vec<&Self> {
            self.words_with_breaks().map(|x| x.as_bytes()).collect()
        }

        #[cfg(feature = "unicode")]
        fn tokenize_graphemes(&self) -> Vec<&Self> {
            self.graphemes().map(|x| x.as_bytes()).collect()
        }

        fn tokenize_chars(&self) -> Vec<&Self> {
            self.char_indices()
                .map(move |(start, end, _)| &self[start..end])
                .collect()
        }

        fn as_str(&self) -> Option<&str> {
            std::str::from_utf8(self).ok()
        }

        fn to_string_lossy(&self) -> Cow<'_, str> {
            String::from_utf8_lossy(self)
        }

        fn ends_with_newline(&self) -> bool {
            matches!(self.last_byte(), Some(b'\r') | Some(b'\n'))
        }

        fn len(&self) -> usize {
            <[u8]>::len(self)
        }

        fn slice(&self, rng: Range<usize>) -> &Self {
            &self[rng]
        }

        fn as_bytes(&self) -> &[u8] {
            self
        }
    }
}

#[test]
fn test_split_lines() {
    assert_eq!(
        DiffableStr::tokenize_lines("first\nsecond\rthird\r\nfourth\nlast"),
        vec!["first\n", "second\r", "third\r\n", "fourth\n", "last"]
    );
    assert_eq!(DiffableStr::tokenize_lines("\n\n"), vec!["\n", "\n"]);
    assert_eq!(DiffableStr::tokenize_lines("\n"), vec!["\n"]);
    assert!(DiffableStr::tokenize_lines("").is_empty());
}

#[test]
fn test_split_words() {
    assert_eq!(
        DiffableStr::tokenize_words("foo    bar baz\n\n  aha"),
        ["foo", "    ", "bar", " ", "baz", "\n\n  ", "aha"]
    );
}

#[test]
fn test_split_chars() {
    assert_eq!(
        DiffableStr::tokenize_chars("abcfö❄️"),
        vec!["a", "b", "c", "f", "ö", "❄", "\u{fe0f}"]
    );
}

#[test]
#[cfg(feature = "unicode")]
fn test_split_graphemes() {
    assert_eq!(
        DiffableStr::tokenize_graphemes("abcfö❄️"),
        vec!["a", "b", "c", "f", "ö", "❄️"]
    );
}

#[test]
#[cfg(feature = "bytes")]
fn test_split_lines_bytes() {
    assert_eq!(
        DiffableStr::tokenize_lines("first\nsecond\rthird\r\nfourth\nlast".as_bytes()),
        vec![
            "first\n".as_bytes(),
            "second\r".as_bytes(),
            "third\r\n".as_bytes(),
            "fourth\n".as_bytes(),
            "last".as_bytes()
        ]
    );
    assert_eq!(
        DiffableStr::tokenize_lines("\n\n".as_bytes()),
        vec!["\n".as_bytes(), "\n".as_bytes()]
    );
    assert_eq!(
        DiffableStr::tokenize_lines("\n".as_bytes()),
        vec!["\n".as_bytes()]
    );
    assert!(DiffableStr::tokenize_lines("".as_bytes()).is_empty());
}

#[test]
#[cfg(feature = "bytes")]
fn test_split_words_bytes() {
    assert_eq!(
        DiffableStr::tokenize_words("foo    bar baz\n\n  aha".as_bytes()),
        [
            &b"foo"[..],
            &b"    "[..],
            &b"bar"[..],
            &b" "[..],
            &b"baz"[..],
            &b"\n\n  "[..],
            &b"aha"[..]
        ]
    );
}

#[test]
#[cfg(feature = "bytes")]
fn test_split_chars_bytes() {
    assert_eq!(
        DiffableStr::tokenize_chars("abcfö❄️".as_bytes()),
        vec![
            &b"a"[..],
            &b"b"[..],
            &b"c"[..],
            &b"f"[..],
            "ö".as_bytes(),
            "❄".as_bytes(),
            "\u{fe0f}".as_bytes()
        ]
    );
}

#[test]
#[cfg(all(feature = "bytes", feature = "unicode"))]
fn test_split_graphemes_bytes() {
    assert_eq!(
        DiffableStr::tokenize_graphemes("abcfö❄️".as_bytes()),
        vec![
            &b"a"[..],
            &b"b"[..],
            &b"c"[..],
            &b"f"[..],
            "ö".as_bytes(),
            "❄️".as_bytes()
        ]
    );
}

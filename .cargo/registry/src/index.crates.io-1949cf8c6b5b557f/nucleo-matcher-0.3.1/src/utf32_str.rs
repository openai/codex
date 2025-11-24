use std::borrow::Cow;
use std::ops::{Bound, RangeBounds};
use std::{fmt, slice};

use crate::chars;

/// A UTF32 encoded (char array) string that is used as an input to (fuzzy) matching.
///
/// Usually rusts' utf8 encoded strings are great. However during fuzzy matching
/// operates on codepoints (it should operate on graphemes but that's too much
/// hassle to deal with). We want to quickly iterate these codepoints between
/// (up to 5 times) during matching.
///
/// Doing codepoint segmentation on the fly not only blows trough the cache
/// (lookuptables and Icache) but also has nontrivial runtime compared to the
/// matching itself. Furthermore there are a lot of exta optimizations available
/// for ascii only text (but checking during each match has too much overhead).
///
/// Ofcourse this comes at exta memory cost as we usually still need the ut8
/// encoded variant for rendering. In the (dominant) case of ascii-only text
/// we don't require a copy. Furthermore fuzzy matching usually is applied while
/// the user is typing on the fly so the same item is potentially matched many
/// times (making the the upfront cost more worth it). That means that its
/// basically always worth it to presegment the string.
///
/// For usecases that only match (a lot of) strings once its possible to keep
/// char buffer around that is filled with the presegmented chars
///
/// Another advantage of this approach is that the matcher will naturally
/// produce char indices (instead of utf8 offsets) anyway. With a
/// codepoint basic representation like this the indices can be used
/// directly
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub enum Utf32Str<'a> {
    /// A string represented as ASCII encoded bytes.
    /// Correctness invariant: must only contain valid ASCII (<=127)
    Ascii(&'a [u8]),
    /// A string represented as an array of unicode codepoints (basically UTF-32).
    Unicode(&'a [char]),
}

impl<'a> Utf32Str<'a> {
    /// Convenience method to construct a `Utf32Str` from a normal utf8 str
    pub fn new(str: &'a str, buf: &'a mut Vec<char>) -> Self {
        if str.is_ascii() {
            Utf32Str::Ascii(str.as_bytes())
        } else {
            buf.clear();
            buf.extend(crate::chars::graphemes(str));
            if buf.iter().all(|c| c.is_ascii()) {
                return Utf32Str::Ascii(str.as_bytes());
            }
            Utf32Str::Unicode(&*buf)
        }
    }

    /// Returns the number of characters in this string.
    #[inline]
    pub fn len(self) -> usize {
        match self {
            Utf32Str::Unicode(codepoints) => codepoints.len(),
            Utf32Str::Ascii(ascii_bytes) => ascii_bytes.len(),
        }
    }

    /// Returns whether this string is empty.
    #[inline]
    pub fn is_empty(self) -> bool {
        match self {
            Utf32Str::Unicode(codepoints) => codepoints.is_empty(),
            Utf32Str::Ascii(ascii_bytes) => ascii_bytes.is_empty(),
        }
    }

    /// Creates a slice with a string that contains the characters in
    /// the specified **character range**.
    #[inline]
    pub fn slice(self, range: impl RangeBounds<usize>) -> Utf32Str<'a> {
        let start = match range.start_bound() {
            Bound::Included(&start) => start,
            Bound::Excluded(&start) => start + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&end) => end + 1,
            Bound::Excluded(&end) => end,
            Bound::Unbounded => self.len(),
        };
        match self {
            Utf32Str::Ascii(bytes) => Utf32Str::Ascii(&bytes[start..end]),
            Utf32Str::Unicode(codepoints) => Utf32Str::Unicode(&codepoints[start..end]),
        }
    }

    /// Returns the number of leading whitespaces in this string
    #[inline]
    pub(crate) fn leading_white_space(self) -> usize {
        match self {
            Utf32Str::Ascii(bytes) => bytes
                .iter()
                .position(|b| !b.is_ascii_whitespace())
                .unwrap_or(0),
            Utf32Str::Unicode(codepoints) => codepoints
                .iter()
                .position(|c| !c.is_whitespace())
                .unwrap_or(0),
        }
    }

    /// Returns the number of leading whitespaces in this string
    #[inline]
    pub(crate) fn trailing_white_space(self) -> usize {
        match self {
            Utf32Str::Ascii(bytes) => bytes
                .iter()
                .rev()
                .position(|b| !b.is_ascii_whitespace())
                .unwrap_or(0),
            Utf32Str::Unicode(codepoints) => codepoints
                .iter()
                .rev()
                .position(|c| !c.is_whitespace())
                .unwrap_or(0),
        }
    }

    /// Same as `slice` but accepts a u32 range for convenience since
    /// those are the indices returned by the matcher.
    #[inline]
    pub fn slice_u32(self, range: impl RangeBounds<u32>) -> Utf32Str<'a> {
        let start = match range.start_bound() {
            Bound::Included(&start) => start as usize,
            Bound::Excluded(&start) => start as usize + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&end) => end as usize + 1,
            Bound::Excluded(&end) => end as usize,
            Bound::Unbounded => self.len(),
        };
        match self {
            Utf32Str::Ascii(bytes) => Utf32Str::Ascii(&bytes[start..end]),
            Utf32Str::Unicode(codepoints) => Utf32Str::Unicode(&codepoints[start..end]),
        }
    }

    /// Returns whether this string only contains ascii text.
    pub fn is_ascii(self) -> bool {
        matches!(self, Utf32Str::Ascii(_))
    }

    /// Returns the `n`th character in this string.
    pub fn get(self, n: u32) -> char {
        match self {
            Utf32Str::Ascii(bytes) => bytes[n as usize] as char,
            Utf32Str::Unicode(codepoints) => codepoints[n as usize],
        }
    }
    pub(crate) fn last(self) -> char {
        match self {
            Utf32Str::Ascii(bytes) => bytes[bytes.len() - 1] as char,
            Utf32Str::Unicode(codepoints) => codepoints[codepoints.len() - 1],
        }
    }

    pub(crate) fn first(self) -> char {
        match self {
            Utf32Str::Ascii(bytes) => bytes[0] as char,
            Utf32Str::Unicode(codepoints) => codepoints[0],
        }
    }

    /// Returns an iterator over the characters in this string
    pub fn chars(self) -> Chars<'a> {
        match self {
            Utf32Str::Ascii(bytes) => Chars::Ascii(bytes.iter()),
            Utf32Str::Unicode(codepoints) => Chars::Unicode(codepoints.iter()),
        }
    }
}

impl fmt::Debug for Utf32Str<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"")?;
        for c in self.chars() {
            for c in c.escape_debug() {
                write!(f, "{c}")?
            }
        }
        write!(f, "\"")
    }
}

impl fmt::Display for Utf32Str<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in self.chars() {
            write!(f, "{c}")?
        }
        Ok(())
    }
}

pub enum Chars<'a> {
    Ascii(slice::Iter<'a, u8>),
    Unicode(slice::Iter<'a, char>),
}
impl<'a> Iterator for Chars<'a> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Chars::Ascii(iter) => iter.next().map(|&c| c as char),
            Chars::Unicode(iter) => iter.next().copied(),
        }
    }
}

impl DoubleEndedIterator for Chars<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Chars::Ascii(iter) => iter.next_back().map(|&c| c as char),
            Chars::Unicode(iter) => iter.next_back().copied(),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
/// An owned version of [`Utf32Str`].
pub enum Utf32String {
    /// A string represented as ASCII encoded bytes.
    /// Correctness invariant: must only contain valid ASCII (<=127)
    Ascii(Box<str>),
    /// A string represented as an array of unicode codepoints (basically UTF-32).
    Unicode(Box<[char]>),
}

impl Default for Utf32String {
    fn default() -> Self {
        Self::Ascii(String::new().into_boxed_str())
    }
}

impl Utf32String {
    /// Returns the number of characters in this string.
    #[inline]
    pub fn len(&self) -> usize {
        match self {
            Utf32String::Unicode(codepoints) => codepoints.len(),
            Utf32String::Ascii(ascii_bytes) => ascii_bytes.len(),
        }
    }

    /// Returns whether this string is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        match self {
            Utf32String::Unicode(codepoints) => codepoints.is_empty(),
            Utf32String::Ascii(ascii_bytes) => ascii_bytes.is_empty(),
        }
    }

    /// Creates a slice with a string that contains the characters in
    /// the specified **character range**.
    #[inline]
    pub fn slice(&self, range: impl RangeBounds<usize>) -> Utf32Str {
        let start = match range.start_bound() {
            Bound::Included(&start) => start,
            Bound::Excluded(&start) => start + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&end) => end + 1,
            Bound::Excluded(&end) => end,
            Bound::Unbounded => self.len(),
        };
        match self {
            Utf32String::Ascii(bytes) => Utf32Str::Ascii(&bytes.as_bytes()[start..end]),
            Utf32String::Unicode(codepoints) => Utf32Str::Unicode(&codepoints[start..end]),
        }
    }

    /// Same as `slice` but accepts a u32 range for convenience since
    /// those are the indices returned by the matcher.
    #[inline]
    pub fn slice_u32(&self, range: impl RangeBounds<u32>) -> Utf32Str {
        let start = match range.start_bound() {
            Bound::Included(&start) => start,
            Bound::Excluded(&start) => start + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&end) => end + 1,
            Bound::Excluded(&end) => end,
            Bound::Unbounded => self.len() as u32,
        };
        match self {
            Utf32String::Ascii(bytes) => {
                Utf32Str::Ascii(&bytes.as_bytes()[start as usize..end as usize])
            }
            Utf32String::Unicode(codepoints) => {
                Utf32Str::Unicode(&codepoints[start as usize..end as usize])
            }
        }
    }
}

impl From<&str> for Utf32String {
    #[inline]
    fn from(value: &str) -> Self {
        if value.is_ascii() {
            Self::Ascii(value.to_owned().into_boxed_str())
        } else {
            Self::Unicode(chars::graphemes(value).collect())
        }
    }
}

impl From<Box<str>> for Utf32String {
    fn from(value: Box<str>) -> Self {
        if value.is_ascii() {
            Self::Ascii(value)
        } else {
            Self::Unicode(chars::graphemes(&value).collect())
        }
    }
}

impl From<String> for Utf32String {
    #[inline]
    fn from(value: String) -> Self {
        value.into_boxed_str().into()
    }
}

impl<'a> From<Cow<'a, str>> for Utf32String {
    #[inline]
    fn from(value: Cow<'a, str>) -> Self {
        match value {
            Cow::Borrowed(value) => value.into(),
            Cow::Owned(value) => value.into(),
        }
    }
}

impl fmt::Debug for Utf32String {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.slice(..))
    }
}

impl fmt::Display for Utf32String {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.slice(..))
    }
}

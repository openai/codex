// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

pub mod errors;
mod langid;
mod locale;

pub use errors::ParseError;
pub use langid::*;

pub use locale::*;

// Safety-usable invariant: returns a prefix of `slice`
const fn skip_before_separator(slice: &[u8]) -> &[u8] {
    let mut end = 0;
    // Invariant: end ≤ slice.len() since len is a nonnegative integer and end is 0

    #[expect(clippy::indexing_slicing)] // very protected, should optimize out
    while end < slice.len() && !matches!(slice[end], b'-') {
        // Invariant at beginning of loop: end < slice.len()
        // Advance until we reach end of slice or a separator.
        end += 1;
        // Invariant at end of loop: end ≤ slice.len()
    }

    // Notice: this slice may be empty for cases like `"en-"` or `"en--US"`
    // SAFETY: end ≤ slice.len() by while loop
    // Safety-usable invariant upheld: returned a prefix of the slice
    unsafe { slice.split_at_unchecked(end).0 }
}

// `SubtagIterator` is a helper iterator for [`LanguageIdentifier`] and [`Locale`] parsing.
//
// It is quite extraordinary due to focus on performance and Rust limitations for `const`
// functions.
//
// The iterator is eager and fallible allowing it to reject invalid slices such as `"-"`, `"-en"`,
// `"en-"` etc.
//
// The iterator provides methods available for static users - `next_manual` and `peek_manual`,
// as well as typical `Peekable` iterator APIs - `next` and `peek`.
//
// All methods return an `Option` of a `Result`.
#[derive(Copy, Clone, Debug)]
pub struct SubtagIterator<'a> {
    remaining: &'a [u8],
    // Safety invariant: current is a prefix of remaining
    current: Option<&'a [u8]>,
}

impl<'a> SubtagIterator<'a> {
    pub const fn new(rest: &'a [u8]) -> Self {
        Self {
            remaining: rest,
            // Safety invariant upheld: skip_before_separator() returns a prefix of `rest`
            current: Some(skip_before_separator(rest)),
        }
    }

    pub const fn next_const(mut self) -> (Self, Option<&'a [u8]>) {
        let Some(result) = self.current else {
            return (self, None);
        };

        self.current = if result.len() < self.remaining.len() {
            // If there is more after `result`, by construction `current` starts with a separator
            // SAFETY: `self.remaining` is strictly longer than `result` due to `result` being a prefix (from the safety invariant)
            self.remaining = unsafe { self.remaining.split_at_unchecked(result.len() + 1).1 };
            // Safety invariant upheld: skip_before_separator() returns a prefix of `rest`, and we don't
            // mutate self.remaining after this
            Some(skip_before_separator(self.remaining))
        } else {
            None
        };
        (self, Some(result))
    }

    pub const fn peek(&self) -> Option<&'a [u8]> {
        self.current
    }
}

impl<'a> Iterator for SubtagIterator<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        let (s, res) = self.next_const();
        *self = s;
        res
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn slice_to_str(input: &[u8]) -> &str {
        std::str::from_utf8(input).unwrap()
    }

    #[test]
    fn subtag_iterator_peek_test() {
        let slice = "de-at-u-ca-foobar";
        let mut si = SubtagIterator::new(slice.as_bytes());

        assert_eq!(si.peek().map(slice_to_str), Some("de"));
        assert_eq!(si.peek().map(slice_to_str), Some("de"));
        assert_eq!(si.next().map(slice_to_str), Some("de"));

        assert_eq!(si.peek().map(slice_to_str), Some("at"));
        assert_eq!(si.peek().map(slice_to_str), Some("at"));
        assert_eq!(si.next().map(slice_to_str), Some("at"));
    }

    #[test]
    fn subtag_iterator_test() {
        let slice = "";
        let mut si = SubtagIterator::new(slice.as_bytes());
        assert_eq!(si.next().map(slice_to_str), Some(""));

        let slice = "-";
        let mut si = SubtagIterator::new(slice.as_bytes());
        assert_eq!(si.next().map(slice_to_str), Some(""));

        let slice = "-en";
        let mut si = SubtagIterator::new(slice.as_bytes());
        assert_eq!(si.next().map(slice_to_str), Some(""));
        assert_eq!(si.next().map(slice_to_str), Some("en"));
        assert_eq!(si.next(), None);

        let slice = "en";
        let si = SubtagIterator::new(slice.as_bytes());
        assert_eq!(si.map(slice_to_str).collect::<Vec<_>>(), vec!["en",]);

        let slice = "en-";
        let si = SubtagIterator::new(slice.as_bytes());
        assert_eq!(si.map(slice_to_str).collect::<Vec<_>>(), vec!["en", "",]);

        let slice = "--";
        let mut si = SubtagIterator::new(slice.as_bytes());
        assert_eq!(si.next().map(slice_to_str), Some(""));
        assert_eq!(si.next().map(slice_to_str), Some(""));
        assert_eq!(si.next().map(slice_to_str), Some(""));
        assert_eq!(si.next(), None);

        let slice = "-en-";
        let mut si = SubtagIterator::new(slice.as_bytes());
        assert_eq!(si.next().map(slice_to_str), Some(""));
        assert_eq!(si.next().map(slice_to_str), Some("en"));
        assert_eq!(si.next().map(slice_to_str), Some(""));
        assert_eq!(si.next(), None);

        let slice = "de-at-u-ca-foobar";
        let si = SubtagIterator::new(slice.as_bytes());
        assert_eq!(
            si.map(slice_to_str).collect::<Vec<_>>(),
            vec!["de", "at", "u", "ca", "foobar",]
        );
    }

    #[test]
    fn skip_before_separator_test() {
        let current = skip_before_separator(b"");
        assert_eq!(current, b"");

        let current = skip_before_separator(b"en");
        assert_eq!(current, b"en");

        let current = skip_before_separator(b"en-");
        assert_eq!(current, b"en");

        let current = skip_before_separator(b"en--US");
        assert_eq!(current, b"en");

        let current = skip_before_separator(b"-US");
        assert_eq!(current, b"");

        let current = skip_before_separator(b"US");
        assert_eq!(current, b"US");

        let current = skip_before_separator(b"-");
        assert_eq!(current, b"");
    }
}

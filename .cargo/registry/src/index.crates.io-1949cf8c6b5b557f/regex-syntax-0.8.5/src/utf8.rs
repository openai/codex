/*!
Converts ranges of Unicode scalar values to equivalent ranges of UTF-8 bytes.

This is sub-module is useful for constructing byte based automatons that need
to embed UTF-8 decoding. The most common use of this module is in conjunction
with the [`hir::ClassUnicodeRange`](crate::hir::ClassUnicodeRange) type.

See the documentation on the `Utf8Sequences` iterator for more details and
an example.

# Wait, what is this?

This is simplest to explain with an example. Let's say you wanted to test
whether a particular byte sequence was a Cyrillic character. One possible
scalar value range is `[0400-04FF]`. The set of allowed bytes for this
range can be expressed as a sequence of byte ranges:

```text
[D0-D3][80-BF]
```

This is simple enough: simply encode the boundaries, `0400` encodes to
`D0 80` and `04FF` encodes to `D3 BF`, and create ranges from each
corresponding pair of bytes: `D0` to `D3` and `80` to `BF`.

However, what if you wanted to add the Cyrillic Supplementary characters to
your range? Your range might then become `[0400-052F]`. The same procedure
as above doesn't quite work because `052F` encodes to `D4 AF`. The byte ranges
you'd get from the previous transformation would be `[D0-D4][80-AF]`. However,
this isn't quite correct because this range doesn't capture many characters,
for example, `04FF` (because its last byte, `BF` isn't in the range `80-AF`).

Instead, you need multiple sequences of byte ranges:

```text
[D0-D3][80-BF]  # matches codepoints 0400-04FF
[D4][80-AF]     # matches codepoints 0500-052F
```

This gets even more complicated if you want bigger ranges, particularly if
they naively contain surrogate codepoints. For example, the sequence of byte
ranges for the basic multilingual plane (`[0000-FFFF]`) look like this:

```text
[0-7F]
[C2-DF][80-BF]
[E0][A0-BF][80-BF]
[E1-EC][80-BF][80-BF]
[ED][80-9F][80-BF]
[EE-EF][80-BF][80-BF]
```

Note that the byte ranges above will *not* match any erroneous encoding of
UTF-8, including encodings of surrogate codepoints.

And, of course, for all of Unicode (`[000000-10FFFF]`):

```text
[0-7F]
[C2-DF][80-BF]
[E0][A0-BF][80-BF]
[E1-EC][80-BF][80-BF]
[ED][80-9F][80-BF]
[EE-EF][80-BF][80-BF]
[F0][90-BF][80-BF][80-BF]
[F1-F3][80-BF][80-BF][80-BF]
[F4][80-8F][80-BF][80-BF]
```

This module automates the process of creating these byte ranges from ranges of
Unicode scalar values.

# Lineage

I got the idea and general implementation strategy from Russ Cox in his
[article on regexps](https://web.archive.org/web/20160404141123/https://swtch.com/~rsc/regexp/regexp3.html) and RE2.
Russ Cox got it from Ken Thompson's `grep` (no source, folk lore?).
I also got the idea from
[Lucene](https://github.com/apache/lucene-solr/blob/ae93f4e7ac6a3908046391de35d4f50a0d3c59ca/lucene/core/src/java/org/apache/lucene/util/automaton/UTF32ToUTF8.java),
which uses it for executing automata on their term index.
*/

use core::{char, fmt, iter::FusedIterator, slice};

use alloc::{vec, vec::Vec};

const MAX_UTF8_BYTES: usize = 4;

/// Utf8Sequence represents a sequence of byte ranges.
///
/// To match a Utf8Sequence, a candidate byte sequence must match each
/// successive range.
///
/// For example, if there are two ranges, `[C2-DF][80-BF]`, then the byte
/// sequence `\xDD\x61` would not match because `0x61 < 0x80`.
#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub enum Utf8Sequence {
    /// One byte range.
    One(Utf8Range),
    /// Two successive byte ranges.
    Two([Utf8Range; 2]),
    /// Three successive byte ranges.
    Three([Utf8Range; 3]),
    /// Four successive byte ranges.
    Four([Utf8Range; 4]),
}

impl Utf8Sequence {
    /// Creates a new UTF-8 sequence from the encoded bytes of a scalar value
    /// range.
    ///
    /// This assumes that `start` and `end` have the same length.
    fn from_encoded_range(start: &[u8], end: &[u8]) -> Self {
        assert_eq!(start.len(), end.len());
        match start.len() {
            2 => Utf8Sequence::Two([
                Utf8Range::new(start[0], end[0]),
                Utf8Range::new(start[1], end[1]),
            ]),
            3 => Utf8Sequence::Three([
                Utf8Range::new(start[0], end[0]),
                Utf8Range::new(start[1], end[1]),
                Utf8Range::new(start[2], end[2]),
            ]),
            4 => Utf8Sequence::Four([
                Utf8Range::new(start[0], end[0]),
                Utf8Range::new(start[1], end[1]),
                Utf8Range::new(start[2], end[2]),
                Utf8Range::new(start[3], end[3]),
            ]),
            n => unreachable!("invalid encoded length: {}", n),
        }
    }

    /// Returns the underlying sequence of byte ranges as a slice.
    pub fn as_slice(&self) -> &[Utf8Range] {
        use self::Utf8Sequence::*;
        match *self {
            One(ref r) => slice::from_ref(r),
            Two(ref r) => &r[..],
            Three(ref r) => &r[..],
            Four(ref r) => &r[..],
        }
    }

    /// Returns the number of byte ranges in this sequence.
    ///
    /// The length is guaranteed to be in the closed interval `[1, 4]`.
    pub fn len(&self) -> usize {
        self.as_slice().len()
    }

    /// Reverses the ranges in this sequence.
    ///
    /// For example, if this corresponds to the following sequence:
    ///
    /// ```text
    /// [D0-D3][80-BF]
    /// ```
    ///
    /// Then after reversal, it will be
    ///
    /// ```text
    /// [80-BF][D0-D3]
    /// ```
    ///
    /// This is useful when one is constructing a UTF-8 automaton to match
    /// character classes in reverse.
    pub fn reverse(&mut self) {
        match *self {
            Utf8Sequence::One(_) => {}
            Utf8Sequence::Two(ref mut x) => x.reverse(),
            Utf8Sequence::Three(ref mut x) => x.reverse(),
            Utf8Sequence::Four(ref mut x) => x.reverse(),
        }
    }

    /// Returns true if and only if a prefix of `bytes` matches this sequence
    /// of byte ranges.
    pub fn matches(&self, bytes: &[u8]) -> bool {
        if bytes.len() < self.len() {
            return false;
        }
        for (&b, r) in bytes.iter().zip(self) {
            if !r.matches(b) {
                return false;
            }
        }
        true
    }
}

impl<'a> IntoIterator for &'a Utf8Sequence {
    type IntoIter = slice::Iter<'a, Utf8Range>;
    type Item = &'a Utf8Range;

    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

impl fmt::Debug for Utf8Sequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use self::Utf8Sequence::*;
        match *self {
            One(ref r) => write!(f, "{:?}", r),
            Two(ref r) => write!(f, "{:?}{:?}", r[0], r[1]),
            Three(ref r) => write!(f, "{:?}{:?}{:?}", r[0], r[1], r[2]),
            Four(ref r) => {
                write!(f, "{:?}{:?}{:?}{:?}", r[0], r[1], r[2], r[3])
            }
        }
    }
}

/// A single inclusive range of UTF-8 bytes.
#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub struct Utf8Range {
    /// Start of byte range (inclusive).
    pub start: u8,
    /// End of byte range (inclusive).
    pub end: u8,
}

impl Utf8Range {
    fn new(start: u8, end: u8) -> Self {
        Utf8Range { start, end }
    }

    /// Returns true if and only if the given byte is in this range.
    pub fn matches(&self, b: u8) -> bool {
        self.start <= b && b <= self.end
    }
}

impl fmt::Debug for Utf8Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.start == self.end {
            write!(f, "[{:X}]", self.start)
        } else {
            write!(f, "[{:X}-{:X}]", self.start, self.end)
        }
    }
}

/// An iterator over ranges of matching UTF-8 byte sequences.
///
/// The iteration represents an alternation of comprehensive byte sequences
/// that match precisely the set of UTF-8 encoded scalar values.
///
/// A byte sequence corresponds to one of the scalar values in the range given
/// if and only if it completely matches exactly one of the sequences of byte
/// ranges produced by this iterator.
///
/// Each sequence of byte ranges matches a unique set of bytes. That is, no two
/// sequences will match the same bytes.
///
/// # Example
///
/// This shows how to match an arbitrary byte sequence against a range of
/// scalar values.
///
/// ```rust
/// use regex_syntax::utf8::{Utf8Sequences, Utf8Sequence};
///
/// fn matches(seqs: &[Utf8Sequence], bytes: &[u8]) -> bool {
///     for range in seqs {
///         if range.matches(bytes) {
///             return true;
///         }
///     }
///     false
/// }
///
/// // Test the basic multilingual plane.
/// let seqs: Vec<_> = Utf8Sequences::new('\u{0}', '\u{FFFF}').collect();
///
/// // UTF-8 encoding of 'a'.
/// assert!(matches(&seqs, &[0x61]));
/// // UTF-8 encoding of '☃' (`\u{2603}`).
/// assert!(matches(&seqs, &[0xE2, 0x98, 0x83]));
/// // UTF-8 encoding of `\u{10348}` (outside the BMP).
/// assert!(!matches(&seqs, &[0xF0, 0x90, 0x8D, 0x88]));
/// // Tries to match against a UTF-8 encoding of a surrogate codepoint,
/// // which is invalid UTF-8, and therefore fails, despite the fact that
/// // the corresponding codepoint (0xD800) falls in the range given.
/// assert!(!matches(&seqs, &[0xED, 0xA0, 0x80]));
/// // And fails against plain old invalid UTF-8.
/// assert!(!matches(&seqs, &[0xFF, 0xFF]));
/// ```
///
/// If this example seems circuitous, that's because it is! It's meant to be
/// illustrative. In practice, you could just try to decode your byte sequence
/// and compare it with the scalar value range directly. However, this is not
/// always possible (for example, in a byte based automaton).
#[derive(Debug)]
pub struct Utf8Sequences {
    range_stack: Vec<ScalarRange>,
}

impl Utf8Sequences {
    /// Create a new iterator over UTF-8 byte ranges for the scalar value range
    /// given.
    pub fn new(start: char, end: char) -> Self {
        let range =
            ScalarRange { start: u32::from(start), end: u32::from(end) };
        Utf8Sequences { range_stack: vec![range] }
    }

    /// reset resets the scalar value range.
    /// Any existing state is cleared, but resources may be reused.
    ///
    /// N.B. Benchmarks say that this method is dubious.
    #[doc(hidden)]
    pub fn reset(&mut self, start: char, end: char) {
        self.range_stack.clear();
        self.push(u32::from(start), u32::from(end));
    }

    fn push(&mut self, start: u32, end: u32) {
        self.range_stack.push(ScalarRange { start, end });
    }
}

struct ScalarRange {
    start: u32,
    end: u32,
}

impl fmt::Debug for ScalarRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ScalarRange({:X}, {:X})", self.start, self.end)
    }
}

impl Iterator for Utf8Sequences {
    type Item = Utf8Sequence;

    fn next(&mut self) -> Option<Self::Item> {
        'TOP: while let Some(mut r) = self.range_stack.pop() {
            'INNER: loop {
                if let Some((r1, r2)) = r.split() {
                    self.push(r2.start, r2.end);
                    r.start = r1.start;
                    r.end = r1.end;
                    continue 'INNER;
                }
                if !r.is_valid() {
                    continue 'TOP;
                }
                for i in 1..MAX_UTF8_BYTES {
                    let max = max_scalar_value(i);
                    if r.start <= max && max < r.end {
                        self.push(max + 1, r.end);
                        r.end = max;
                        continue 'INNER;
                    }
                }
                if let Some(ascii_range) = r.as_ascii() {
                    return Some(Utf8Sequence::One(ascii_range));
                }
                for i in 1..MAX_UTF8_BYTES {
                    let m = (1 << (6 * i)) - 1;
                    if (r.start & !m) != (r.end & !m) {
                        if (r.start & m) != 0 {
                            self.push((r.start | m) + 1, r.end);
                            r.end = r.start | m;
                            continue 'INNER;
                        }
                        if (r.end & m) != m {
                            self.push(r.end & !m, r.end);
                            r.end = (r.end & !m) - 1;
                            continue 'INNER;
                        }
                    }
                }
                let mut start = [0; MAX_UTF8_BYTES];
                let mut end = [0; MAX_UTF8_BYTES];
                let n = r.encode(&mut start, &mut end);
                return Some(Utf8Sequence::from_encoded_range(
                    &start[0..n],
                    &end[0..n],
                ));
            }
        }
        None
    }
}

impl FusedIterator for Utf8Sequences {}

impl ScalarRange {
    /// split splits this range if it overlaps with a surrogate codepoint.
    ///
    /// Either or both ranges may be invalid.
    fn split(&self) -> Option<(ScalarRange, ScalarRange)> {
        if self.start < 0xE000 && self.end > 0xD7FF {
            Some((
                ScalarRange { start: self.start, end: 0xD7FF },
                ScalarRange { start: 0xE000, end: self.end },
            ))
        } else {
            None
        }
    }

    /// is_valid returns true if and only if start <= end.
    fn is_valid(&self) -> bool {
        self.start <= self.end
    }

    /// as_ascii returns this range as a Utf8Range if and only if all scalar
    /// values in this range can be encoded as a single byte.
    fn as_ascii(&self) -> Option<Utf8Range> {
        if self.is_ascii() {
            let start = u8::try_from(self.start).unwrap();
            let end = u8::try_from(self.end).unwrap();
            Some(Utf8Range::new(start, end))
        } else {
            None
        }
    }

    /// is_ascii returns true if the range is ASCII only (i.e., takes a single
    /// byte to encode any scalar value).
    fn is_ascii(&self) -> bool {
        self.is_valid() && self.end <= 0x7f
    }

    /// encode writes the UTF-8 encoding of the start and end of this range
    /// to the corresponding destination slices, and returns the number of
    /// bytes written.
    ///
    /// The slices should have room for at least `MAX_UTF8_BYTES`.
    fn encode(&self, start: &mut [u8], end: &mut [u8]) -> usize {
        let cs = char::from_u32(self.start).unwrap();
        let ce = char::from_u32(self.end).unwrap();
        let ss = cs.encode_utf8(start);
        let se = ce.encode_utf8(end);
        assert_eq!(ss.len(), se.len());
        ss.len()
    }
}

fn max_scalar_value(nbytes: usize) -> u32 {
    match nbytes {
        1 => 0x007F,
        2 => 0x07FF,
        3 => 0xFFFF,
        4 => 0x0010_FFFF,
        _ => unreachable!("invalid UTF-8 byte sequence size"),
    }
}

#[cfg(test)]
mod tests {
    use core::char;

    use alloc::{vec, vec::Vec};

    use crate::utf8::{Utf8Range, Utf8Sequences};

    fn rutf8(s: u8, e: u8) -> Utf8Range {
        Utf8Range::new(s, e)
    }

    fn never_accepts_surrogate_codepoints(start: char, end: char) {
        for cp in 0xD800..0xE000 {
            let buf = encode_surrogate(cp);
            for r in Utf8Sequences::new(start, end) {
                if r.matches(&buf) {
                    panic!(
                        "Sequence ({:X}, {:X}) contains range {:?}, \
                         which matches surrogate code point {:X} \
                         with encoded bytes {:?}",
                        u32::from(start),
                        u32::from(end),
                        r,
                        cp,
                        buf,
                    );
                }
            }
        }
    }

    #[test]
    fn codepoints_no_surrogates() {
        never_accepts_surrogate_codepoints('\u{0}', '\u{FFFF}');
        never_accepts_surrogate_codepoints('\u{0}', '\u{10FFFF}');
        never_accepts_surrogate_codepoints('\u{0}', '\u{10FFFE}');
        never_accepts_surrogate_codepoints('\u{80}', '\u{10FFFF}');
        never_accepts_surrogate_codepoints('\u{D7FF}', '\u{E000}');
    }

    #[test]
    fn single_codepoint_one_sequence() {
        // Tests that every range of scalar values that contains a single
        // scalar value is recognized by one sequence of byte ranges.
        for i in 0x0..=0x0010_FFFF {
            let c = match char::from_u32(i) {
                None => continue,
                Some(c) => c,
            };
            let seqs: Vec<_> = Utf8Sequences::new(c, c).collect();
            assert_eq!(seqs.len(), 1);
        }
    }

    #[test]
    fn bmp() {
        use crate::utf8::Utf8Sequence::*;

        let seqs = Utf8Sequences::new('\u{0}', '\u{FFFF}').collect::<Vec<_>>();
        assert_eq!(
            seqs,
            vec![
                One(rutf8(0x0, 0x7F)),
                Two([rutf8(0xC2, 0xDF), rutf8(0x80, 0xBF)]),
                Three([
                    rutf8(0xE0, 0xE0),
                    rutf8(0xA0, 0xBF),
                    rutf8(0x80, 0xBF)
                ]),
                Three([
                    rutf8(0xE1, 0xEC),
                    rutf8(0x80, 0xBF),
                    rutf8(0x80, 0xBF)
                ]),
                Three([
                    rutf8(0xED, 0xED),
                    rutf8(0x80, 0x9F),
                    rutf8(0x80, 0xBF)
                ]),
                Three([
                    rutf8(0xEE, 0xEF),
                    rutf8(0x80, 0xBF),
                    rutf8(0x80, 0xBF)
                ]),
            ]
        );
    }

    #[test]
    fn reverse() {
        use crate::utf8::Utf8Sequence::*;

        let mut s = One(rutf8(0xA, 0xB));
        s.reverse();
        assert_eq!(s.as_slice(), &[rutf8(0xA, 0xB)]);

        let mut s = Two([rutf8(0xA, 0xB), rutf8(0xB, 0xC)]);
        s.reverse();
        assert_eq!(s.as_slice(), &[rutf8(0xB, 0xC), rutf8(0xA, 0xB)]);

        let mut s = Three([rutf8(0xA, 0xB), rutf8(0xB, 0xC), rutf8(0xC, 0xD)]);
        s.reverse();
        assert_eq!(
            s.as_slice(),
            &[rutf8(0xC, 0xD), rutf8(0xB, 0xC), rutf8(0xA, 0xB)]
        );

        let mut s = Four([
            rutf8(0xA, 0xB),
            rutf8(0xB, 0xC),
            rutf8(0xC, 0xD),
            rutf8(0xD, 0xE),
        ]);
        s.reverse();
        assert_eq!(
            s.as_slice(),
            &[
                rutf8(0xD, 0xE),
                rutf8(0xC, 0xD),
                rutf8(0xB, 0xC),
                rutf8(0xA, 0xB)
            ]
        );
    }

    fn encode_surrogate(cp: u32) -> [u8; 3] {
        const TAG_CONT: u8 = 0b1000_0000;
        const TAG_THREE_B: u8 = 0b1110_0000;

        assert!(0xD800 <= cp && cp < 0xE000);
        let mut dst = [0; 3];
        dst[0] = u8::try_from(cp >> 12 & 0x0F).unwrap() | TAG_THREE_B;
        dst[1] = u8::try_from(cp >> 6 & 0x3F).unwrap() | TAG_CONT;
        dst[2] = u8::try_from(cp & 0x3F).unwrap() | TAG_CONT;
        dst
    }
}

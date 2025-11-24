/*!
Types and routines for working with look-around assertions.

This module principally defines two types:

* [`Look`] enumerates all of the assertions supported by this crate.
* [`LookSet`] provides a way to efficiently store a set of [`Look`] values.
* [`LookMatcher`] provides routines for checking whether a `Look` or a
`LookSet` matches at a particular position in a haystack.
*/

// LAMENTATION: Sadly, a lot of the API of `Look` and `LookSet` were basically
// copied verbatim from the regex-syntax crate. I would have no problems using
// the regex-syntax types and defining the matching routines (only found
// in this crate) as free functions, except the `Look` and `LookSet` types
// are used in lots of places. Including in places we expect to work when
// regex-syntax is *not* enabled, such as in the definition of the NFA itself.
//
// Thankfully the code we copy is pretty simple and there isn't much of it.
// Otherwise, the rest of this module deals with *matching* the assertions,
// which is not something that regex-syntax handles.

use crate::util::{escape::DebugByte, utf8};

/// A look-around assertion.
///
/// An assertion matches at a position between characters in a haystack.
/// Namely, it does not actually "consume" any input as most parts of a regular
/// expression do. Assertions are a way of stating that some property must be
/// true at a particular point during matching.
///
/// For example, `(?m)^[a-z]+$` is a pattern that:
///
/// * Scans the haystack for a position at which `(?m:^)` is satisfied. That
/// occurs at either the beginning of the haystack, or immediately following
/// a `\n` character.
/// * Looks for one or more occurrences of `[a-z]`.
/// * Once `[a-z]+` has matched as much as it can, an overall match is only
/// reported when `[a-z]+` stops just before a `\n`.
///
/// So in this case, `abc` and `\nabc\n` match, but `\nabc1\n` does not.
///
/// Assertions are also called "look-around," "look-behind" and "look-ahead."
/// Specifically, some assertions are look-behind (like `^`), other assertions
/// are look-ahead (like `$`) and yet other assertions are both look-ahead and
/// look-behind (like `\b`).
///
/// # Assertions in an NFA
///
/// An assertion in a [`thompson::NFA`](crate::nfa::thompson::NFA) can be
/// thought of as a conditional epsilon transition. That is, a matching engine
/// like the [`PikeVM`](crate::nfa::thompson::pikevm::PikeVM) only permits
/// moving through conditional epsilon transitions when their condition
/// is satisfied at whatever position the `PikeVM` is currently at in the
/// haystack.
///
/// How assertions are handled in a `DFA` is trickier, since a DFA does not
/// have epsilon transitions at all. In this case, they are compiled into the
/// automaton itself, at the expense of more states than what would be required
/// without an assertion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Look {
    /// Match the beginning of text. Specifically, this matches at the starting
    /// position of the input.
    Start = 1 << 0,
    /// Match the end of text. Specifically, this matches at the ending
    /// position of the input.
    End = 1 << 1,
    /// Match the beginning of a line or the beginning of text. Specifically,
    /// this matches at the starting position of the input, or at the position
    /// immediately following a `\n` character.
    StartLF = 1 << 2,
    /// Match the end of a line or the end of text. Specifically, this matches
    /// at the end position of the input, or at the position immediately
    /// preceding a `\n` character.
    EndLF = 1 << 3,
    /// Match the beginning of a line or the beginning of text. Specifically,
    /// this matches at the starting position of the input, or at the position
    /// immediately following either a `\r` or `\n` character, but never after
    /// a `\r` when a `\n` follows.
    StartCRLF = 1 << 4,
    /// Match the end of a line or the end of text. Specifically, this matches
    /// at the end position of the input, or at the position immediately
    /// preceding a `\r` or `\n` character, but never before a `\n` when a `\r`
    /// precedes it.
    EndCRLF = 1 << 5,
    /// Match an ASCII-only word boundary. That is, this matches a position
    /// where the left adjacent character and right adjacent character
    /// correspond to a word and non-word or a non-word and word character.
    WordAscii = 1 << 6,
    /// Match an ASCII-only negation of a word boundary.
    WordAsciiNegate = 1 << 7,
    /// Match a Unicode-aware word boundary. That is, this matches a position
    /// where the left adjacent character and right adjacent character
    /// correspond to a word and non-word or a non-word and word character.
    WordUnicode = 1 << 8,
    /// Match a Unicode-aware negation of a word boundary.
    WordUnicodeNegate = 1 << 9,
    /// Match the start of an ASCII-only word boundary. That is, this matches a
    /// position at either the beginning of the haystack or where the previous
    /// character is not a word character and the following character is a word
    /// character.
    WordStartAscii = 1 << 10,
    /// Match the end of an ASCII-only word boundary. That is, this matches
    /// a position at either the end of the haystack or where the previous
    /// character is a word character and the following character is not a word
    /// character.
    WordEndAscii = 1 << 11,
    /// Match the start of a Unicode word boundary. That is, this matches a
    /// position at either the beginning of the haystack or where the previous
    /// character is not a word character and the following character is a word
    /// character.
    WordStartUnicode = 1 << 12,
    /// Match the end of a Unicode word boundary. That is, this matches a
    /// position at either the end of the haystack or where the previous
    /// character is a word character and the following character is not a word
    /// character.
    WordEndUnicode = 1 << 13,
    /// Match the start half of an ASCII-only word boundary. That is, this
    /// matches a position at either the beginning of the haystack or where the
    /// previous character is not a word character.
    WordStartHalfAscii = 1 << 14,
    /// Match the end half of an ASCII-only word boundary. That is, this
    /// matches a position at either the end of the haystack or where the
    /// following character is not a word character.
    WordEndHalfAscii = 1 << 15,
    /// Match the start half of a Unicode word boundary. That is, this matches
    /// a position at either the beginning of the haystack or where the
    /// previous character is not a word character.
    WordStartHalfUnicode = 1 << 16,
    /// Match the end half of a Unicode word boundary. That is, this matches
    /// a position at either the end of the haystack or where the following
    /// character is not a word character.
    WordEndHalfUnicode = 1 << 17,
}

impl Look {
    /// Flip the look-around assertion to its equivalent for reverse searches.
    /// For example, `StartLF` gets translated to `EndLF`.
    ///
    /// Some assertions, such as `WordUnicode`, remain the same since they
    /// match the same positions regardless of the direction of the search.
    #[inline]
    pub const fn reversed(self) -> Look {
        match self {
            Look::Start => Look::End,
            Look::End => Look::Start,
            Look::StartLF => Look::EndLF,
            Look::EndLF => Look::StartLF,
            Look::StartCRLF => Look::EndCRLF,
            Look::EndCRLF => Look::StartCRLF,
            Look::WordAscii => Look::WordAscii,
            Look::WordAsciiNegate => Look::WordAsciiNegate,
            Look::WordUnicode => Look::WordUnicode,
            Look::WordUnicodeNegate => Look::WordUnicodeNegate,
            Look::WordStartAscii => Look::WordEndAscii,
            Look::WordEndAscii => Look::WordStartAscii,
            Look::WordStartUnicode => Look::WordEndUnicode,
            Look::WordEndUnicode => Look::WordStartUnicode,
            Look::WordStartHalfAscii => Look::WordEndHalfAscii,
            Look::WordEndHalfAscii => Look::WordStartHalfAscii,
            Look::WordStartHalfUnicode => Look::WordEndHalfUnicode,
            Look::WordEndHalfUnicode => Look::WordStartHalfUnicode,
        }
    }

    /// Return the underlying representation of this look-around enumeration
    /// as an integer. Giving the return value to the [`Look::from_repr`]
    /// constructor is guaranteed to return the same look-around variant that
    /// one started with within a semver compatible release of this crate.
    #[inline]
    pub const fn as_repr(self) -> u32 {
        // AFAIK, 'as' is the only way to zero-cost convert an int enum to an
        // actual int.
        self as u32
    }

    /// Given the underlying representation of a `Look` value, return the
    /// corresponding `Look` value if the representation is valid. Otherwise
    /// `None` is returned.
    #[inline]
    pub const fn from_repr(repr: u32) -> Option<Look> {
        match repr {
            0b00_0000_0000_0000_0001 => Some(Look::Start),
            0b00_0000_0000_0000_0010 => Some(Look::End),
            0b00_0000_0000_0000_0100 => Some(Look::StartLF),
            0b00_0000_0000_0000_1000 => Some(Look::EndLF),
            0b00_0000_0000_0001_0000 => Some(Look::StartCRLF),
            0b00_0000_0000_0010_0000 => Some(Look::EndCRLF),
            0b00_0000_0000_0100_0000 => Some(Look::WordAscii),
            0b00_0000_0000_1000_0000 => Some(Look::WordAsciiNegate),
            0b00_0000_0001_0000_0000 => Some(Look::WordUnicode),
            0b00_0000_0010_0000_0000 => Some(Look::WordUnicodeNegate),
            0b00_0000_0100_0000_0000 => Some(Look::WordStartAscii),
            0b00_0000_1000_0000_0000 => Some(Look::WordEndAscii),
            0b00_0001_0000_0000_0000 => Some(Look::WordStartUnicode),
            0b00_0010_0000_0000_0000 => Some(Look::WordEndUnicode),
            0b00_0100_0000_0000_0000 => Some(Look::WordStartHalfAscii),
            0b00_1000_0000_0000_0000 => Some(Look::WordEndHalfAscii),
            0b01_0000_0000_0000_0000 => Some(Look::WordStartHalfUnicode),
            0b10_0000_0000_0000_0000 => Some(Look::WordEndHalfUnicode),
            _ => None,
        }
    }

    /// Returns a convenient single codepoint representation of this
    /// look-around assertion. Each assertion is guaranteed to be represented
    /// by a distinct character.
    ///
    /// This is useful for succinctly representing a look-around assertion in
    /// human friendly but succinct output intended for a programmer working on
    /// regex internals.
    #[inline]
    pub const fn as_char(self) -> char {
        match self {
            Look::Start => 'A',
            Look::End => 'z',
            Look::StartLF => '^',
            Look::EndLF => '$',
            Look::StartCRLF => 'r',
            Look::EndCRLF => 'R',
            Look::WordAscii => 'b',
            Look::WordAsciiNegate => 'B',
            Look::WordUnicode => '𝛃',
            Look::WordUnicodeNegate => '𝚩',
            Look::WordStartAscii => '<',
            Look::WordEndAscii => '>',
            Look::WordStartUnicode => '〈',
            Look::WordEndUnicode => '〉',
            Look::WordStartHalfAscii => '◁',
            Look::WordEndHalfAscii => '▷',
            Look::WordStartHalfUnicode => '◀',
            Look::WordEndHalfUnicode => '▶',
        }
    }
}

/// LookSet is a memory-efficient set of look-around assertions.
///
/// This is useful for efficiently tracking look-around assertions. For
/// example, a [`thompson::NFA`](crate::nfa::thompson::NFA) provides properties
/// that return `LookSet`s.
#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct LookSet {
    /// The underlying representation this set is exposed to make it possible
    /// to store it somewhere efficiently. The representation is that
    /// of a bitset, where each assertion occupies bit `i` where
    /// `i = Look::as_repr()`.
    ///
    /// Note that users of this internal representation must permit the full
    /// range of `u16` values to be represented. For example, even if the
    /// current implementation only makes use of the 10 least significant bits,
    /// it may use more bits in a future semver compatible release.
    pub bits: u32,
}

impl LookSet {
    /// Create an empty set of look-around assertions.
    #[inline]
    pub fn empty() -> LookSet {
        LookSet { bits: 0 }
    }

    /// Create a full set of look-around assertions.
    ///
    /// This set contains all possible look-around assertions.
    #[inline]
    pub fn full() -> LookSet {
        LookSet { bits: !0 }
    }

    /// Create a look-around set containing the look-around assertion given.
    ///
    /// This is a convenience routine for creating an empty set and inserting
    /// one look-around assertions.
    #[inline]
    pub fn singleton(look: Look) -> LookSet {
        LookSet::empty().insert(look)
    }

    /// Returns the total number of look-around assertions in this set.
    #[inline]
    pub fn len(self) -> usize {
        // OK because max value always fits in a u8, which in turn always
        // fits in a usize, regardless of target.
        usize::try_from(self.bits.count_ones()).unwrap()
    }

    /// Returns true if and only if this set is empty.
    #[inline]
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Returns true if and only if the given look-around assertion is in this
    /// set.
    #[inline]
    pub fn contains(self, look: Look) -> bool {
        self.bits & look.as_repr() != 0
    }

    /// Returns true if and only if this set contains any anchor assertions.
    /// This includes both "start/end of haystack" and "start/end of line."
    #[inline]
    pub fn contains_anchor(&self) -> bool {
        self.contains_anchor_haystack() || self.contains_anchor_line()
    }

    /// Returns true if and only if this set contains any "start/end of
    /// haystack" anchors. This doesn't include "start/end of line" anchors.
    #[inline]
    pub fn contains_anchor_haystack(&self) -> bool {
        self.contains(Look::Start) || self.contains(Look::End)
    }

    /// Returns true if and only if this set contains any "start/end of line"
    /// anchors. This doesn't include "start/end of haystack" anchors. This
    /// includes both `\n` line anchors and CRLF (`\r\n`) aware line anchors.
    #[inline]
    pub fn contains_anchor_line(&self) -> bool {
        self.contains(Look::StartLF)
            || self.contains(Look::EndLF)
            || self.contains(Look::StartCRLF)
            || self.contains(Look::EndCRLF)
    }

    /// Returns true if and only if this set contains any "start/end of line"
    /// anchors that only treat `\n` as line terminators. This does not include
    /// haystack anchors or CRLF aware line anchors.
    #[inline]
    pub fn contains_anchor_lf(&self) -> bool {
        self.contains(Look::StartLF) || self.contains(Look::EndLF)
    }

    /// Returns true if and only if this set contains any "start/end of line"
    /// anchors that are CRLF-aware. This doesn't include "start/end of
    /// haystack" or "start/end of line-feed" anchors.
    #[inline]
    pub fn contains_anchor_crlf(&self) -> bool {
        self.contains(Look::StartCRLF) || self.contains(Look::EndCRLF)
    }

    /// Returns true if and only if this set contains any word boundary or
    /// negated word boundary assertions. This include both Unicode and ASCII
    /// word boundaries.
    #[inline]
    pub fn contains_word(self) -> bool {
        self.contains_word_unicode() || self.contains_word_ascii()
    }

    /// Returns true if and only if this set contains any Unicode word boundary
    /// or negated Unicode word boundary assertions.
    #[inline]
    pub fn contains_word_unicode(self) -> bool {
        self.contains(Look::WordUnicode)
            || self.contains(Look::WordUnicodeNegate)
            || self.contains(Look::WordStartUnicode)
            || self.contains(Look::WordEndUnicode)
            || self.contains(Look::WordStartHalfUnicode)
            || self.contains(Look::WordEndHalfUnicode)
    }

    /// Returns true if and only if this set contains any ASCII word boundary
    /// or negated ASCII word boundary assertions.
    #[inline]
    pub fn contains_word_ascii(self) -> bool {
        self.contains(Look::WordAscii)
            || self.contains(Look::WordAsciiNegate)
            || self.contains(Look::WordStartAscii)
            || self.contains(Look::WordEndAscii)
            || self.contains(Look::WordStartHalfAscii)
            || self.contains(Look::WordEndHalfAscii)
    }

    /// Returns an iterator over all of the look-around assertions in this set.
    #[inline]
    pub fn iter(self) -> LookSetIter {
        LookSetIter { set: self }
    }

    /// Return a new set that is equivalent to the original, but with the given
    /// assertion added to it. If the assertion is already in the set, then the
    /// returned set is equivalent to the original.
    #[inline]
    pub fn insert(self, look: Look) -> LookSet {
        LookSet { bits: self.bits | look.as_repr() }
    }

    /// Updates this set in place with the result of inserting the given
    /// assertion into this set.
    #[inline]
    pub fn set_insert(&mut self, look: Look) {
        *self = self.insert(look);
    }

    /// Return a new set that is equivalent to the original, but with the given
    /// assertion removed from it. If the assertion is not in the set, then the
    /// returned set is equivalent to the original.
    #[inline]
    pub fn remove(self, look: Look) -> LookSet {
        LookSet { bits: self.bits & !look.as_repr() }
    }

    /// Updates this set in place with the result of removing the given
    /// assertion from this set.
    #[inline]
    pub fn set_remove(&mut self, look: Look) {
        *self = self.remove(look);
    }

    /// Returns a new set that is the result of subtracting the given set from
    /// this set.
    #[inline]
    pub fn subtract(self, other: LookSet) -> LookSet {
        LookSet { bits: self.bits & !other.bits }
    }

    /// Updates this set in place with the result of subtracting the given set
    /// from this set.
    #[inline]
    pub fn set_subtract(&mut self, other: LookSet) {
        *self = self.subtract(other);
    }

    /// Returns a new set that is the union of this and the one given.
    #[inline]
    pub fn union(self, other: LookSet) -> LookSet {
        LookSet { bits: self.bits | other.bits }
    }

    /// Updates this set in place with the result of unioning it with the one
    /// given.
    #[inline]
    pub fn set_union(&mut self, other: LookSet) {
        *self = self.union(other);
    }

    /// Returns a new set that is the intersection of this and the one given.
    #[inline]
    pub fn intersect(self, other: LookSet) -> LookSet {
        LookSet { bits: self.bits & other.bits }
    }

    /// Updates this set in place with the result of intersecting it with the
    /// one given.
    #[inline]
    pub fn set_intersect(&mut self, other: LookSet) {
        *self = self.intersect(other);
    }

    /// Return a `LookSet` from the slice given as a native endian 32-bit
    /// integer.
    ///
    /// # Panics
    ///
    /// This panics if `slice.len() < 4`.
    #[inline]
    pub fn read_repr(slice: &[u8]) -> LookSet {
        let bits = u32::from_ne_bytes(slice[..4].try_into().unwrap());
        LookSet { bits }
    }

    /// Write a `LookSet` as a native endian 32-bit integer to the beginning
    /// of the slice given.
    ///
    /// # Panics
    ///
    /// This panics if `slice.len() < 4`.
    #[inline]
    pub fn write_repr(self, slice: &mut [u8]) {
        let raw = self.bits.to_ne_bytes();
        slice[0] = raw[0];
        slice[1] = raw[1];
        slice[2] = raw[2];
        slice[3] = raw[3];
    }

    /// Checks that all assertions in this set can be matched.
    ///
    /// Some assertions, such as Unicode word boundaries, require optional (but
    /// enabled by default) tables that may not be available. If there are
    /// assertions in this set that require tables that are not available, then
    /// this will return an error.
    ///
    /// Specifically, this returns an error when the the
    /// `unicode-word-boundary` feature is _not_ enabled _and_ this set
    /// contains a Unicode word boundary assertion.
    ///
    /// It can be useful to use this on the result of
    /// [`NFA::look_set_any`](crate::nfa::thompson::NFA::look_set_any)
    /// when building a matcher engine to ensure methods like
    /// [`LookMatcher::matches_set`] do not panic at search time.
    pub fn available(self) -> Result<(), UnicodeWordBoundaryError> {
        if self.contains_word_unicode() {
            UnicodeWordBoundaryError::check()?;
        }
        Ok(())
    }
}

impl core::fmt::Debug for LookSet {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        if self.is_empty() {
            return write!(f, "∅");
        }
        for look in self.iter() {
            write!(f, "{}", look.as_char())?;
        }
        Ok(())
    }
}

/// An iterator over all look-around assertions in a [`LookSet`].
///
/// This iterator is created by [`LookSet::iter`].
#[derive(Clone, Debug)]
pub struct LookSetIter {
    set: LookSet,
}

impl Iterator for LookSetIter {
    type Item = Look;

    #[inline]
    fn next(&mut self) -> Option<Look> {
        if self.set.is_empty() {
            return None;
        }
        // We'll never have more than u8::MAX distinct look-around assertions,
        // so 'bit' will always fit into a u16.
        let bit = u16::try_from(self.set.bits.trailing_zeros()).unwrap();
        let look = Look::from_repr(1 << bit)?;
        self.set = self.set.remove(look);
        Some(look)
    }
}

/// A matcher for look-around assertions.
///
/// This matcher permits configuring aspects of how look-around assertions are
/// matched.
///
/// # Example
///
/// A `LookMatcher` can change the line terminator used for matching multi-line
/// anchors such as `(?m:^)` and `(?m:$)`.
///
/// ```
/// use regex_automata::{
///     nfa::thompson::{self, pikevm::PikeVM},
///     util::look::LookMatcher,
///     Match, Input,
/// };
///
/// let mut lookm = LookMatcher::new();
/// lookm.set_line_terminator(b'\x00');
///
/// let re = PikeVM::builder()
///     .thompson(thompson::Config::new().look_matcher(lookm))
///     .build(r"(?m)^[a-z]+$")?;
/// let mut cache = re.create_cache();
///
/// // Multi-line assertions now use NUL as a terminator.
/// assert_eq!(
///     Some(Match::must(0, 1..4)),
///     re.find(&mut cache, b"\x00abc\x00"),
/// );
/// // ... and \n is no longer recognized as a terminator.
/// assert_eq!(
///     None,
///     re.find(&mut cache, b"\nabc\n"),
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct LookMatcher {
    lineterm: DebugByte,
}

impl LookMatcher {
    /// Creates a new default matcher for look-around assertions.
    pub fn new() -> LookMatcher {
        LookMatcher { lineterm: DebugByte(b'\n') }
    }

    /// Sets the line terminator for use with `(?m:^)` and `(?m:$)`.
    ///
    /// Namely, instead of `^` matching after `\n` and `$` matching immediately
    /// before a `\n`, this will cause it to match after and before the byte
    /// given.
    ///
    /// It can occasionally be useful to use this to configure the line
    /// terminator to the NUL byte when searching binary data.
    ///
    /// Note that this does not apply to CRLF-aware line anchors such as
    /// `(?Rm:^)` and `(?Rm:$)`. CRLF-aware line anchors are hard-coded to
    /// use `\r` and `\n`.
    pub fn set_line_terminator(&mut self, byte: u8) -> &mut LookMatcher {
        self.lineterm.0 = byte;
        self
    }

    /// Returns the line terminator that was configured for this matcher.
    ///
    /// If no line terminator was configured, then this returns `\n`.
    ///
    /// Note that the line terminator should only be used for matching `(?m:^)`
    /// and `(?m:$)` assertions. It specifically should _not_ be used for
    /// matching the CRLF aware assertions `(?Rm:^)` and `(?Rm:$)`.
    pub fn get_line_terminator(&self) -> u8 {
        self.lineterm.0
    }

    /// Returns true when the position `at` in `haystack` satisfies the given
    /// look-around assertion.
    ///
    /// # Panics
    ///
    /// This panics when testing any Unicode word boundary assertion in this
    /// set and when the Unicode word data is not available. Specifically, this
    /// only occurs when the `unicode-word-boundary` feature is not enabled.
    ///
    /// Since it's generally expected that this routine is called inside of
    /// a matching engine, callers should check the error condition when
    /// building the matching engine. If there is a Unicode word boundary
    /// in the matcher and the data isn't available, then the matcher should
    /// fail to build.
    ///
    /// Callers can check the error condition with [`LookSet::available`].
    ///
    /// This also may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    #[inline]
    pub fn matches(&self, look: Look, haystack: &[u8], at: usize) -> bool {
        self.matches_inline(look, haystack, at)
    }

    /// Like `matches`, but forcefully inlined.
    ///
    /// # Panics
    ///
    /// This panics when testing any Unicode word boundary assertion in this
    /// set and when the Unicode word data is not available. Specifically, this
    /// only occurs when the `unicode-word-boundary` feature is not enabled.
    ///
    /// Since it's generally expected that this routine is called inside of
    /// a matching engine, callers should check the error condition when
    /// building the matching engine. If there is a Unicode word boundary
    /// in the matcher and the data isn't available, then the matcher should
    /// fail to build.
    ///
    /// Callers can check the error condition with [`LookSet::available`].
    ///
    /// This also may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn matches_inline(
        &self,
        look: Look,
        haystack: &[u8],
        at: usize,
    ) -> bool {
        match look {
            Look::Start => self.is_start(haystack, at),
            Look::End => self.is_end(haystack, at),
            Look::StartLF => self.is_start_lf(haystack, at),
            Look::EndLF => self.is_end_lf(haystack, at),
            Look::StartCRLF => self.is_start_crlf(haystack, at),
            Look::EndCRLF => self.is_end_crlf(haystack, at),
            Look::WordAscii => self.is_word_ascii(haystack, at),
            Look::WordAsciiNegate => self.is_word_ascii_negate(haystack, at),
            Look::WordUnicode => self.is_word_unicode(haystack, at).unwrap(),
            Look::WordUnicodeNegate => {
                self.is_word_unicode_negate(haystack, at).unwrap()
            }
            Look::WordStartAscii => self.is_word_start_ascii(haystack, at),
            Look::WordEndAscii => self.is_word_end_ascii(haystack, at),
            Look::WordStartUnicode => {
                self.is_word_start_unicode(haystack, at).unwrap()
            }
            Look::WordEndUnicode => {
                self.is_word_end_unicode(haystack, at).unwrap()
            }
            Look::WordStartHalfAscii => {
                self.is_word_start_half_ascii(haystack, at)
            }
            Look::WordEndHalfAscii => {
                self.is_word_end_half_ascii(haystack, at)
            }
            Look::WordStartHalfUnicode => {
                self.is_word_start_half_unicode(haystack, at).unwrap()
            }
            Look::WordEndHalfUnicode => {
                self.is_word_end_half_unicode(haystack, at).unwrap()
            }
        }
    }

    /// Returns true when _all_ of the assertions in the given set match at the
    /// given position in the haystack.
    ///
    /// # Panics
    ///
    /// This panics when testing any Unicode word boundary assertion in this
    /// set and when the Unicode word data is not available. Specifically, this
    /// only occurs when the `unicode-word-boundary` feature is not enabled.
    ///
    /// Since it's generally expected that this routine is called inside of
    /// a matching engine, callers should check the error condition when
    /// building the matching engine. If there is a Unicode word boundary
    /// in the matcher and the data isn't available, then the matcher should
    /// fail to build.
    ///
    /// Callers can check the error condition with [`LookSet::available`].
    ///
    /// This also may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    #[inline]
    pub fn matches_set(
        &self,
        set: LookSet,
        haystack: &[u8],
        at: usize,
    ) -> bool {
        self.matches_set_inline(set, haystack, at)
    }

    /// Like `LookSet::matches`, but forcefully inlined for perf.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn matches_set_inline(
        &self,
        set: LookSet,
        haystack: &[u8],
        at: usize,
    ) -> bool {
        // This used to luse LookSet::iter with Look::matches on each element,
        // but that proved to be quite diastrous for perf. The manual "if
        // the set has this assertion, check it" turns out to be quite a bit
        // faster.
        if set.contains(Look::Start) {
            if !self.is_start(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::End) {
            if !self.is_end(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::StartLF) {
            if !self.is_start_lf(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::EndLF) {
            if !self.is_end_lf(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::StartCRLF) {
            if !self.is_start_crlf(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::EndCRLF) {
            if !self.is_end_crlf(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::WordAscii) {
            if !self.is_word_ascii(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::WordAsciiNegate) {
            if !self.is_word_ascii_negate(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::WordUnicode) {
            if !self.is_word_unicode(haystack, at).unwrap() {
                return false;
            }
        }
        if set.contains(Look::WordUnicodeNegate) {
            if !self.is_word_unicode_negate(haystack, at).unwrap() {
                return false;
            }
        }
        if set.contains(Look::WordStartAscii) {
            if !self.is_word_start_ascii(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::WordEndAscii) {
            if !self.is_word_end_ascii(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::WordStartUnicode) {
            if !self.is_word_start_unicode(haystack, at).unwrap() {
                return false;
            }
        }
        if set.contains(Look::WordEndUnicode) {
            if !self.is_word_end_unicode(haystack, at).unwrap() {
                return false;
            }
        }
        if set.contains(Look::WordStartHalfAscii) {
            if !self.is_word_start_half_ascii(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::WordEndHalfAscii) {
            if !self.is_word_end_half_ascii(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::WordStartHalfUnicode) {
            if !self.is_word_start_half_unicode(haystack, at).unwrap() {
                return false;
            }
        }
        if set.contains(Look::WordEndHalfUnicode) {
            if !self.is_word_end_half_unicode(haystack, at).unwrap() {
                return false;
            }
        }
        true
    }

    /// Split up the given byte classes into equivalence classes in a way that
    /// is consistent with this look-around assertion.
    #[cfg(feature = "alloc")]
    pub(crate) fn add_to_byteset(
        &self,
        look: Look,
        set: &mut crate::util::alphabet::ByteClassSet,
    ) {
        match look {
            Look::Start | Look::End => {}
            Look::StartLF | Look::EndLF => {
                set.set_range(self.lineterm.0, self.lineterm.0);
            }
            Look::StartCRLF | Look::EndCRLF => {
                set.set_range(b'\r', b'\r');
                set.set_range(b'\n', b'\n');
            }
            Look::WordAscii
            | Look::WordAsciiNegate
            | Look::WordUnicode
            | Look::WordUnicodeNegate
            | Look::WordStartAscii
            | Look::WordEndAscii
            | Look::WordStartUnicode
            | Look::WordEndUnicode
            | Look::WordStartHalfAscii
            | Look::WordEndHalfAscii
            | Look::WordStartHalfUnicode
            | Look::WordEndHalfUnicode => {
                // We need to mark all ranges of bytes whose pairs result in
                // evaluating \b differently. This isn't technically correct
                // for Unicode word boundaries, but DFAs can't handle those
                // anyway, and thus, the byte classes don't need to either
                // since they are themselves only used in DFAs.
                //
                // FIXME: It seems like the calls to 'set_range' here are
                // completely invariant, which means we could just hard-code
                // them here without needing to write a loop. And we only need
                // to do this dance at most once per regex.
                //
                // FIXME: Is this correct for \B?
                let iswb = utf8::is_word_byte;
                // This unwrap is OK because we guard every use of 'asu8' with
                // a check that the input is <= 255.
                let asu8 = |b: u16| u8::try_from(b).unwrap();
                let mut b1: u16 = 0;
                let mut b2: u16;
                while b1 <= 255 {
                    b2 = b1 + 1;
                    while b2 <= 255 && iswb(asu8(b1)) == iswb(asu8(b2)) {
                        b2 += 1;
                    }
                    // The guards above guarantee that b2 can never get any
                    // bigger.
                    assert!(b2 <= 256);
                    // Subtracting 1 from b2 is always OK because it is always
                    // at least 1 greater than b1, and the assert above
                    // guarantees that the asu8 conversion will succeed.
                    set.set_range(asu8(b1), asu8(b2.checked_sub(1).unwrap()));
                    b1 = b2;
                }
            }
        }
    }

    /// Returns true when [`Look::Start`] is satisfied `at` the given position
    /// in `haystack`.
    ///
    /// # Panics
    ///
    /// This may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    #[inline]
    pub fn is_start(&self, _haystack: &[u8], at: usize) -> bool {
        at == 0
    }

    /// Returns true when [`Look::End`] is satisfied `at` the given position in
    /// `haystack`.
    ///
    /// # Panics
    ///
    /// This may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    #[inline]
    pub fn is_end(&self, haystack: &[u8], at: usize) -> bool {
        at == haystack.len()
    }

    /// Returns true when [`Look::StartLF`] is satisfied `at` the given
    /// position in `haystack`.
    ///
    /// # Panics
    ///
    /// This may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    #[inline]
    pub fn is_start_lf(&self, haystack: &[u8], at: usize) -> bool {
        self.is_start(haystack, at) || haystack[at - 1] == self.lineterm.0
    }

    /// Returns true when [`Look::EndLF`] is satisfied `at` the given position
    /// in `haystack`.
    ///
    /// # Panics
    ///
    /// This may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    #[inline]
    pub fn is_end_lf(&self, haystack: &[u8], at: usize) -> bool {
        self.is_end(haystack, at) || haystack[at] == self.lineterm.0
    }

    /// Returns true when [`Look::StartCRLF`] is satisfied `at` the given
    /// position in `haystack`.
    ///
    /// # Panics
    ///
    /// This may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    #[inline]
    pub fn is_start_crlf(&self, haystack: &[u8], at: usize) -> bool {
        self.is_start(haystack, at)
            || haystack[at - 1] == b'\n'
            || (haystack[at - 1] == b'\r'
                && (at >= haystack.len() || haystack[at] != b'\n'))
    }

    /// Returns true when [`Look::EndCRLF`] is satisfied `at` the given
    /// position in `haystack`.
    ///
    /// # Panics
    ///
    /// This may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    #[inline]
    pub fn is_end_crlf(&self, haystack: &[u8], at: usize) -> bool {
        self.is_end(haystack, at)
            || haystack[at] == b'\r'
            || (haystack[at] == b'\n'
                && (at == 0 || haystack[at - 1] != b'\r'))
    }

    /// Returns true when [`Look::WordAscii`] is satisfied `at` the given
    /// position in `haystack`.
    ///
    /// # Panics
    ///
    /// This may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    #[inline]
    pub fn is_word_ascii(&self, haystack: &[u8], at: usize) -> bool {
        let word_before = at > 0 && utf8::is_word_byte(haystack[at - 1]);
        let word_after =
            at < haystack.len() && utf8::is_word_byte(haystack[at]);
        word_before != word_after
    }

    /// Returns true when [`Look::WordAsciiNegate`] is satisfied `at` the given
    /// position in `haystack`.
    ///
    /// # Panics
    ///
    /// This may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    #[inline]
    pub fn is_word_ascii_negate(&self, haystack: &[u8], at: usize) -> bool {
        !self.is_word_ascii(haystack, at)
    }

    /// Returns true when [`Look::WordUnicode`] is satisfied `at` the given
    /// position in `haystack`.
    ///
    /// # Panics
    ///
    /// This may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    ///
    /// # Errors
    ///
    /// This returns an error when Unicode word boundary tables
    /// are not available. Specifically, this only occurs when the
    /// `unicode-word-boundary` feature is not enabled.
    #[inline]
    pub fn is_word_unicode(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, UnicodeWordBoundaryError> {
        let word_before = is_word_char::rev(haystack, at)?;
        let word_after = is_word_char::fwd(haystack, at)?;
        Ok(word_before != word_after)
    }

    /// Returns true when [`Look::WordUnicodeNegate`] is satisfied `at` the
    /// given position in `haystack`.
    ///
    /// # Panics
    ///
    /// This may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    ///
    /// # Errors
    ///
    /// This returns an error when Unicode word boundary tables
    /// are not available. Specifically, this only occurs when the
    /// `unicode-word-boundary` feature is not enabled.
    #[inline]
    pub fn is_word_unicode_negate(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, UnicodeWordBoundaryError> {
        // This is pretty subtle. Why do we need to do UTF-8 decoding here?
        // Well... at time of writing, the is_word_char_{fwd,rev} routines will
        // only return true if there is a valid UTF-8 encoding of a "word"
        // codepoint, and false in every other case (including invalid UTF-8).
        // This means that in regions of invalid UTF-8 (which might be a
        // subset of valid UTF-8!), it would result in \B matching. While this
        // would be questionable in the context of truly invalid UTF-8, it is
        // *certainly* wrong to report match boundaries that split the encoding
        // of a codepoint. So to work around this, we ensure that we can decode
        // a codepoint on either side of `at`. If either direction fails, then
        // we don't permit \B to match at all.
        //
        // Now, this isn't exactly optimal from a perf perspective. We could
        // try and detect this in is_word_char::{fwd,rev}, but it's not clear
        // if it's worth it. \B is, after all, rarely used. Even worse,
        // is_word_char::{fwd,rev} could do its own UTF-8 decoding, and so this
        // will wind up doing UTF-8 decoding twice. Owch. We could fix this
        // with more code complexity, but it just doesn't feel worth it for \B.
        //
        // And in particular, we do *not* have to do this with \b, because \b
        // *requires* that at least one side of `at` be a "word" codepoint,
        // which in turn implies one side of `at` must be valid UTF-8. This in
        // turn implies that \b can never split a valid UTF-8 encoding of a
        // codepoint. In the case where one side of `at` is truly invalid UTF-8
        // and the other side IS a word codepoint, then we want \b to match
        // since it represents a valid UTF-8 boundary. It also makes sense. For
        // example, you'd want \b\w+\b to match 'abc' in '\xFFabc\xFF'.
        //
        // Note also that this is not just '!is_word_unicode(..)' like it is
        // for the ASCII case. For example, neither \b nor \B is satisfied
        // within invalid UTF-8 sequences.
        let word_before = at > 0
            && match utf8::decode_last(&haystack[..at]) {
                None | Some(Err(_)) => return Ok(false),
                Some(Ok(_)) => is_word_char::rev(haystack, at)?,
            };
        let word_after = at < haystack.len()
            && match utf8::decode(&haystack[at..]) {
                None | Some(Err(_)) => return Ok(false),
                Some(Ok(_)) => is_word_char::fwd(haystack, at)?,
            };
        Ok(word_before == word_after)
    }

    /// Returns true when [`Look::WordStartAscii`] is satisfied `at` the given
    /// position in `haystack`.
    ///
    /// # Panics
    ///
    /// This may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    #[inline]
    pub fn is_word_start_ascii(&self, haystack: &[u8], at: usize) -> bool {
        let word_before = at > 0 && utf8::is_word_byte(haystack[at - 1]);
        let word_after =
            at < haystack.len() && utf8::is_word_byte(haystack[at]);
        !word_before && word_after
    }

    /// Returns true when [`Look::WordEndAscii`] is satisfied `at` the given
    /// position in `haystack`.
    ///
    /// # Panics
    ///
    /// This may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    #[inline]
    pub fn is_word_end_ascii(&self, haystack: &[u8], at: usize) -> bool {
        let word_before = at > 0 && utf8::is_word_byte(haystack[at - 1]);
        let word_after =
            at < haystack.len() && utf8::is_word_byte(haystack[at]);
        word_before && !word_after
    }

    /// Returns true when [`Look::WordStartUnicode`] is satisfied `at` the
    /// given position in `haystack`.
    ///
    /// # Panics
    ///
    /// This may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    ///
    /// # Errors
    ///
    /// This returns an error when Unicode word boundary tables
    /// are not available. Specifically, this only occurs when the
    /// `unicode-word-boundary` feature is not enabled.
    #[inline]
    pub fn is_word_start_unicode(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, UnicodeWordBoundaryError> {
        let word_before = is_word_char::rev(haystack, at)?;
        let word_after = is_word_char::fwd(haystack, at)?;
        Ok(!word_before && word_after)
    }

    /// Returns true when [`Look::WordEndUnicode`] is satisfied `at` the
    /// given position in `haystack`.
    ///
    /// # Panics
    ///
    /// This may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    ///
    /// # Errors
    ///
    /// This returns an error when Unicode word boundary tables
    /// are not available. Specifically, this only occurs when the
    /// `unicode-word-boundary` feature is not enabled.
    #[inline]
    pub fn is_word_end_unicode(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, UnicodeWordBoundaryError> {
        let word_before = is_word_char::rev(haystack, at)?;
        let word_after = is_word_char::fwd(haystack, at)?;
        Ok(word_before && !word_after)
    }

    /// Returns true when [`Look::WordStartHalfAscii`] is satisfied `at` the
    /// given position in `haystack`.
    ///
    /// # Panics
    ///
    /// This may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    #[inline]
    pub fn is_word_start_half_ascii(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> bool {
        let word_before = at > 0 && utf8::is_word_byte(haystack[at - 1]);
        !word_before
    }

    /// Returns true when [`Look::WordEndHalfAscii`] is satisfied `at` the
    /// given position in `haystack`.
    ///
    /// # Panics
    ///
    /// This may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    #[inline]
    pub fn is_word_end_half_ascii(&self, haystack: &[u8], at: usize) -> bool {
        let word_after =
            at < haystack.len() && utf8::is_word_byte(haystack[at]);
        !word_after
    }

    /// Returns true when [`Look::WordStartHalfUnicode`] is satisfied `at` the
    /// given position in `haystack`.
    ///
    /// # Panics
    ///
    /// This may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    ///
    /// # Errors
    ///
    /// This returns an error when Unicode word boundary tables
    /// are not available. Specifically, this only occurs when the
    /// `unicode-word-boundary` feature is not enabled.
    #[inline]
    pub fn is_word_start_half_unicode(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, UnicodeWordBoundaryError> {
        // See `is_word_unicode_negate` for why we need to do this. We don't
        // need to do it for `is_word_start_unicode` because that guarantees
        // that the position matched falls on a valid UTF-8 boundary given
        // that the right side must be in \w.
        let word_before = at > 0
            && match utf8::decode_last(&haystack[..at]) {
                None | Some(Err(_)) => return Ok(false),
                Some(Ok(_)) => is_word_char::rev(haystack, at)?,
            };
        Ok(!word_before)
    }

    /// Returns true when [`Look::WordEndHalfUnicode`] is satisfied `at` the
    /// given position in `haystack`.
    ///
    /// # Panics
    ///
    /// This may panic when `at > haystack.len()`. Note that `at ==
    /// haystack.len()` is legal and guaranteed not to panic.
    ///
    /// # Errors
    ///
    /// This returns an error when Unicode word boundary tables
    /// are not available. Specifically, this only occurs when the
    /// `unicode-word-boundary` feature is not enabled.
    #[inline]
    pub fn is_word_end_half_unicode(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, UnicodeWordBoundaryError> {
        // See `is_word_unicode_negate` for why we need to do this. We don't
        // need to do it for `is_word_end_unicode` because that guarantees
        // that the position matched falls on a valid UTF-8 boundary given
        // that the left side must be in \w.
        let word_after = at < haystack.len()
            && match utf8::decode(&haystack[at..]) {
                None | Some(Err(_)) => return Ok(false),
                Some(Ok(_)) => is_word_char::fwd(haystack, at)?,
            };
        Ok(!word_after)
    }
}

impl Default for LookMatcher {
    fn default() -> LookMatcher {
        LookMatcher::new()
    }
}

/// An error that occurs when the Unicode-aware `\w` class is unavailable.
///
/// This error can occur when the data tables necessary for the Unicode aware
/// Perl character class `\w` are unavailable. The `\w` class is used to
/// determine whether a codepoint is considered a word character or not when
/// determining whether a Unicode aware `\b` (or `\B`) matches at a particular
/// position.
///
/// This error can only occur when the `unicode-word-boundary` feature is
/// disabled.
#[derive(Clone, Debug)]
pub struct UnicodeWordBoundaryError(());

impl UnicodeWordBoundaryError {
    #[cfg(not(feature = "unicode-word-boundary"))]
    pub(crate) fn new() -> UnicodeWordBoundaryError {
        UnicodeWordBoundaryError(())
    }

    /// Returns an error if and only if Unicode word boundary data is
    /// unavailable.
    pub fn check() -> Result<(), UnicodeWordBoundaryError> {
        is_word_char::check()
    }
}

#[cfg(feature = "std")]
impl std::error::Error for UnicodeWordBoundaryError {}

impl core::fmt::Display for UnicodeWordBoundaryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Unicode-aware \\b and \\B are unavailable because the \
             requisite data tables are missing, please enable the \
             unicode-word-boundary feature"
        )
    }
}

// Below are FOUR different ways for checking whether whether a "word"
// codepoint exists at a particular position in the haystack. The four
// different approaches are, in order of preference:
//
// 1. Parse '\w', convert to an NFA, convert to a fully compiled DFA on the
// first call, and then use that DFA for all subsequent calls.
// 2. Do UTF-8 decoding and use regex_syntax::is_word_character if available.
// 3. Do UTF-8 decoding and use our own 'perl_word' table.
// 4. Return an error.
//
// The reason for all of these approaches is a combination of perf and
// permitting one to build regex-automata without the Unicode data necessary
// for handling Unicode-aware word boundaries. (In which case, '(?-u:\b)' would
// still work.)
//
// The DFA approach is the fastest, but it requires the regex parser, the
// NFA compiler, the DFA builder and the DFA search runtime. That's a lot to
// bring in, but if it's available, it's (probably) the best we can do.
//
// Approaches (2) and (3) are effectively equivalent, but (2) reuses the
// data in regex-syntax and avoids duplicating it in regex-automata.
//
// Finally, (4) unconditionally returns an error since the requisite data isn't
// available anywhere.
//
// There are actually more approaches possible that we didn't implement. For
// example, if the DFA builder is available but the syntax parser is not, we
// could technically hand construct our own NFA from the 'perl_word' data
// table. But to avoid some pretty hairy code duplication, we would in turn
// need to pull the UTF-8 compiler out of the NFA compiler. Yikes.
//
// A possibly more sensible alternative is to use a lazy DFA when the full
// DFA builder isn't available...
//
// Yet another choice would be to build the full DFA and then embed it into the
// source. Then we'd only need to bring in the DFA search runtime, which is
// considerably smaller than the DFA builder code. The problem here is that the
// Debian people have spooked me[1] into avoiding cyclic dependencies. Namely,
// we'd need to build regex-cli, which depends on regex-automata in order to
// build some part of regex-automata. But to be honest, something like this has
// to be allowed somehow? I just don't know what the right process is.
//
// There are perhaps other choices as well. Why did I stop at these 4? Because
// I wanted to preserve my sanity. I suspect I'll wind up adding the lazy DFA
// approach eventually, as the benefits of the DFA approach are somewhat
// compelling. The 'boundary-words-holmes' benchmark tests this. (Note that
// the commands below no longer work. If necessary, we should re-capitulate
// the benchmark from whole cloth in rebar.)
//
//   $ regex-cli bench measure -f boundary-words-holmes -e pikevm > dfa.csv
//
// Then I changed the code below so that the util/unicode_data/perl_word table
// was used and re-ran the benchmark:
//
//   $ regex-cli bench measure -f boundary-words-holmes -e pikevm > table.csv
//
// And compared them:
//
//   $ regex-cli bench diff dfa.csv table.csv
//   benchmark                             engine                 dfa        table
//   ---------                             ------                 ---        -----
//   internal/count/boundary-words-holmes  regex/automata/pikevm  18.6 MB/s  12.9 MB/s
//
// Which is a nice improvement.
//
// UPDATE: It turns out that it takes approximately 22ms to build the reverse
// DFA for \w. (And about 3ms for the forward DFA.) It's probably not much in
// the grand scheme things, but that is a significant latency cost. So I'm not
// sure that's a good idea. I then tried using a lazy DFA instead, and that
// eliminated the overhead, but since the lazy DFA requires mutable working
// memory, that requires introducing a 'Cache' for every simultaneous call.
//
// I ended up deciding for now to just keep the "UTF-8 decode and check the
// table." The DFA and lazy DFA approaches are still below, but commented out.
//
// [1]: https://github.com/BurntSushi/ucd-generate/issues/11

/*
/// A module that looks for word codepoints using lazy DFAs.
#[cfg(all(
    feature = "unicode-word-boundary",
    feature = "syntax",
    feature = "unicode-perl",
    feature = "hybrid"
))]
mod is_word_char {
    use alloc::vec::Vec;

    use crate::{
        hybrid::dfa::{Cache, DFA},
        nfa::thompson::NFA,
        util::{lazy::Lazy, pool::Pool, primitives::StateID},
        Anchored, Input,
    };

    pub(super) fn check() -> Result<(), super::UnicodeWordBoundaryError> {
        Ok(())
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(super) fn fwd(
        haystack: &[u8],
        mut at: usize,
    ) -> Result<bool, super::UnicodeWordBoundaryError> {
        static WORD: Lazy<DFA> = Lazy::new(|| DFA::new(r"\w").unwrap());
        static CACHE: Lazy<Pool<Cache>> =
            Lazy::new(|| Pool::new(|| WORD.create_cache()));
        let dfa = Lazy::get(&WORD);
        let mut cache = Lazy::get(&CACHE).get();
        let mut sid = dfa
            .start_state_forward(
                &mut cache,
                &Input::new("").anchored(Anchored::Yes),
            )
            .unwrap();
        while at < haystack.len() {
            let byte = haystack[at];
            sid = dfa.next_state(&mut cache, sid, byte).unwrap();
            at += 1;
            if sid.is_tagged() {
                if sid.is_match() {
                    return Ok(true);
                } else if sid.is_dead() {
                    return Ok(false);
                }
            }
        }
        Ok(dfa.next_eoi_state(&mut cache, sid).unwrap().is_match())
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(super) fn rev(
        haystack: &[u8],
        mut at: usize,
    ) -> Result<bool, super::UnicodeWordBoundaryError> {
        static WORD: Lazy<DFA> = Lazy::new(|| {
            DFA::builder()
                .thompson(NFA::config().reverse(true))
                .build(r"\w")
                .unwrap()
        });
        static CACHE: Lazy<Pool<Cache>> =
            Lazy::new(|| Pool::new(|| WORD.create_cache()));
        let dfa = Lazy::get(&WORD);
        let mut cache = Lazy::get(&CACHE).get();
        let mut sid = dfa
            .start_state_reverse(
                &mut cache,
                &Input::new("").anchored(Anchored::Yes),
            )
            .unwrap();
        while at > 0 {
            at -= 1;
            let byte = haystack[at];
            sid = dfa.next_state(&mut cache, sid, byte).unwrap();
            if sid.is_tagged() {
                if sid.is_match() {
                    return Ok(true);
                } else if sid.is_dead() {
                    return Ok(false);
                }
            }
        }
        Ok(dfa.next_eoi_state(&mut cache, sid).unwrap().is_match())
    }
}
*/

/*
/// A module that looks for word codepoints using fully compiled DFAs.
#[cfg(all(
    feature = "unicode-word-boundary",
    feature = "syntax",
    feature = "unicode-perl",
    feature = "dfa-build"
))]
mod is_word_char {
    use alloc::vec::Vec;

    use crate::{
        dfa::{dense::DFA, Automaton, StartKind},
        nfa::thompson::NFA,
        util::{lazy::Lazy, primitives::StateID},
        Anchored, Input,
    };

    pub(super) fn check() -> Result<(), super::UnicodeWordBoundaryError> {
        Ok(())
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(super) fn fwd(
        haystack: &[u8],
        mut at: usize,
    ) -> Result<bool, super::UnicodeWordBoundaryError> {
        static WORD: Lazy<(DFA<Vec<u32>>, StateID)> = Lazy::new(|| {
            let dfa = DFA::builder()
                .configure(DFA::config().start_kind(StartKind::Anchored))
                .build(r"\w")
                .unwrap();
            // OK because our regex has no look-around.
            let start_id = dfa.universal_start_state(Anchored::Yes).unwrap();
            (dfa, start_id)
        });
        let &(ref dfa, mut sid) = Lazy::get(&WORD);
        while at < haystack.len() {
            let byte = haystack[at];
            sid = dfa.next_state(sid, byte);
            at += 1;
            if dfa.is_special_state(sid) {
                if dfa.is_match_state(sid) {
                    return Ok(true);
                } else if dfa.is_dead_state(sid) {
                    return Ok(false);
                }
            }
        }
        Ok(dfa.is_match_state(dfa.next_eoi_state(sid)))
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(super) fn rev(
        haystack: &[u8],
        mut at: usize,
    ) -> Result<bool, super::UnicodeWordBoundaryError> {
        static WORD: Lazy<(DFA<Vec<u32>>, StateID)> = Lazy::new(|| {
            let dfa = DFA::builder()
                .configure(DFA::config().start_kind(StartKind::Anchored))
                // From ad hoc measurements, it looks like setting
                // shrink==false is slightly faster than shrink==true. I kind
                // of feel like this indicates that shrinking is probably a
                // failure, although it can help in some cases. Sigh.
                .thompson(NFA::config().reverse(true).shrink(false))
                .build(r"\w")
                .unwrap();
            // OK because our regex has no look-around.
            let start_id = dfa.universal_start_state(Anchored::Yes).unwrap();
            (dfa, start_id)
        });
        let &(ref dfa, mut sid) = Lazy::get(&WORD);
        while at > 0 {
            at -= 1;
            let byte = haystack[at];
            sid = dfa.next_state(sid, byte);
            if dfa.is_special_state(sid) {
                if dfa.is_match_state(sid) {
                    return Ok(true);
                } else if dfa.is_dead_state(sid) {
                    return Ok(false);
                }
            }
        }
        Ok(dfa.is_match_state(dfa.next_eoi_state(sid)))
    }
}
*/

/// A module that looks for word codepoints using regex-syntax's data tables.
#[cfg(all(
    feature = "unicode-word-boundary",
    feature = "syntax",
    feature = "unicode-perl",
))]
mod is_word_char {
    use regex_syntax::try_is_word_character;

    use crate::util::utf8;

    pub(super) fn check() -> Result<(), super::UnicodeWordBoundaryError> {
        Ok(())
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(super) fn fwd(
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, super::UnicodeWordBoundaryError> {
        Ok(match utf8::decode(&haystack[at..]) {
            None | Some(Err(_)) => false,
            Some(Ok(ch)) => try_is_word_character(ch).expect(
                "since unicode-word-boundary, syntax and unicode-perl \
                 are all enabled, it is expected that \
                 try_is_word_character succeeds",
            ),
        })
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(super) fn rev(
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, super::UnicodeWordBoundaryError> {
        Ok(match utf8::decode_last(&haystack[..at]) {
            None | Some(Err(_)) => false,
            Some(Ok(ch)) => try_is_word_character(ch).expect(
                "since unicode-word-boundary, syntax and unicode-perl \
                 are all enabled, it is expected that \
                 try_is_word_character succeeds",
            ),
        })
    }
}

/// A module that looks for word codepoints using regex-automata's data tables
/// (which are only compiled when regex-syntax's tables aren't available).
///
/// Note that the cfg should match the one in src/util/unicode_data/mod.rs for
/// perl_word.
#[cfg(all(
    feature = "unicode-word-boundary",
    not(all(feature = "syntax", feature = "unicode-perl")),
))]
mod is_word_char {
    use crate::util::utf8;

    pub(super) fn check() -> Result<(), super::UnicodeWordBoundaryError> {
        Ok(())
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(super) fn fwd(
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, super::UnicodeWordBoundaryError> {
        Ok(match utf8::decode(&haystack[at..]) {
            None | Some(Err(_)) => false,
            Some(Ok(ch)) => is_word_character(ch),
        })
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(super) fn rev(
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, super::UnicodeWordBoundaryError> {
        Ok(match utf8::decode_last(&haystack[..at]) {
            None | Some(Err(_)) => false,
            Some(Ok(ch)) => is_word_character(ch),
        })
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn is_word_character(c: char) -> bool {
        use crate::util::{unicode_data::perl_word::PERL_WORD, utf8};

        if u8::try_from(c).map_or(false, utf8::is_word_byte) {
            return true;
        }
        PERL_WORD
            .binary_search_by(|&(start, end)| {
                use core::cmp::Ordering;

                if start <= c && c <= end {
                    Ordering::Equal
                } else if start > c {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            })
            .is_ok()
    }
}

/// A module that always returns an error if Unicode word boundaries are
/// disabled. When this feature is disabled, then regex-automata will not
/// include its own data tables even if regex-syntax is disabled.
#[cfg(not(feature = "unicode-word-boundary"))]
mod is_word_char {
    pub(super) fn check() -> Result<(), super::UnicodeWordBoundaryError> {
        Err(super::UnicodeWordBoundaryError::new())
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(super) fn fwd(
        _bytes: &[u8],
        _at: usize,
    ) -> Result<bool, super::UnicodeWordBoundaryError> {
        Err(super::UnicodeWordBoundaryError::new())
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(super) fn rev(
        _bytes: &[u8],
        _at: usize,
    ) -> Result<bool, super::UnicodeWordBoundaryError> {
        Err(super::UnicodeWordBoundaryError::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! testlook {
        ($look:expr, $haystack:expr, $at:expr) => {
            LookMatcher::default().matches($look, $haystack.as_bytes(), $at)
        };
    }

    #[test]
    fn look_matches_start_line() {
        let look = Look::StartLF;

        assert!(testlook!(look, "", 0));
        assert!(testlook!(look, "\n", 0));
        assert!(testlook!(look, "\n", 1));
        assert!(testlook!(look, "a", 0));
        assert!(testlook!(look, "\na", 1));

        assert!(!testlook!(look, "a", 1));
        assert!(!testlook!(look, "a\na", 1));
    }

    #[test]
    fn look_matches_end_line() {
        let look = Look::EndLF;

        assert!(testlook!(look, "", 0));
        assert!(testlook!(look, "\n", 1));
        assert!(testlook!(look, "\na", 0));
        assert!(testlook!(look, "\na", 2));
        assert!(testlook!(look, "a\na", 1));

        assert!(!testlook!(look, "a", 0));
        assert!(!testlook!(look, "\na", 1));
        assert!(!testlook!(look, "a\na", 0));
        assert!(!testlook!(look, "a\na", 2));
    }

    #[test]
    fn look_matches_start_text() {
        let look = Look::Start;

        assert!(testlook!(look, "", 0));
        assert!(testlook!(look, "\n", 0));
        assert!(testlook!(look, "a", 0));

        assert!(!testlook!(look, "\n", 1));
        assert!(!testlook!(look, "\na", 1));
        assert!(!testlook!(look, "a", 1));
        assert!(!testlook!(look, "a\na", 1));
    }

    #[test]
    fn look_matches_end_text() {
        let look = Look::End;

        assert!(testlook!(look, "", 0));
        assert!(testlook!(look, "\n", 1));
        assert!(testlook!(look, "\na", 2));

        assert!(!testlook!(look, "\na", 0));
        assert!(!testlook!(look, "a\na", 1));
        assert!(!testlook!(look, "a", 0));
        assert!(!testlook!(look, "\na", 1));
        assert!(!testlook!(look, "a\na", 0));
        assert!(!testlook!(look, "a\na", 2));
    }

    #[test]
    #[cfg(all(not(miri), feature = "unicode-word-boundary"))]
    fn look_matches_word_unicode() {
        let look = Look::WordUnicode;

        // \xF0\x9D\x9B\x83 = 𝛃 (in \w)
        // \xF0\x90\x86\x80 = 𐆀 (not in \w)

        // Simple ASCII word boundaries.
        assert!(testlook!(look, "a", 0));
        assert!(testlook!(look, "a", 1));
        assert!(testlook!(look, "a ", 1));
        assert!(testlook!(look, " a ", 1));
        assert!(testlook!(look, " a ", 2));

        // Unicode word boundaries with a non-ASCII codepoint.
        assert!(testlook!(look, "𝛃", 0));
        assert!(testlook!(look, "𝛃", 4));
        assert!(testlook!(look, "𝛃 ", 4));
        assert!(testlook!(look, " 𝛃 ", 1));
        assert!(testlook!(look, " 𝛃 ", 5));

        // Unicode word boundaries between non-ASCII codepoints.
        assert!(testlook!(look, "𝛃𐆀", 0));
        assert!(testlook!(look, "𝛃𐆀", 4));

        // Non word boundaries for ASCII.
        assert!(!testlook!(look, "", 0));
        assert!(!testlook!(look, "ab", 1));
        assert!(!testlook!(look, "a ", 2));
        assert!(!testlook!(look, " a ", 0));
        assert!(!testlook!(look, " a ", 3));

        // Non word boundaries with a non-ASCII codepoint.
        assert!(!testlook!(look, "𝛃b", 4));
        assert!(!testlook!(look, "𝛃 ", 5));
        assert!(!testlook!(look, " 𝛃 ", 0));
        assert!(!testlook!(look, " 𝛃 ", 6));
        assert!(!testlook!(look, "𝛃", 1));
        assert!(!testlook!(look, "𝛃", 2));
        assert!(!testlook!(look, "𝛃", 3));

        // Non word boundaries with non-ASCII codepoints.
        assert!(!testlook!(look, "𝛃𐆀", 1));
        assert!(!testlook!(look, "𝛃𐆀", 2));
        assert!(!testlook!(look, "𝛃𐆀", 3));
        assert!(!testlook!(look, "𝛃𐆀", 5));
        assert!(!testlook!(look, "𝛃𐆀", 6));
        assert!(!testlook!(look, "𝛃𐆀", 7));
        assert!(!testlook!(look, "𝛃𐆀", 8));
    }

    #[test]
    fn look_matches_word_ascii() {
        let look = Look::WordAscii;

        // \xF0\x9D\x9B\x83 = 𝛃 (in \w)
        // \xF0\x90\x86\x80 = 𐆀 (not in \w)

        // Simple ASCII word boundaries.
        assert!(testlook!(look, "a", 0));
        assert!(testlook!(look, "a", 1));
        assert!(testlook!(look, "a ", 1));
        assert!(testlook!(look, " a ", 1));
        assert!(testlook!(look, " a ", 2));

        // Unicode word boundaries with a non-ASCII codepoint. Since this is
        // an ASCII word boundary, none of these match.
        assert!(!testlook!(look, "𝛃", 0));
        assert!(!testlook!(look, "𝛃", 4));
        assert!(!testlook!(look, "𝛃 ", 4));
        assert!(!testlook!(look, " 𝛃 ", 1));
        assert!(!testlook!(look, " 𝛃 ", 5));

        // Unicode word boundaries between non-ASCII codepoints. Again, since
        // this is an ASCII word boundary, none of these match.
        assert!(!testlook!(look, "𝛃𐆀", 0));
        assert!(!testlook!(look, "𝛃𐆀", 4));

        // Non word boundaries for ASCII.
        assert!(!testlook!(look, "", 0));
        assert!(!testlook!(look, "ab", 1));
        assert!(!testlook!(look, "a ", 2));
        assert!(!testlook!(look, " a ", 0));
        assert!(!testlook!(look, " a ", 3));

        // Non word boundaries with a non-ASCII codepoint.
        assert!(testlook!(look, "𝛃b", 4));
        assert!(!testlook!(look, "𝛃 ", 5));
        assert!(!testlook!(look, " 𝛃 ", 0));
        assert!(!testlook!(look, " 𝛃 ", 6));
        assert!(!testlook!(look, "𝛃", 1));
        assert!(!testlook!(look, "𝛃", 2));
        assert!(!testlook!(look, "𝛃", 3));

        // Non word boundaries with non-ASCII codepoints.
        assert!(!testlook!(look, "𝛃𐆀", 1));
        assert!(!testlook!(look, "𝛃𐆀", 2));
        assert!(!testlook!(look, "𝛃𐆀", 3));
        assert!(!testlook!(look, "𝛃𐆀", 5));
        assert!(!testlook!(look, "𝛃𐆀", 6));
        assert!(!testlook!(look, "𝛃𐆀", 7));
        assert!(!testlook!(look, "𝛃𐆀", 8));
    }

    #[test]
    #[cfg(all(not(miri), feature = "unicode-word-boundary"))]
    fn look_matches_word_unicode_negate() {
        let look = Look::WordUnicodeNegate;

        // \xF0\x9D\x9B\x83 = 𝛃 (in \w)
        // \xF0\x90\x86\x80 = 𐆀 (not in \w)

        // Simple ASCII word boundaries.
        assert!(!testlook!(look, "a", 0));
        assert!(!testlook!(look, "a", 1));
        assert!(!testlook!(look, "a ", 1));
        assert!(!testlook!(look, " a ", 1));
        assert!(!testlook!(look, " a ", 2));

        // Unicode word boundaries with a non-ASCII codepoint.
        assert!(!testlook!(look, "𝛃", 0));
        assert!(!testlook!(look, "𝛃", 4));
        assert!(!testlook!(look, "𝛃 ", 4));
        assert!(!testlook!(look, " 𝛃 ", 1));
        assert!(!testlook!(look, " 𝛃 ", 5));

        // Unicode word boundaries between non-ASCII codepoints.
        assert!(!testlook!(look, "𝛃𐆀", 0));
        assert!(!testlook!(look, "𝛃𐆀", 4));

        // Non word boundaries for ASCII.
        assert!(testlook!(look, "", 0));
        assert!(testlook!(look, "ab", 1));
        assert!(testlook!(look, "a ", 2));
        assert!(testlook!(look, " a ", 0));
        assert!(testlook!(look, " a ", 3));

        // Non word boundaries with a non-ASCII codepoint.
        assert!(testlook!(look, "𝛃b", 4));
        assert!(testlook!(look, "𝛃 ", 5));
        assert!(testlook!(look, " 𝛃 ", 0));
        assert!(testlook!(look, " 𝛃 ", 6));
        // These don't match because they could otherwise return an offset that
        // splits the UTF-8 encoding of a codepoint.
        assert!(!testlook!(look, "𝛃", 1));
        assert!(!testlook!(look, "𝛃", 2));
        assert!(!testlook!(look, "𝛃", 3));

        // Non word boundaries with non-ASCII codepoints. These also don't
        // match because they could otherwise return an offset that splits the
        // UTF-8 encoding of a codepoint.
        assert!(!testlook!(look, "𝛃𐆀", 1));
        assert!(!testlook!(look, "𝛃𐆀", 2));
        assert!(!testlook!(look, "𝛃𐆀", 3));
        assert!(!testlook!(look, "𝛃𐆀", 5));
        assert!(!testlook!(look, "𝛃𐆀", 6));
        assert!(!testlook!(look, "𝛃𐆀", 7));
        // But this one does, since 𐆀 isn't a word codepoint, and 8 is the end
        // of the haystack. So the "end" of the haystack isn't a word and 𐆀
        // isn't a word, thus, \B matches.
        assert!(testlook!(look, "𝛃𐆀", 8));
    }

    #[test]
    fn look_matches_word_ascii_negate() {
        let look = Look::WordAsciiNegate;

        // \xF0\x9D\x9B\x83 = 𝛃 (in \w)
        // \xF0\x90\x86\x80 = 𐆀 (not in \w)

        // Simple ASCII word boundaries.
        assert!(!testlook!(look, "a", 0));
        assert!(!testlook!(look, "a", 1));
        assert!(!testlook!(look, "a ", 1));
        assert!(!testlook!(look, " a ", 1));
        assert!(!testlook!(look, " a ", 2));

        // Unicode word boundaries with a non-ASCII codepoint. Since this is
        // an ASCII word boundary, none of these match.
        assert!(testlook!(look, "𝛃", 0));
        assert!(testlook!(look, "𝛃", 4));
        assert!(testlook!(look, "𝛃 ", 4));
        assert!(testlook!(look, " 𝛃 ", 1));
        assert!(testlook!(look, " 𝛃 ", 5));

        // Unicode word boundaries between non-ASCII codepoints. Again, since
        // this is an ASCII word boundary, none of these match.
        assert!(testlook!(look, "𝛃𐆀", 0));
        assert!(testlook!(look, "𝛃𐆀", 4));

        // Non word boundaries for ASCII.
        assert!(testlook!(look, "", 0));
        assert!(testlook!(look, "ab", 1));
        assert!(testlook!(look, "a ", 2));
        assert!(testlook!(look, " a ", 0));
        assert!(testlook!(look, " a ", 3));

        // Non word boundaries with a non-ASCII codepoint.
        assert!(!testlook!(look, "𝛃b", 4));
        assert!(testlook!(look, "𝛃 ", 5));
        assert!(testlook!(look, " 𝛃 ", 0));
        assert!(testlook!(look, " 𝛃 ", 6));
        assert!(testlook!(look, "𝛃", 1));
        assert!(testlook!(look, "𝛃", 2));
        assert!(testlook!(look, "𝛃", 3));

        // Non word boundaries with non-ASCII codepoints.
        assert!(testlook!(look, "𝛃𐆀", 1));
        assert!(testlook!(look, "𝛃𐆀", 2));
        assert!(testlook!(look, "𝛃𐆀", 3));
        assert!(testlook!(look, "𝛃𐆀", 5));
        assert!(testlook!(look, "𝛃𐆀", 6));
        assert!(testlook!(look, "𝛃𐆀", 7));
        assert!(testlook!(look, "𝛃𐆀", 8));
    }

    #[test]
    fn look_matches_word_start_ascii() {
        let look = Look::WordStartAscii;

        // \xF0\x9D\x9B\x83 = 𝛃 (in \w)
        // \xF0\x90\x86\x80 = 𐆀 (not in \w)

        // Simple ASCII word boundaries.
        assert!(testlook!(look, "a", 0));
        assert!(!testlook!(look, "a", 1));
        assert!(!testlook!(look, "a ", 1));
        assert!(testlook!(look, " a ", 1));
        assert!(!testlook!(look, " a ", 2));

        // Unicode word boundaries with a non-ASCII codepoint. Since this is
        // an ASCII word boundary, none of these match.
        assert!(!testlook!(look, "𝛃", 0));
        assert!(!testlook!(look, "𝛃", 4));
        assert!(!testlook!(look, "𝛃 ", 4));
        assert!(!testlook!(look, " 𝛃 ", 1));
        assert!(!testlook!(look, " 𝛃 ", 5));

        // Unicode word boundaries between non-ASCII codepoints. Again, since
        // this is an ASCII word boundary, none of these match.
        assert!(!testlook!(look, "𝛃𐆀", 0));
        assert!(!testlook!(look, "𝛃𐆀", 4));

        // Non word boundaries for ASCII.
        assert!(!testlook!(look, "", 0));
        assert!(!testlook!(look, "ab", 1));
        assert!(!testlook!(look, "a ", 2));
        assert!(!testlook!(look, " a ", 0));
        assert!(!testlook!(look, " a ", 3));

        // Non word boundaries with a non-ASCII codepoint.
        assert!(testlook!(look, "𝛃b", 4));
        assert!(!testlook!(look, "b𝛃", 1));
        assert!(!testlook!(look, "𝛃 ", 5));
        assert!(!testlook!(look, " 𝛃 ", 0));
        assert!(!testlook!(look, " 𝛃 ", 6));
        assert!(!testlook!(look, "𝛃", 1));
        assert!(!testlook!(look, "𝛃", 2));
        assert!(!testlook!(look, "𝛃", 3));

        // Non word boundaries with non-ASCII codepoints.
        assert!(!testlook!(look, "𝛃𐆀", 1));
        assert!(!testlook!(look, "𝛃𐆀", 2));
        assert!(!testlook!(look, "𝛃𐆀", 3));
        assert!(!testlook!(look, "𝛃𐆀", 5));
        assert!(!testlook!(look, "𝛃𐆀", 6));
        assert!(!testlook!(look, "𝛃𐆀", 7));
        assert!(!testlook!(look, "𝛃𐆀", 8));
    }

    #[test]
    fn look_matches_word_end_ascii() {
        let look = Look::WordEndAscii;

        // \xF0\x9D\x9B\x83 = 𝛃 (in \w)
        // \xF0\x90\x86\x80 = 𐆀 (not in \w)

        // Simple ASCII word boundaries.
        assert!(!testlook!(look, "a", 0));
        assert!(testlook!(look, "a", 1));
        assert!(testlook!(look, "a ", 1));
        assert!(!testlook!(look, " a ", 1));
        assert!(testlook!(look, " a ", 2));

        // Unicode word boundaries with a non-ASCII codepoint. Since this is
        // an ASCII word boundary, none of these match.
        assert!(!testlook!(look, "𝛃", 0));
        assert!(!testlook!(look, "𝛃", 4));
        assert!(!testlook!(look, "𝛃 ", 4));
        assert!(!testlook!(look, " 𝛃 ", 1));
        assert!(!testlook!(look, " 𝛃 ", 5));

        // Unicode word boundaries between non-ASCII codepoints. Again, since
        // this is an ASCII word boundary, none of these match.
        assert!(!testlook!(look, "𝛃𐆀", 0));
        assert!(!testlook!(look, "𝛃𐆀", 4));

        // Non word boundaries for ASCII.
        assert!(!testlook!(look, "", 0));
        assert!(!testlook!(look, "ab", 1));
        assert!(!testlook!(look, "a ", 2));
        assert!(!testlook!(look, " a ", 0));
        assert!(!testlook!(look, " a ", 3));

        // Non word boundaries with a non-ASCII codepoint.
        assert!(!testlook!(look, "𝛃b", 4));
        assert!(testlook!(look, "b𝛃", 1));
        assert!(!testlook!(look, "𝛃 ", 5));
        assert!(!testlook!(look, " 𝛃 ", 0));
        assert!(!testlook!(look, " 𝛃 ", 6));
        assert!(!testlook!(look, "𝛃", 1));
        assert!(!testlook!(look, "𝛃", 2));
        assert!(!testlook!(look, "𝛃", 3));

        // Non word boundaries with non-ASCII codepoints.
        assert!(!testlook!(look, "𝛃𐆀", 1));
        assert!(!testlook!(look, "𝛃𐆀", 2));
        assert!(!testlook!(look, "𝛃𐆀", 3));
        assert!(!testlook!(look, "𝛃𐆀", 5));
        assert!(!testlook!(look, "𝛃𐆀", 6));
        assert!(!testlook!(look, "𝛃𐆀", 7));
        assert!(!testlook!(look, "𝛃𐆀", 8));
    }

    #[test]
    #[cfg(all(not(miri), feature = "unicode-word-boundary"))]
    fn look_matches_word_start_unicode() {
        let look = Look::WordStartUnicode;

        // \xF0\x9D\x9B\x83 = 𝛃 (in \w)
        // \xF0\x90\x86\x80 = 𐆀 (not in \w)

        // Simple ASCII word boundaries.
        assert!(testlook!(look, "a", 0));
        assert!(!testlook!(look, "a", 1));
        assert!(!testlook!(look, "a ", 1));
        assert!(testlook!(look, " a ", 1));
        assert!(!testlook!(look, " a ", 2));

        // Unicode word boundaries with a non-ASCII codepoint.
        assert!(testlook!(look, "𝛃", 0));
        assert!(!testlook!(look, "𝛃", 4));
        assert!(!testlook!(look, "𝛃 ", 4));
        assert!(testlook!(look, " 𝛃 ", 1));
        assert!(!testlook!(look, " 𝛃 ", 5));

        // Unicode word boundaries between non-ASCII codepoints.
        assert!(testlook!(look, "𝛃𐆀", 0));
        assert!(!testlook!(look, "𝛃𐆀", 4));

        // Non word boundaries for ASCII.
        assert!(!testlook!(look, "", 0));
        assert!(!testlook!(look, "ab", 1));
        assert!(!testlook!(look, "a ", 2));
        assert!(!testlook!(look, " a ", 0));
        assert!(!testlook!(look, " a ", 3));

        // Non word boundaries with a non-ASCII codepoint.
        assert!(!testlook!(look, "𝛃b", 4));
        assert!(!testlook!(look, "b𝛃", 1));
        assert!(!testlook!(look, "𝛃 ", 5));
        assert!(!testlook!(look, " 𝛃 ", 0));
        assert!(!testlook!(look, " 𝛃 ", 6));
        assert!(!testlook!(look, "𝛃", 1));
        assert!(!testlook!(look, "𝛃", 2));
        assert!(!testlook!(look, "𝛃", 3));

        // Non word boundaries with non-ASCII codepoints.
        assert!(!testlook!(look, "𝛃𐆀", 1));
        assert!(!testlook!(look, "𝛃𐆀", 2));
        assert!(!testlook!(look, "𝛃𐆀", 3));
        assert!(!testlook!(look, "𝛃𐆀", 5));
        assert!(!testlook!(look, "𝛃𐆀", 6));
        assert!(!testlook!(look, "𝛃𐆀", 7));
        assert!(!testlook!(look, "𝛃𐆀", 8));
    }

    #[test]
    #[cfg(all(not(miri), feature = "unicode-word-boundary"))]
    fn look_matches_word_end_unicode() {
        let look = Look::WordEndUnicode;

        // \xF0\x9D\x9B\x83 = 𝛃 (in \w)
        // \xF0\x90\x86\x80 = 𐆀 (not in \w)

        // Simple ASCII word boundaries.
        assert!(!testlook!(look, "a", 0));
        assert!(testlook!(look, "a", 1));
        assert!(testlook!(look, "a ", 1));
        assert!(!testlook!(look, " a ", 1));
        assert!(testlook!(look, " a ", 2));

        // Unicode word boundaries with a non-ASCII codepoint.
        assert!(!testlook!(look, "𝛃", 0));
        assert!(testlook!(look, "𝛃", 4));
        assert!(testlook!(look, "𝛃 ", 4));
        assert!(!testlook!(look, " 𝛃 ", 1));
        assert!(testlook!(look, " 𝛃 ", 5));

        // Unicode word boundaries between non-ASCII codepoints.
        assert!(!testlook!(look, "𝛃𐆀", 0));
        assert!(testlook!(look, "𝛃𐆀", 4));

        // Non word boundaries for ASCII.
        assert!(!testlook!(look, "", 0));
        assert!(!testlook!(look, "ab", 1));
        assert!(!testlook!(look, "a ", 2));
        assert!(!testlook!(look, " a ", 0));
        assert!(!testlook!(look, " a ", 3));

        // Non word boundaries with a non-ASCII codepoint.
        assert!(!testlook!(look, "𝛃b", 4));
        assert!(!testlook!(look, "b𝛃", 1));
        assert!(!testlook!(look, "𝛃 ", 5));
        assert!(!testlook!(look, " 𝛃 ", 0));
        assert!(!testlook!(look, " 𝛃 ", 6));
        assert!(!testlook!(look, "𝛃", 1));
        assert!(!testlook!(look, "𝛃", 2));
        assert!(!testlook!(look, "𝛃", 3));

        // Non word boundaries with non-ASCII codepoints.
        assert!(!testlook!(look, "𝛃𐆀", 1));
        assert!(!testlook!(look, "𝛃𐆀", 2));
        assert!(!testlook!(look, "𝛃𐆀", 3));
        assert!(!testlook!(look, "𝛃𐆀", 5));
        assert!(!testlook!(look, "𝛃𐆀", 6));
        assert!(!testlook!(look, "𝛃𐆀", 7));
        assert!(!testlook!(look, "𝛃𐆀", 8));
    }

    #[test]
    fn look_matches_word_start_half_ascii() {
        let look = Look::WordStartHalfAscii;

        // \xF0\x9D\x9B\x83 = 𝛃 (in \w)
        // \xF0\x90\x86\x80 = 𐆀 (not in \w)

        // Simple ASCII word boundaries.
        assert!(testlook!(look, "a", 0));
        assert!(!testlook!(look, "a", 1));
        assert!(!testlook!(look, "a ", 1));
        assert!(testlook!(look, " a ", 1));
        assert!(!testlook!(look, " a ", 2));

        // Unicode word boundaries with a non-ASCII codepoint. Since this is
        // an ASCII word boundary, none of these match.
        assert!(testlook!(look, "𝛃", 0));
        assert!(testlook!(look, "𝛃", 4));
        assert!(testlook!(look, "𝛃 ", 4));
        assert!(testlook!(look, " 𝛃 ", 1));
        assert!(testlook!(look, " 𝛃 ", 5));

        // Unicode word boundaries between non-ASCII codepoints. Again, since
        // this is an ASCII word boundary, none of these match.
        assert!(testlook!(look, "𝛃𐆀", 0));
        assert!(testlook!(look, "𝛃𐆀", 4));

        // Non word boundaries for ASCII.
        assert!(testlook!(look, "", 0));
        assert!(!testlook!(look, "ab", 1));
        assert!(testlook!(look, "a ", 2));
        assert!(testlook!(look, " a ", 0));
        assert!(testlook!(look, " a ", 3));

        // Non word boundaries with a non-ASCII codepoint.
        assert!(testlook!(look, "𝛃b", 4));
        assert!(!testlook!(look, "b𝛃", 1));
        assert!(testlook!(look, "𝛃 ", 5));
        assert!(testlook!(look, " 𝛃 ", 0));
        assert!(testlook!(look, " 𝛃 ", 6));
        assert!(testlook!(look, "𝛃", 1));
        assert!(testlook!(look, "𝛃", 2));
        assert!(testlook!(look, "𝛃", 3));

        // Non word boundaries with non-ASCII codepoints.
        assert!(testlook!(look, "𝛃𐆀", 1));
        assert!(testlook!(look, "𝛃𐆀", 2));
        assert!(testlook!(look, "𝛃𐆀", 3));
        assert!(testlook!(look, "𝛃𐆀", 5));
        assert!(testlook!(look, "𝛃𐆀", 6));
        assert!(testlook!(look, "𝛃𐆀", 7));
        assert!(testlook!(look, "𝛃𐆀", 8));
    }

    #[test]
    fn look_matches_word_end_half_ascii() {
        let look = Look::WordEndHalfAscii;

        // \xF0\x9D\x9B\x83 = 𝛃 (in \w)
        // \xF0\x90\x86\x80 = 𐆀 (not in \w)

        // Simple ASCII word boundaries.
        assert!(!testlook!(look, "a", 0));
        assert!(testlook!(look, "a", 1));
        assert!(testlook!(look, "a ", 1));
        assert!(!testlook!(look, " a ", 1));
        assert!(testlook!(look, " a ", 2));

        // Unicode word boundaries with a non-ASCII codepoint. Since this is
        // an ASCII word boundary, none of these match.
        assert!(testlook!(look, "𝛃", 0));
        assert!(testlook!(look, "𝛃", 4));
        assert!(testlook!(look, "𝛃 ", 4));
        assert!(testlook!(look, " 𝛃 ", 1));
        assert!(testlook!(look, " 𝛃 ", 5));

        // Unicode word boundaries between non-ASCII codepoints. Again, since
        // this is an ASCII word boundary, none of these match.
        assert!(testlook!(look, "𝛃𐆀", 0));
        assert!(testlook!(look, "𝛃𐆀", 4));

        // Non word boundaries for ASCII.
        assert!(testlook!(look, "", 0));
        assert!(!testlook!(look, "ab", 1));
        assert!(testlook!(look, "a ", 2));
        assert!(testlook!(look, " a ", 0));
        assert!(testlook!(look, " a ", 3));

        // Non word boundaries with a non-ASCII codepoint.
        assert!(!testlook!(look, "𝛃b", 4));
        assert!(testlook!(look, "b𝛃", 1));
        assert!(testlook!(look, "𝛃 ", 5));
        assert!(testlook!(look, " 𝛃 ", 0));
        assert!(testlook!(look, " 𝛃 ", 6));
        assert!(testlook!(look, "𝛃", 1));
        assert!(testlook!(look, "𝛃", 2));
        assert!(testlook!(look, "𝛃", 3));

        // Non word boundaries with non-ASCII codepoints.
        assert!(testlook!(look, "𝛃𐆀", 1));
        assert!(testlook!(look, "𝛃𐆀", 2));
        assert!(testlook!(look, "𝛃𐆀", 3));
        assert!(testlook!(look, "𝛃𐆀", 5));
        assert!(testlook!(look, "𝛃𐆀", 6));
        assert!(testlook!(look, "𝛃𐆀", 7));
        assert!(testlook!(look, "𝛃𐆀", 8));
    }

    #[test]
    #[cfg(all(not(miri), feature = "unicode-word-boundary"))]
    fn look_matches_word_start_half_unicode() {
        let look = Look::WordStartHalfUnicode;

        // \xF0\x9D\x9B\x83 = 𝛃 (in \w)
        // \xF0\x90\x86\x80 = 𐆀 (not in \w)

        // Simple ASCII word boundaries.
        assert!(testlook!(look, "a", 0));
        assert!(!testlook!(look, "a", 1));
        assert!(!testlook!(look, "a ", 1));
        assert!(testlook!(look, " a ", 1));
        assert!(!testlook!(look, " a ", 2));

        // Unicode word boundaries with a non-ASCII codepoint.
        assert!(testlook!(look, "𝛃", 0));
        assert!(!testlook!(look, "𝛃", 4));
        assert!(!testlook!(look, "𝛃 ", 4));
        assert!(testlook!(look, " 𝛃 ", 1));
        assert!(!testlook!(look, " 𝛃 ", 5));

        // Unicode word boundaries between non-ASCII codepoints.
        assert!(testlook!(look, "𝛃𐆀", 0));
        assert!(!testlook!(look, "𝛃𐆀", 4));

        // Non word boundaries for ASCII.
        assert!(testlook!(look, "", 0));
        assert!(!testlook!(look, "ab", 1));
        assert!(testlook!(look, "a ", 2));
        assert!(testlook!(look, " a ", 0));
        assert!(testlook!(look, " a ", 3));

        // Non word boundaries with a non-ASCII codepoint.
        assert!(!testlook!(look, "𝛃b", 4));
        assert!(!testlook!(look, "b𝛃", 1));
        assert!(testlook!(look, "𝛃 ", 5));
        assert!(testlook!(look, " 𝛃 ", 0));
        assert!(testlook!(look, " 𝛃 ", 6));
        assert!(!testlook!(look, "𝛃", 1));
        assert!(!testlook!(look, "𝛃", 2));
        assert!(!testlook!(look, "𝛃", 3));

        // Non word boundaries with non-ASCII codepoints.
        assert!(!testlook!(look, "𝛃𐆀", 1));
        assert!(!testlook!(look, "𝛃𐆀", 2));
        assert!(!testlook!(look, "𝛃𐆀", 3));
        assert!(!testlook!(look, "𝛃𐆀", 5));
        assert!(!testlook!(look, "𝛃𐆀", 6));
        assert!(!testlook!(look, "𝛃𐆀", 7));
        assert!(testlook!(look, "𝛃𐆀", 8));
    }

    #[test]
    #[cfg(all(not(miri), feature = "unicode-word-boundary"))]
    fn look_matches_word_end_half_unicode() {
        let look = Look::WordEndHalfUnicode;

        // \xF0\x9D\x9B\x83 = 𝛃 (in \w)
        // \xF0\x90\x86\x80 = 𐆀 (not in \w)

        // Simple ASCII word boundaries.
        assert!(!testlook!(look, "a", 0));
        assert!(testlook!(look, "a", 1));
        assert!(testlook!(look, "a ", 1));
        assert!(!testlook!(look, " a ", 1));
        assert!(testlook!(look, " a ", 2));

        // Unicode word boundaries with a non-ASCII codepoint.
        assert!(!testlook!(look, "𝛃", 0));
        assert!(testlook!(look, "𝛃", 4));
        assert!(testlook!(look, "𝛃 ", 4));
        assert!(!testlook!(look, " 𝛃 ", 1));
        assert!(testlook!(look, " 𝛃 ", 5));

        // Unicode word boundaries between non-ASCII codepoints.
        assert!(!testlook!(look, "𝛃𐆀", 0));
        assert!(testlook!(look, "𝛃𐆀", 4));

        // Non word boundaries for ASCII.
        assert!(testlook!(look, "", 0));
        assert!(!testlook!(look, "ab", 1));
        assert!(testlook!(look, "a ", 2));
        assert!(testlook!(look, " a ", 0));
        assert!(testlook!(look, " a ", 3));

        // Non word boundaries with a non-ASCII codepoint.
        assert!(!testlook!(look, "𝛃b", 4));
        assert!(!testlook!(look, "b𝛃", 1));
        assert!(testlook!(look, "𝛃 ", 5));
        assert!(testlook!(look, " 𝛃 ", 0));
        assert!(testlook!(look, " 𝛃 ", 6));
        assert!(!testlook!(look, "𝛃", 1));
        assert!(!testlook!(look, "𝛃", 2));
        assert!(!testlook!(look, "𝛃", 3));

        // Non word boundaries with non-ASCII codepoints.
        assert!(!testlook!(look, "𝛃𐆀", 1));
        assert!(!testlook!(look, "𝛃𐆀", 2));
        assert!(!testlook!(look, "𝛃𐆀", 3));
        assert!(!testlook!(look, "𝛃𐆀", 5));
        assert!(!testlook!(look, "𝛃𐆀", 6));
        assert!(!testlook!(look, "𝛃𐆀", 7));
        assert!(testlook!(look, "𝛃𐆀", 8));
    }

    #[test]
    fn look_set() {
        let mut f = LookSet::default();
        assert!(!f.contains(Look::Start));
        assert!(!f.contains(Look::End));
        assert!(!f.contains(Look::StartLF));
        assert!(!f.contains(Look::EndLF));
        assert!(!f.contains(Look::WordUnicode));
        assert!(!f.contains(Look::WordUnicodeNegate));
        assert!(!f.contains(Look::WordAscii));
        assert!(!f.contains(Look::WordAsciiNegate));

        f = f.insert(Look::Start);
        assert!(f.contains(Look::Start));
        f = f.remove(Look::Start);
        assert!(!f.contains(Look::Start));

        f = f.insert(Look::End);
        assert!(f.contains(Look::End));
        f = f.remove(Look::End);
        assert!(!f.contains(Look::End));

        f = f.insert(Look::StartLF);
        assert!(f.contains(Look::StartLF));
        f = f.remove(Look::StartLF);
        assert!(!f.contains(Look::StartLF));

        f = f.insert(Look::EndLF);
        assert!(f.contains(Look::EndLF));
        f = f.remove(Look::EndLF);
        assert!(!f.contains(Look::EndLF));

        f = f.insert(Look::StartCRLF);
        assert!(f.contains(Look::StartCRLF));
        f = f.remove(Look::StartCRLF);
        assert!(!f.contains(Look::StartCRLF));

        f = f.insert(Look::EndCRLF);
        assert!(f.contains(Look::EndCRLF));
        f = f.remove(Look::EndCRLF);
        assert!(!f.contains(Look::EndCRLF));

        f = f.insert(Look::WordUnicode);
        assert!(f.contains(Look::WordUnicode));
        f = f.remove(Look::WordUnicode);
        assert!(!f.contains(Look::WordUnicode));

        f = f.insert(Look::WordUnicodeNegate);
        assert!(f.contains(Look::WordUnicodeNegate));
        f = f.remove(Look::WordUnicodeNegate);
        assert!(!f.contains(Look::WordUnicodeNegate));

        f = f.insert(Look::WordAscii);
        assert!(f.contains(Look::WordAscii));
        f = f.remove(Look::WordAscii);
        assert!(!f.contains(Look::WordAscii));

        f = f.insert(Look::WordAsciiNegate);
        assert!(f.contains(Look::WordAsciiNegate));
        f = f.remove(Look::WordAsciiNegate);
        assert!(!f.contains(Look::WordAsciiNegate));

        f = f.insert(Look::WordStartAscii);
        assert!(f.contains(Look::WordStartAscii));
        f = f.remove(Look::WordStartAscii);
        assert!(!f.contains(Look::WordStartAscii));

        f = f.insert(Look::WordEndAscii);
        assert!(f.contains(Look::WordEndAscii));
        f = f.remove(Look::WordEndAscii);
        assert!(!f.contains(Look::WordEndAscii));

        f = f.insert(Look::WordStartUnicode);
        assert!(f.contains(Look::WordStartUnicode));
        f = f.remove(Look::WordStartUnicode);
        assert!(!f.contains(Look::WordStartUnicode));

        f = f.insert(Look::WordEndUnicode);
        assert!(f.contains(Look::WordEndUnicode));
        f = f.remove(Look::WordEndUnicode);
        assert!(!f.contains(Look::WordEndUnicode));

        f = f.insert(Look::WordStartHalfAscii);
        assert!(f.contains(Look::WordStartHalfAscii));
        f = f.remove(Look::WordStartHalfAscii);
        assert!(!f.contains(Look::WordStartHalfAscii));

        f = f.insert(Look::WordEndHalfAscii);
        assert!(f.contains(Look::WordEndHalfAscii));
        f = f.remove(Look::WordEndHalfAscii);
        assert!(!f.contains(Look::WordEndHalfAscii));

        f = f.insert(Look::WordStartHalfUnicode);
        assert!(f.contains(Look::WordStartHalfUnicode));
        f = f.remove(Look::WordStartHalfUnicode);
        assert!(!f.contains(Look::WordStartHalfUnicode));

        f = f.insert(Look::WordEndHalfUnicode);
        assert!(f.contains(Look::WordEndHalfUnicode));
        f = f.remove(Look::WordEndHalfUnicode);
        assert!(!f.contains(Look::WordEndHalfUnicode));
    }

    #[test]
    fn look_set_iter() {
        let set = LookSet::empty();
        assert_eq!(0, set.iter().count());

        let set = LookSet::full();
        assert_eq!(18, set.iter().count());

        let set =
            LookSet::empty().insert(Look::StartLF).insert(Look::WordUnicode);
        assert_eq!(2, set.iter().count());

        let set = LookSet::empty().insert(Look::StartLF);
        assert_eq!(1, set.iter().count());

        let set = LookSet::empty().insert(Look::WordAsciiNegate);
        assert_eq!(1, set.iter().count());

        let set = LookSet::empty().insert(Look::WordEndHalfUnicode);
        assert_eq!(1, set.iter().count());
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn look_set_debug() {
        let res = alloc::format!("{:?}", LookSet::empty());
        assert_eq!("∅", res);
        let res = alloc::format!("{:?}", LookSet::full());
        assert_eq!("Az^$rRbB𝛃𝚩<>〈〉◁▷◀▶", res);
    }
}

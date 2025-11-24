use alloc::{boxed::Box, string::String, vec, vec::Vec};

use crate::{error::Error, utf8};

mod parse;

/// Escapes all regular expression meta characters in `pattern`.
///
/// The string returned may be safely used as a literal in a regular
/// expression.
pub fn escape(pattern: &str) -> String {
    let mut buf = String::new();
    buf.reserve(pattern.len());
    for ch in pattern.chars() {
        if is_meta_character(ch) {
            buf.push('\\');
        }
        buf.push(ch);
    }
    buf
}

/// Returns true if the given character has significance in a regex.
///
/// Generally speaking, these are the only characters which _must_ be escaped
/// in order to match their literal meaning. For example, to match a literal
/// `|`, one could write `\|`. Sometimes escaping isn't always necessary. For
/// example, `-` is treated as a meta character because of its significance
/// for writing ranges inside of character classes, but the regex `-` will
/// match a literal `-` because `-` has no special meaning outside of character
/// classes.
///
/// In order to determine whether a character may be escaped at all, the
/// [`is_escapable_character`] routine should be used. The difference between
/// `is_meta_character` and `is_escapable_character` is that the latter will
/// return true for some characters that are _not_ meta characters. For
/// example, `%` and `\%` both match a literal `%` in all contexts. In other
/// words, `is_escapable_character` includes "superfluous" escapes.
///
/// Note that the set of characters for which this function returns `true` or
/// `false` is fixed and won't change in a semver compatible release. (In this
/// case, "semver compatible release" actually refers to the `regex` crate
/// itself, since reducing or expanding the set of meta characters would be a
/// breaking change for not just `regex-syntax` but also `regex` itself.)
fn is_meta_character(c: char) -> bool {
    match c {
        '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{'
        | '}' | '^' | '$' | '#' | '&' | '-' | '~' => true,
        _ => false,
    }
}

/// Returns true if the given character can be escaped in a regex.
///
/// This returns true in all cases that `is_meta_character` returns true, but
/// also returns true in some cases where `is_meta_character` returns false.
/// For example, `%` is not a meta character, but it is escapable. That is,
/// `%` and `\%` both match a literal `%` in all contexts.
///
/// The purpose of this routine is to provide knowledge about what characters
/// may be escaped. Namely, most regex engines permit "superfluous" escapes
/// where characters without any special significance may be escaped even
/// though there is no actual _need_ to do so.
///
/// This will return false for some characters. For example, `e` is not
/// escapable. Therefore, `\e` will either result in a parse error (which is
/// true today), or it could backwards compatibly evolve into a new construct
/// with its own meaning. Indeed, that is the purpose of banning _some_
/// superfluous escapes: it provides a way to evolve the syntax in a compatible
/// manner.
fn is_escapable_character(c: char) -> bool {
    // Certainly escapable if it's a meta character.
    if is_meta_character(c) {
        return true;
    }
    // Any character that isn't ASCII is definitely not escapable. There's
    // no real need to allow things like \☃ right?
    if !c.is_ascii() {
        return false;
    }
    // Otherwise, we basically say that everything is escapable unless it's a
    // letter or digit. Things like \3 are either octal (when enabled) or an
    // error, and we should keep it that way. Otherwise, letters are reserved
    // for adding new syntax in a backwards compatible way.
    match c {
        '0'..='9' | 'A'..='Z' | 'a'..='z' => false,
        // While not currently supported, we keep these as not escapable to
        // give us some flexibility with respect to supporting the \< and
        // \> word boundary assertions in the future. By rejecting them as
        // escapable, \< and \> will result in a parse error. Thus, we can
        // turn them into something else in the future without it being a
        // backwards incompatible change.
        '<' | '>' => false,
        _ => true,
    }
}

/// The configuration for a regex parser.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Config {
    /// The maximum number of times we're allowed to recurse.
    ///
    /// Note that unlike the regex-syntax parser, we actually use recursion in
    /// this parser for simplicity. My hope is that by setting a conservative
    /// default call limit and providing a way to configure it, that we can
    /// keep this simplification. But if we must, we can re-work the parser to
    /// put the call stack on the heap like regex-syntax does.
    pub(crate) nest_limit: u32,
    /// Various flags that control how a pattern is interpreted.
    pub(crate) flags: Flags,
}

impl Default for Config {
    fn default() -> Config {
        Config { nest_limit: 50, flags: Flags::default() }
    }
}

/// Various flags that control the interpretation of the pattern.
///
/// These can be set via explicit configuration in code, or change dynamically
/// during parsing via inline flags. For example, `foo(?i:bar)baz` will match
/// `foo` and `baz` case sensitively and `bar` case insensitively (assuming a
/// default configuration).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Flags {
    /// Whether to match case insensitively.
    ///
    /// This is the `i` flag.
    pub(crate) case_insensitive: bool,
    /// Whether `^` and `$` should be treated as line anchors or not.
    ///
    /// This is the `m` flag.
    pub(crate) multi_line: bool,
    /// Whether `.` should match line terminators or not.
    ///
    /// This is the `s` flag.
    pub(crate) dot_matches_new_line: bool,
    /// Whether to swap the meaning of greedy and non-greedy operators.
    ///
    /// This is the `U` flag.
    pub(crate) swap_greed: bool,
    /// Whether to enable CRLF mode.
    ///
    /// This is the `R` flag.
    pub(crate) crlf: bool,
    /// Whether to ignore whitespace. i.e., verbose mode.
    ///
    /// This is the `x` flag.
    pub(crate) ignore_whitespace: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Hir {
    kind: HirKind,
    is_start_anchored: bool,
    is_match_empty: bool,
    static_explicit_captures_len: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HirKind {
    Empty,
    Char(char),
    Class(Class),
    Look(Look),
    Repetition(Repetition),
    Capture(Capture),
    Concat(Vec<Hir>),
    Alternation(Vec<Hir>),
}

impl Hir {
    /// Parses the given pattern string with the given configuration into a
    /// structured representation. If the pattern is invalid, then an error
    /// is returned.
    pub(crate) fn parse(config: Config, pattern: &str) -> Result<Hir, Error> {
        self::parse::Parser::new(config, pattern).parse()
    }

    /// Returns the underlying kind of this high-level intermediate
    /// representation.
    ///
    /// Note that there is explicitly no way to build an `Hir` directly from
    /// an `HirKind`. If you need to do that, then you must do case analysis
    /// on the `HirKind` and call the appropriate smart constructor on `Hir`.
    pub(crate) fn kind(&self) -> &HirKind {
        &self.kind
    }

    /// Returns true if and only if this Hir expression can only match at the
    /// beginning of a haystack.
    pub(crate) fn is_start_anchored(&self) -> bool {
        self.is_start_anchored
    }

    /// Returns true if and only if this Hir expression can match the empty
    /// string.
    pub(crate) fn is_match_empty(&self) -> bool {
        self.is_match_empty
    }

    /// If the pattern always reports the same number of matching capture groups
    /// for every match, then this returns the number of those groups. This
    /// doesn't include the implicit group found in every pattern.
    pub(crate) fn static_explicit_captures_len(&self) -> Option<usize> {
        self.static_explicit_captures_len
    }

    fn fail() -> Hir {
        let kind = HirKind::Class(Class { ranges: vec![] });
        Hir {
            kind,
            is_start_anchored: false,
            is_match_empty: false,
            static_explicit_captures_len: Some(0),
        }
    }

    fn empty() -> Hir {
        let kind = HirKind::Empty;
        Hir {
            kind,
            is_start_anchored: false,
            is_match_empty: true,
            static_explicit_captures_len: Some(0),
        }
    }

    fn char(ch: char) -> Hir {
        let kind = HirKind::Char(ch);
        Hir {
            kind,
            is_start_anchored: false,
            is_match_empty: false,
            static_explicit_captures_len: Some(0),
        }
    }

    fn class(class: Class) -> Hir {
        let kind = HirKind::Class(class);
        Hir {
            kind,
            is_start_anchored: false,
            is_match_empty: false,
            static_explicit_captures_len: Some(0),
        }
    }

    fn look(look: Look) -> Hir {
        let kind = HirKind::Look(look);
        Hir {
            kind,
            is_start_anchored: matches!(look, Look::Start),
            is_match_empty: true,
            static_explicit_captures_len: Some(0),
        }
    }

    fn repetition(rep: Repetition) -> Hir {
        if rep.min == 0 && rep.max == Some(0) {
            return Hir::empty();
        } else if rep.min == 1 && rep.max == Some(1) {
            return *rep.sub;
        }
        let is_start_anchored = rep.min > 0 && rep.sub.is_start_anchored;
        let is_match_empty = rep.min == 0 || rep.sub.is_match_empty;
        let mut static_explicit_captures_len =
            rep.sub.static_explicit_captures_len;
        // If the static captures len of the sub-expression is not known or
        // is greater than zero, then it automatically propagates to the
        // repetition, regardless of the repetition. Otherwise, it might
        // change, but only when the repetition can match 0 times.
        if rep.min == 0
            && static_explicit_captures_len.map_or(false, |len| len > 0)
        {
            // If we require a match 0 times, then our captures len is
            // guaranteed to be zero. Otherwise, if we *can* match the empty
            // string, then it's impossible to know how many captures will be
            // in the resulting match.
            if rep.max == Some(0) {
                static_explicit_captures_len = Some(0);
            } else {
                static_explicit_captures_len = None;
            }
        }
        Hir {
            kind: HirKind::Repetition(rep),
            is_start_anchored,
            is_match_empty,
            static_explicit_captures_len,
        }
    }

    fn capture(cap: Capture) -> Hir {
        let is_start_anchored = cap.sub.is_start_anchored;
        let is_match_empty = cap.sub.is_match_empty;
        let static_explicit_captures_len = cap
            .sub
            .static_explicit_captures_len
            .map(|len| len.saturating_add(1));
        let kind = HirKind::Capture(cap);
        Hir {
            kind,
            is_start_anchored,
            is_match_empty,
            static_explicit_captures_len,
        }
    }

    fn concat(mut subs: Vec<Hir>) -> Hir {
        if subs.is_empty() {
            Hir::empty()
        } else if subs.len() == 1 {
            subs.pop().unwrap()
        } else {
            let is_start_anchored = subs[0].is_start_anchored;
            let mut is_match_empty = true;
            let mut static_explicit_captures_len = Some(0usize);
            for sub in subs.iter() {
                is_match_empty = is_match_empty && sub.is_match_empty;
                static_explicit_captures_len = static_explicit_captures_len
                    .and_then(|len1| {
                        Some((len1, sub.static_explicit_captures_len?))
                    })
                    .and_then(|(len1, len2)| Some(len1.saturating_add(len2)));
            }
            Hir {
                kind: HirKind::Concat(subs),
                is_start_anchored,
                is_match_empty,
                static_explicit_captures_len,
            }
        }
    }

    fn alternation(mut subs: Vec<Hir>) -> Hir {
        if subs.is_empty() {
            Hir::fail()
        } else if subs.len() == 1 {
            subs.pop().unwrap()
        } else {
            let mut it = subs.iter().peekable();
            let mut is_start_anchored =
                it.peek().map_or(false, |sub| sub.is_start_anchored);
            let mut is_match_empty =
                it.peek().map_or(false, |sub| sub.is_match_empty);
            let mut static_explicit_captures_len =
                it.peek().and_then(|sub| sub.static_explicit_captures_len);
            for sub in it {
                is_start_anchored = is_start_anchored && sub.is_start_anchored;
                is_match_empty = is_match_empty || sub.is_match_empty;
                if static_explicit_captures_len
                    != sub.static_explicit_captures_len
                {
                    static_explicit_captures_len = None;
                }
            }
            Hir {
                kind: HirKind::Alternation(subs),
                is_start_anchored,
                is_match_empty,
                static_explicit_captures_len,
            }
        }
    }
}

impl HirKind {
    /// Returns a slice of this kind's sub-expressions, if any.
    fn subs(&self) -> &[Hir] {
        use core::slice::from_ref;

        match *self {
            HirKind::Empty
            | HirKind::Char(_)
            | HirKind::Class(_)
            | HirKind::Look(_) => &[],
            HirKind::Repetition(Repetition { ref sub, .. }) => from_ref(sub),
            HirKind::Capture(Capture { ref sub, .. }) => from_ref(sub),
            HirKind::Concat(ref subs) => subs,
            HirKind::Alternation(ref subs) => subs,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Class {
    pub(crate) ranges: Vec<ClassRange>,
}

impl Class {
    /// Create a new class from the given ranges. The ranges may be provided
    /// in any order or may even overlap. They will be automatically
    /// canonicalized.
    fn new<I: IntoIterator<Item = ClassRange>>(ranges: I) -> Class {
        let mut class = Class { ranges: ranges.into_iter().collect() };
        class.canonicalize();
        class
    }

    /// Expand this class such that it matches the ASCII codepoints in this set
    /// case insensitively.
    fn ascii_case_fold(&mut self) {
        let len = self.ranges.len();
        for i in 0..len {
            if let Some(folded) = self.ranges[i].ascii_case_fold() {
                self.ranges.push(folded);
            }
        }
        self.canonicalize();
    }

    /// Negate this set.
    ///
    /// For all `x` where `x` is any element, if `x` was in this set, then it
    /// will not be in this set after negation.
    fn negate(&mut self) {
        const MIN: char = '\x00';
        const MAX: char = char::MAX;

        if self.ranges.is_empty() {
            self.ranges.push(ClassRange { start: MIN, end: MAX });
            return;
        }

        // There should be a way to do this in-place with constant memory,
        // but I couldn't figure out a simple way to do it. So just append
        // the negation to the end of this range, and then drain it before
        // we're done.
        let drain_end = self.ranges.len();

        // If our class doesn't start the minimum possible char, then negation
        // needs to include all codepoints up to the minimum in this set.
        if self.ranges[0].start > MIN {
            self.ranges.push(ClassRange {
                start: MIN,
                // OK because we know it's bigger than MIN.
                end: prev_char(self.ranges[0].start).unwrap(),
            });
        }
        for i in 1..drain_end {
            // let lower = self.ranges[i - 1].upper().increment();
            // let upper = self.ranges[i].lower().decrement();
            // self.ranges.push(I::create(lower, upper));
            self.ranges.push(ClassRange {
                // OK because we know i-1 is never the last range and therefore
                // there must be a range greater than it. It therefore follows
                // that 'end' can never be char::MAX, and thus there must be
                // a next char.
                start: next_char(self.ranges[i - 1].end).unwrap(),
                // Since 'i' is guaranteed to never be the first range, it
                // follows that there is always a range before this and thus
                // 'start' can never be '\x00'. Thus, there must be a previous
                // char.
                end: prev_char(self.ranges[i].start).unwrap(),
            });
        }
        if self.ranges[drain_end - 1].end < MAX {
            // let lower = self.ranges[drain_end - 1].upper().increment();
            // self.ranges.push(I::create(lower, I::Bound::max_value()));
            self.ranges.push(ClassRange {
                // OK because we know 'end' is less than char::MAX, and thus
                // there is a next char.
                start: next_char(self.ranges[drain_end - 1].end).unwrap(),
                end: MAX,
            });
        }
        self.ranges.drain(..drain_end);
        // We don't need to canonicalize because we processed the ranges above
        // in canonical order and the new ranges we added based on those are
        // also necessarily in canonical order.
    }

    /// Converts this set into a canonical ordering.
    fn canonicalize(&mut self) {
        if self.is_canonical() {
            return;
        }
        self.ranges.sort();
        assert!(!self.ranges.is_empty());

        // Is there a way to do this in-place with constant memory? I couldn't
        // figure out a way to do it. So just append the canonicalization to
        // the end of this range, and then drain it before we're done.
        let drain_end = self.ranges.len();
        for oldi in 0..drain_end {
            // If we've added at least one new range, then check if we can
            // merge this range in the previously added range.
            if self.ranges.len() > drain_end {
                let (last, rest) = self.ranges.split_last_mut().unwrap();
                if let Some(union) = last.union(&rest[oldi]) {
                    *last = union;
                    continue;
                }
            }
            self.ranges.push(self.ranges[oldi]);
        }
        self.ranges.drain(..drain_end);
    }

    /// Returns true if and only if this class is in a canonical ordering.
    fn is_canonical(&self) -> bool {
        for pair in self.ranges.windows(2) {
            if pair[0] >= pair[1] {
                return false;
            }
            if pair[0].is_contiguous(&pair[1]) {
                return false;
            }
        }
        true
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) struct ClassRange {
    pub(crate) start: char,
    pub(crate) end: char,
}

impl ClassRange {
    /// Apply simple case folding to this byte range. Only ASCII case mappings
    /// (for A-Za-z) are applied.
    ///
    /// Additional ranges are appended to the given vector. Canonical ordering
    /// is *not* maintained in the given vector.
    fn ascii_case_fold(&self) -> Option<ClassRange> {
        if !(ClassRange { start: 'a', end: 'z' }).is_intersection_empty(self) {
            let start = core::cmp::max(self.start, 'a');
            let end = core::cmp::min(self.end, 'z');
            return Some(ClassRange {
                start: char::try_from(u32::from(start) - 32).unwrap(),
                end: char::try_from(u32::from(end) - 32).unwrap(),
            });
        }
        if !(ClassRange { start: 'A', end: 'Z' }).is_intersection_empty(self) {
            let start = core::cmp::max(self.start, 'A');
            let end = core::cmp::min(self.end, 'Z');
            return Some(ClassRange {
                start: char::try_from(u32::from(start) + 32).unwrap(),
                end: char::try_from(u32::from(end) + 32).unwrap(),
            });
        }
        None
    }

    /// Union the given overlapping range into this range.
    ///
    /// If the two ranges aren't contiguous, then this returns `None`.
    fn union(&self, other: &ClassRange) -> Option<ClassRange> {
        if !self.is_contiguous(other) {
            return None;
        }
        let start = core::cmp::min(self.start, other.start);
        let end = core::cmp::max(self.end, other.end);
        Some(ClassRange { start, end })
    }

    /// Returns true if and only if the two ranges are contiguous. Two ranges
    /// are contiguous if and only if the ranges are either overlapping or
    /// adjacent.
    fn is_contiguous(&self, other: &ClassRange) -> bool {
        let (s1, e1) = (u32::from(self.start), u32::from(self.end));
        let (s2, e2) = (u32::from(other.start), u32::from(other.end));
        core::cmp::max(s1, s2) <= core::cmp::min(e1, e2).saturating_add(1)
    }

    /// Returns true if and only if the intersection of this range and the
    /// other range is empty.
    fn is_intersection_empty(&self, other: &ClassRange) -> bool {
        let (s1, e1) = (self.start, self.end);
        let (s2, e2) = (other.start, other.end);
        core::cmp::max(s1, s2) > core::cmp::min(e1, e2)
    }
}

/// The high-level intermediate representation for a look-around assertion.
///
/// An assertion match is always zero-length. Also called an "empty match."
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Look {
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
    Word = 1 << 6,
    /// Match an ASCII-only negation of a word boundary.
    WordNegate = 1 << 7,
    /// Match the start of an ASCII-only word boundary. That is, this matches a
    /// position at either the beginning of the haystack or where the previous
    /// character is not a word character and the following character is a word
    /// character.
    WordStart = 1 << 8,
    /// Match the end of an ASCII-only word boundary. That is, this matches
    /// a position at either the end of the haystack or where the previous
    /// character is a word character and the following character is not a word
    /// character.
    WordEnd = 1 << 9,
    /// Match the start half of an ASCII-only word boundary. That is, this
    /// matches a position at either the beginning of the haystack or where the
    /// previous character is not a word character.
    WordStartHalf = 1 << 10,
    /// Match the end half of an ASCII-only word boundary. That is, this
    /// matches a position at either the end of the haystack or where the
    /// following character is not a word character.
    WordEndHalf = 1 << 11,
}

impl Look {
    /// Returns true if the given position in the given haystack matches this
    /// look-around assertion.
    pub(crate) fn is_match(&self, haystack: &[u8], at: usize) -> bool {
        use self::Look::*;

        match *self {
            Start => at == 0,
            End => at == haystack.len(),
            StartLF => at == 0 || haystack[at - 1] == b'\n',
            EndLF => at == haystack.len() || haystack[at] == b'\n',
            StartCRLF => {
                at == 0
                    || haystack[at - 1] == b'\n'
                    || (haystack[at - 1] == b'\r'
                        && (at >= haystack.len() || haystack[at] != b'\n'))
            }
            EndCRLF => {
                at == haystack.len()
                    || haystack[at] == b'\r'
                    || (haystack[at] == b'\n'
                        && (at == 0 || haystack[at - 1] != b'\r'))
            }
            Word => {
                let word_before =
                    at > 0 && utf8::is_word_byte(haystack[at - 1]);
                let word_after =
                    at < haystack.len() && utf8::is_word_byte(haystack[at]);
                word_before != word_after
            }
            WordNegate => {
                let word_before =
                    at > 0 && utf8::is_word_byte(haystack[at - 1]);
                let word_after =
                    at < haystack.len() && utf8::is_word_byte(haystack[at]);
                word_before == word_after
            }
            WordStart => {
                let word_before =
                    at > 0 && utf8::is_word_byte(haystack[at - 1]);
                let word_after =
                    at < haystack.len() && utf8::is_word_byte(haystack[at]);
                !word_before && word_after
            }
            WordEnd => {
                let word_before =
                    at > 0 && utf8::is_word_byte(haystack[at - 1]);
                let word_after =
                    at < haystack.len() && utf8::is_word_byte(haystack[at]);
                word_before && !word_after
            }
            WordStartHalf => {
                let word_before =
                    at > 0 && utf8::is_word_byte(haystack[at - 1]);
                !word_before
            }
            WordEndHalf => {
                let word_after =
                    at < haystack.len() && utf8::is_word_byte(haystack[at]);
                !word_after
            }
        }
    }
}

/// The high-level intermediate representation of a repetition operator.
///
/// A repetition operator permits the repetition of an arbitrary
/// sub-expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Repetition {
    /// The minimum range of the repetition.
    ///
    /// Note that special cases like `?`, `+` and `*` all get translated into
    /// the ranges `{0,1}`, `{1,}` and `{0,}`, respectively.
    ///
    /// When `min` is zero, this expression can match the empty string
    /// regardless of what its sub-expression is.
    pub(crate) min: u32,
    /// The maximum range of the repetition.
    ///
    /// Note that when `max` is `None`, `min` acts as a lower bound but where
    /// there is no upper bound. For something like `x{5}` where the min and
    /// max are equivalent, `min` will be set to `5` and `max` will be set to
    /// `Some(5)`.
    pub(crate) max: Option<u32>,
    /// Whether this repetition operator is greedy or not. A greedy operator
    /// will match as much as it can. A non-greedy operator will match as
    /// little as it can.
    ///
    /// Typically, operators are greedy by default and are only non-greedy when
    /// a `?` suffix is used, e.g., `(expr)*` is greedy while `(expr)*?` is
    /// not. However, this can be inverted via the `U` "ungreedy" flag.
    pub(crate) greedy: bool,
    /// The expression being repeated.
    pub(crate) sub: Box<Hir>,
}

/// The high-level intermediate representation for a capturing group.
///
/// A capturing group always has an index and a child expression. It may
/// also have a name associated with it (e.g., `(?P<foo>\w)`), but it's not
/// necessary.
///
/// Note that there is no explicit representation of a non-capturing group
/// in a `Hir`. Instead, non-capturing grouping is handled automatically by
/// the recursive structure of the `Hir` itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Capture {
    /// The capture index of the capture.
    pub(crate) index: u32,
    /// The name of the capture, if it exists.
    pub(crate) name: Option<Box<str>>,
    /// The expression inside the capturing group, which may be empty.
    pub(crate) sub: Box<Hir>,
}

fn next_char(ch: char) -> Option<char> {
    // Skip over the surrogate range.
    if ch == '\u{D7FF}' {
        return Some('\u{E000}');
    }
    // OK because char::MAX < u32::MAX and we handle U+D7FF above.
    char::from_u32(u32::from(ch).checked_add(1).unwrap())
}

fn prev_char(ch: char) -> Option<char> {
    // Skip over the surrogate range.
    if ch == '\u{E000}' {
        return Some('\u{D7FF}');
    }
    // OK because subtracting 1 from any valid scalar value other than 0
    // and U+E000 yields a valid scalar value.
    Some(char::from_u32(u32::from(ch).checked_sub(1)?).unwrap())
}

impl Drop for Hir {
    fn drop(&mut self) {
        use core::mem;

        match *self.kind() {
            HirKind::Empty
            | HirKind::Char(_)
            | HirKind::Class(_)
            | HirKind::Look(_) => return,
            HirKind::Capture(ref x) if x.sub.kind.subs().is_empty() => return,
            HirKind::Repetition(ref x) if x.sub.kind.subs().is_empty() => {
                return
            }
            HirKind::Concat(ref x) if x.is_empty() => return,
            HirKind::Alternation(ref x) if x.is_empty() => return,
            _ => {}
        }

        let mut stack = vec![mem::replace(self, Hir::empty())];
        while let Some(mut expr) = stack.pop() {
            match expr.kind {
                HirKind::Empty
                | HirKind::Char(_)
                | HirKind::Class(_)
                | HirKind::Look(_) => {}
                HirKind::Capture(ref mut x) => {
                    stack.push(mem::replace(&mut x.sub, Hir::empty()));
                }
                HirKind::Repetition(ref mut x) => {
                    stack.push(mem::replace(&mut x.sub, Hir::empty()));
                }
                HirKind::Concat(ref mut x) => {
                    stack.extend(x.drain(..));
                }
                HirKind::Alternation(ref mut x) => {
                    stack.extend(x.drain(..));
                }
            }
        }
    }
}

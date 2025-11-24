use core::cell::{Cell, RefCell};

use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use crate::{
    error::Error,
    hir::{self, Config, Flags, Hir, HirKind},
};

// These are all of the errors that can occur while parsing a regex. Unlike
// regex-syntax, our errors are not particularly great. They are just enough
// to get a general sense of what went wrong. But in exchange, the error
// reporting mechanism is *much* simpler than what's in regex-syntax.
//
// By convention, we use each of these messages in exactly one place. That
// way, every branch that leads to an error has a unique message. This in turn
// means that given a message, one can precisely identify which part of the
// parser reported it.
//
// Finally, we give names to each message so that we can reference them in
// tests.
const ERR_TOO_MUCH_NESTING: &str = "pattern has too much nesting";
const ERR_TOO_MANY_CAPTURES: &str = "too many capture groups";
const ERR_DUPLICATE_CAPTURE_NAME: &str = "duplicate capture group name";
const ERR_UNCLOSED_GROUP: &str = "found open group without closing ')'";
const ERR_UNCLOSED_GROUP_QUESTION: &str =
    "expected closing ')', but got end of pattern";
const ERR_UNOPENED_GROUP: &str = "found closing ')' without matching '('";
const ERR_LOOK_UNSUPPORTED: &str = "look-around is not supported";
const ERR_EMPTY_FLAGS: &str = "empty flag directive '(?)' is not allowed";
const ERR_MISSING_GROUP_NAME: &str =
    "expected capture group name, but got end of pattern";
const ERR_INVALID_GROUP_NAME: &str = "invalid group name";
const ERR_UNCLOSED_GROUP_NAME: &str =
    "expected end of capture group name, but got end of pattern";
const ERR_EMPTY_GROUP_NAME: &str = "empty capture group names are not allowed";
const ERR_FLAG_UNRECOGNIZED: &str = "unrecognized inline flag";
const ERR_FLAG_REPEATED_NEGATION: &str =
    "inline flag negation cannot be repeated";
const ERR_FLAG_DUPLICATE: &str = "duplicate inline flag is not allowed";
const ERR_FLAG_UNEXPECTED_EOF: &str =
    "expected ':' or ')' to end inline flags, but got end of pattern";
const ERR_FLAG_DANGLING_NEGATION: &str =
    "inline flags cannot end with negation directive";
const ERR_DECIMAL_NO_DIGITS: &str =
    "expected decimal number, but found no digits";
const ERR_DECIMAL_INVALID: &str = "got invalid decimal number";
const ERR_HEX_BRACE_INVALID_DIGIT: &str =
    "expected hexadecimal number in braces, but got non-hex digit";
const ERR_HEX_BRACE_UNEXPECTED_EOF: &str =
    "expected hexadecimal number, but saw end of pattern before closing brace";
const ERR_HEX_BRACE_EMPTY: &str =
    "expected hexadecimal number in braces, but got no digits";
const ERR_HEX_BRACE_INVALID: &str = "got invalid hexadecimal number in braces";
const ERR_HEX_FIXED_UNEXPECTED_EOF: &str =
    "expected fixed length hexadecimal number, but saw end of pattern first";
const ERR_HEX_FIXED_INVALID_DIGIT: &str =
    "expected fixed length hexadecimal number, but got non-hex digit";
const ERR_HEX_FIXED_INVALID: &str =
    "got invalid fixed length hexadecimal number";
const ERR_HEX_UNEXPECTED_EOF: &str =
    "expected hexadecimal number, but saw end of pattern first";
const ERR_ESCAPE_UNEXPECTED_EOF: &str =
    "saw start of escape sequence, but saw end of pattern before it finished";
const ERR_BACKREF_UNSUPPORTED: &str = "backreferences are not supported";
const ERR_UNICODE_CLASS_UNSUPPORTED: &str =
    "Unicode character classes are not supported";
const ERR_ESCAPE_UNRECOGNIZED: &str = "unrecognized escape sequence";
const ERR_POSIX_CLASS_UNRECOGNIZED: &str =
    "unrecognized POSIX character class";
const ERR_UNCOUNTED_REP_SUB_MISSING: &str =
    "uncounted repetition operator must be applied to a sub-expression";
const ERR_COUNTED_REP_SUB_MISSING: &str =
    "counted repetition operator must be applied to a sub-expression";
const ERR_COUNTED_REP_UNCLOSED: &str =
    "found unclosed counted repetition operator";
const ERR_COUNTED_REP_MIN_UNCLOSED: &str =
    "found incomplete and unclosed counted repetition operator";
const ERR_COUNTED_REP_COMMA_UNCLOSED: &str =
    "found counted repetition operator with a comma that is unclosed";
const ERR_COUNTED_REP_MIN_MAX_UNCLOSED: &str =
    "found counted repetition with min and max that is unclosed";
const ERR_COUNTED_REP_INVALID: &str =
    "expected closing brace for counted repetition, but got something else";
const ERR_COUNTED_REP_INVALID_RANGE: &str =
    "found counted repetition with a min bigger than its max";
const ERR_CLASS_UNCLOSED_AFTER_ITEM: &str =
    "non-empty character class has no closing bracket";
const ERR_CLASS_INVALID_RANGE_ITEM: &str =
    "character class ranges must start and end with a single character";
const ERR_CLASS_INVALID_ITEM: &str =
    "invalid escape sequence in character class";
const ERR_CLASS_UNCLOSED_AFTER_DASH: &str =
    "non-empty character class has no closing bracket after dash";
const ERR_CLASS_UNCLOSED_AFTER_NEGATION: &str =
    "negated character class has no closing bracket";
const ERR_CLASS_UNCLOSED_AFTER_CLOSING: &str =
    "character class begins with literal ']' but has no closing bracket";
const ERR_CLASS_INVALID_RANGE: &str = "invalid range in character class";
const ERR_CLASS_UNCLOSED: &str = "found unclosed character class";
const ERR_CLASS_NEST_UNSUPPORTED: &str =
    "nested character classes are not supported";
const ERR_CLASS_INTERSECTION_UNSUPPORTED: &str =
    "character class intersection is not supported";
const ERR_CLASS_DIFFERENCE_UNSUPPORTED: &str =
    "character class difference is not supported";
const ERR_CLASS_SYMDIFFERENCE_UNSUPPORTED: &str =
    "character class symmetric difference is not supported";
const ERR_SPECIAL_WORD_BOUNDARY_UNCLOSED: &str =
    "special word boundary assertion is unclosed or has an invalid character";
const ERR_SPECIAL_WORD_BOUNDARY_UNRECOGNIZED: &str =
    "special word boundary assertion is unrecognized";
const ERR_SPECIAL_WORD_OR_REP_UNEXPECTED_EOF: &str =
    "found start of special word boundary or repetition without an end";

/// A regular expression parser.
///
/// This parses a string representation of a regular expression into an
/// abstract syntax tree. The size of the tree is proportional to the length
/// of the regular expression pattern.
///
/// A `Parser` can be configured in more detail via a [`ParserBuilder`].
#[derive(Clone, Debug)]
pub(super) struct Parser<'a> {
    /// The configuration of the parser as given by the caller.
    config: Config,
    /// The pattern we're parsing as given by the caller.
    pattern: &'a str,
    /// The call depth of the parser. This is incremented for each
    /// sub-expression parsed. Its peak value is the maximum nesting of the
    /// pattern.
    depth: Cell<u32>,
    /// The current position of the parser.
    pos: Cell<usize>,
    /// The current codepoint of the parser. The codepoint corresponds to the
    /// codepoint encoded in `pattern` beginning at `pos`.
    ///
    /// This is `None` if and only if `pos == pattern.len()`.
    char: Cell<Option<char>>,
    /// The current capture index.
    capture_index: Cell<u32>,
    /// The flags that are currently set.
    flags: RefCell<Flags>,
    /// A sorted sequence of capture names. This is used to detect duplicate
    /// capture names and report an error if one is detected.
    capture_names: RefCell<Vec<String>>,
}

/// The constructor and a variety of helper routines.
impl<'a> Parser<'a> {
    /// Build a parser from this configuration with the given pattern.
    pub(super) fn new(config: Config, pattern: &'a str) -> Parser<'a> {
        Parser {
            config,
            pattern,
            depth: Cell::new(0),
            pos: Cell::new(0),
            char: Cell::new(pattern.chars().next()),
            capture_index: Cell::new(0),
            flags: RefCell::new(config.flags),
            capture_names: RefCell::new(vec![]),
        }
    }

    /// Returns the full pattern string that we're parsing.
    fn pattern(&self) -> &str {
        self.pattern
    }

    /// Return the current byte offset of the parser.
    ///
    /// The offset starts at `0` from the beginning of the regular expression
    /// pattern string.
    fn pos(&self) -> usize {
        self.pos.get()
    }

    /// Increments the call depth of the parser.
    ///
    /// If the call depth would exceed the configured nest limit, then this
    /// returns an error.
    ///
    /// This returns the old depth.
    fn increment_depth(&self) -> Result<u32, Error> {
        let old = self.depth.get();
        if old > self.config.nest_limit {
            return Err(Error::new(ERR_TOO_MUCH_NESTING));
        }
        // OK because our depth starts at 0, and we return an error if it
        // ever reaches the limit. So the call depth can never exceed u32::MAX.
        let new = old.checked_add(1).unwrap();
        self.depth.set(new);
        Ok(old)
    }

    /// Decrements the call depth of the parser.
    ///
    /// This panics if the current depth is 0.
    fn decrement_depth(&self) {
        let old = self.depth.get();
        // If this fails then the caller has a bug in how they're incrementing
        // and decrementing the depth of the parser's call stack.
        let new = old.checked_sub(1).unwrap();
        self.depth.set(new);
    }

    /// Return the codepoint at the current position of the parser.
    ///
    /// This panics if the parser is positioned at the end of the pattern.
    fn char(&self) -> char {
        self.char.get().expect("codepoint, but parser is done")
    }

    /// Returns true if the next call to `bump` would return false.
    fn is_done(&self) -> bool {
        self.pos() == self.pattern.len()
    }

    /// Returns the flags that are current set for this regex.
    fn flags(&self) -> Flags {
        *self.flags.borrow()
    }

    /// Bump the parser to the next Unicode scalar value.
    ///
    /// If the end of the input has been reached, then `false` is returned.
    fn bump(&self) -> bool {
        if self.is_done() {
            return false;
        }
        self.pos.set(self.pos() + self.char().len_utf8());
        self.char.set(self.pattern()[self.pos()..].chars().next());
        self.char.get().is_some()
    }

    /// If the substring starting at the current position of the parser has
    /// the given prefix, then bump the parser to the character immediately
    /// following the prefix and return true. Otherwise, don't bump the parser
    /// and return false.
    fn bump_if(&self, prefix: &str) -> bool {
        if self.pattern()[self.pos()..].starts_with(prefix) {
            for _ in 0..prefix.chars().count() {
                self.bump();
            }
            true
        } else {
            false
        }
    }

    /// Bump the parser, and if the `x` flag is enabled, bump through any
    /// subsequent spaces. Return true if and only if the parser is not done.
    fn bump_and_bump_space(&self) -> bool {
        if !self.bump() {
            return false;
        }
        self.bump_space();
        !self.is_done()
    }

    /// If the `x` flag is enabled (i.e., whitespace insensitivity with
    /// comments), then this will advance the parser through all whitespace
    /// and comments to the next non-whitespace non-comment byte.
    ///
    /// If the `x` flag is disabled, then this is a no-op.
    ///
    /// This should be used selectively throughout the parser where
    /// arbitrary whitespace is permitted when the `x` flag is enabled. For
    /// example, `{   5  , 6}` is equivalent to `{5,6}`.
    fn bump_space(&self) {
        if !self.flags().ignore_whitespace {
            return;
        }
        while !self.is_done() {
            if self.char().is_whitespace() {
                self.bump();
            } else if self.char() == '#' {
                self.bump();
                while !self.is_done() {
                    let c = self.char();
                    self.bump();
                    if c == '\n' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    /// Peek at the next character in the input without advancing the parser.
    ///
    /// If the input has been exhausted, then this returns `None`.
    fn peek(&self) -> Option<char> {
        if self.is_done() {
            return None;
        }
        self.pattern()[self.pos() + self.char().len_utf8()..].chars().next()
    }

    /// Peeks at the next character in the pattern from the current offset, and
    /// will ignore spaces when the parser is in whitespace insensitive mode.
    fn peek_space(&self) -> Option<char> {
        if !self.flags().ignore_whitespace {
            return self.peek();
        }
        if self.is_done() {
            return None;
        }
        let mut start = self.pos() + self.char().len_utf8();
        let mut in_comment = false;
        for (i, ch) in self.pattern()[start..].char_indices() {
            if ch.is_whitespace() {
                continue;
            } else if !in_comment && ch == '#' {
                in_comment = true;
            } else if in_comment && ch == '\n' {
                in_comment = false;
            } else {
                start += i;
                break;
            }
        }
        self.pattern()[start..].chars().next()
    }

    /// Return the next capturing index. Each subsequent call increments the
    /// internal index. Since the way capture indices are computed is a public
    /// API guarantee, use of this routine depends on the parser being depth
    /// first and left-to-right.
    ///
    /// If the capture limit is exceeded, then an error is returned.
    fn next_capture_index(&self) -> Result<u32, Error> {
        let current = self.capture_index.get();
        let next = current
            .checked_add(1)
            .ok_or_else(|| Error::new(ERR_TOO_MANY_CAPTURES))?;
        self.capture_index.set(next);
        Ok(next)
    }

    /// Adds the given capture name to this parser. If this capture name has
    /// already been used, then an error is returned.
    fn add_capture_name(&self, name: &str) -> Result<(), Error> {
        let mut names = self.capture_names.borrow_mut();
        match names.binary_search_by(|n| name.cmp(n)) {
            Ok(_) => Err(Error::new(ERR_DUPLICATE_CAPTURE_NAME)),
            Err(i) => {
                names.insert(i, name.to_string());
                Ok(())
            }
        }
    }

    /// Returns true if and only if the parser is positioned at a look-around
    /// prefix. The conditions under which this returns true must always
    /// correspond to a regular expression that would otherwise be consider
    /// invalid.
    ///
    /// This should only be called immediately after parsing the opening of
    /// a group or a set of flags.
    fn is_lookaround_prefix(&self) -> bool {
        self.bump_if("?=")
            || self.bump_if("?!")
            || self.bump_if("?<=")
            || self.bump_if("?<!")
    }
}

/// The actual parser. We try to break out each kind of regex syntax into its
/// own routine.
impl<'a> Parser<'a> {
    pub(super) fn parse(&self) -> Result<Hir, Error> {
        let hir = self.parse_inner()?;
        // While we also check nesting during parsing, that only checks the
        // number of recursive parse calls. It does not necessarily cover
        // all possible recursive nesting of the Hir itself. For example,
        // repetition operators don't require recursive parse calls. So one
        // can stack them arbitrarily without overflowing the stack in the
        // *parser*. But then if one recurses over the resulting Hir, a stack
        // overflow is possible. So here we check the Hir nesting level
        // thoroughly to ensure it isn't nested too deeply.
        //
        // Note that we do still need the nesting limit check in the parser as
        // well, since that will avoid overflowing the stack during parse time
        // before the complete Hir value is constructed.
        check_hir_nesting(&hir, self.config.nest_limit)?;
        Ok(hir)
    }

    fn parse_inner(&self) -> Result<Hir, Error> {
        let depth = self.increment_depth()?;
        let mut alternates = vec![];
        let mut concat = vec![];
        loop {
            self.bump_space();
            if self.is_done() {
                break;
            }
            match self.char() {
                '(' => {
                    // Save the old flags and reset them only when we close
                    // the group.
                    let oldflags = *self.flags.borrow();
                    if let Some(sub) = self.parse_group()? {
                        concat.push(sub);
                        // We only reset them here because if 'parse_group'
                        // returns None, then that means it handled a flag
                        // directive, e.g., '(?ism)'. And the whole point is
                        // that those flags remain active until either disabled
                        // or the end of the pattern or current group.
                        *self.flags.borrow_mut() = oldflags;
                    }
                    if self.char.get() != Some(')') {
                        return Err(Error::new(ERR_UNCLOSED_GROUP));
                    }
                    self.bump();
                }
                ')' => {
                    if depth == 0 {
                        return Err(Error::new(ERR_UNOPENED_GROUP));
                    }
                    break;
                }
                '|' => {
                    alternates.push(Hir::concat(core::mem::take(&mut concat)));
                    self.bump();
                }
                '[' => concat.push(self.parse_class()?),
                '?' | '*' | '+' => {
                    concat = self.parse_uncounted_repetition(concat)?;
                }
                '{' => {
                    concat = self.parse_counted_repetition(concat)?;
                }
                _ => concat.push(self.parse_primitive()?),
            }
        }
        self.decrement_depth();
        alternates.push(Hir::concat(concat));
        // N.B. This strips off the "alternation" if there's only one branch.
        Ok(Hir::alternation(alternates))
    }

    /// Parses a "primitive" pattern. A primitive is any expression that does
    /// not contain any sub-expressions.
    ///
    /// This assumes the parser is pointing at the beginning of the primitive.
    fn parse_primitive(&self) -> Result<Hir, Error> {
        let ch = self.char();
        self.bump();
        match ch {
            '\\' => self.parse_escape(),
            '.' => Ok(self.hir_dot()),
            '^' => Ok(self.hir_anchor_start()),
            '$' => Ok(self.hir_anchor_end()),
            ch => Ok(self.hir_char(ch)),
        }
    }

    /// Parse an escape sequence. This always results in a "primitive" HIR,
    /// that is, an HIR with no sub-expressions.
    ///
    /// This assumes the parser is positioned at the start of the sequence,
    /// immediately *after* the `\`. It advances the parser to the first
    /// position immediately following the escape sequence.
    fn parse_escape(&self) -> Result<Hir, Error> {
        if self.is_done() {
            return Err(Error::new(ERR_ESCAPE_UNEXPECTED_EOF));
        }
        let ch = self.char();
        // Put some of the more complicated routines into helpers.
        match ch {
            '0'..='9' => return Err(Error::new(ERR_BACKREF_UNSUPPORTED)),
            'p' | 'P' => {
                return Err(Error::new(ERR_UNICODE_CLASS_UNSUPPORTED))
            }
            'x' | 'u' | 'U' => return self.parse_hex(),
            'd' | 's' | 'w' | 'D' | 'S' | 'W' => {
                return Ok(self.parse_perl_class());
            }
            _ => {}
        }

        // Handle all of the one letter sequences inline.
        self.bump();
        if hir::is_meta_character(ch) || hir::is_escapable_character(ch) {
            return Ok(self.hir_char(ch));
        }
        let special = |ch| Ok(self.hir_char(ch));
        match ch {
            'a' => special('\x07'),
            'f' => special('\x0C'),
            't' => special('\t'),
            'n' => special('\n'),
            'r' => special('\r'),
            'v' => special('\x0B'),
            'A' => Ok(Hir::look(hir::Look::Start)),
            'z' => Ok(Hir::look(hir::Look::End)),
            'b' => {
                let mut hir = Hir::look(hir::Look::Word);
                if !self.is_done() && self.char() == '{' {
                    if let Some(special) =
                        self.maybe_parse_special_word_boundary()?
                    {
                        hir = special;
                    }
                }
                Ok(hir)
            }
            'B' => Ok(Hir::look(hir::Look::WordNegate)),
            '<' => Ok(Hir::look(hir::Look::WordStart)),
            '>' => Ok(Hir::look(hir::Look::WordEnd)),
            _ => Err(Error::new(ERR_ESCAPE_UNRECOGNIZED)),
        }
    }

    /// Attempt to parse a specialty word boundary. That is, `\b{start}`,
    /// `\b{end}`, `\b{start-half}` or `\b{end-half}`.
    ///
    /// This is similar to `maybe_parse_ascii_class` in that, in most cases,
    /// if it fails it will just return `None` with no error. This is done
    /// because `\b{5}` is a valid expression and we want to let that be parsed
    /// by the existing counted repetition parsing code. (I thought about just
    /// invoking the counted repetition code from here, but it seemed a little
    /// ham-fisted.)
    ///
    /// Unlike `maybe_parse_ascii_class` though, this can return an error.
    /// Namely, if we definitely know it isn't a counted repetition, then we
    /// return an error specific to the specialty word boundaries.
    ///
    /// This assumes the parser is positioned at a `{` immediately following
    /// a `\b`. When `None` is returned, the parser is returned to the position
    /// at which it started: pointing at a `{`.
    ///
    /// The position given should correspond to the start of the `\b`.
    fn maybe_parse_special_word_boundary(&self) -> Result<Option<Hir>, Error> {
        assert_eq!(self.char(), '{');

        let is_valid_char = |c| match c {
            'A'..='Z' | 'a'..='z' | '-' => true,
            _ => false,
        };
        let start = self.pos();
        if !self.bump_and_bump_space() {
            return Err(Error::new(ERR_SPECIAL_WORD_OR_REP_UNEXPECTED_EOF));
        }
        // This is one of the critical bits: if the first non-whitespace
        // character isn't in [-A-Za-z] (i.e., this can't be a special word
        // boundary), then we bail and let the counted repetition parser deal
        // with this.
        if !is_valid_char(self.char()) {
            self.pos.set(start);
            self.char.set(Some('{'));
            return Ok(None);
        }

        // Now collect up our chars until we see a '}'.
        let mut scratch = String::new();
        while !self.is_done() && is_valid_char(self.char()) {
            scratch.push(self.char());
            self.bump_and_bump_space();
        }
        if self.is_done() || self.char() != '}' {
            return Err(Error::new(ERR_SPECIAL_WORD_BOUNDARY_UNCLOSED));
        }
        self.bump();
        let kind = match scratch.as_str() {
            "start" => hir::Look::WordStart,
            "end" => hir::Look::WordEnd,
            "start-half" => hir::Look::WordStartHalf,
            "end-half" => hir::Look::WordEndHalf,
            _ => {
                return Err(Error::new(ERR_SPECIAL_WORD_BOUNDARY_UNRECOGNIZED))
            }
        };
        Ok(Some(Hir::look(kind)))
    }

    /// Parse a hex representation of a Unicode codepoint. This handles both
    /// hex notations, i.e., `\xFF` and `\x{FFFF}`. This expects the parser to
    /// be positioned at the `x`, `u` or `U` prefix. The parser is advanced to
    /// the first character immediately following the hexadecimal literal.
    fn parse_hex(&self) -> Result<Hir, Error> {
        let digit_len = match self.char() {
            'x' => 2,
            'u' => 4,
            'U' => 8,
            unk => unreachable!(
                "invalid start of fixed length hexadecimal number {}",
                unk
            ),
        };
        if !self.bump_and_bump_space() {
            return Err(Error::new(ERR_HEX_UNEXPECTED_EOF));
        }
        if self.char() == '{' {
            self.parse_hex_brace()
        } else {
            self.parse_hex_digits(digit_len)
        }
    }

    /// Parse an N-digit hex representation of a Unicode codepoint. This
    /// expects the parser to be positioned at the first digit and will advance
    /// the parser to the first character immediately following the escape
    /// sequence.
    ///
    /// The number of digits given must be 2 (for `\xNN`), 4 (for `\uNNNN`)
    /// or 8 (for `\UNNNNNNNN`).
    fn parse_hex_digits(&self, digit_len: usize) -> Result<Hir, Error> {
        let mut scratch = String::new();
        for i in 0..digit_len {
            if i > 0 && !self.bump_and_bump_space() {
                return Err(Error::new(ERR_HEX_FIXED_UNEXPECTED_EOF));
            }
            if !is_hex(self.char()) {
                return Err(Error::new(ERR_HEX_FIXED_INVALID_DIGIT));
            }
            scratch.push(self.char());
        }
        // The final bump just moves the parser past the literal, which may
        // be EOF.
        self.bump_and_bump_space();
        match u32::from_str_radix(&scratch, 16).ok().and_then(char::from_u32) {
            None => Err(Error::new(ERR_HEX_FIXED_INVALID)),
            Some(ch) => Ok(self.hir_char(ch)),
        }
    }

    /// Parse a hex representation of any Unicode scalar value. This expects
    /// the parser to be positioned at the opening brace `{` and will advance
    /// the parser to the first character following the closing brace `}`.
    fn parse_hex_brace(&self) -> Result<Hir, Error> {
        let mut scratch = String::new();
        while self.bump_and_bump_space() && self.char() != '}' {
            if !is_hex(self.char()) {
                return Err(Error::new(ERR_HEX_BRACE_INVALID_DIGIT));
            }
            scratch.push(self.char());
        }
        if self.is_done() {
            return Err(Error::new(ERR_HEX_BRACE_UNEXPECTED_EOF));
        }
        assert_eq!(self.char(), '}');
        self.bump_and_bump_space();

        if scratch.is_empty() {
            return Err(Error::new(ERR_HEX_BRACE_EMPTY));
        }
        match u32::from_str_radix(&scratch, 16).ok().and_then(char::from_u32) {
            None => Err(Error::new(ERR_HEX_BRACE_INVALID)),
            Some(ch) => Ok(self.hir_char(ch)),
        }
    }

    /// Parse a decimal number into a u32 while trimming leading and trailing
    /// whitespace.
    ///
    /// This expects the parser to be positioned at the first position where
    /// a decimal digit could occur. This will advance the parser to the byte
    /// immediately following the last contiguous decimal digit.
    ///
    /// If no decimal digit could be found or if there was a problem parsing
    /// the complete set of digits into a u32, then an error is returned.
    fn parse_decimal(&self) -> Result<u32, Error> {
        let mut scratch = String::new();
        while !self.is_done() && self.char().is_whitespace() {
            self.bump();
        }
        while !self.is_done() && '0' <= self.char() && self.char() <= '9' {
            scratch.push(self.char());
            self.bump_and_bump_space();
        }
        while !self.is_done() && self.char().is_whitespace() {
            self.bump_and_bump_space();
        }
        let digits = scratch.as_str();
        if digits.is_empty() {
            return Err(Error::new(ERR_DECIMAL_NO_DIGITS));
        }
        match u32::from_str_radix(digits, 10).ok() {
            Some(n) => Ok(n),
            None => Err(Error::new(ERR_DECIMAL_INVALID)),
        }
    }

    /// Parses an uncounted repetition operator. An uncounted repetition
    /// operator includes `?`, `*` and `+`, but does not include the `{m,n}`
    /// syntax. The current character should be one of `?`, `*` or `+`. Any
    /// other character will result in a panic.
    ///
    /// This assumes that the parser is currently positioned at the repetition
    /// operator and advances the parser to the first character after the
    /// operator. (Note that the operator may include a single additional `?`,
    /// which makes the operator ungreedy.)
    ///
    /// The caller should include the concatenation that is being built. The
    /// concatenation returned includes the repetition operator applied to the
    /// last expression in the given concatenation.
    ///
    /// If the concatenation is empty, then this returns an error.
    fn parse_uncounted_repetition(
        &self,
        mut concat: Vec<Hir>,
    ) -> Result<Vec<Hir>, Error> {
        let sub = match concat.pop() {
            Some(hir) => Box::new(hir),
            None => {
                return Err(Error::new(ERR_UNCOUNTED_REP_SUB_MISSING));
            }
        };
        let (min, max) = match self.char() {
            '?' => (0, Some(1)),
            '*' => (0, None),
            '+' => (1, None),
            unk => unreachable!("unrecognized repetition operator '{}'", unk),
        };
        let mut greedy = true;
        if self.bump() && self.char() == '?' {
            greedy = false;
            self.bump();
        }
        if self.flags().swap_greed {
            greedy = !greedy;
        }
        concat.push(Hir::repetition(hir::Repetition {
            min,
            max,
            greedy,
            sub,
        }));
        Ok(concat)
    }

    /// Parses a counted repetition operation. A counted repetition operator
    /// corresponds to the `{m,n}` syntax, and does not include the `?`, `*` or
    /// `+` operators.
    ///
    /// This assumes that the parser is currently at the opening `{` and
    /// advances the parser to the first character after the operator. (Note
    /// that the operator may include a single additional `?`, which makes the
    /// operator ungreedy.)
    ///
    /// The caller should include the concatenation that is being built. The
    /// concatenation returned includes the repetition operator applied to the
    /// last expression in the given concatenation.
    ///
    /// If the concatenation is empty, then this returns an error.
    fn parse_counted_repetition(
        &self,
        mut concat: Vec<Hir>,
    ) -> Result<Vec<Hir>, Error> {
        assert_eq!(self.char(), '{', "expected opening brace");
        let sub = match concat.pop() {
            Some(hir) => Box::new(hir),
            None => {
                return Err(Error::new(ERR_COUNTED_REP_SUB_MISSING));
            }
        };
        if !self.bump_and_bump_space() {
            return Err(Error::new(ERR_COUNTED_REP_UNCLOSED));
        }
        let min = self.parse_decimal()?;
        let mut max = Some(min);
        if self.is_done() {
            return Err(Error::new(ERR_COUNTED_REP_MIN_UNCLOSED));
        }
        if self.char() == ',' {
            if !self.bump_and_bump_space() {
                return Err(Error::new(ERR_COUNTED_REP_COMMA_UNCLOSED));
            }
            if self.char() != '}' {
                max = Some(self.parse_decimal()?);
            } else {
                max = None;
            }
            if self.is_done() {
                return Err(Error::new(ERR_COUNTED_REP_MIN_MAX_UNCLOSED));
            }
        }
        if self.char() != '}' {
            return Err(Error::new(ERR_COUNTED_REP_INVALID));
        }

        let mut greedy = true;
        if self.bump_and_bump_space() && self.char() == '?' {
            greedy = false;
            self.bump();
        }
        if self.flags().swap_greed {
            greedy = !greedy;
        }

        if max.map_or(false, |max| min > max) {
            return Err(Error::new(ERR_COUNTED_REP_INVALID_RANGE));
        }
        concat.push(Hir::repetition(hir::Repetition {
            min,
            max,
            greedy,
            sub,
        }));
        Ok(concat)
    }

    /// Parses the part of a pattern that starts with a `(`. This is usually
    /// a group sub-expression, but might just be a directive that enables
    /// (or disables) certain flags.
    ///
    /// This assumes the parser is pointing at the opening `(`.
    fn parse_group(&self) -> Result<Option<Hir>, Error> {
        assert_eq!(self.char(), '(');
        self.bump_and_bump_space();
        if self.is_lookaround_prefix() {
            return Err(Error::new(ERR_LOOK_UNSUPPORTED));
        }
        if self.bump_if("?P<") || self.bump_if("?<") {
            let index = self.next_capture_index()?;
            let name = Some(Box::from(self.parse_capture_name()?));
            let sub = Box::new(self.parse_inner()?);
            let cap = hir::Capture { index, name, sub };
            Ok(Some(Hir::capture(cap)))
        } else if self.bump_if("?") {
            if self.is_done() {
                return Err(Error::new(ERR_UNCLOSED_GROUP_QUESTION));
            }
            let start = self.pos();
            // The flags get reset in the top-level 'parse' routine.
            *self.flags.borrow_mut() = self.parse_flags()?;
            let consumed = self.pos() - start;
            if self.char() == ')' {
                // We don't allow empty flags, e.g., `(?)`.
                if consumed == 0 {
                    return Err(Error::new(ERR_EMPTY_FLAGS));
                }
                Ok(None)
            } else {
                assert_eq!(':', self.char());
                self.bump();
                self.parse_inner().map(Some)
            }
        } else {
            let index = self.next_capture_index()?;
            let sub = Box::new(self.parse_inner()?);
            let cap = hir::Capture { index, name: None, sub };
            Ok(Some(Hir::capture(cap)))
        }
    }

    /// Parses a capture group name. Assumes that the parser is positioned at
    /// the first character in the name following the opening `<` (and may
    /// possibly be EOF). This advances the parser to the first character
    /// following the closing `>`.
    fn parse_capture_name(&self) -> Result<&str, Error> {
        if self.is_done() {
            return Err(Error::new(ERR_MISSING_GROUP_NAME));
        }
        let start = self.pos();
        loop {
            if self.char() == '>' {
                break;
            }
            if !is_capture_char(self.char(), self.pos() == start) {
                return Err(Error::new(ERR_INVALID_GROUP_NAME));
            }
            if !self.bump() {
                break;
            }
        }
        let end = self.pos();
        if self.is_done() {
            return Err(Error::new(ERR_UNCLOSED_GROUP_NAME));
        }
        assert_eq!(self.char(), '>');
        self.bump();
        let name = &self.pattern()[start..end];
        if name.is_empty() {
            return Err(Error::new(ERR_EMPTY_GROUP_NAME));
        }
        self.add_capture_name(name)?;
        Ok(name)
    }

    /// Parse a sequence of flags starting at the current character.
    ///
    /// This advances the parser to the character immediately following the
    /// flags, which is guaranteed to be either `:` or `)`.
    ///
    /// # Errors
    ///
    /// If any flags are duplicated, then an error is returned.
    ///
    /// If the negation operator is used more than once, then an error is
    /// returned.
    ///
    /// If no flags could be found or if the negation operation is not followed
    /// by any flags, then an error is returned.
    fn parse_flags(&self) -> Result<Flags, Error> {
        let mut flags = *self.flags.borrow();
        let mut negate = false;
        // Keeps track of whether the previous flag item was a '-'. We use this
        // to detect whether there is a dangling '-', which is invalid.
        let mut last_was_negation = false;
        // A set to keep track of the flags we've seen. Since all flags are
        // ASCII, we only need 128 bytes.
        let mut seen = [false; 128];
        while self.char() != ':' && self.char() != ')' {
            if self.char() == '-' {
                last_was_negation = true;
                if negate {
                    return Err(Error::new(ERR_FLAG_REPEATED_NEGATION));
                }
                negate = true;
            } else {
                last_was_negation = false;
                self.parse_flag(&mut flags, negate)?;
                // OK because every valid flag is ASCII, and we're only here if
                // the flag is valid.
                let flag_byte = u8::try_from(self.char()).unwrap();
                if seen[usize::from(flag_byte)] {
                    return Err(Error::new(ERR_FLAG_DUPLICATE));
                }
                seen[usize::from(flag_byte)] = true;
            }
            if !self.bump() {
                return Err(Error::new(ERR_FLAG_UNEXPECTED_EOF));
            }
        }
        if last_was_negation {
            return Err(Error::new(ERR_FLAG_DANGLING_NEGATION));
        }
        Ok(flags)
    }

    /// Parse the current character as a flag. Do not advance the parser.
    ///
    /// This sets the appropriate boolean value in place on the set of flags
    /// given. The boolean is inverted when `negate` is true.
    ///
    /// # Errors
    ///
    /// If the flag is not recognized, then an error is returned.
    fn parse_flag(
        &self,
        flags: &mut Flags,
        negate: bool,
    ) -> Result<(), Error> {
        let enabled = !negate;
        match self.char() {
            'i' => flags.case_insensitive = enabled,
            'm' => flags.multi_line = enabled,
            's' => flags.dot_matches_new_line = enabled,
            'U' => flags.swap_greed = enabled,
            'R' => flags.crlf = enabled,
            'x' => flags.ignore_whitespace = enabled,
            // We make a special exception for this flag where we let it
            // through as a recognized flag, but treat it as a no-op. This in
            // practice retains some compatibility with the regex crate. It is
            // a little suspect to do this, but for example, '(?-u:\b).+' in
            // the regex crate is equivalent to '\b.+' in regex-lite.
            'u' => {}
            _ => return Err(Error::new(ERR_FLAG_UNRECOGNIZED)),
        }
        Ok(())
    }

    /// Parse a standard character class consisting primarily of characters or
    /// character ranges.
    ///
    /// This assumes the parser is positioned at the opening `[`. If parsing
    /// is successful, then the parser is advanced to the position immediately
    /// following the closing `]`.
    fn parse_class(&self) -> Result<Hir, Error> {
        assert_eq!(self.char(), '[');

        let mut union = vec![];
        if !self.bump_and_bump_space() {
            return Err(Error::new(ERR_CLASS_UNCLOSED));
        }
        // Determine whether the class is negated or not.
        let negate = if self.char() != '^' {
            false
        } else {
            if !self.bump_and_bump_space() {
                return Err(Error::new(ERR_CLASS_UNCLOSED_AFTER_NEGATION));
            }
            true
        };
        // Accept any number of `-` as literal `-`.
        while self.char() == '-' {
            union.push(hir::ClassRange { start: '-', end: '-' });
            if !self.bump_and_bump_space() {
                return Err(Error::new(ERR_CLASS_UNCLOSED_AFTER_DASH));
            }
        }
        // If `]` is the *first* char in a set, then interpret it as a literal
        // `]`. That is, an empty class is impossible to write.
        if union.is_empty() && self.char() == ']' {
            union.push(hir::ClassRange { start: ']', end: ']' });
            if !self.bump_and_bump_space() {
                return Err(Error::new(ERR_CLASS_UNCLOSED_AFTER_CLOSING));
            }
        }
        loop {
            self.bump_space();
            if self.is_done() {
                return Err(Error::new(ERR_CLASS_UNCLOSED));
            }
            match self.char() {
                '[' => {
                    // Attempt to treat this as the beginning of a POSIX class.
                    // If POSIX class parsing fails, then the parser backs up
                    // to `[`.
                    if let Some(class) = self.maybe_parse_posix_class() {
                        union.extend_from_slice(&class.ranges);
                        continue;
                    }
                    // ... otherwise we don't support nested classes.
                    return Err(Error::new(ERR_CLASS_NEST_UNSUPPORTED));
                }
                ']' => {
                    self.bump();
                    let mut class = hir::Class::new(union);
                    // Note that we must apply case folding before negation!
                    // Consider `(?i)[^x]`. If we applied negation first, then
                    // the result would be the character class that matched any
                    // Unicode scalar value.
                    if self.flags().case_insensitive {
                        class.ascii_case_fold();
                    }
                    if negate {
                        class.negate();
                    }
                    return Ok(Hir::class(class));
                }
                '&' if self.peek() == Some('&') => {
                    return Err(Error::new(
                        ERR_CLASS_INTERSECTION_UNSUPPORTED,
                    ));
                }
                '-' if self.peek() == Some('-') => {
                    return Err(Error::new(ERR_CLASS_DIFFERENCE_UNSUPPORTED));
                }
                '~' if self.peek() == Some('~') => {
                    return Err(Error::new(
                        ERR_CLASS_SYMDIFFERENCE_UNSUPPORTED,
                    ));
                }
                _ => self.parse_class_range(&mut union)?,
            }
        }
    }

    /// Parse a single primitive item in a character class set. The item to
    /// be parsed can either be one of a simple literal character, a range
    /// between two simple literal characters or a "primitive" character
    /// class like `\w`.
    ///
    /// If an invalid escape is found, or if a character class is found where
    /// a simple literal is expected (e.g., in a range), then an error is
    /// returned.
    ///
    /// Otherwise, the range (or ranges) are appended to the given union of
    /// ranges.
    fn parse_class_range(
        &self,
        union: &mut Vec<hir::ClassRange>,
    ) -> Result<(), Error> {
        let prim1 = self.parse_class_item()?;
        self.bump_space();
        if self.is_done() {
            return Err(Error::new(ERR_CLASS_UNCLOSED_AFTER_ITEM));
        }
        // If the next char isn't a `-`, then we don't have a range.
        // There are two exceptions. If the char after a `-` is a `]`, then
        // `-` is interpreted as a literal `-`. Alternatively, if the char
        // after a `-` is a `-`, then `--` corresponds to a "difference"
        // operation. (Which we don't support in regex-lite, but error about
        // specifically in an effort to be loud about differences between the
        // main regex crate where possible.)
        if self.char() != '-'
            || self.peek_space() == Some(']')
            || self.peek_space() == Some('-')
        {
            union.extend_from_slice(&into_class_item_ranges(prim1)?);
            return Ok(());
        }
        // OK, now we're parsing a range, so bump past the `-` and parse the
        // second half of the range.
        if !self.bump_and_bump_space() {
            return Err(Error::new(ERR_CLASS_UNCLOSED_AFTER_DASH));
        }
        let prim2 = self.parse_class_item()?;
        let range = hir::ClassRange {
            start: into_class_item_range(prim1)?,
            end: into_class_item_range(prim2)?,
        };
        if range.start > range.end {
            return Err(Error::new(ERR_CLASS_INVALID_RANGE));
        }
        union.push(range);
        Ok(())
    }

    /// Parse a single item in a character class as a primitive, where the
    /// primitive either consists of a verbatim literal or a single escape
    /// sequence.
    ///
    /// This assumes the parser is positioned at the beginning of a primitive,
    /// and advances the parser to the first position after the primitive if
    /// successful.
    ///
    /// Note that it is the caller's responsibility to report an error if an
    /// illegal primitive was parsed.
    fn parse_class_item(&self) -> Result<Hir, Error> {
        let ch = self.char();
        self.bump();
        if ch == '\\' {
            self.parse_escape()
        } else {
            Ok(Hir::char(ch))
        }
    }

    /// Attempt to parse a POSIX character class, e.g., `[:alnum:]`.
    ///
    /// This assumes the parser is positioned at the opening `[`.
    ///
    /// If no valid POSIX character class could be found, then this does not
    /// advance the parser and `None` is returned. Otherwise, the parser is
    /// advanced to the first byte following the closing `]` and the
    /// corresponding POSIX class is returned.
    fn maybe_parse_posix_class(&self) -> Option<hir::Class> {
        // POSIX character classes are interesting from a parsing perspective
        // because parsing cannot fail with any interesting error. For example,
        // in order to use an POSIX character class, it must be enclosed in
        // double brackets, e.g., `[[:alnum:]]`. Alternatively, you might think
        // of it as "POSIX character classes have the syntax `[:NAME:]` which
        // can only appear within character brackets." This means that things
        // like `[[:lower:]A]` are legal constructs.
        //
        // However, if one types an incorrect POSIX character class, e.g.,
        // `[[:loower:]]`, then we treat that as if it were normal nested
        // character class containing the characters `:elorw`. (Which isn't
        // supported and results in an error in regex-lite.) One might argue
        // that we should return an error instead since the repeated colons
        // give away the intent to write an POSIX class. But what if the user
        // typed `[[:lower]]` instead? How can we tell that was intended to be
        // a POSIX class and not just a normal nested class?
        //
        // Reasonable people can probably disagree over this, but for better
        // or worse, we implement semantics that never fails at the expense of
        // better failure modes.
        assert_eq!(self.char(), '[');

        // If parsing fails, then we back up the parser to this starting point.
        let start_pos = self.pos();
        let start_char = self.char.get();
        let reset = || {
            self.pos.set(start_pos);
            self.char.set(start_char);
        };

        let mut negated = false;
        if !self.bump() || self.char() != ':' {
            reset();
            return None;
        }
        if !self.bump() {
            reset();
            return None;
        }
        if self.char() == '^' {
            negated = true;
            if !self.bump() {
                reset();
                return None;
            }
        }
        let name_start = self.pos();
        while self.char() != ':' && self.bump() {}
        if self.is_done() {
            reset();
            return None;
        }
        let name = &self.pattern()[name_start..self.pos()];
        if !self.bump_if(":]") {
            reset();
            return None;
        }
        if let Ok(ranges) = posix_class(name) {
            let mut class = hir::Class::new(ranges);
            if negated {
                class.negate();
            }
            return Some(class);
        }
        reset();
        None
    }

    /// Parse a Perl character class, e.g., `\d` or `\W`. This assumes the
    /// parser is currently at a valid character class name and will be
    /// advanced to the character immediately following the class.
    fn parse_perl_class(&self) -> Hir {
        let ch = self.char();
        self.bump();
        let mut class = hir::Class::new(match ch {
            'd' | 'D' => posix_class("digit").unwrap(),
            's' | 'S' => posix_class("space").unwrap(),
            'w' | 'W' => posix_class("word").unwrap(),
            unk => unreachable!("invalid Perl class \\{}", unk),
        });
        if ch.is_ascii_uppercase() {
            class.negate();
        }
        Hir::class(class)
    }

    fn hir_dot(&self) -> Hir {
        if self.flags().dot_matches_new_line {
            Hir::class(hir::Class::new([hir::ClassRange {
                start: '\x00',
                end: '\u{10FFFF}',
            }]))
        } else if self.flags().crlf {
            Hir::class(hir::Class::new([
                hir::ClassRange { start: '\x00', end: '\x09' },
                hir::ClassRange { start: '\x0B', end: '\x0C' },
                hir::ClassRange { start: '\x0E', end: '\u{10FFFF}' },
            ]))
        } else {
            Hir::class(hir::Class::new([
                hir::ClassRange { start: '\x00', end: '\x09' },
                hir::ClassRange { start: '\x0B', end: '\u{10FFFF}' },
            ]))
        }
    }

    fn hir_anchor_start(&self) -> Hir {
        let look = if self.flags().multi_line {
            if self.flags().crlf {
                hir::Look::StartCRLF
            } else {
                hir::Look::StartLF
            }
        } else {
            hir::Look::Start
        };
        Hir::look(look)
    }

    fn hir_anchor_end(&self) -> Hir {
        let look = if self.flags().multi_line {
            if self.flags().crlf {
                hir::Look::EndCRLF
            } else {
                hir::Look::EndLF
            }
        } else {
            hir::Look::End
        };
        Hir::look(look)
    }

    fn hir_char(&self, ch: char) -> Hir {
        if self.flags().case_insensitive {
            let this = hir::ClassRange { start: ch, end: ch };
            if let Some(folded) = this.ascii_case_fold() {
                return Hir::class(hir::Class::new([this, folded]));
            }
        }
        Hir::char(ch)
    }
}

/// This checks the depth of the given `Hir` value, and if it exceeds the given
/// limit, then an error is returned.
fn check_hir_nesting(hir: &Hir, limit: u32) -> Result<(), Error> {
    fn recurse(hir: &Hir, limit: u32, depth: u32) -> Result<(), Error> {
        if depth > limit {
            return Err(Error::new(ERR_TOO_MUCH_NESTING));
        }
        let Some(next_depth) = depth.checked_add(1) else {
            return Err(Error::new(ERR_TOO_MUCH_NESTING));
        };
        match *hir.kind() {
            HirKind::Empty
            | HirKind::Char(_)
            | HirKind::Class(_)
            | HirKind::Look(_) => Ok(()),
            HirKind::Repetition(hir::Repetition { ref sub, .. }) => {
                recurse(sub, limit, next_depth)
            }
            HirKind::Capture(hir::Capture { ref sub, .. }) => {
                recurse(sub, limit, next_depth)
            }
            HirKind::Concat(ref subs) | HirKind::Alternation(ref subs) => {
                for sub in subs.iter() {
                    recurse(sub, limit, next_depth)?;
                }
                Ok(())
            }
        }
    }
    recurse(hir, limit, 0)
}

/// Converts the given Hir to a literal char if the Hir is just a single
/// character. Otherwise this returns an error.
///
/// This is useful in contexts where you can only accept a single character,
/// but where it is convenient to parse something more general. For example,
/// parsing a single part of a character class range. It's useful to reuse
/// the literal parsing code, but that code can itself return entire classes
/// which can't be used as the start/end of a class range.
fn into_class_item_range(hir: Hir) -> Result<char, Error> {
    match hir.kind {
        HirKind::Char(ch) => Ok(ch),
        _ => Err(Error::new(ERR_CLASS_INVALID_RANGE_ITEM)),
    }
}

fn into_class_item_ranges(
    mut hir: Hir,
) -> Result<Vec<hir::ClassRange>, Error> {
    match core::mem::replace(&mut hir.kind, HirKind::Empty) {
        HirKind::Char(ch) => Ok(vec![hir::ClassRange { start: ch, end: ch }]),
        HirKind::Class(hir::Class { ranges }) => Ok(ranges),
        _ => Err(Error::new(ERR_CLASS_INVALID_ITEM)),
    }
}

/// Returns an iterator of character class ranges for the given named POSIX
/// character class. If no such character class exists for the name given, then
/// an error is returned.
fn posix_class(
    kind: &str,
) -> Result<impl Iterator<Item = hir::ClassRange>, Error> {
    let slice: &'static [(u8, u8)] = match kind {
        "alnum" => &[(b'0', b'9'), (b'A', b'Z'), (b'a', b'z')],
        "alpha" => &[(b'A', b'Z'), (b'a', b'z')],
        "ascii" => &[(b'\x00', b'\x7F')],
        "blank" => &[(b'\t', b'\t'), (b' ', b' ')],
        "cntrl" => &[(b'\x00', b'\x1F'), (b'\x7F', b'\x7F')],
        "digit" => &[(b'0', b'9')],
        "graph" => &[(b'!', b'~')],
        "lower" => &[(b'a', b'z')],
        "print" => &[(b' ', b'~')],
        "punct" => &[(b'!', b'/'), (b':', b'@'), (b'[', b'`'), (b'{', b'~')],
        "space" => &[
            (b'\t', b'\t'),
            (b'\n', b'\n'),
            (b'\x0B', b'\x0B'),
            (b'\x0C', b'\x0C'),
            (b'\r', b'\r'),
            (b' ', b' '),
        ],
        "upper" => &[(b'A', b'Z')],
        "word" => &[(b'0', b'9'), (b'A', b'Z'), (b'_', b'_'), (b'a', b'z')],
        "xdigit" => &[(b'0', b'9'), (b'A', b'F'), (b'a', b'f')],
        _ => return Err(Error::new(ERR_POSIX_CLASS_UNRECOGNIZED)),
    };
    Ok(slice.iter().map(|&(start, end)| hir::ClassRange {
        start: char::from(start),
        end: char::from(end),
    }))
}

/// Returns true if the given character is a hexadecimal digit.
fn is_hex(c: char) -> bool {
    ('0' <= c && c <= '9') || ('a' <= c && c <= 'f') || ('A' <= c && c <= 'F')
}

/// Returns true if the given character is a valid in a capture group name.
///
/// If `first` is true, then `c` is treated as the first character in the
/// group name (which must be alphabetic or underscore).
fn is_capture_char(c: char, first: bool) -> bool {
    if first {
        c == '_' || c.is_alphabetic()
    } else {
        c == '_' || c == '.' || c == '[' || c == ']' || c.is_alphanumeric()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pattern: &str) -> Hir {
        Parser::new(Config::default(), pattern).parse_inner().unwrap()
    }

    fn perr(pattern: &str) -> String {
        Parser::new(Config::default(), pattern)
            .parse_inner()
            .unwrap_err()
            .to_string()
    }

    fn class<I: IntoIterator<Item = (char, char)>>(it: I) -> Hir {
        Hir::class(hir::Class::new(
            it.into_iter().map(|(start, end)| hir::ClassRange { start, end }),
        ))
    }

    fn singles<I: IntoIterator<Item = char>>(it: I) -> Hir {
        Hir::class(hir::Class::new(
            it.into_iter().map(|ch| hir::ClassRange { start: ch, end: ch }),
        ))
    }

    fn posix(name: &str) -> Hir {
        Hir::class(hir::Class::new(posix_class(name).unwrap()))
    }

    fn cap(index: u32, sub: Hir) -> Hir {
        Hir::capture(hir::Capture { index, name: None, sub: Box::new(sub) })
    }

    fn named_cap(index: u32, name: &str, sub: Hir) -> Hir {
        Hir::capture(hir::Capture {
            index,
            name: Some(Box::from(name)),
            sub: Box::new(sub),
        })
    }

    #[test]
    fn ok_literal() {
        assert_eq!(p("a"), Hir::char('a'));
        assert_eq!(p("ab"), Hir::concat(vec![Hir::char('a'), Hir::char('b')]));
        assert_eq!(p("💩"), Hir::char('💩'));
    }

    #[test]
    fn ok_meta_escapes() {
        assert_eq!(p(r"\*"), Hir::char('*'));
        assert_eq!(p(r"\+"), Hir::char('+'));
        assert_eq!(p(r"\?"), Hir::char('?'));
        assert_eq!(p(r"\|"), Hir::char('|'));
        assert_eq!(p(r"\("), Hir::char('('));
        assert_eq!(p(r"\)"), Hir::char(')'));
        assert_eq!(p(r"\^"), Hir::char('^'));
        assert_eq!(p(r"\$"), Hir::char('$'));
        assert_eq!(p(r"\["), Hir::char('['));
        assert_eq!(p(r"\]"), Hir::char(']'));
    }

    #[test]
    fn ok_special_escapes() {
        assert_eq!(p(r"\a"), Hir::char('\x07'));
        assert_eq!(p(r"\f"), Hir::char('\x0C'));
        assert_eq!(p(r"\t"), Hir::char('\t'));
        assert_eq!(p(r"\n"), Hir::char('\n'));
        assert_eq!(p(r"\r"), Hir::char('\r'));
        assert_eq!(p(r"\v"), Hir::char('\x0B'));
        assert_eq!(p(r"\A"), Hir::look(hir::Look::Start));
        assert_eq!(p(r"\z"), Hir::look(hir::Look::End));
        assert_eq!(p(r"\b"), Hir::look(hir::Look::Word));
        assert_eq!(p(r"\B"), Hir::look(hir::Look::WordNegate));
    }

    #[test]
    fn ok_hex() {
        // fixed length
        assert_eq!(p(r"\x41"), Hir::char('A'));
        assert_eq!(p(r"\u2603"), Hir::char('☃'));
        assert_eq!(p(r"\U0001F4A9"), Hir::char('💩'));
        // braces
        assert_eq!(p(r"\x{1F4A9}"), Hir::char('💩'));
        assert_eq!(p(r"\u{1F4A9}"), Hir::char('💩'));
        assert_eq!(p(r"\U{1F4A9}"), Hir::char('💩'));
    }

    #[test]
    fn ok_perl() {
        assert_eq!(p(r"\d"), posix("digit"));
        assert_eq!(p(r"\s"), posix("space"));
        assert_eq!(p(r"\w"), posix("word"));

        let negated = |name| {
            let mut class = hir::Class::new(posix_class(name).unwrap());
            class.negate();
            Hir::class(class)
        };
        assert_eq!(p(r"\D"), negated("digit"));
        assert_eq!(p(r"\S"), negated("space"));
        assert_eq!(p(r"\W"), negated("word"));
    }

    #[test]
    fn ok_flags_and_primitives() {
        assert_eq!(p(r"a"), Hir::char('a'));
        assert_eq!(p(r"(?i:a)"), singles(['A', 'a']));

        assert_eq!(p(r"^"), Hir::look(hir::Look::Start));
        assert_eq!(p(r"(?m:^)"), Hir::look(hir::Look::StartLF));
        assert_eq!(p(r"(?mR:^)"), Hir::look(hir::Look::StartCRLF));

        assert_eq!(p(r"$"), Hir::look(hir::Look::End));
        assert_eq!(p(r"(?m:$)"), Hir::look(hir::Look::EndLF));
        assert_eq!(p(r"(?mR:$)"), Hir::look(hir::Look::EndCRLF));

        assert_eq!(p(r"."), class([('\x00', '\x09'), ('\x0B', '\u{10FFFF}')]));
        assert_eq!(
            p(r"(?R:.)"),
            class([
                ('\x00', '\x09'),
                ('\x0B', '\x0C'),
                ('\x0E', '\u{10FFFF}'),
            ])
        );
        assert_eq!(p(r"(?s:.)"), class([('\x00', '\u{10FFFF}')]));
        assert_eq!(p(r"(?sR:.)"), class([('\x00', '\u{10FFFF}')]));
    }

    #[test]
    fn ok_alternate() {
        assert_eq!(
            p(r"a|b"),
            Hir::alternation(vec![Hir::char('a'), Hir::char('b')])
        );
        assert_eq!(
            p(r"(?:a|b)"),
            Hir::alternation(vec![Hir::char('a'), Hir::char('b')])
        );

        assert_eq!(
            p(r"(a|b)"),
            cap(1, Hir::alternation(vec![Hir::char('a'), Hir::char('b')]))
        );
        assert_eq!(
            p(r"(?<foo>a|b)"),
            named_cap(
                1,
                "foo",
                Hir::alternation(vec![Hir::char('a'), Hir::char('b')])
            )
        );

        assert_eq!(
            p(r"a|b|c"),
            Hir::alternation(vec![
                Hir::char('a'),
                Hir::char('b'),
                Hir::char('c')
            ])
        );

        assert_eq!(
            p(r"ax|by|cz"),
            Hir::alternation(vec![
                Hir::concat(vec![Hir::char('a'), Hir::char('x')]),
                Hir::concat(vec![Hir::char('b'), Hir::char('y')]),
                Hir::concat(vec![Hir::char('c'), Hir::char('z')]),
            ])
        );
        assert_eq!(
            p(r"(ax|(by|(cz)))"),
            cap(
                1,
                Hir::alternation(vec![
                    Hir::concat(vec![Hir::char('a'), Hir::char('x')]),
                    cap(
                        2,
                        Hir::alternation(vec![
                            Hir::concat(vec![Hir::char('b'), Hir::char('y')]),
                            cap(
                                3,
                                Hir::concat(vec![
                                    Hir::char('c'),
                                    Hir::char('z')
                                ])
                            ),
                        ])
                    ),
                ])
            )
        );

        assert_eq!(
            p(r"|"),
            Hir::alternation(vec![Hir::empty(), Hir::empty()])
        );
        assert_eq!(
            p(r"||"),
            Hir::alternation(vec![Hir::empty(), Hir::empty(), Hir::empty()])
        );

        assert_eq!(
            p(r"a|"),
            Hir::alternation(vec![Hir::char('a'), Hir::empty()])
        );
        assert_eq!(
            p(r"|a"),
            Hir::alternation(vec![Hir::empty(), Hir::char('a')])
        );

        assert_eq!(
            p(r"(|)"),
            cap(1, Hir::alternation(vec![Hir::empty(), Hir::empty()]))
        );
        assert_eq!(
            p(r"(a|)"),
            cap(1, Hir::alternation(vec![Hir::char('a'), Hir::empty()]))
        );
        assert_eq!(
            p(r"(|a)"),
            cap(1, Hir::alternation(vec![Hir::empty(), Hir::char('a')]))
        );
    }

    #[test]
    fn ok_flag_group() {
        assert_eq!(
            p("a(?i:b)"),
            Hir::concat(vec![Hir::char('a'), singles(['B', 'b'])])
        );
    }

    #[test]
    fn ok_flag_directive() {
        assert_eq!(p("(?i)a"), singles(['A', 'a']));
        assert_eq!(p("a(?i)"), Hir::char('a'));
        assert_eq!(
            p("a(?i)b"),
            Hir::concat(vec![Hir::char('a'), singles(['B', 'b'])])
        );
        assert_eq!(
            p("a(?i)a(?-i)a"),
            Hir::concat(vec![
                Hir::char('a'),
                singles(['A', 'a']),
                Hir::char('a'),
            ])
        );
        assert_eq!(
            p("a(?:(?i)a)a"),
            Hir::concat(vec![
                Hir::char('a'),
                singles(['A', 'a']),
                Hir::char('a'),
            ])
        );
        assert_eq!(
            p("a((?i)a)a"),
            Hir::concat(vec![
                Hir::char('a'),
                cap(1, singles(['A', 'a'])),
                Hir::char('a'),
            ])
        );
    }

    #[test]
    fn ok_uncounted_repetition() {
        assert_eq!(
            p(r"a?"),
            Hir::repetition(hir::Repetition {
                min: 0,
                max: Some(1),
                greedy: true,
                sub: Box::new(Hir::char('a')),
            }),
        );
        assert_eq!(
            p(r"a*"),
            Hir::repetition(hir::Repetition {
                min: 0,
                max: None,
                greedy: true,
                sub: Box::new(Hir::char('a')),
            }),
        );
        assert_eq!(
            p(r"a+"),
            Hir::repetition(hir::Repetition {
                min: 1,
                max: None,
                greedy: true,
                sub: Box::new(Hir::char('a')),
            }),
        );

        assert_eq!(
            p(r"a??"),
            Hir::repetition(hir::Repetition {
                min: 0,
                max: Some(1),
                greedy: false,
                sub: Box::new(Hir::char('a')),
            }),
        );
        assert_eq!(
            p(r"a*?"),
            Hir::repetition(hir::Repetition {
                min: 0,
                max: None,
                greedy: false,
                sub: Box::new(Hir::char('a')),
            }),
        );
        assert_eq!(
            p(r"a+?"),
            Hir::repetition(hir::Repetition {
                min: 1,
                max: None,
                greedy: false,
                sub: Box::new(Hir::char('a')),
            }),
        );

        assert_eq!(
            p(r"a?b"),
            Hir::concat(vec![
                Hir::repetition(hir::Repetition {
                    min: 0,
                    max: Some(1),
                    greedy: true,
                    sub: Box::new(Hir::char('a')),
                }),
                Hir::char('b'),
            ]),
        );

        assert_eq!(
            p(r"ab?"),
            Hir::concat(vec![
                Hir::char('a'),
                Hir::repetition(hir::Repetition {
                    min: 0,
                    max: Some(1),
                    greedy: true,
                    sub: Box::new(Hir::char('b')),
                }),
            ]),
        );

        assert_eq!(
            p(r"(?:ab)?"),
            Hir::repetition(hir::Repetition {
                min: 0,
                max: Some(1),
                greedy: true,
                sub: Box::new(Hir::concat(vec![
                    Hir::char('a'),
                    Hir::char('b')
                ])),
            }),
        );

        assert_eq!(
            p(r"(ab)?"),
            Hir::repetition(hir::Repetition {
                min: 0,
                max: Some(1),
                greedy: true,
                sub: Box::new(cap(
                    1,
                    Hir::concat(vec![Hir::char('a'), Hir::char('b')])
                )),
            }),
        );

        assert_eq!(
            p(r"|a?"),
            Hir::alternation(vec![
                Hir::empty(),
                Hir::repetition(hir::Repetition {
                    min: 0,
                    max: Some(1),
                    greedy: true,
                    sub: Box::new(Hir::char('a')),
                })
            ]),
        );
    }

    #[test]
    fn ok_counted_repetition() {
        assert_eq!(
            p(r"a{5}"),
            Hir::repetition(hir::Repetition {
                min: 5,
                max: Some(5),
                greedy: true,
                sub: Box::new(Hir::char('a')),
            }),
        );
        assert_eq!(
            p(r"a{5}?"),
            Hir::repetition(hir::Repetition {
                min: 5,
                max: Some(5),
                greedy: false,
                sub: Box::new(Hir::char('a')),
            }),
        );

        assert_eq!(
            p(r"a{5,}"),
            Hir::repetition(hir::Repetition {
                min: 5,
                max: None,
                greedy: true,
                sub: Box::new(Hir::char('a')),
            }),
        );

        assert_eq!(
            p(r"a{5,9}"),
            Hir::repetition(hir::Repetition {
                min: 5,
                max: Some(9),
                greedy: true,
                sub: Box::new(Hir::char('a')),
            }),
        );

        assert_eq!(
            p(r"ab{5}c"),
            Hir::concat(vec![
                Hir::char('a'),
                Hir::repetition(hir::Repetition {
                    min: 5,
                    max: Some(5),
                    greedy: true,
                    sub: Box::new(Hir::char('b')),
                }),
                Hir::char('c'),
            ]),
        );

        assert_eq!(
            p(r"a{ 5 }"),
            Hir::repetition(hir::Repetition {
                min: 5,
                max: Some(5),
                greedy: true,
                sub: Box::new(Hir::char('a')),
            }),
        );
        assert_eq!(
            p(r"a{ 5 , 9 }"),
            Hir::repetition(hir::Repetition {
                min: 5,
                max: Some(9),
                greedy: true,
                sub: Box::new(Hir::char('a')),
            }),
        );
    }

    #[test]
    fn ok_group_unnamed() {
        assert_eq!(p("(a)"), cap(1, Hir::char('a')));
        assert_eq!(
            p("(ab)"),
            cap(1, Hir::concat(vec![Hir::char('a'), Hir::char('b')]))
        );
    }

    #[test]
    fn ok_group_named() {
        assert_eq!(p("(?P<foo>a)"), named_cap(1, "foo", Hir::char('a')));
        assert_eq!(p("(?<foo>a)"), named_cap(1, "foo", Hir::char('a')));

        assert_eq!(
            p("(?P<foo>ab)"),
            named_cap(
                1,
                "foo",
                Hir::concat(vec![Hir::char('a'), Hir::char('b')])
            )
        );
        assert_eq!(
            p("(?<foo>ab)"),
            named_cap(
                1,
                "foo",
                Hir::concat(vec![Hir::char('a'), Hir::char('b')])
            )
        );

        assert_eq!(p(r"(?<a>z)"), named_cap(1, "a", Hir::char('z')));
        assert_eq!(p(r"(?P<a>z)"), named_cap(1, "a", Hir::char('z')));

        assert_eq!(p(r"(?<a_1>z)"), named_cap(1, "a_1", Hir::char('z')));
        assert_eq!(p(r"(?P<a_1>z)"), named_cap(1, "a_1", Hir::char('z')));

        assert_eq!(p(r"(?<a.1>z)"), named_cap(1, "a.1", Hir::char('z')));
        assert_eq!(p(r"(?P<a.1>z)"), named_cap(1, "a.1", Hir::char('z')));

        assert_eq!(p(r"(?<a[1]>z)"), named_cap(1, "a[1]", Hir::char('z')));
        assert_eq!(p(r"(?P<a[1]>z)"), named_cap(1, "a[1]", Hir::char('z')));

        assert_eq!(p(r"(?<a¾>z)"), named_cap(1, "a¾", Hir::char('z')));
        assert_eq!(p(r"(?P<a¾>z)"), named_cap(1, "a¾", Hir::char('z')));

        assert_eq!(p(r"(?<名字>z)"), named_cap(1, "名字", Hir::char('z')));
        assert_eq!(p(r"(?P<名字>z)"), named_cap(1, "名字", Hir::char('z')));
    }

    #[test]
    fn ok_class() {
        assert_eq!(p(r"[a]"), singles(['a']));
        assert_eq!(p(r"[a\]]"), singles(['a', ']']));
        assert_eq!(p(r"[a\-z]"), singles(['a', '-', 'z']));
        assert_eq!(p(r"[ab]"), class([('a', 'b')]));
        assert_eq!(p(r"[a-]"), singles(['a', '-']));
        assert_eq!(p(r"[-a]"), singles(['a', '-']));
        assert_eq!(p(r"[--a]"), singles(['a', '-']));
        assert_eq!(p(r"[---a]"), singles(['a', '-']));
        assert_eq!(p(r"[[:alnum:]]"), posix("alnum"));
        assert_eq!(p(r"[\w]"), posix("word"));
        assert_eq!(p(r"[a\wz]"), posix("word"));
        assert_eq!(p(r"[\s\S]"), class([('\x00', '\u{10FFFF}')]));
        assert_eq!(p(r"[^\s\S]"), Hir::fail());
        assert_eq!(p(r"[a-cx-z]"), class([('a', 'c'), ('x', 'z')]));
        assert_eq!(p(r"[☃-⛄]"), class([('☃', '⛄')]));
        assert_eq!(p(r"[]]"), singles([']']));
        assert_eq!(p(r"[]a]"), singles([']', 'a']));
        assert_eq!(p(r"[]\[]"), singles(['[', ']']));
        assert_eq!(p(r"[\[]"), singles(['[']));

        assert_eq!(p(r"(?i)[a]"), singles(['A', 'a']));
        assert_eq!(p(r"(?i)[A]"), singles(['A', 'a']));
        assert_eq!(p(r"(?i)[k]"), singles(['K', 'k']));
        assert_eq!(p(r"(?i)[s]"), singles(['S', 's']));
        assert_eq!(p(r"(?i)[β]"), singles(['β']));

        assert_eq!(p(r"[^^]"), class([('\x00', ']'), ('_', '\u{10FFFF}')]));
        assert_eq!(
            p(r"[^-a]"),
            class([('\x00', ','), ('.', '`'), ('b', '\u{10FFFF}')])
        );

        assert_eq!(
            p(r"[-]a]"),
            Hir::concat(vec![singles(['-']), Hir::char('a'), Hir::char(']')])
        );
    }

    #[test]
    fn ok_verbatim() {
        assert_eq!(
            p(r"(?x)a{5,9} ?"),
            Hir::repetition(hir::Repetition {
                min: 5,
                max: Some(9),
                greedy: false,
                sub: Box::new(Hir::char('a')),
            })
        );
        assert_eq!(p(r"(?x)[   a]"), singles(['a']));
        assert_eq!(
            p(r"(?x)[ ^  a]"),
            class([('\x00', '`'), ('b', '\u{10FFFF}')])
        );
        assert_eq!(p(r"(?x)[ - a]"), singles(['a', '-']));
        assert_eq!(p(r"(?x)[ ] a]"), singles([']', 'a']));

        assert_eq!(
            p(r"(?x)a b"),
            Hir::concat(vec![Hir::char('a'), Hir::char('b')])
        );
        assert_eq!(
            p(r"(?x)a b(?-x)a b"),
            Hir::concat(vec![
                Hir::char('a'),
                Hir::char('b'),
                Hir::char('a'),
                Hir::char(' '),
                Hir::char('b'),
            ])
        );
        assert_eq!(
            p(r"a (?x:a )a "),
            Hir::concat(vec![
                Hir::char('a'),
                Hir::char(' '),
                Hir::char('a'),
                Hir::char('a'),
                Hir::char(' '),
            ])
        );
        assert_eq!(
            p(r"(?x)( ?P<foo> a )"),
            named_cap(1, "foo", Hir::char('a')),
        );
        assert_eq!(p(r"(?x)(  a )"), cap(1, Hir::char('a')));
        assert_eq!(p(r"(?x)(   ?:  a )"), Hir::char('a'));
        assert_eq!(p(r"(?x)\x { 53 }"), Hir::char('\x53'));
        assert_eq!(p(r"(?x)\ "), Hir::char(' '));
    }

    #[test]
    fn ok_comments() {
        let pat = "(?x)
# This is comment 1.
foo # This is comment 2.
  # This is comment 3.
bar
# This is comment 4.";
        assert_eq!(
            p(pat),
            Hir::concat(vec![
                Hir::char('f'),
                Hir::char('o'),
                Hir::char('o'),
                Hir::char('b'),
                Hir::char('a'),
                Hir::char('r'),
            ])
        );
    }

    #[test]
    fn err_standard() {
        assert_eq!(
            ERR_TOO_MUCH_NESTING,
            perr("(((((((((((((((((((((((((((((((((((((((((((((((((((a)))))))))))))))))))))))))))))))))))))))))))))))))))"),
        );
        // This one is tricky, because the only way it can happen is if the
        // number of captures overflows u32. Perhaps we should allow setting a
        // lower limit?
        // assert_eq!(ERR_TOO_MANY_CAPTURES, perr(""));
        assert_eq!(ERR_DUPLICATE_CAPTURE_NAME, perr(r"(?P<a>y)(?P<a>z)"));
        assert_eq!(ERR_UNCLOSED_GROUP, perr("("));
        assert_eq!(ERR_UNCLOSED_GROUP_QUESTION, perr("(?"));
        assert_eq!(ERR_UNOPENED_GROUP, perr(")"));
        assert_eq!(ERR_LOOK_UNSUPPORTED, perr(r"(?=a)"));
        assert_eq!(ERR_LOOK_UNSUPPORTED, perr(r"(?!a)"));
        assert_eq!(ERR_LOOK_UNSUPPORTED, perr(r"(?<=a)"));
        assert_eq!(ERR_LOOK_UNSUPPORTED, perr(r"(?<!a)"));
        assert_eq!(ERR_EMPTY_FLAGS, perr(r"(?)"));
        assert_eq!(ERR_MISSING_GROUP_NAME, perr(r"(?P<"));
        assert_eq!(ERR_MISSING_GROUP_NAME, perr(r"(?<"));
        assert_eq!(ERR_INVALID_GROUP_NAME, perr(r"(?P<1abc>z)"));
        assert_eq!(ERR_INVALID_GROUP_NAME, perr(r"(?<1abc>z)"));
        assert_eq!(ERR_INVALID_GROUP_NAME, perr(r"(?<¾>z)"));
        assert_eq!(ERR_INVALID_GROUP_NAME, perr(r"(?<¾a>z)"));
        assert_eq!(ERR_INVALID_GROUP_NAME, perr(r"(?<☃>z)"));
        assert_eq!(ERR_INVALID_GROUP_NAME, perr(r"(?<a☃>z)"));
        assert_eq!(ERR_UNCLOSED_GROUP_NAME, perr(r"(?P<foo"));
        assert_eq!(ERR_UNCLOSED_GROUP_NAME, perr(r"(?<foo"));
        assert_eq!(ERR_EMPTY_GROUP_NAME, perr(r"(?P<>z)"));
        assert_eq!(ERR_EMPTY_GROUP_NAME, perr(r"(?<>z)"));
        assert_eq!(ERR_FLAG_UNRECOGNIZED, perr(r"(?z:foo)"));
        assert_eq!(ERR_FLAG_REPEATED_NEGATION, perr(r"(?s-i-R)"));
        assert_eq!(ERR_FLAG_DUPLICATE, perr(r"(?isi)"));
        assert_eq!(ERR_FLAG_DUPLICATE, perr(r"(?is-i)"));
        assert_eq!(ERR_FLAG_UNEXPECTED_EOF, perr(r"(?is"));
        assert_eq!(ERR_FLAG_DANGLING_NEGATION, perr(r"(?is-:foo)"));
        assert_eq!(ERR_HEX_BRACE_INVALID_DIGIT, perr(r"\x{Z}"));
        assert_eq!(ERR_HEX_BRACE_UNEXPECTED_EOF, perr(r"\x{"));
        assert_eq!(ERR_HEX_BRACE_UNEXPECTED_EOF, perr(r"\x{A"));
        assert_eq!(ERR_HEX_BRACE_EMPTY, perr(r"\x{}"));
        assert_eq!(ERR_HEX_BRACE_INVALID, perr(r"\x{FFFFFFFFFFFFFFFFF}"));
        assert_eq!(ERR_HEX_FIXED_UNEXPECTED_EOF, perr(r"\xA"));
        assert_eq!(ERR_HEX_FIXED_INVALID_DIGIT, perr(r"\xZ"));
        assert_eq!(ERR_HEX_FIXED_INVALID_DIGIT, perr(r"\xZA"));
        assert_eq!(ERR_HEX_FIXED_INVALID_DIGIT, perr(r"\xAZ"));
        assert_eq!(ERR_HEX_FIXED_INVALID, perr(r"\uD800"));
        assert_eq!(ERR_HEX_FIXED_INVALID, perr(r"\UFFFFFFFF"));
        assert_eq!(ERR_HEX_UNEXPECTED_EOF, perr(r"\x"));
        assert_eq!(ERR_ESCAPE_UNEXPECTED_EOF, perr(r"\"));
        assert_eq!(ERR_BACKREF_UNSUPPORTED, perr(r"\0"));
        assert_eq!(ERR_BACKREF_UNSUPPORTED, perr(r"\1"));
        assert_eq!(ERR_BACKREF_UNSUPPORTED, perr(r"\8"));
        assert_eq!(ERR_UNICODE_CLASS_UNSUPPORTED, perr(r"\pL"));
        assert_eq!(ERR_UNICODE_CLASS_UNSUPPORTED, perr(r"\p{L}"));
        assert_eq!(ERR_ESCAPE_UNRECOGNIZED, perr(r"\i"));
        assert_eq!(ERR_UNCOUNTED_REP_SUB_MISSING, perr(r"?"));
        assert_eq!(ERR_UNCOUNTED_REP_SUB_MISSING, perr(r"*"));
        assert_eq!(ERR_UNCOUNTED_REP_SUB_MISSING, perr(r"+"));
        assert_eq!(ERR_UNCOUNTED_REP_SUB_MISSING, perr(r"(+)"));
        assert_eq!(ERR_UNCOUNTED_REP_SUB_MISSING, perr(r"|?"));
        assert_eq!(ERR_UNCOUNTED_REP_SUB_MISSING, perr(r"(?i)?"));
        assert_eq!(ERR_COUNTED_REP_SUB_MISSING, perr(r"{5}"));
        assert_eq!(ERR_COUNTED_REP_SUB_MISSING, perr(r"({5})"));
        assert_eq!(ERR_COUNTED_REP_SUB_MISSING, perr(r"(?i){5}"));
        assert_eq!(ERR_COUNTED_REP_UNCLOSED, perr(r"a{"));
        assert_eq!(ERR_COUNTED_REP_MIN_UNCLOSED, perr(r"a{5"));
        assert_eq!(ERR_COUNTED_REP_COMMA_UNCLOSED, perr(r"a{5,"));
        assert_eq!(ERR_COUNTED_REP_MIN_MAX_UNCLOSED, perr(r"a{5,6"));
        assert_eq!(ERR_COUNTED_REP_INVALID, perr(r"a{5,6Z"));
        assert_eq!(ERR_COUNTED_REP_INVALID_RANGE, perr(r"a{6,5}"));
        assert_eq!(ERR_DECIMAL_NO_DIGITS, perr(r"a{}"));
        assert_eq!(ERR_DECIMAL_NO_DIGITS, perr(r"a{]}"));
        assert_eq!(ERR_DECIMAL_INVALID, perr(r"a{999999999999999}"));
        assert_eq!(ERR_CLASS_UNCLOSED_AFTER_ITEM, perr(r"[a"));
        assert_eq!(ERR_CLASS_INVALID_RANGE_ITEM, perr(r"[\w-a]"));
        assert_eq!(ERR_CLASS_INVALID_RANGE_ITEM, perr(r"[a-\w]"));
        assert_eq!(ERR_CLASS_INVALID_ITEM, perr(r"[\b]"));
        assert_eq!(ERR_CLASS_UNCLOSED_AFTER_DASH, perr(r"[a-"));
        assert_eq!(ERR_CLASS_UNCLOSED_AFTER_NEGATION, perr(r"[^"));
        assert_eq!(ERR_CLASS_UNCLOSED_AFTER_CLOSING, perr(r"[]"));
        assert_eq!(ERR_CLASS_INVALID_RANGE, perr(r"[z-a]"));
        assert_eq!(ERR_CLASS_UNCLOSED, perr(r"["));
        assert_eq!(ERR_CLASS_UNCLOSED, perr(r"[a-z"));
        assert_eq!(ERR_CLASS_NEST_UNSUPPORTED, perr(r"[a-z[A-Z]]"));
        assert_eq!(ERR_CLASS_NEST_UNSUPPORTED, perr(r"[[:alnum]]"));
        assert_eq!(ERR_CLASS_INTERSECTION_UNSUPPORTED, perr(r"[a&&b]"));
        assert_eq!(ERR_CLASS_DIFFERENCE_UNSUPPORTED, perr(r"[a--b]"));
        assert_eq!(ERR_CLASS_SYMDIFFERENCE_UNSUPPORTED, perr(r"[a~~b]"));
        assert_eq!(ERR_SPECIAL_WORD_BOUNDARY_UNCLOSED, perr(r"\b{foo"));
        assert_eq!(ERR_SPECIAL_WORD_BOUNDARY_UNCLOSED, perr(r"\b{foo!}"));
        assert_eq!(ERR_SPECIAL_WORD_BOUNDARY_UNRECOGNIZED, perr(r"\b{foo}"));
        assert_eq!(ERR_SPECIAL_WORD_OR_REP_UNEXPECTED_EOF, perr(r"\b{"));
        assert_eq!(ERR_SPECIAL_WORD_OR_REP_UNEXPECTED_EOF, perr(r"(?x)\b{ "));
    }

    #[test]
    fn err_verbatim() {
        // See: https://github.com/rust-lang/regex/issues/792
        assert_eq!(ERR_CLASS_UNCLOSED_AFTER_DASH, perr(r"(?x)[-#]"));
        assert_eq!(ERR_CLASS_UNCLOSED_AFTER_ITEM, perr(r"(?x)[a "));
        assert_eq!(ERR_CLASS_UNCLOSED_AFTER_DASH, perr(r"(?x)[a- "));
        assert_eq!(ERR_CLASS_UNCLOSED, perr(r"(?x)[         "));
    }

    // This tests a bug fix where the nest limit checker wasn't decrementing
    // its depth during post-traversal, which causes long regexes to trip
    // the default limit too aggressively.
    #[test]
    fn regression_454_nest_too_big() {
        let pattern = r#"
        2(?:
          [45]\d{3}|
          7(?:
            1[0-267]|
            2[0-289]|
            3[0-29]|
            4[01]|
            5[1-3]|
            6[013]|
            7[0178]|
            91
          )|
          8(?:
            0[125]|
            [139][1-6]|
            2[0157-9]|
            41|
            6[1-35]|
            7[1-5]|
            8[1-8]|
            90
          )|
          9(?:
            0[0-2]|
            1[0-4]|
            2[568]|
            3[3-6]|
            5[5-7]|
            6[0167]|
            7[15]|
            8[0146-9]
          )
        )\d{4}
        "#;
        p(pattern);
    }

    // This tests that we treat a trailing `-` in a character class as a
    // literal `-` even when whitespace mode is enabled and there is whitespace
    // after the trailing `-`.
    #[test]
    fn regression_455_trailing_dash_ignore_whitespace() {
        p("(?x)[ / - ]");
        p("(?x)[ a - ]");
        p("(?x)[
            a
            - ]
        ");
        p("(?x)[
            a # wat
            - ]
        ");

        perr("(?x)[ / -");
        perr("(?x)[ / - ");
        perr(
            "(?x)[
            / -
        ",
        );
        perr(
            "(?x)[
            / - # wat
        ",
        );
    }

    #[test]
    fn regression_capture_indices() {
        let got = p(r"(a|ab|c|bcd){4,10}(d*)");
        assert_eq!(
            got,
            Hir::concat(vec![
                Hir::repetition(hir::Repetition {
                    min: 4,
                    max: Some(10),
                    greedy: true,
                    sub: Box::new(cap(
                        1,
                        Hir::alternation(vec![
                            Hir::char('a'),
                            Hir::concat(vec![Hir::char('a'), Hir::char('b')]),
                            Hir::char('c'),
                            Hir::concat(vec![
                                Hir::char('b'),
                                Hir::char('c'),
                                Hir::char('d')
                            ]),
                        ])
                    ))
                }),
                cap(
                    2,
                    Hir::repetition(hir::Repetition {
                        min: 0,
                        max: None,
                        greedy: true,
                        sub: Box::new(Hir::char('d')),
                    })
                ),
            ])
        );
    }
}

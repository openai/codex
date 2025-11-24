/*!
Defines a high-level intermediate representation for regular expressions.
*/
use std::char;
use std::cmp;
use std::error;
use std::fmt;
use std::result;
use std::u8;

use crate::ast::Span;
use crate::hir::interval::{Interval, IntervalSet, IntervalSetIter};
use crate::unicode;

pub use crate::hir::visitor::{visit, Visitor};
pub use crate::unicode::CaseFoldError;

mod interval;
pub mod literal;
pub mod print;
pub mod translate;
mod visitor;

/// An error that can occur while translating an `Ast` to a `Hir`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    /// The kind of error.
    kind: ErrorKind,
    /// The original pattern that the translator's Ast was parsed from. Every
    /// span in an error is a valid range into this string.
    pattern: String,
    /// The span of this error, derived from the Ast given to the translator.
    span: Span,
}

impl Error {
    /// Return the type of this error.
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    /// The original pattern string in which this error occurred.
    ///
    /// Every span reported by this error is reported in terms of this string.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Return the span at which this error occurred.
    pub fn span(&self) -> &Span {
        &self.span
    }
}

/// The type of an error that occurred while building an `Hir`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    /// This error occurs when a Unicode feature is used when Unicode
    /// support is disabled. For example `(?-u:\pL)` would trigger this error.
    UnicodeNotAllowed,
    /// This error occurs when translating a pattern that could match a byte
    /// sequence that isn't UTF-8 and `allow_invalid_utf8` was disabled.
    InvalidUtf8,
    /// This occurs when an unrecognized Unicode property name could not
    /// be found.
    UnicodePropertyNotFound,
    /// This occurs when an unrecognized Unicode property value could not
    /// be found.
    UnicodePropertyValueNotFound,
    /// This occurs when a Unicode-aware Perl character class (`\w`, `\s` or
    /// `\d`) could not be found. This can occur when the `unicode-perl`
    /// crate feature is not enabled.
    UnicodePerlClassNotFound,
    /// This occurs when the Unicode simple case mapping tables are not
    /// available, and the regular expression required Unicode aware case
    /// insensitivity.
    UnicodeCaseUnavailable,
    /// This occurs when the translator attempts to construct a character class
    /// that is empty.
    ///
    /// Note that this restriction in the translator may be removed in the
    /// future.
    EmptyClassNotAllowed,
    /// Hints that destructuring should not be exhaustive.
    ///
    /// This enum may grow additional variants, so this makes sure clients
    /// don't count on exhaustive matching. (Otherwise, adding a new variant
    /// could break existing code.)
    #[doc(hidden)]
    __Nonexhaustive,
}

impl ErrorKind {
    // TODO: Remove this method entirely on the next breaking semver release.
    #[allow(deprecated)]
    fn description(&self) -> &str {
        use self::ErrorKind::*;
        match *self {
            UnicodeNotAllowed => "Unicode not allowed here",
            InvalidUtf8 => "pattern can match invalid UTF-8",
            UnicodePropertyNotFound => "Unicode property not found",
            UnicodePropertyValueNotFound => "Unicode property value not found",
            UnicodePerlClassNotFound => {
                "Unicode-aware Perl class not found \
                 (make sure the unicode-perl feature is enabled)"
            }
            UnicodeCaseUnavailable => {
                "Unicode-aware case insensitivity matching is not available \
                 (make sure the unicode-case feature is enabled)"
            }
            EmptyClassNotAllowed => "empty character classes are not allowed",
            __Nonexhaustive => unreachable!(),
        }
    }
}

impl error::Error for Error {
    // TODO: Remove this method entirely on the next breaking semver release.
    #[allow(deprecated)]
    fn description(&self) -> &str {
        self.kind.description()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        crate::error::Formatter::from(self).fmt(f)
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO: Remove this on the next breaking semver release.
        #[allow(deprecated)]
        f.write_str(self.description())
    }
}

/// A high-level intermediate representation (HIR) for a regular expression.
///
/// The HIR of a regular expression represents an intermediate step between its
/// abstract syntax (a structured description of the concrete syntax) and
/// compiled byte codes. The purpose of HIR is to make regular expressions
/// easier to analyze. In particular, the AST is much more complex than the
/// HIR. For example, while an AST supports arbitrarily nested character
/// classes, the HIR will flatten all nested classes into a single set. The HIR
/// will also "compile away" every flag present in the concrete syntax. For
/// example, users of HIR expressions never need to worry about case folding;
/// it is handled automatically by the translator (e.g., by translating `(?i)A`
/// to `[aA]`).
///
/// If the HIR was produced by a translator that disallows invalid UTF-8, then
/// the HIR is guaranteed to match UTF-8 exclusively.
///
/// This type defines its own destructor that uses constant stack space and
/// heap space proportional to the size of the HIR.
///
/// The specific type of an HIR expression can be accessed via its `kind`
/// or `into_kind` methods. This extra level of indirection exists for two
/// reasons:
///
/// 1. Construction of an HIR expression *must* use the constructor methods
///    on this `Hir` type instead of building the `HirKind` values directly.
///    This permits construction to enforce invariants like "concatenations
///    always consist of two or more sub-expressions."
/// 2. Every HIR expression contains attributes that are defined inductively,
///    and can be computed cheaply during the construction process. For
///    example, one such attribute is whether the expression must match at the
///    beginning of the text.
///
/// Also, an `Hir`'s `fmt::Display` implementation prints an HIR as a regular
/// expression pattern string, and uses constant stack space and heap space
/// proportional to the size of the `Hir`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hir {
    /// The underlying HIR kind.
    kind: HirKind,
    /// Analysis info about this HIR, computed during construction.
    info: HirInfo,
}

/// The kind of an arbitrary `Hir` expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirKind {
    /// The empty regular expression, which matches everything, including the
    /// empty string.
    Empty,
    /// A single literal character that matches exactly this character.
    Literal(Literal),
    /// A single character class that matches any of the characters in the
    /// class. A class can either consist of Unicode scalar values as
    /// characters, or it can use bytes.
    Class(Class),
    /// An anchor assertion. An anchor assertion match always has zero length.
    Anchor(Anchor),
    /// A word boundary assertion, which may or may not be Unicode aware. A
    /// word boundary assertion match always has zero length.
    WordBoundary(WordBoundary),
    /// A repetition operation applied to a child expression.
    Repetition(Repetition),
    /// A possibly capturing group, which contains a child expression.
    Group(Group),
    /// A concatenation of expressions. A concatenation always has at least two
    /// child expressions.
    ///
    /// A concatenation matches only if each of its child expression matches
    /// one after the other.
    Concat(Vec<Hir>),
    /// An alternation of expressions. An alternation always has at least two
    /// child expressions.
    ///
    /// An alternation matches only if at least one of its child expression
    /// matches. If multiple expressions match, then the leftmost is preferred.
    Alternation(Vec<Hir>),
}

impl Hir {
    /// Returns a reference to the underlying HIR kind.
    pub fn kind(&self) -> &HirKind {
        &self.kind
    }

    /// Consumes ownership of this HIR expression and returns its underlying
    /// `HirKind`.
    pub fn into_kind(mut self) -> HirKind {
        use std::mem;
        mem::replace(&mut self.kind, HirKind::Empty)
    }

    /// Returns an empty HIR expression.
    ///
    /// An empty HIR expression always matches, including the empty string.
    pub fn empty() -> Hir {
        let mut info = HirInfo::new();
        info.set_always_utf8(true);
        info.set_all_assertions(true);
        info.set_anchored_start(false);
        info.set_anchored_end(false);
        info.set_line_anchored_start(false);
        info.set_line_anchored_end(false);
        info.set_any_anchored_start(false);
        info.set_any_anchored_end(false);
        info.set_match_empty(true);
        info.set_literal(false);
        info.set_alternation_literal(false);
        Hir { kind: HirKind::Empty, info }
    }

    /// Creates a literal HIR expression.
    ///
    /// If the given literal has a `Byte` variant with an ASCII byte, then this
    /// method panics. This enforces the invariant that `Byte` variants are
    /// only used to express matching of invalid UTF-8.
    pub fn literal(lit: Literal) -> Hir {
        if let Literal::Byte(b) = lit {
            assert!(b > 0x7F);
        }

        let mut info = HirInfo::new();
        info.set_always_utf8(lit.is_unicode());
        info.set_all_assertions(false);
        info.set_anchored_start(false);
        info.set_anchored_end(false);
        info.set_line_anchored_start(false);
        info.set_line_anchored_end(false);
        info.set_any_anchored_start(false);
        info.set_any_anchored_end(false);
        info.set_match_empty(false);
        info.set_literal(true);
        info.set_alternation_literal(true);
        Hir { kind: HirKind::Literal(lit), info }
    }

    /// Creates a class HIR expression.
    pub fn class(class: Class) -> Hir {
        let mut info = HirInfo::new();
        info.set_always_utf8(class.is_always_utf8());
        info.set_all_assertions(false);
        info.set_anchored_start(false);
        info.set_anchored_end(false);
        info.set_line_anchored_start(false);
        info.set_line_anchored_end(false);
        info.set_any_anchored_start(false);
        info.set_any_anchored_end(false);
        info.set_match_empty(false);
        info.set_literal(false);
        info.set_alternation_literal(false);
        Hir { kind: HirKind::Class(class), info }
    }

    /// Creates an anchor assertion HIR expression.
    pub fn anchor(anchor: Anchor) -> Hir {
        let mut info = HirInfo::new();
        info.set_always_utf8(true);
        info.set_all_assertions(true);
        info.set_anchored_start(false);
        info.set_anchored_end(false);
        info.set_line_anchored_start(false);
        info.set_line_anchored_end(false);
        info.set_any_anchored_start(false);
        info.set_any_anchored_end(false);
        info.set_match_empty(true);
        info.set_literal(false);
        info.set_alternation_literal(false);
        if let Anchor::StartText = anchor {
            info.set_anchored_start(true);
            info.set_line_anchored_start(true);
            info.set_any_anchored_start(true);
        }
        if let Anchor::EndText = anchor {
            info.set_anchored_end(true);
            info.set_line_anchored_end(true);
            info.set_any_anchored_end(true);
        }
        if let Anchor::StartLine = anchor {
            info.set_line_anchored_start(true);
        }
        if let Anchor::EndLine = anchor {
            info.set_line_anchored_end(true);
        }
        Hir { kind: HirKind::Anchor(anchor), info }
    }

    /// Creates a word boundary assertion HIR expression.
    pub fn word_boundary(word_boundary: WordBoundary) -> Hir {
        let mut info = HirInfo::new();
        info.set_always_utf8(true);
        info.set_all_assertions(true);
        info.set_anchored_start(false);
        info.set_anchored_end(false);
        info.set_line_anchored_start(false);
        info.set_line_anchored_end(false);
        info.set_any_anchored_start(false);
        info.set_any_anchored_end(false);
        info.set_literal(false);
        info.set_alternation_literal(false);
        // A negated word boundary matches '', so that's fine. But \b does not
        // match \b, so why do we say it can match the empty string? Well,
        // because, if you search for \b against 'a', it will report [0, 0) and
        // [1, 1) as matches, and both of those matches correspond to the empty
        // string. Thus, only *certain* empty strings match \b, which similarly
        // applies to \B.
        info.set_match_empty(true);
        // Negated ASCII word boundaries can match invalid UTF-8.
        if let WordBoundary::AsciiNegate = word_boundary {
            info.set_always_utf8(false);
        }
        Hir { kind: HirKind::WordBoundary(word_boundary), info }
    }

    /// Creates a repetition HIR expression.
    pub fn repetition(rep: Repetition) -> Hir {
        let mut info = HirInfo::new();
        info.set_always_utf8(rep.hir.is_always_utf8());
        info.set_all_assertions(rep.hir.is_all_assertions());
        // If this operator can match the empty string, then it can never
        // be anchored.
        info.set_anchored_start(
            !rep.is_match_empty() && rep.hir.is_anchored_start(),
        );
        info.set_anchored_end(
            !rep.is_match_empty() && rep.hir.is_anchored_end(),
        );
        info.set_line_anchored_start(
            !rep.is_match_empty() && rep.hir.is_anchored_start(),
        );
        info.set_line_anchored_end(
            !rep.is_match_empty() && rep.hir.is_anchored_end(),
        );
        info.set_any_anchored_start(rep.hir.is_any_anchored_start());
        info.set_any_anchored_end(rep.hir.is_any_anchored_end());
        info.set_match_empty(rep.is_match_empty() || rep.hir.is_match_empty());
        info.set_literal(false);
        info.set_alternation_literal(false);
        Hir { kind: HirKind::Repetition(rep), info }
    }

    /// Creates a group HIR expression.
    pub fn group(group: Group) -> Hir {
        let mut info = HirInfo::new();
        info.set_always_utf8(group.hir.is_always_utf8());
        info.set_all_assertions(group.hir.is_all_assertions());
        info.set_anchored_start(group.hir.is_anchored_start());
        info.set_anchored_end(group.hir.is_anchored_end());
        info.set_line_anchored_start(group.hir.is_line_anchored_start());
        info.set_line_anchored_end(group.hir.is_line_anchored_end());
        info.set_any_anchored_start(group.hir.is_any_anchored_start());
        info.set_any_anchored_end(group.hir.is_any_anchored_end());
        info.set_match_empty(group.hir.is_match_empty());
        info.set_literal(false);
        info.set_alternation_literal(false);
        Hir { kind: HirKind::Group(group), info }
    }

    /// Returns the concatenation of the given expressions.
    ///
    /// This flattens the concatenation as appropriate.
    pub fn concat(mut exprs: Vec<Hir>) -> Hir {
        match exprs.len() {
            0 => Hir::empty(),
            1 => exprs.pop().unwrap(),
            _ => {
                let mut info = HirInfo::new();
                info.set_always_utf8(true);
                info.set_all_assertions(true);
                info.set_any_anchored_start(false);
                info.set_any_anchored_end(false);
                info.set_match_empty(true);
                info.set_literal(true);
                info.set_alternation_literal(true);

                // Some attributes require analyzing all sub-expressions.
                for e in &exprs {
                    let x = info.is_always_utf8() && e.is_always_utf8();
                    info.set_always_utf8(x);

                    let x = info.is_all_assertions() && e.is_all_assertions();
                    info.set_all_assertions(x);

                    let x = info.is_any_anchored_start()
                        || e.is_any_anchored_start();
                    info.set_any_anchored_start(x);

                    let x =
                        info.is_any_anchored_end() || e.is_any_anchored_end();
                    info.set_any_anchored_end(x);

                    let x = info.is_match_empty() && e.is_match_empty();
                    info.set_match_empty(x);

                    let x = info.is_literal() && e.is_literal();
                    info.set_literal(x);

                    let x = info.is_alternation_literal()
                        && e.is_alternation_literal();
                    info.set_alternation_literal(x);
                }
                // Anchored attributes require something slightly more
                // sophisticated. Normally, WLOG, to determine whether an
                // expression is anchored to the start, we'd only need to check
                // the first expression of a concatenation. However,
                // expressions like `$\b^` are still anchored to the start,
                // but the first expression in the concatenation *isn't*
                // anchored to the start. So the "first" expression to look at
                // is actually one that is either not an assertion or is
                // specifically the StartText assertion.
                info.set_anchored_start(
                    exprs
                        .iter()
                        .take_while(|e| {
                            e.is_anchored_start() || e.is_all_assertions()
                        })
                        .any(|e| e.is_anchored_start()),
                );
                // Similarly for the end anchor, but in reverse.
                info.set_anchored_end(
                    exprs
                        .iter()
                        .rev()
                        .take_while(|e| {
                            e.is_anchored_end() || e.is_all_assertions()
                        })
                        .any(|e| e.is_anchored_end()),
                );
                // Repeat the process for line anchors.
                info.set_line_anchored_start(
                    exprs
                        .iter()
                        .take_while(|e| {
                            e.is_line_anchored_start() || e.is_all_assertions()
                        })
                        .any(|e| e.is_line_anchored_start()),
                );
                info.set_line_anchored_end(
                    exprs
                        .iter()
                        .rev()
                        .take_while(|e| {
                            e.is_line_anchored_end() || e.is_all_assertions()
                        })
                        .any(|e| e.is_line_anchored_end()),
                );
                Hir { kind: HirKind::Concat(exprs), info }
            }
        }
    }

    /// Returns the alternation of the given expressions.
    ///
    /// This flattens the alternation as appropriate.
    pub fn alternation(mut exprs: Vec<Hir>) -> Hir {
        match exprs.len() {
            0 => Hir::empty(),
            1 => exprs.pop().unwrap(),
            _ => {
                let mut info = HirInfo::new();
                info.set_always_utf8(true);
                info.set_all_assertions(true);
                info.set_anchored_start(true);
                info.set_anchored_end(true);
                info.set_line_anchored_start(true);
                info.set_line_anchored_end(true);
                info.set_any_anchored_start(false);
                info.set_any_anchored_end(false);
                info.set_match_empty(false);
                info.set_literal(false);
                info.set_alternation_literal(true);

                // Some attributes require analyzing all sub-expressions.
                for e in &exprs {
                    let x = info.is_always_utf8() && e.is_always_utf8();
                    info.set_always_utf8(x);

                    let x = info.is_all_assertions() && e.is_all_assertions();
                    info.set_all_assertions(x);

                    let x = info.is_anchored_start() && e.is_anchored_start();
                    info.set_anchored_start(x);

                    let x = info.is_anchored_end() && e.is_anchored_end();
                    info.set_anchored_end(x);

                    let x = info.is_line_anchored_start()
                        && e.is_line_anchored_start();
                    info.set_line_anchored_start(x);

                    let x = info.is_line_anchored_end()
                        && e.is_line_anchored_end();
                    info.set_line_anchored_end(x);

                    let x = info.is_any_anchored_start()
                        || e.is_any_anchored_start();
                    info.set_any_anchored_start(x);

                    let x =
                        info.is_any_anchored_end() || e.is_any_anchored_end();
                    info.set_any_anchored_end(x);

                    let x = info.is_match_empty() || e.is_match_empty();
                    info.set_match_empty(x);

                    let x = info.is_alternation_literal() && e.is_literal();
                    info.set_alternation_literal(x);
                }
                Hir { kind: HirKind::Alternation(exprs), info }
            }
        }
    }

    /// Build an HIR expression for `.`.
    ///
    /// A `.` expression matches any character except for `\n`. To build an
    /// expression that matches any character, including `\n`, use the `any`
    /// method.
    ///
    /// If `bytes` is `true`, then this assumes characters are limited to a
    /// single byte.
    pub fn dot(bytes: bool) -> Hir {
        if bytes {
            let mut cls = ClassBytes::empty();
            cls.push(ClassBytesRange::new(b'\0', b'\x09'));
            cls.push(ClassBytesRange::new(b'\x0B', b'\xFF'));
            Hir::class(Class::Bytes(cls))
        } else {
            let mut cls = ClassUnicode::empty();
            cls.push(ClassUnicodeRange::new('\0', '\x09'));
            cls.push(ClassUnicodeRange::new('\x0B', '\u{10FFFF}'));
            Hir::class(Class::Unicode(cls))
        }
    }

    /// Build an HIR expression for `(?s).`.
    ///
    /// A `(?s).` expression matches any character, including `\n`. To build an
    /// expression that matches any character except for `\n`, then use the
    /// `dot` method.
    ///
    /// If `bytes` is `true`, then this assumes characters are limited to a
    /// single byte.
    pub fn any(bytes: bool) -> Hir {
        if bytes {
            let mut cls = ClassBytes::empty();
            cls.push(ClassBytesRange::new(b'\0', b'\xFF'));
            Hir::class(Class::Bytes(cls))
        } else {
            let mut cls = ClassUnicode::empty();
            cls.push(ClassUnicodeRange::new('\0', '\u{10FFFF}'));
            Hir::class(Class::Unicode(cls))
        }
    }

    /// Return true if and only if this HIR will always match valid UTF-8.
    ///
    /// When this returns false, then it is possible for this HIR expression
    /// to match invalid UTF-8.
    pub fn is_always_utf8(&self) -> bool {
        self.info.is_always_utf8()
    }

    /// Returns true if and only if this entire HIR expression is made up of
    /// zero-width assertions.
    ///
    /// This includes expressions like `^$\b\A\z` and even `((\b)+())*^`, but
    /// not `^a`.
    pub fn is_all_assertions(&self) -> bool {
        self.info.is_all_assertions()
    }

    /// Return true if and only if this HIR is required to match from the
    /// beginning of text. This includes expressions like `^foo`, `^(foo|bar)`,
    /// `^foo|^bar` but not `^foo|bar`.
    pub fn is_anchored_start(&self) -> bool {
        self.info.is_anchored_start()
    }

    /// Return true if and only if this HIR is required to match at the end
    /// of text. This includes expressions like `foo$`, `(foo|bar)$`,
    /// `foo$|bar$` but not `foo$|bar`.
    pub fn is_anchored_end(&self) -> bool {
        self.info.is_anchored_end()
    }

    /// Return true if and only if this HIR is required to match from the
    /// beginning of text or the beginning of a line. This includes expressions
    /// like `^foo`, `(?m)^foo`, `^(foo|bar)`, `^(foo|bar)`, `(?m)^foo|^bar`
    /// but not `^foo|bar` or `(?m)^foo|bar`.
    ///
    /// Note that if `is_anchored_start` is `true`, then
    /// `is_line_anchored_start` will also be `true`. The reverse implication
    /// is not true. For example, `(?m)^foo` is line anchored, but not
    /// `is_anchored_start`.
    pub fn is_line_anchored_start(&self) -> bool {
        self.info.is_line_anchored_start()
    }

    /// Return true if and only if this HIR is required to match at the
    /// end of text or the end of a line. This includes expressions like
    /// `foo$`, `(?m)foo$`, `(foo|bar)$`, `(?m)(foo|bar)$`, `foo$|bar$`,
    /// `(?m)(foo|bar)$`, but not `foo$|bar` or `(?m)foo$|bar`.
    ///
    /// Note that if `is_anchored_end` is `true`, then
    /// `is_line_anchored_end` will also be `true`. The reverse implication
    /// is not true. For example, `(?m)foo$` is line anchored, but not
    /// `is_anchored_end`.
    pub fn is_line_anchored_end(&self) -> bool {
        self.info.is_line_anchored_end()
    }

    /// Return true if and only if this HIR contains any sub-expression that
    /// is required to match at the beginning of text. Specifically, this
    /// returns true if the `^` symbol (when multiline mode is disabled) or the
    /// `\A` escape appear anywhere in the regex.
    pub fn is_any_anchored_start(&self) -> bool {
        self.info.is_any_anchored_start()
    }

    /// Return true if and only if this HIR contains any sub-expression that is
    /// required to match at the end of text. Specifically, this returns true
    /// if the `$` symbol (when multiline mode is disabled) or the `\z` escape
    /// appear anywhere in the regex.
    pub fn is_any_anchored_end(&self) -> bool {
        self.info.is_any_anchored_end()
    }

    /// Return true if and only if the empty string is part of the language
    /// matched by this regular expression.
    ///
    /// This includes `a*`, `a?b*`, `a{0}`, `()`, `()+`, `^$`, `a|b?`, `\b`
    /// and `\B`, but not `a` or `a+`.
    pub fn is_match_empty(&self) -> bool {
        self.info.is_match_empty()
    }

    /// Return true if and only if this HIR is a simple literal. This is only
    /// true when this HIR expression is either itself a `Literal` or a
    /// concatenation of only `Literal`s.
    ///
    /// For example, `f` and `foo` are literals, but `f+`, `(foo)`, `foo()`,
    /// `` are not (even though that contain sub-expressions that are literals).
    pub fn is_literal(&self) -> bool {
        self.info.is_literal()
    }

    /// Return true if and only if this HIR is either a simple literal or an
    /// alternation of simple literals. This is only
    /// true when this HIR expression is either itself a `Literal` or a
    /// concatenation of only `Literal`s or an alternation of only `Literal`s.
    ///
    /// For example, `f`, `foo`, `a|b|c`, and `foo|bar|baz` are alternation
    /// literals, but `f+`, `(foo)`, `foo()`, ``
    /// are not (even though that contain sub-expressions that are literals).
    pub fn is_alternation_literal(&self) -> bool {
        self.info.is_alternation_literal()
    }
}

impl HirKind {
    /// Return true if and only if this HIR is the empty regular expression.
    ///
    /// Note that this is not defined inductively. That is, it only tests if
    /// this kind is the `Empty` variant. To get the inductive definition,
    /// use the `is_match_empty` method on [`Hir`](struct.Hir.html).
    pub fn is_empty(&self) -> bool {
        match *self {
            HirKind::Empty => true,
            _ => false,
        }
    }

    /// Returns true if and only if this kind has any (including possibly
    /// empty) subexpressions.
    pub fn has_subexprs(&self) -> bool {
        match *self {
            HirKind::Empty
            | HirKind::Literal(_)
            | HirKind::Class(_)
            | HirKind::Anchor(_)
            | HirKind::WordBoundary(_) => false,
            HirKind::Group(_)
            | HirKind::Repetition(_)
            | HirKind::Concat(_)
            | HirKind::Alternation(_) => true,
        }
    }
}

/// Print a display representation of this Hir.
///
/// The result of this is a valid regular expression pattern string.
///
/// This implementation uses constant stack space and heap space proportional
/// to the size of the `Hir`.
impl fmt::Display for Hir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use crate::hir::print::Printer;
        Printer::new().print(self, f)
    }
}

/// The high-level intermediate representation of a literal.
///
/// A literal corresponds to a single character, where a character is either
/// defined by a Unicode scalar value or an arbitrary byte. Unicode characters
/// are preferred whenever possible. In particular, a `Byte` variant is only
/// ever produced when it could match invalid UTF-8.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Literal {
    /// A single character represented by a Unicode scalar value.
    Unicode(char),
    /// A single character represented by an arbitrary byte.
    Byte(u8),
}

impl Literal {
    /// Returns true if and only if this literal corresponds to a Unicode
    /// scalar value.
    pub fn is_unicode(&self) -> bool {
        match *self {
            Literal::Unicode(_) => true,
            Literal::Byte(b) if b <= 0x7F => true,
            Literal::Byte(_) => false,
        }
    }
}

/// The high-level intermediate representation of a character class.
///
/// A character class corresponds to a set of characters. A character is either
/// defined by a Unicode scalar value or a byte. Unicode characters are used
/// by default, while bytes are used when Unicode mode (via the `u` flag) is
/// disabled.
///
/// A character class, regardless of its character type, is represented by a
/// sequence of non-overlapping non-adjacent ranges of characters.
///
/// Note that unlike [`Literal`](enum.Literal.html), a `Bytes` variant may
/// be produced even when it exclusively matches valid UTF-8. This is because
/// a `Bytes` variant represents an intention by the author of the regular
/// expression to disable Unicode mode, which in turn impacts the semantics of
/// case insensitive matching. For example, `(?i)k` and `(?i-u)k` will not
/// match the same set of strings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Class {
    /// A set of characters represented by Unicode scalar values.
    Unicode(ClassUnicode),
    /// A set of characters represented by arbitrary bytes (one byte per
    /// character).
    Bytes(ClassBytes),
}

impl Class {
    /// Apply Unicode simple case folding to this character class, in place.
    /// The character class will be expanded to include all simple case folded
    /// character variants.
    ///
    /// If this is a byte oriented character class, then this will be limited
    /// to the ASCII ranges `A-Z` and `a-z`.
    pub fn case_fold_simple(&mut self) {
        match *self {
            Class::Unicode(ref mut x) => x.case_fold_simple(),
            Class::Bytes(ref mut x) => x.case_fold_simple(),
        }
    }

    /// Negate this character class in place.
    ///
    /// After completion, this character class will contain precisely the
    /// characters that weren't previously in the class.
    pub fn negate(&mut self) {
        match *self {
            Class::Unicode(ref mut x) => x.negate(),
            Class::Bytes(ref mut x) => x.negate(),
        }
    }

    /// Returns true if and only if this character class will only ever match
    /// valid UTF-8.
    ///
    /// A character class can match invalid UTF-8 only when the following
    /// conditions are met:
    ///
    /// 1. The translator was configured to permit generating an expression
    ///    that can match invalid UTF-8. (By default, this is disabled.)
    /// 2. Unicode mode (via the `u` flag) was disabled either in the concrete
    ///    syntax or in the parser builder. By default, Unicode mode is
    ///    enabled.
    pub fn is_always_utf8(&self) -> bool {
        match *self {
            Class::Unicode(_) => true,
            Class::Bytes(ref x) => x.is_all_ascii(),
        }
    }
}

/// A set of characters represented by Unicode scalar values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassUnicode {
    set: IntervalSet<ClassUnicodeRange>,
}

impl ClassUnicode {
    /// Create a new class from a sequence of ranges.
    ///
    /// The given ranges do not need to be in any specific order, and ranges
    /// may overlap.
    pub fn new<I>(ranges: I) -> ClassUnicode
    where
        I: IntoIterator<Item = ClassUnicodeRange>,
    {
        ClassUnicode { set: IntervalSet::new(ranges) }
    }

    /// Create a new class with no ranges.
    pub fn empty() -> ClassUnicode {
        ClassUnicode::new(vec![])
    }

    /// Add a new range to this set.
    pub fn push(&mut self, range: ClassUnicodeRange) {
        self.set.push(range);
    }

    /// Return an iterator over all ranges in this class.
    ///
    /// The iterator yields ranges in ascending order.
    pub fn iter(&self) -> ClassUnicodeIter<'_> {
        ClassUnicodeIter(self.set.iter())
    }

    /// Return the underlying ranges as a slice.
    pub fn ranges(&self) -> &[ClassUnicodeRange] {
        self.set.intervals()
    }

    /// Expand this character class such that it contains all case folded
    /// characters, according to Unicode's "simple" mapping. For example, if
    /// this class consists of the range `a-z`, then applying case folding will
    /// result in the class containing both the ranges `a-z` and `A-Z`.
    ///
    /// # Panics
    ///
    /// This routine panics when the case mapping data necessary for this
    /// routine to complete is unavailable. This occurs when the `unicode-case`
    /// feature is not enabled.
    ///
    /// Callers should prefer using `try_case_fold_simple` instead, which will
    /// return an error instead of panicking.
    pub fn case_fold_simple(&mut self) {
        self.set
            .case_fold_simple()
            .expect("unicode-case feature must be enabled");
    }

    /// Expand this character class such that it contains all case folded
    /// characters, according to Unicode's "simple" mapping. For example, if
    /// this class consists of the range `a-z`, then applying case folding will
    /// result in the class containing both the ranges `a-z` and `A-Z`.
    ///
    /// # Error
    ///
    /// This routine returns an error when the case mapping data necessary
    /// for this routine to complete is unavailable. This occurs when the
    /// `unicode-case` feature is not enabled.
    pub fn try_case_fold_simple(
        &mut self,
    ) -> result::Result<(), CaseFoldError> {
        self.set.case_fold_simple()
    }

    /// Negate this character class.
    ///
    /// For all `c` where `c` is a Unicode scalar value, if `c` was in this
    /// set, then it will not be in this set after negation.
    pub fn negate(&mut self) {
        self.set.negate();
    }

    /// Union this character class with the given character class, in place.
    pub fn union(&mut self, other: &ClassUnicode) {
        self.set.union(&other.set);
    }

    /// Intersect this character class with the given character class, in
    /// place.
    pub fn intersect(&mut self, other: &ClassUnicode) {
        self.set.intersect(&other.set);
    }

    /// Subtract the given character class from this character class, in place.
    pub fn difference(&mut self, other: &ClassUnicode) {
        self.set.difference(&other.set);
    }

    /// Compute the symmetric difference of the given character classes, in
    /// place.
    ///
    /// This computes the symmetric difference of two character classes. This
    /// removes all elements in this class that are also in the given class,
    /// but all adds all elements from the given class that aren't in this
    /// class. That is, the class will contain all elements in either class,
    /// but will not contain any elements that are in both classes.
    pub fn symmetric_difference(&mut self, other: &ClassUnicode) {
        self.set.symmetric_difference(&other.set);
    }

    /// Returns true if and only if this character class will either match
    /// nothing or only ASCII bytes. Stated differently, this returns false
    /// if and only if this class contains a non-ASCII codepoint.
    pub fn is_all_ascii(&self) -> bool {
        self.set.intervals().last().map_or(true, |r| r.end <= '\x7F')
    }
}

/// An iterator over all ranges in a Unicode character class.
///
/// The lifetime `'a` refers to the lifetime of the underlying class.
#[derive(Debug)]
pub struct ClassUnicodeIter<'a>(IntervalSetIter<'a, ClassUnicodeRange>);

impl<'a> Iterator for ClassUnicodeIter<'a> {
    type Item = &'a ClassUnicodeRange;

    fn next(&mut self) -> Option<&'a ClassUnicodeRange> {
        self.0.next()
    }
}

/// A single range of characters represented by Unicode scalar values.
///
/// The range is closed. That is, the start and end of the range are included
/// in the range.
#[derive(Clone, Copy, Default, Eq, PartialEq, PartialOrd, Ord)]
pub struct ClassUnicodeRange {
    start: char,
    end: char,
}

impl fmt::Debug for ClassUnicodeRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let start = if !self.start.is_whitespace() && !self.start.is_control()
        {
            self.start.to_string()
        } else {
            format!("0x{:X}", self.start as u32)
        };
        let end = if !self.end.is_whitespace() && !self.end.is_control() {
            self.end.to_string()
        } else {
            format!("0x{:X}", self.end as u32)
        };
        f.debug_struct("ClassUnicodeRange")
            .field("start", &start)
            .field("end", &end)
            .finish()
    }
}

impl Interval for ClassUnicodeRange {
    type Bound = char;

    #[inline]
    fn lower(&self) -> char {
        self.start
    }
    #[inline]
    fn upper(&self) -> char {
        self.end
    }
    #[inline]
    fn set_lower(&mut self, bound: char) {
        self.start = bound;
    }
    #[inline]
    fn set_upper(&mut self, bound: char) {
        self.end = bound;
    }

    /// Apply simple case folding to this Unicode scalar value range.
    ///
    /// Additional ranges are appended to the given vector. Canonical ordering
    /// is *not* maintained in the given vector.
    fn case_fold_simple(
        &self,
        ranges: &mut Vec<ClassUnicodeRange>,
    ) -> Result<(), unicode::CaseFoldError> {
        if !unicode::contains_simple_case_mapping(self.start, self.end)? {
            return Ok(());
        }
        let start = self.start as u32;
        let end = (self.end as u32).saturating_add(1);
        let mut next_simple_cp = None;
        for cp in (start..end).filter_map(char::from_u32) {
            if next_simple_cp.map_or(false, |next| cp < next) {
                continue;
            }
            let it = match unicode::simple_fold(cp)? {
                Ok(it) => it,
                Err(next) => {
                    next_simple_cp = next;
                    continue;
                }
            };
            for cp_folded in it {
                ranges.push(ClassUnicodeRange::new(cp_folded, cp_folded));
            }
        }
        Ok(())
    }
}

impl ClassUnicodeRange {
    /// Create a new Unicode scalar value range for a character class.
    ///
    /// The returned range is always in a canonical form. That is, the range
    /// returned always satisfies the invariant that `start <= end`.
    pub fn new(start: char, end: char) -> ClassUnicodeRange {
        ClassUnicodeRange::create(start, end)
    }

    /// Return the start of this range.
    ///
    /// The start of a range is always less than or equal to the end of the
    /// range.
    pub fn start(&self) -> char {
        self.start
    }

    /// Return the end of this range.
    ///
    /// The end of a range is always greater than or equal to the start of the
    /// range.
    pub fn end(&self) -> char {
        self.end
    }
}

/// A set of characters represented by arbitrary bytes (where one byte
/// corresponds to one character).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassBytes {
    set: IntervalSet<ClassBytesRange>,
}

impl ClassBytes {
    /// Create a new class from a sequence of ranges.
    ///
    /// The given ranges do not need to be in any specific order, and ranges
    /// may overlap.
    pub fn new<I>(ranges: I) -> ClassBytes
    where
        I: IntoIterator<Item = ClassBytesRange>,
    {
        ClassBytes { set: IntervalSet::new(ranges) }
    }

    /// Create a new class with no ranges.
    pub fn empty() -> ClassBytes {
        ClassBytes::new(vec![])
    }

    /// Add a new range to this set.
    pub fn push(&mut self, range: ClassBytesRange) {
        self.set.push(range);
    }

    /// Return an iterator over all ranges in this class.
    ///
    /// The iterator yields ranges in ascending order.
    pub fn iter(&self) -> ClassBytesIter<'_> {
        ClassBytesIter(self.set.iter())
    }

    /// Return the underlying ranges as a slice.
    pub fn ranges(&self) -> &[ClassBytesRange] {
        self.set.intervals()
    }

    /// Expand this character class such that it contains all case folded
    /// characters. For example, if this class consists of the range `a-z`,
    /// then applying case folding will result in the class containing both the
    /// ranges `a-z` and `A-Z`.
    ///
    /// Note that this only applies ASCII case folding, which is limited to the
    /// characters `a-z` and `A-Z`.
    pub fn case_fold_simple(&mut self) {
        self.set.case_fold_simple().expect("ASCII case folding never fails");
    }

    /// Negate this byte class.
    ///
    /// For all `b` where `b` is a any byte, if `b` was in this set, then it
    /// will not be in this set after negation.
    pub fn negate(&mut self) {
        self.set.negate();
    }

    /// Union this byte class with the given byte class, in place.
    pub fn union(&mut self, other: &ClassBytes) {
        self.set.union(&other.set);
    }

    /// Intersect this byte class with the given byte class, in place.
    pub fn intersect(&mut self, other: &ClassBytes) {
        self.set.intersect(&other.set);
    }

    /// Subtract the given byte class from this byte class, in place.
    pub fn difference(&mut self, other: &ClassBytes) {
        self.set.difference(&other.set);
    }

    /// Compute the symmetric difference of the given byte classes, in place.
    ///
    /// This computes the symmetric difference of two byte classes. This
    /// removes all elements in this class that are also in the given class,
    /// but all adds all elements from the given class that aren't in this
    /// class. That is, the class will contain all elements in either class,
    /// but will not contain any elements that are in both classes.
    pub fn symmetric_difference(&mut self, other: &ClassBytes) {
        self.set.symmetric_difference(&other.set);
    }

    /// Returns true if and only if this character class will either match
    /// nothing or only ASCII bytes. Stated differently, this returns false
    /// if and only if this class contains a non-ASCII byte.
    pub fn is_all_ascii(&self) -> bool {
        self.set.intervals().last().map_or(true, |r| r.end <= 0x7F)
    }
}

/// An iterator over all ranges in a byte character class.
///
/// The lifetime `'a` refers to the lifetime of the underlying class.
#[derive(Debug)]
pub struct ClassBytesIter<'a>(IntervalSetIter<'a, ClassBytesRange>);

impl<'a> Iterator for ClassBytesIter<'a> {
    type Item = &'a ClassBytesRange;

    fn next(&mut self) -> Option<&'a ClassBytesRange> {
        self.0.next()
    }
}

/// A single range of characters represented by arbitrary bytes.
///
/// The range is closed. That is, the start and end of the range are included
/// in the range.
#[derive(Clone, Copy, Default, Eq, PartialEq, PartialOrd, Ord)]
pub struct ClassBytesRange {
    start: u8,
    end: u8,
}

impl Interval for ClassBytesRange {
    type Bound = u8;

    #[inline]
    fn lower(&self) -> u8 {
        self.start
    }
    #[inline]
    fn upper(&self) -> u8 {
        self.end
    }
    #[inline]
    fn set_lower(&mut self, bound: u8) {
        self.start = bound;
    }
    #[inline]
    fn set_upper(&mut self, bound: u8) {
        self.end = bound;
    }

    /// Apply simple case folding to this byte range. Only ASCII case mappings
    /// (for a-z) are applied.
    ///
    /// Additional ranges are appended to the given vector. Canonical ordering
    /// is *not* maintained in the given vector.
    fn case_fold_simple(
        &self,
        ranges: &mut Vec<ClassBytesRange>,
    ) -> Result<(), unicode::CaseFoldError> {
        if !ClassBytesRange::new(b'a', b'z').is_intersection_empty(self) {
            let lower = cmp::max(self.start, b'a');
            let upper = cmp::min(self.end, b'z');
            ranges.push(ClassBytesRange::new(lower - 32, upper - 32));
        }
        if !ClassBytesRange::new(b'A', b'Z').is_intersection_empty(self) {
            let lower = cmp::max(self.start, b'A');
            let upper = cmp::min(self.end, b'Z');
            ranges.push(ClassBytesRange::new(lower + 32, upper + 32));
        }
        Ok(())
    }
}

impl ClassBytesRange {
    /// Create a new byte range for a character class.
    ///
    /// The returned range is always in a canonical form. That is, the range
    /// returned always satisfies the invariant that `start <= end`.
    pub fn new(start: u8, end: u8) -> ClassBytesRange {
        ClassBytesRange::create(start, end)
    }

    /// Return the start of this range.
    ///
    /// The start of a range is always less than or equal to the end of the
    /// range.
    pub fn start(&self) -> u8 {
        self.start
    }

    /// Return the end of this range.
    ///
    /// The end of a range is always greater than or equal to the start of the
    /// range.
    pub fn end(&self) -> u8 {
        self.end
    }
}

impl fmt::Debug for ClassBytesRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("ClassBytesRange");
        if self.start <= 0x7F {
            debug.field("start", &(self.start as char));
        } else {
            debug.field("start", &self.start);
        }
        if self.end <= 0x7F {
            debug.field("end", &(self.end as char));
        } else {
            debug.field("end", &self.end);
        }
        debug.finish()
    }
}

/// The high-level intermediate representation for an anchor assertion.
///
/// A matching anchor assertion is always zero-length.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Anchor {
    /// Match the beginning of a line or the beginning of text. Specifically,
    /// this matches at the starting position of the input, or at the position
    /// immediately following a `\n` character.
    StartLine,
    /// Match the end of a line or the end of text. Specifically,
    /// this matches at the end position of the input, or at the position
    /// immediately preceding a `\n` character.
    EndLine,
    /// Match the beginning of text. Specifically, this matches at the starting
    /// position of the input.
    StartText,
    /// Match the end of text. Specifically, this matches at the ending
    /// position of the input.
    EndText,
}

/// The high-level intermediate representation for a word-boundary assertion.
///
/// A matching word boundary assertion is always zero-length.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WordBoundary {
    /// Match a Unicode-aware word boundary. That is, this matches a position
    /// where the left adjacent character and right adjacent character
    /// correspond to a word and non-word or a non-word and word character.
    Unicode,
    /// Match a Unicode-aware negation of a word boundary.
    UnicodeNegate,
    /// Match an ASCII-only word boundary. That is, this matches a position
    /// where the left adjacent character and right adjacent character
    /// correspond to a word and non-word or a non-word and word character.
    Ascii,
    /// Match an ASCII-only negation of a word boundary.
    AsciiNegate,
}

impl WordBoundary {
    /// Returns true if and only if this word boundary assertion is negated.
    pub fn is_negated(&self) -> bool {
        match *self {
            WordBoundary::Unicode | WordBoundary::Ascii => false,
            WordBoundary::UnicodeNegate | WordBoundary::AsciiNegate => true,
        }
    }
}

/// The high-level intermediate representation for a group.
///
/// This represents one of three possible group types:
///
/// 1. A non-capturing group (e.g., `(?:expr)`).
/// 2. A capturing group (e.g., `(expr)`).
/// 3. A named capturing group (e.g., `(?P<name>expr)`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Group {
    /// The kind of this group. If it is a capturing group, then the kind
    /// contains the capture group index (and the name, if it is a named
    /// group).
    pub kind: GroupKind,
    /// The expression inside the capturing group, which may be empty.
    pub hir: Box<Hir>,
}

/// The kind of group.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GroupKind {
    /// A normal unnamed capturing group.
    ///
    /// The value is the capture index of the group.
    CaptureIndex(u32),
    /// A named capturing group.
    CaptureName {
        /// The name of the group.
        name: String,
        /// The capture index of the group.
        index: u32,
    },
    /// A non-capturing group.
    NonCapturing,
}

/// The high-level intermediate representation of a repetition operator.
///
/// A repetition operator permits the repetition of an arbitrary
/// sub-expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Repetition {
    /// The kind of this repetition operator.
    pub kind: RepetitionKind,
    /// Whether this repetition operator is greedy or not. A greedy operator
    /// will match as much as it can. A non-greedy operator will match as
    /// little as it can.
    ///
    /// Typically, operators are greedy by default and are only non-greedy when
    /// a `?` suffix is used, e.g., `(expr)*` is greedy while `(expr)*?` is
    /// not. However, this can be inverted via the `U` "ungreedy" flag.
    pub greedy: bool,
    /// The expression being repeated.
    pub hir: Box<Hir>,
}

impl Repetition {
    /// Returns true if and only if this repetition operator makes it possible
    /// to match the empty string.
    ///
    /// Note that this is not defined inductively. For example, while `a*`
    /// will report `true`, `()+` will not, even though `()` matches the empty
    /// string and one or more occurrences of something that matches the empty
    /// string will always match the empty string. In order to get the
    /// inductive definition, see the corresponding method on
    /// [`Hir`](struct.Hir.html).
    pub fn is_match_empty(&self) -> bool {
        match self.kind {
            RepetitionKind::ZeroOrOne => true,
            RepetitionKind::ZeroOrMore => true,
            RepetitionKind::OneOrMore => false,
            RepetitionKind::Range(RepetitionRange::Exactly(m)) => m == 0,
            RepetitionKind::Range(RepetitionRange::AtLeast(m)) => m == 0,
            RepetitionKind::Range(RepetitionRange::Bounded(m, _)) => m == 0,
        }
    }
}

/// The kind of a repetition operator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepetitionKind {
    /// Matches a sub-expression zero or one times.
    ZeroOrOne,
    /// Matches a sub-expression zero or more times.
    ZeroOrMore,
    /// Matches a sub-expression one or more times.
    OneOrMore,
    /// Matches a sub-expression within a bounded range of times.
    Range(RepetitionRange),
}

/// The kind of a counted repetition operator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepetitionRange {
    /// Matches a sub-expression exactly this many times.
    Exactly(u32),
    /// Matches a sub-expression at least this many times.
    AtLeast(u32),
    /// Matches a sub-expression at least `m` times and at most `n` times.
    Bounded(u32, u32),
}

/// A custom `Drop` impl is used for `HirKind` such that it uses constant stack
/// space but heap space proportional to the depth of the total `Hir`.
impl Drop for Hir {
    fn drop(&mut self) {
        use std::mem;

        match *self.kind() {
            HirKind::Empty
            | HirKind::Literal(_)
            | HirKind::Class(_)
            | HirKind::Anchor(_)
            | HirKind::WordBoundary(_) => return,
            HirKind::Group(ref x) if !x.hir.kind.has_subexprs() => return,
            HirKind::Repetition(ref x) if !x.hir.kind.has_subexprs() => return,
            HirKind::Concat(ref x) if x.is_empty() => return,
            HirKind::Alternation(ref x) if x.is_empty() => return,
            _ => {}
        }

        let mut stack = vec![mem::replace(self, Hir::empty())];
        while let Some(mut expr) = stack.pop() {
            match expr.kind {
                HirKind::Empty
                | HirKind::Literal(_)
                | HirKind::Class(_)
                | HirKind::Anchor(_)
                | HirKind::WordBoundary(_) => {}
                HirKind::Group(ref mut x) => {
                    stack.push(mem::replace(&mut x.hir, Hir::empty()));
                }
                HirKind::Repetition(ref mut x) => {
                    stack.push(mem::replace(&mut x.hir, Hir::empty()));
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

/// A type that documents various attributes of an HIR expression.
///
/// These attributes are typically defined inductively on the HIR.
#[derive(Clone, Debug, Eq, PartialEq)]
struct HirInfo {
    /// Represent yes/no questions by a bitfield to conserve space, since
    /// this is included in every HIR expression.
    ///
    /// If more attributes need to be added, it is OK to increase the size of
    /// this as appropriate.
    bools: u16,
}

// A simple macro for defining bitfield accessors/mutators.
macro_rules! define_bool {
    ($bit:expr, $is_fn_name:ident, $set_fn_name:ident) => {
        fn $is_fn_name(&self) -> bool {
            self.bools & (0b1 << $bit) > 0
        }

        fn $set_fn_name(&mut self, yes: bool) {
            if yes {
                self.bools |= 1 << $bit;
            } else {
                self.bools &= !(1 << $bit);
            }
        }
    };
}

impl HirInfo {
    fn new() -> HirInfo {
        HirInfo { bools: 0 }
    }

    define_bool!(0, is_always_utf8, set_always_utf8);
    define_bool!(1, is_all_assertions, set_all_assertions);
    define_bool!(2, is_anchored_start, set_anchored_start);
    define_bool!(3, is_anchored_end, set_anchored_end);
    define_bool!(4, is_line_anchored_start, set_line_anchored_start);
    define_bool!(5, is_line_anchored_end, set_line_anchored_end);
    define_bool!(6, is_any_anchored_start, set_any_anchored_start);
    define_bool!(7, is_any_anchored_end, set_any_anchored_end);
    define_bool!(8, is_match_empty, set_match_empty);
    define_bool!(9, is_literal, set_literal);
    define_bool!(10, is_alternation_literal, set_alternation_literal);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uclass(ranges: &[(char, char)]) -> ClassUnicode {
        let ranges: Vec<ClassUnicodeRange> = ranges
            .iter()
            .map(|&(s, e)| ClassUnicodeRange::new(s, e))
            .collect();
        ClassUnicode::new(ranges)
    }

    fn bclass(ranges: &[(u8, u8)]) -> ClassBytes {
        let ranges: Vec<ClassBytesRange> =
            ranges.iter().map(|&(s, e)| ClassBytesRange::new(s, e)).collect();
        ClassBytes::new(ranges)
    }

    fn uranges(cls: &ClassUnicode) -> Vec<(char, char)> {
        cls.iter().map(|x| (x.start(), x.end())).collect()
    }

    #[cfg(feature = "unicode-case")]
    fn ucasefold(cls: &ClassUnicode) -> ClassUnicode {
        let mut cls_ = cls.clone();
        cls_.case_fold_simple();
        cls_
    }

    fn uunion(cls1: &ClassUnicode, cls2: &ClassUnicode) -> ClassUnicode {
        let mut cls_ = cls1.clone();
        cls_.union(cls2);
        cls_
    }

    fn uintersect(cls1: &ClassUnicode, cls2: &ClassUnicode) -> ClassUnicode {
        let mut cls_ = cls1.clone();
        cls_.intersect(cls2);
        cls_
    }

    fn udifference(cls1: &ClassUnicode, cls2: &ClassUnicode) -> ClassUnicode {
        let mut cls_ = cls1.clone();
        cls_.difference(cls2);
        cls_
    }

    fn usymdifference(
        cls1: &ClassUnicode,
        cls2: &ClassUnicode,
    ) -> ClassUnicode {
        let mut cls_ = cls1.clone();
        cls_.symmetric_difference(cls2);
        cls_
    }

    fn unegate(cls: &ClassUnicode) -> ClassUnicode {
        let mut cls_ = cls.clone();
        cls_.negate();
        cls_
    }

    fn branges(cls: &ClassBytes) -> Vec<(u8, u8)> {
        cls.iter().map(|x| (x.start(), x.end())).collect()
    }

    fn bcasefold(cls: &ClassBytes) -> ClassBytes {
        let mut cls_ = cls.clone();
        cls_.case_fold_simple();
        cls_
    }

    fn bunion(cls1: &ClassBytes, cls2: &ClassBytes) -> ClassBytes {
        let mut cls_ = cls1.clone();
        cls_.union(cls2);
        cls_
    }

    fn bintersect(cls1: &ClassBytes, cls2: &ClassBytes) -> ClassBytes {
        let mut cls_ = cls1.clone();
        cls_.intersect(cls2);
        cls_
    }

    fn bdifference(cls1: &ClassBytes, cls2: &ClassBytes) -> ClassBytes {
        let mut cls_ = cls1.clone();
        cls_.difference(cls2);
        cls_
    }

    fn bsymdifference(cls1: &ClassBytes, cls2: &ClassBytes) -> ClassBytes {
        let mut cls_ = cls1.clone();
        cls_.symmetric_difference(cls2);
        cls_
    }

    fn bnegate(cls: &ClassBytes) -> ClassBytes {
        let mut cls_ = cls.clone();
        cls_.negate();
        cls_
    }

    #[test]
    fn class_range_canonical_unicode() {
        let range = ClassUnicodeRange::new('\u{00FF}', '\0');
        assert_eq!('\0', range.start());
        assert_eq!('\u{00FF}', range.end());
    }

    #[test]
    fn class_range_canonical_bytes() {
        let range = ClassBytesRange::new(b'\xFF', b'\0');
        assert_eq!(b'\0', range.start());
        assert_eq!(b'\xFF', range.end());
    }

    #[test]
    fn class_canonicalize_unicode() {
        let cls = uclass(&[('a', 'c'), ('x', 'z')]);
        let expected = vec![('a', 'c'), ('x', 'z')];
        assert_eq!(expected, uranges(&cls));

        let cls = uclass(&[('x', 'z'), ('a', 'c')]);
        let expected = vec![('a', 'c'), ('x', 'z')];
        assert_eq!(expected, uranges(&cls));

        let cls = uclass(&[('x', 'z'), ('w', 'y')]);
        let expected = vec![('w', 'z')];
        assert_eq!(expected, uranges(&cls));

        let cls = uclass(&[
            ('c', 'f'),
            ('a', 'g'),
            ('d', 'j'),
            ('a', 'c'),
            ('m', 'p'),
            ('l', 's'),
        ]);
        let expected = vec![('a', 'j'), ('l', 's')];
        assert_eq!(expected, uranges(&cls));

        let cls = uclass(&[('x', 'z'), ('u', 'w')]);
        let expected = vec![('u', 'z')];
        assert_eq!(expected, uranges(&cls));

        let cls = uclass(&[('\x00', '\u{10FFFF}'), ('\x00', '\u{10FFFF}')]);
        let expected = vec![('\x00', '\u{10FFFF}')];
        assert_eq!(expected, uranges(&cls));

        let cls = uclass(&[('a', 'a'), ('b', 'b')]);
        let expected = vec![('a', 'b')];
        assert_eq!(expected, uranges(&cls));
    }

    #[test]
    fn class_canonicalize_bytes() {
        let cls = bclass(&[(b'a', b'c'), (b'x', b'z')]);
        let expected = vec![(b'a', b'c'), (b'x', b'z')];
        assert_eq!(expected, branges(&cls));

        let cls = bclass(&[(b'x', b'z'), (b'a', b'c')]);
        let expected = vec![(b'a', b'c'), (b'x', b'z')];
        assert_eq!(expected, branges(&cls));

        let cls = bclass(&[(b'x', b'z'), (b'w', b'y')]);
        let expected = vec![(b'w', b'z')];
        assert_eq!(expected, branges(&cls));

        let cls = bclass(&[
            (b'c', b'f'),
            (b'a', b'g'),
            (b'd', b'j'),
            (b'a', b'c'),
            (b'm', b'p'),
            (b'l', b's'),
        ]);
        let expected = vec![(b'a', b'j'), (b'l', b's')];
        assert_eq!(expected, branges(&cls));

        let cls = bclass(&[(b'x', b'z'), (b'u', b'w')]);
        let expected = vec![(b'u', b'z')];
        assert_eq!(expected, branges(&cls));

        let cls = bclass(&[(b'\x00', b'\xFF'), (b'\x00', b'\xFF')]);
        let expected = vec![(b'\x00', b'\xFF')];
        assert_eq!(expected, branges(&cls));

        let cls = bclass(&[(b'a', b'a'), (b'b', b'b')]);
        let expected = vec![(b'a', b'b')];
        assert_eq!(expected, branges(&cls));
    }

    #[test]
    #[cfg(feature = "unicode-case")]
    fn class_case_fold_unicode() {
        let cls = uclass(&[
            ('C', 'F'),
            ('A', 'G'),
            ('D', 'J'),
            ('A', 'C'),
            ('M', 'P'),
            ('L', 'S'),
            ('c', 'f'),
        ]);
        let expected = uclass(&[
            ('A', 'J'),
            ('L', 'S'),
            ('a', 'j'),
            ('l', 's'),
            ('\u{17F}', '\u{17F}'),
        ]);
        assert_eq!(expected, ucasefold(&cls));

        let cls = uclass(&[('A', 'Z')]);
        let expected = uclass(&[
            ('A', 'Z'),
            ('a', 'z'),
            ('\u{17F}', '\u{17F}'),
            ('\u{212A}', '\u{212A}'),
        ]);
        assert_eq!(expected, ucasefold(&cls));

        let cls = uclass(&[('a', 'z')]);
        let expected = uclass(&[
            ('A', 'Z'),
            ('a', 'z'),
            ('\u{17F}', '\u{17F}'),
            ('\u{212A}', '\u{212A}'),
        ]);
        assert_eq!(expected, ucasefold(&cls));

        let cls = uclass(&[('A', 'A'), ('_', '_')]);
        let expected = uclass(&[('A', 'A'), ('_', '_'), ('a', 'a')]);
        assert_eq!(expected, ucasefold(&cls));

        let cls = uclass(&[('A', 'A'), ('=', '=')]);
        let expected = uclass(&[('=', '='), ('A', 'A'), ('a', 'a')]);
        assert_eq!(expected, ucasefold(&cls));

        let cls = uclass(&[('\x00', '\x10')]);
        assert_eq!(cls, ucasefold(&cls));

        let cls = uclass(&[('k', 'k')]);
        let expected =
            uclass(&[('K', 'K'), ('k', 'k'), ('\u{212A}', '\u{212A}')]);
        assert_eq!(expected, ucasefold(&cls));

        let cls = uclass(&[('@', '@')]);
        assert_eq!(cls, ucasefold(&cls));
    }

    #[test]
    #[cfg(not(feature = "unicode-case"))]
    fn class_case_fold_unicode_disabled() {
        let mut cls = uclass(&[
            ('C', 'F'),
            ('A', 'G'),
            ('D', 'J'),
            ('A', 'C'),
            ('M', 'P'),
            ('L', 'S'),
            ('c', 'f'),
        ]);
        assert!(cls.try_case_fold_simple().is_err());
    }

    #[test]
    #[should_panic]
    #[cfg(not(feature = "unicode-case"))]
    fn class_case_fold_unicode_disabled_panics() {
        let mut cls = uclass(&[
            ('C', 'F'),
            ('A', 'G'),
            ('D', 'J'),
            ('A', 'C'),
            ('M', 'P'),
            ('L', 'S'),
            ('c', 'f'),
        ]);
        cls.case_fold_simple();
    }

    #[test]
    fn class_case_fold_bytes() {
        let cls = bclass(&[
            (b'C', b'F'),
            (b'A', b'G'),
            (b'D', b'J'),
            (b'A', b'C'),
            (b'M', b'P'),
            (b'L', b'S'),
            (b'c', b'f'),
        ]);
        let expected =
            bclass(&[(b'A', b'J'), (b'L', b'S'), (b'a', b'j'), (b'l', b's')]);
        assert_eq!(expected, bcasefold(&cls));

        let cls = bclass(&[(b'A', b'Z')]);
        let expected = bclass(&[(b'A', b'Z'), (b'a', b'z')]);
        assert_eq!(expected, bcasefold(&cls));

        let cls = bclass(&[(b'a', b'z')]);
        let expected = bclass(&[(b'A', b'Z'), (b'a', b'z')]);
        assert_eq!(expected, bcasefold(&cls));

        let cls = bclass(&[(b'A', b'A'), (b'_', b'_')]);
        let expected = bclass(&[(b'A', b'A'), (b'_', b'_'), (b'a', b'a')]);
        assert_eq!(expected, bcasefold(&cls));

        let cls = bclass(&[(b'A', b'A'), (b'=', b'=')]);
        let expected = bclass(&[(b'=', b'='), (b'A', b'A'), (b'a', b'a')]);
        assert_eq!(expected, bcasefold(&cls));

        let cls = bclass(&[(b'\x00', b'\x10')]);
        assert_eq!(cls, bcasefold(&cls));

        let cls = bclass(&[(b'k', b'k')]);
        let expected = bclass(&[(b'K', b'K'), (b'k', b'k')]);
        assert_eq!(expected, bcasefold(&cls));

        let cls = bclass(&[(b'@', b'@')]);
        assert_eq!(cls, bcasefold(&cls));
    }

    #[test]
    fn class_negate_unicode() {
        let cls = uclass(&[('a', 'a')]);
        let expected = uclass(&[('\x00', '\x60'), ('\x62', '\u{10FFFF}')]);
        assert_eq!(expected, unegate(&cls));

        let cls = uclass(&[('a', 'a'), ('b', 'b')]);
        let expected = uclass(&[('\x00', '\x60'), ('\x63', '\u{10FFFF}')]);
        assert_eq!(expected, unegate(&cls));

        let cls = uclass(&[('a', 'c'), ('x', 'z')]);
        let expected = uclass(&[
            ('\x00', '\x60'),
            ('\x64', '\x77'),
            ('\x7B', '\u{10FFFF}'),
        ]);
        assert_eq!(expected, unegate(&cls));

        let cls = uclass(&[('\x00', 'a')]);
        let expected = uclass(&[('\x62', '\u{10FFFF}')]);
        assert_eq!(expected, unegate(&cls));

        let cls = uclass(&[('a', '\u{10FFFF}')]);
        let expected = uclass(&[('\x00', '\x60')]);
        assert_eq!(expected, unegate(&cls));

        let cls = uclass(&[('\x00', '\u{10FFFF}')]);
        let expected = uclass(&[]);
        assert_eq!(expected, unegate(&cls));

        let cls = uclass(&[]);
        let expected = uclass(&[('\x00', '\u{10FFFF}')]);
        assert_eq!(expected, unegate(&cls));

        let cls =
            uclass(&[('\x00', '\u{10FFFD}'), ('\u{10FFFF}', '\u{10FFFF}')]);
        let expected = uclass(&[('\u{10FFFE}', '\u{10FFFE}')]);
        assert_eq!(expected, unegate(&cls));

        let cls = uclass(&[('\x00', '\u{D7FF}')]);
        let expected = uclass(&[('\u{E000}', '\u{10FFFF}')]);
        assert_eq!(expected, unegate(&cls));

        let cls = uclass(&[('\x00', '\u{D7FE}')]);
        let expected = uclass(&[('\u{D7FF}', '\u{10FFFF}')]);
        assert_eq!(expected, unegate(&cls));

        let cls = uclass(&[('\u{E000}', '\u{10FFFF}')]);
        let expected = uclass(&[('\x00', '\u{D7FF}')]);
        assert_eq!(expected, unegate(&cls));

        let cls = uclass(&[('\u{E001}', '\u{10FFFF}')]);
        let expected = uclass(&[('\x00', '\u{E000}')]);
        assert_eq!(expected, unegate(&cls));
    }

    #[test]
    fn class_negate_bytes() {
        let cls = bclass(&[(b'a', b'a')]);
        let expected = bclass(&[(b'\x00', b'\x60'), (b'\x62', b'\xFF')]);
        assert_eq!(expected, bnegate(&cls));

        let cls = bclass(&[(b'a', b'a'), (b'b', b'b')]);
        let expected = bclass(&[(b'\x00', b'\x60'), (b'\x63', b'\xFF')]);
        assert_eq!(expected, bnegate(&cls));

        let cls = bclass(&[(b'a', b'c'), (b'x', b'z')]);
        let expected = bclass(&[
            (b'\x00', b'\x60'),
            (b'\x64', b'\x77'),
            (b'\x7B', b'\xFF'),
        ]);
        assert_eq!(expected, bnegate(&cls));

        let cls = bclass(&[(b'\x00', b'a')]);
        let expected = bclass(&[(b'\x62', b'\xFF')]);
        assert_eq!(expected, bnegate(&cls));

        let cls = bclass(&[(b'a', b'\xFF')]);
        let expected = bclass(&[(b'\x00', b'\x60')]);
        assert_eq!(expected, bnegate(&cls));

        let cls = bclass(&[(b'\x00', b'\xFF')]);
        let expected = bclass(&[]);
        assert_eq!(expected, bnegate(&cls));

        let cls = bclass(&[]);
        let expected = bclass(&[(b'\x00', b'\xFF')]);
        assert_eq!(expected, bnegate(&cls));

        let cls = bclass(&[(b'\x00', b'\xFD'), (b'\xFF', b'\xFF')]);
        let expected = bclass(&[(b'\xFE', b'\xFE')]);
        assert_eq!(expected, bnegate(&cls));
    }

    #[test]
    fn class_union_unicode() {
        let cls1 = uclass(&[('a', 'g'), ('m', 't'), ('A', 'C')]);
        let cls2 = uclass(&[('a', 'z')]);
        let expected = uclass(&[('a', 'z'), ('A', 'C')]);
        assert_eq!(expected, uunion(&cls1, &cls2));
    }

    #[test]
    fn class_union_bytes() {
        let cls1 = bclass(&[(b'a', b'g'), (b'm', b't'), (b'A', b'C')]);
        let cls2 = bclass(&[(b'a', b'z')]);
        let expected = bclass(&[(b'a', b'z'), (b'A', b'C')]);
        assert_eq!(expected, bunion(&cls1, &cls2));
    }

    #[test]
    fn class_intersect_unicode() {
        let cls1 = uclass(&[]);
        let cls2 = uclass(&[('a', 'a')]);
        let expected = uclass(&[]);
        assert_eq!(expected, uintersect(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'a')]);
        let cls2 = uclass(&[('a', 'a')]);
        let expected = uclass(&[('a', 'a')]);
        assert_eq!(expected, uintersect(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'a')]);
        let cls2 = uclass(&[('b', 'b')]);
        let expected = uclass(&[]);
        assert_eq!(expected, uintersect(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'a')]);
        let cls2 = uclass(&[('a', 'c')]);
        let expected = uclass(&[('a', 'a')]);
        assert_eq!(expected, uintersect(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'b')]);
        let cls2 = uclass(&[('a', 'c')]);
        let expected = uclass(&[('a', 'b')]);
        assert_eq!(expected, uintersect(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'b')]);
        let cls2 = uclass(&[('b', 'c')]);
        let expected = uclass(&[('b', 'b')]);
        assert_eq!(expected, uintersect(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'b')]);
        let cls2 = uclass(&[('c', 'd')]);
        let expected = uclass(&[]);
        assert_eq!(expected, uintersect(&cls1, &cls2));

        let cls1 = uclass(&[('b', 'c')]);
        let cls2 = uclass(&[('a', 'd')]);
        let expected = uclass(&[('b', 'c')]);
        assert_eq!(expected, uintersect(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'b'), ('d', 'e'), ('g', 'h')]);
        let cls2 = uclass(&[('a', 'h')]);
        let expected = uclass(&[('a', 'b'), ('d', 'e'), ('g', 'h')]);
        assert_eq!(expected, uintersect(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'b'), ('d', 'e'), ('g', 'h')]);
        let cls2 = uclass(&[('a', 'b'), ('d', 'e'), ('g', 'h')]);
        let expected = uclass(&[('a', 'b'), ('d', 'e'), ('g', 'h')]);
        assert_eq!(expected, uintersect(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'b'), ('g', 'h')]);
        let cls2 = uclass(&[('d', 'e'), ('k', 'l')]);
        let expected = uclass(&[]);
        assert_eq!(expected, uintersect(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'b'), ('d', 'e'), ('g', 'h')]);
        let cls2 = uclass(&[('h', 'h')]);
        let expected = uclass(&[('h', 'h')]);
        assert_eq!(expected, uintersect(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'b'), ('e', 'f'), ('i', 'j')]);
        let cls2 = uclass(&[('c', 'd'), ('g', 'h'), ('k', 'l')]);
        let expected = uclass(&[]);
        assert_eq!(expected, uintersect(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'b'), ('c', 'd'), ('e', 'f')]);
        let cls2 = uclass(&[('b', 'c'), ('d', 'e'), ('f', 'g')]);
        let expected = uclass(&[('b', 'f')]);
        assert_eq!(expected, uintersect(&cls1, &cls2));
    }

    #[test]
    fn class_intersect_bytes() {
        let cls1 = bclass(&[]);
        let cls2 = bclass(&[(b'a', b'a')]);
        let expected = bclass(&[]);
        assert_eq!(expected, bintersect(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'a')]);
        let cls2 = bclass(&[(b'a', b'a')]);
        let expected = bclass(&[(b'a', b'a')]);
        assert_eq!(expected, bintersect(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'a')]);
        let cls2 = bclass(&[(b'b', b'b')]);
        let expected = bclass(&[]);
        assert_eq!(expected, bintersect(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'a')]);
        let cls2 = bclass(&[(b'a', b'c')]);
        let expected = bclass(&[(b'a', b'a')]);
        assert_eq!(expected, bintersect(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'b')]);
        let cls2 = bclass(&[(b'a', b'c')]);
        let expected = bclass(&[(b'a', b'b')]);
        assert_eq!(expected, bintersect(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'b')]);
        let cls2 = bclass(&[(b'b', b'c')]);
        let expected = bclass(&[(b'b', b'b')]);
        assert_eq!(expected, bintersect(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'b')]);
        let cls2 = bclass(&[(b'c', b'd')]);
        let expected = bclass(&[]);
        assert_eq!(expected, bintersect(&cls1, &cls2));

        let cls1 = bclass(&[(b'b', b'c')]);
        let cls2 = bclass(&[(b'a', b'd')]);
        let expected = bclass(&[(b'b', b'c')]);
        assert_eq!(expected, bintersect(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'b'), (b'd', b'e'), (b'g', b'h')]);
        let cls2 = bclass(&[(b'a', b'h')]);
        let expected = bclass(&[(b'a', b'b'), (b'd', b'e'), (b'g', b'h')]);
        assert_eq!(expected, bintersect(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'b'), (b'd', b'e'), (b'g', b'h')]);
        let cls2 = bclass(&[(b'a', b'b'), (b'd', b'e'), (b'g', b'h')]);
        let expected = bclass(&[(b'a', b'b'), (b'd', b'e'), (b'g', b'h')]);
        assert_eq!(expected, bintersect(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'b'), (b'g', b'h')]);
        let cls2 = bclass(&[(b'd', b'e'), (b'k', b'l')]);
        let expected = bclass(&[]);
        assert_eq!(expected, bintersect(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'b'), (b'd', b'e'), (b'g', b'h')]);
        let cls2 = bclass(&[(b'h', b'h')]);
        let expected = bclass(&[(b'h', b'h')]);
        assert_eq!(expected, bintersect(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'b'), (b'e', b'f'), (b'i', b'j')]);
        let cls2 = bclass(&[(b'c', b'd'), (b'g', b'h'), (b'k', b'l')]);
        let expected = bclass(&[]);
        assert_eq!(expected, bintersect(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'b'), (b'c', b'd'), (b'e', b'f')]);
        let cls2 = bclass(&[(b'b', b'c'), (b'd', b'e'), (b'f', b'g')]);
        let expected = bclass(&[(b'b', b'f')]);
        assert_eq!(expected, bintersect(&cls1, &cls2));
    }

    #[test]
    fn class_difference_unicode() {
        let cls1 = uclass(&[('a', 'a')]);
        let cls2 = uclass(&[('a', 'a')]);
        let expected = uclass(&[]);
        assert_eq!(expected, udifference(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'a')]);
        let cls2 = uclass(&[]);
        let expected = uclass(&[('a', 'a')]);
        assert_eq!(expected, udifference(&cls1, &cls2));

        let cls1 = uclass(&[]);
        let cls2 = uclass(&[('a', 'a')]);
        let expected = uclass(&[]);
        assert_eq!(expected, udifference(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'z')]);
        let cls2 = uclass(&[('a', 'a')]);
        let expected = uclass(&[('b', 'z')]);
        assert_eq!(expected, udifference(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'z')]);
        let cls2 = uclass(&[('z', 'z')]);
        let expected = uclass(&[('a', 'y')]);
        assert_eq!(expected, udifference(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'z')]);
        let cls2 = uclass(&[('m', 'm')]);
        let expected = uclass(&[('a', 'l'), ('n', 'z')]);
        assert_eq!(expected, udifference(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'c'), ('g', 'i'), ('r', 't')]);
        let cls2 = uclass(&[('a', 'z')]);
        let expected = uclass(&[]);
        assert_eq!(expected, udifference(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'c'), ('g', 'i'), ('r', 't')]);
        let cls2 = uclass(&[('d', 'v')]);
        let expected = uclass(&[('a', 'c')]);
        assert_eq!(expected, udifference(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'c'), ('g', 'i'), ('r', 't')]);
        let cls2 = uclass(&[('b', 'g'), ('s', 'u')]);
        let expected = uclass(&[('a', 'a'), ('h', 'i'), ('r', 'r')]);
        assert_eq!(expected, udifference(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'c'), ('g', 'i'), ('r', 't')]);
        let cls2 = uclass(&[('b', 'd'), ('e', 'g'), ('s', 'u')]);
        let expected = uclass(&[('a', 'a'), ('h', 'i'), ('r', 'r')]);
        assert_eq!(expected, udifference(&cls1, &cls2));

        let cls1 = uclass(&[('x', 'z')]);
        let cls2 = uclass(&[('a', 'c'), ('e', 'g'), ('s', 'u')]);
        let expected = uclass(&[('x', 'z')]);
        assert_eq!(expected, udifference(&cls1, &cls2));

        let cls1 = uclass(&[('a', 'z')]);
        let cls2 = uclass(&[('a', 'c'), ('e', 'g'), ('s', 'u')]);
        let expected = uclass(&[('d', 'd'), ('h', 'r'), ('v', 'z')]);
        assert_eq!(expected, udifference(&cls1, &cls2));
    }

    #[test]
    fn class_difference_bytes() {
        let cls1 = bclass(&[(b'a', b'a')]);
        let cls2 = bclass(&[(b'a', b'a')]);
        let expected = bclass(&[]);
        assert_eq!(expected, bdifference(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'a')]);
        let cls2 = bclass(&[]);
        let expected = bclass(&[(b'a', b'a')]);
        assert_eq!(expected, bdifference(&cls1, &cls2));

        let cls1 = bclass(&[]);
        let cls2 = bclass(&[(b'a', b'a')]);
        let expected = bclass(&[]);
        assert_eq!(expected, bdifference(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'z')]);
        let cls2 = bclass(&[(b'a', b'a')]);
        let expected = bclass(&[(b'b', b'z')]);
        assert_eq!(expected, bdifference(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'z')]);
        let cls2 = bclass(&[(b'z', b'z')]);
        let expected = bclass(&[(b'a', b'y')]);
        assert_eq!(expected, bdifference(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'z')]);
        let cls2 = bclass(&[(b'm', b'm')]);
        let expected = bclass(&[(b'a', b'l'), (b'n', b'z')]);
        assert_eq!(expected, bdifference(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'c'), (b'g', b'i'), (b'r', b't')]);
        let cls2 = bclass(&[(b'a', b'z')]);
        let expected = bclass(&[]);
        assert_eq!(expected, bdifference(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'c'), (b'g', b'i'), (b'r', b't')]);
        let cls2 = bclass(&[(b'd', b'v')]);
        let expected = bclass(&[(b'a', b'c')]);
        assert_eq!(expected, bdifference(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'c'), (b'g', b'i'), (b'r', b't')]);
        let cls2 = bclass(&[(b'b', b'g'), (b's', b'u')]);
        let expected = bclass(&[(b'a', b'a'), (b'h', b'i'), (b'r', b'r')]);
        assert_eq!(expected, bdifference(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'c'), (b'g', b'i'), (b'r', b't')]);
        let cls2 = bclass(&[(b'b', b'd'), (b'e', b'g'), (b's', b'u')]);
        let expected = bclass(&[(b'a', b'a'), (b'h', b'i'), (b'r', b'r')]);
        assert_eq!(expected, bdifference(&cls1, &cls2));

        let cls1 = bclass(&[(b'x', b'z')]);
        let cls2 = bclass(&[(b'a', b'c'), (b'e', b'g'), (b's', b'u')]);
        let expected = bclass(&[(b'x', b'z')]);
        assert_eq!(expected, bdifference(&cls1, &cls2));

        let cls1 = bclass(&[(b'a', b'z')]);
        let cls2 = bclass(&[(b'a', b'c'), (b'e', b'g'), (b's', b'u')]);
        let expected = bclass(&[(b'd', b'd'), (b'h', b'r'), (b'v', b'z')]);
        assert_eq!(expected, bdifference(&cls1, &cls2));
    }

    #[test]
    fn class_symmetric_difference_unicode() {
        let cls1 = uclass(&[('a', 'm')]);
        let cls2 = uclass(&[('g', 't')]);
        let expected = uclass(&[('a', 'f'), ('n', 't')]);
        assert_eq!(expected, usymdifference(&cls1, &cls2));
    }

    #[test]
    fn class_symmetric_difference_bytes() {
        let cls1 = bclass(&[(b'a', b'm')]);
        let cls2 = bclass(&[(b'g', b't')]);
        let expected = bclass(&[(b'a', b'f'), (b'n', b't')]);
        assert_eq!(expected, bsymdifference(&cls1, &cls2));
    }

    #[test]
    #[should_panic]
    fn hir_byte_literal_non_ascii() {
        Hir::literal(Literal::Byte(b'a'));
    }

    // We use a thread with an explicit stack size to test that our destructor
    // for Hir can handle arbitrarily sized expressions in constant stack
    // space. In case we run on a platform without threads (WASM?), we limit
    // this test to Windows/Unix.
    #[test]
    #[cfg(any(unix, windows))]
    fn no_stack_overflow_on_drop() {
        use std::thread;

        let run = || {
            let mut expr = Hir::empty();
            for _ in 0..100 {
                expr = Hir::group(Group {
                    kind: GroupKind::NonCapturing,
                    hir: Box::new(expr),
                });
                expr = Hir::repetition(Repetition {
                    kind: RepetitionKind::ZeroOrOne,
                    greedy: true,
                    hir: Box::new(expr),
                });

                expr = Hir {
                    kind: HirKind::Concat(vec![expr]),
                    info: HirInfo::new(),
                };
                expr = Hir {
                    kind: HirKind::Alternation(vec![expr]),
                    info: HirInfo::new(),
                };
            }
            assert!(!expr.kind.is_empty());
        };

        // We run our test on a thread with a small stack size so we can
        // force the issue more easily.
        //
        // NOTE(2023-03-21): See the corresponding test in 'crate::ast::tests'
        // for context on the specific stack size chosen here.
        thread::Builder::new()
            .stack_size(16 << 10)
            .spawn(run)
            .unwrap()
            .join()
            .unwrap();
    }
}

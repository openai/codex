/*!
Defines a high-level intermediate (HIR) representation for regular expressions.

The HIR is represented by the [`Hir`] type, and it principally constructed via
[translation](translate) from an [`Ast`](crate::ast::Ast). Alternatively, users
may use the smart constructors defined on `Hir` to build their own by hand. The
smart constructors simultaneously simplify and "optimize" the HIR, and are also
the same routines used by translation.

Most regex engines only have an HIR like this, and usually construct it
directly from the concrete syntax. This crate however first parses the
concrete syntax into an `Ast`, and only then creates the HIR from the `Ast`,
as mentioned above. It's done this way to facilitate better error reporting,
and to have a structured representation of a regex that faithfully represents
its concrete syntax. Namely, while an `Hir` value can be converted back to an
equivalent regex pattern string, it is unlikely to look like the original due
to its simplified structure.
*/

use core::{char, cmp};

use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use crate::{
    ast::Span,
    hir::interval::{Interval, IntervalSet, IntervalSetIter},
    unicode,
};

pub use crate::{
    hir::visitor::{visit, Visitor},
    unicode::CaseFoldError,
};

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
///
/// This error type is marked as `non_exhaustive`. This means that adding a
/// new variant is not considered a breaking change.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    /// This error occurs when a Unicode feature is used when Unicode
    /// support is disabled. For example `(?-u:\pL)` would trigger this error.
    UnicodeNotAllowed,
    /// This error occurs when translating a pattern that could match a byte
    /// sequence that isn't UTF-8 and `utf8` was enabled.
    InvalidUtf8,
    /// This error occurs when one uses a non-ASCII byte for a line terminator,
    /// but where Unicode mode is enabled and UTF-8 mode is disabled.
    InvalidLineTerminator,
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
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        crate::error::Formatter::from(self).fmt(f)
    }
}

impl core::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use self::ErrorKind::*;

        let msg = match *self {
            UnicodeNotAllowed => "Unicode not allowed here",
            InvalidUtf8 => "pattern can match invalid UTF-8",
            InvalidLineTerminator => "invalid line terminator, must be ASCII",
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
        };
        f.write_str(msg)
    }
}

/// A high-level intermediate representation (HIR) for a regular expression.
///
/// An HIR value is a combination of a [`HirKind`] and a set of [`Properties`].
/// An `HirKind` indicates what kind of regular expression it is (a literal,
/// a repetition, a look-around assertion, etc.), where as a `Properties`
/// describes various facts about the regular expression. For example, whether
/// it matches UTF-8 or if it matches the empty string.
///
/// The HIR of a regular expression represents an intermediate step between
/// its abstract syntax (a structured description of the concrete syntax) and
/// an actual regex matcher. The purpose of HIR is to make regular expressions
/// easier to analyze. In particular, the AST is much more complex than the
/// HIR. For example, while an AST supports arbitrarily nested character
/// classes, the HIR will flatten all nested classes into a single set. The HIR
/// will also "compile away" every flag present in the concrete syntax. For
/// example, users of HIR expressions never need to worry about case folding;
/// it is handled automatically by the translator (e.g., by translating
/// `(?i:A)` to `[aA]`).
///
/// The specific type of an HIR expression can be accessed via its `kind`
/// or `into_kind` methods. This extra level of indirection exists for two
/// reasons:
///
/// 1. Construction of an HIR expression *must* use the constructor methods on
/// this `Hir` type instead of building the `HirKind` values directly. This
/// permits construction to enforce invariants like "concatenations always
/// consist of two or more sub-expressions."
/// 2. Every HIR expression contains attributes that are defined inductively,
/// and can be computed cheaply during the construction process. For example,
/// one such attribute is whether the expression must match at the beginning of
/// the haystack.
///
/// In particular, if you have an `HirKind` value, then there is intentionally
/// no way to build an `Hir` value from it. You instead need to do case
/// analysis on the `HirKind` value and build the `Hir` value using its smart
/// constructors.
///
/// # UTF-8
///
/// If the HIR was produced by a translator with
/// [`TranslatorBuilder::utf8`](translate::TranslatorBuilder::utf8) enabled,
/// then the HIR is guaranteed to match UTF-8 exclusively for all non-empty
/// matches.
///
/// For empty matches, those can occur at any position. It is the
/// responsibility of the regex engine to determine whether empty matches are
/// permitted between the code units of a single codepoint.
///
/// # Stack space
///
/// This type defines its own destructor that uses constant stack space and
/// heap space proportional to the size of the HIR.
///
/// Also, an `Hir`'s `fmt::Display` implementation prints an HIR as a regular
/// expression pattern string, and uses constant stack space and heap space
/// proportional to the size of the `Hir`. The regex it prints is guaranteed to
/// be _semantically_ equivalent to the original concrete syntax, but it may
/// look very different. (And potentially not practically readable by a human.)
///
/// An `Hir`'s `fmt::Debug` implementation currently does not use constant
/// stack space. The implementation will also suppress some details (such as
/// the `Properties` inlined into every `Hir` value to make it less noisy).
#[derive(Clone, Eq, PartialEq)]
pub struct Hir {
    /// The underlying HIR kind.
    kind: HirKind,
    /// Analysis info about this HIR, computed during construction.
    props: Properties,
}

/// Methods for accessing the underlying `HirKind` and `Properties`.
impl Hir {
    /// Returns a reference to the underlying HIR kind.
    pub fn kind(&self) -> &HirKind {
        &self.kind
    }

    /// Consumes ownership of this HIR expression and returns its underlying
    /// `HirKind`.
    pub fn into_kind(mut self) -> HirKind {
        core::mem::replace(&mut self.kind, HirKind::Empty)
    }

    /// Returns the properties computed for this `Hir`.
    pub fn properties(&self) -> &Properties {
        &self.props
    }

    /// Splits this HIR into its constituent parts.
    ///
    /// This is useful because `let Hir { kind, props } = hir;` does not work
    /// because of `Hir`'s custom `Drop` implementation.
    fn into_parts(mut self) -> (HirKind, Properties) {
        (
            core::mem::replace(&mut self.kind, HirKind::Empty),
            core::mem::replace(&mut self.props, Properties::empty()),
        )
    }
}

/// Smart constructors for HIR values.
///
/// These constructors are called "smart" because they do inductive work or
/// simplifications. For example, calling `Hir::repetition` with a repetition
/// like `a{0}` will actually return a `Hir` with a `HirKind::Empty` kind
/// since it is equivalent to an empty regex. Another example is calling
/// `Hir::concat(vec![expr])`. Instead of getting a `HirKind::Concat`, you'll
/// just get back the original `expr` since it's precisely equivalent.
///
/// Smart constructors enable maintaining invariants about the HIR data type
/// while also simulanteously keeping the representation as simple as possible.
impl Hir {
    /// Returns an empty HIR expression.
    ///
    /// An empty HIR expression always matches, including the empty string.
    #[inline]
    pub fn empty() -> Hir {
        let props = Properties::empty();
        Hir { kind: HirKind::Empty, props }
    }

    /// Returns an HIR expression that can never match anything. That is,
    /// the size of the set of strings in the language described by the HIR
    /// returned is `0`.
    ///
    /// This is distinct from [`Hir::empty`] in that the empty string matches
    /// the HIR returned by `Hir::empty`. That is, the set of strings in the
    /// language describe described by `Hir::empty` is non-empty.
    ///
    /// Note that currently, the HIR returned uses an empty character class to
    /// indicate that nothing can match. An equivalent expression that cannot
    /// match is an empty alternation, but all such "fail" expressions are
    /// normalized (via smart constructors) to empty character classes. This is
    /// because empty character classes can be spelled in the concrete syntax
    /// of a regex (e.g., `\P{any}` or `(?-u:[^\x00-\xFF])` or `[a&&b]`), but
    /// empty alternations cannot.
    #[inline]
    pub fn fail() -> Hir {
        let class = Class::Bytes(ClassBytes::empty());
        let props = Properties::class(&class);
        // We can't just call Hir::class here because it defers to Hir::fail
        // in order to canonicalize the Hir value used to represent "cannot
        // match."
        Hir { kind: HirKind::Class(class), props }
    }

    /// Creates a literal HIR expression.
    ///
    /// This accepts anything that can be converted into a `Box<[u8]>`.
    ///
    /// Note that there is no mechanism for storing a `char` or a `Box<str>`
    /// in an HIR. Everything is "just bytes." Whether a `Literal` (or
    /// any HIR node) matches valid UTF-8 exclusively can be queried via
    /// [`Properties::is_utf8`].
    ///
    /// # Example
    ///
    /// This example shows that concatenations of `Literal` HIR values will
    /// automatically get flattened and combined together. So for example, even
    /// if you concat multiple `Literal` values that are themselves not valid
    /// UTF-8, they might add up to valid UTF-8. This also demonstrates just
    /// how "smart" Hir's smart constructors are.
    ///
    /// ```
    /// use regex_syntax::hir::{Hir, HirKind, Literal};
    ///
    /// let literals = vec![
    ///     Hir::literal([0xE2]),
    ///     Hir::literal([0x98]),
    ///     Hir::literal([0x83]),
    /// ];
    /// // Each literal, on its own, is invalid UTF-8.
    /// assert!(literals.iter().all(|hir| !hir.properties().is_utf8()));
    ///
    /// let concat = Hir::concat(literals);
    /// // But the concatenation is valid UTF-8!
    /// assert!(concat.properties().is_utf8());
    ///
    /// // And also notice that the literals have been concatenated into a
    /// // single `Literal`, to the point where there is no explicit `Concat`!
    /// let expected = HirKind::Literal(Literal(Box::from("☃".as_bytes())));
    /// assert_eq!(&expected, concat.kind());
    /// ```
    ///
    /// # Example: building a literal from a `char`
    ///
    /// This example shows how to build a single `Hir` literal from a `char`
    /// value. Since a [`Literal`] is just bytes, we just need to UTF-8
    /// encode a `char` value:
    ///
    /// ```
    /// use regex_syntax::hir::{Hir, HirKind, Literal};
    ///
    /// let ch = '☃';
    /// let got = Hir::literal(ch.encode_utf8(&mut [0; 4]).as_bytes());
    ///
    /// let expected = HirKind::Literal(Literal(Box::from("☃".as_bytes())));
    /// assert_eq!(&expected, got.kind());
    /// ```
    #[inline]
    pub fn literal<B: Into<Box<[u8]>>>(lit: B) -> Hir {
        let bytes = lit.into();
        if bytes.is_empty() {
            return Hir::empty();
        }

        let lit = Literal(bytes);
        let props = Properties::literal(&lit);
        Hir { kind: HirKind::Literal(lit), props }
    }

    /// Creates a class HIR expression. The class may either be defined over
    /// ranges of Unicode codepoints or ranges of raw byte values.
    ///
    /// Note that an empty class is permitted. An empty class is equivalent to
    /// `Hir::fail()`.
    #[inline]
    pub fn class(class: Class) -> Hir {
        if class.is_empty() {
            return Hir::fail();
        } else if let Some(bytes) = class.literal() {
            return Hir::literal(bytes);
        }
        let props = Properties::class(&class);
        Hir { kind: HirKind::Class(class), props }
    }

    /// Creates a look-around assertion HIR expression.
    #[inline]
    pub fn look(look: Look) -> Hir {
        let props = Properties::look(look);
        Hir { kind: HirKind::Look(look), props }
    }

    /// Creates a repetition HIR expression.
    #[inline]
    pub fn repetition(mut rep: Repetition) -> Hir {
        // If the sub-expression of a repetition can only match the empty
        // string, then we force its maximum to be at most 1.
        if rep.sub.properties().maximum_len() == Some(0) {
            rep.min = cmp::min(rep.min, 1);
            rep.max = rep.max.map(|n| cmp::min(n, 1)).or(Some(1));
        }
        // The regex 'a{0}' is always equivalent to the empty regex. This is
        // true even when 'a' is an expression that never matches anything
        // (like '\P{any}').
        //
        // Additionally, the regex 'a{1}' is always equivalent to 'a'.
        if rep.min == 0 && rep.max == Some(0) {
            return Hir::empty();
        } else if rep.min == 1 && rep.max == Some(1) {
            return *rep.sub;
        }
        let props = Properties::repetition(&rep);
        Hir { kind: HirKind::Repetition(rep), props }
    }

    /// Creates a capture HIR expression.
    ///
    /// Note that there is no explicit HIR value for a non-capturing group.
    /// Since a non-capturing group only exists to override precedence in the
    /// concrete syntax and since an HIR already does its own grouping based on
    /// what is parsed, there is no need to explicitly represent non-capturing
    /// groups in the HIR.
    #[inline]
    pub fn capture(capture: Capture) -> Hir {
        let props = Properties::capture(&capture);
        Hir { kind: HirKind::Capture(capture), props }
    }

    /// Returns the concatenation of the given expressions.
    ///
    /// This attempts to flatten and simplify the concatenation as appropriate.
    ///
    /// # Example
    ///
    /// This shows a simple example of basic flattening of both concatenations
    /// and literals.
    ///
    /// ```
    /// use regex_syntax::hir::Hir;
    ///
    /// let hir = Hir::concat(vec![
    ///     Hir::concat(vec![
    ///         Hir::literal([b'a']),
    ///         Hir::literal([b'b']),
    ///         Hir::literal([b'c']),
    ///     ]),
    ///     Hir::concat(vec![
    ///         Hir::literal([b'x']),
    ///         Hir::literal([b'y']),
    ///         Hir::literal([b'z']),
    ///     ]),
    /// ]);
    /// let expected = Hir::literal("abcxyz".as_bytes());
    /// assert_eq!(expected, hir);
    /// ```
    pub fn concat(subs: Vec<Hir>) -> Hir {
        // We rebuild the concatenation by simplifying it. Would be nice to do
        // it in place, but that seems a little tricky?
        let mut new = vec![];
        // This gobbles up any adjacent literals in a concatenation and smushes
        // them together. Basically, when we see a literal, we add its bytes
        // to 'prior_lit', and whenever we see anything else, we first take
        // any bytes in 'prior_lit' and add it to the 'new' concatenation.
        let mut prior_lit: Option<Vec<u8>> = None;
        for sub in subs {
            let (kind, props) = sub.into_parts();
            match kind {
                HirKind::Literal(Literal(bytes)) => {
                    if let Some(ref mut prior_bytes) = prior_lit {
                        prior_bytes.extend_from_slice(&bytes);
                    } else {
                        prior_lit = Some(bytes.to_vec());
                    }
                }
                // We also flatten concats that are direct children of another
                // concat. We only need to do this one level deep since
                // Hir::concat is the only way to build concatenations, and so
                // flattening happens inductively.
                HirKind::Concat(subs2) => {
                    for sub2 in subs2 {
                        let (kind2, props2) = sub2.into_parts();
                        match kind2 {
                            HirKind::Literal(Literal(bytes)) => {
                                if let Some(ref mut prior_bytes) = prior_lit {
                                    prior_bytes.extend_from_slice(&bytes);
                                } else {
                                    prior_lit = Some(bytes.to_vec());
                                }
                            }
                            kind2 => {
                                if let Some(prior_bytes) = prior_lit.take() {
                                    new.push(Hir::literal(prior_bytes));
                                }
                                new.push(Hir { kind: kind2, props: props2 });
                            }
                        }
                    }
                }
                // We can just skip empty HIRs.
                HirKind::Empty => {}
                kind => {
                    if let Some(prior_bytes) = prior_lit.take() {
                        new.push(Hir::literal(prior_bytes));
                    }
                    new.push(Hir { kind, props });
                }
            }
        }
        if let Some(prior_bytes) = prior_lit.take() {
            new.push(Hir::literal(prior_bytes));
        }
        if new.is_empty() {
            return Hir::empty();
        } else if new.len() == 1 {
            return new.pop().unwrap();
        }
        let props = Properties::concat(&new);
        Hir { kind: HirKind::Concat(new), props }
    }

    /// Returns the alternation of the given expressions.
    ///
    /// This flattens and simplifies the alternation as appropriate. This may
    /// include factoring out common prefixes or even rewriting the alternation
    /// as a character class.
    ///
    /// Note that an empty alternation is equivalent to `Hir::fail()`. (It
    /// is not possible for one to write an empty alternation, or even an
    /// alternation with a single sub-expression, in the concrete syntax of a
    /// regex.)
    ///
    /// # Example
    ///
    /// This is a simple example showing how an alternation might get
    /// simplified.
    ///
    /// ```
    /// use regex_syntax::hir::{Hir, Class, ClassUnicode, ClassUnicodeRange};
    ///
    /// let hir = Hir::alternation(vec![
    ///     Hir::literal([b'a']),
    ///     Hir::literal([b'b']),
    ///     Hir::literal([b'c']),
    ///     Hir::literal([b'd']),
    ///     Hir::literal([b'e']),
    ///     Hir::literal([b'f']),
    /// ]);
    /// let expected = Hir::class(Class::Unicode(ClassUnicode::new([
    ///     ClassUnicodeRange::new('a', 'f'),
    /// ])));
    /// assert_eq!(expected, hir);
    /// ```
    ///
    /// And another example showing how common prefixes might get factored
    /// out.
    ///
    /// ```
    /// use regex_syntax::hir::{Hir, Class, ClassUnicode, ClassUnicodeRange};
    ///
    /// let hir = Hir::alternation(vec![
    ///     Hir::concat(vec![
    ///         Hir::literal("abc".as_bytes()),
    ///         Hir::class(Class::Unicode(ClassUnicode::new([
    ///             ClassUnicodeRange::new('A', 'Z'),
    ///         ]))),
    ///     ]),
    ///     Hir::concat(vec![
    ///         Hir::literal("abc".as_bytes()),
    ///         Hir::class(Class::Unicode(ClassUnicode::new([
    ///             ClassUnicodeRange::new('a', 'z'),
    ///         ]))),
    ///     ]),
    /// ]);
    /// let expected = Hir::concat(vec![
    ///     Hir::literal("abc".as_bytes()),
    ///     Hir::alternation(vec![
    ///         Hir::class(Class::Unicode(ClassUnicode::new([
    ///             ClassUnicodeRange::new('A', 'Z'),
    ///         ]))),
    ///         Hir::class(Class::Unicode(ClassUnicode::new([
    ///             ClassUnicodeRange::new('a', 'z'),
    ///         ]))),
    ///     ]),
    /// ]);
    /// assert_eq!(expected, hir);
    /// ```
    ///
    /// Note that these sorts of simplifications are not guaranteed.
    pub fn alternation(subs: Vec<Hir>) -> Hir {
        // We rebuild the alternation by simplifying it. We proceed similarly
        // as the concatenation case. But in this case, there's no literal
        // simplification happening. We're just flattening alternations.
        let mut new = Vec::with_capacity(subs.len());
        for sub in subs {
            let (kind, props) = sub.into_parts();
            match kind {
                HirKind::Alternation(subs2) => {
                    new.extend(subs2);
                }
                kind => {
                    new.push(Hir { kind, props });
                }
            }
        }
        if new.is_empty() {
            return Hir::fail();
        } else if new.len() == 1 {
            return new.pop().unwrap();
        }
        // Now that it's completely flattened, look for the special case of
        // 'char1|char2|...|charN' and collapse that into a class. Note that
        // we look for 'char' first and then bytes. The issue here is that if
        // we find both non-ASCII codepoints and non-ASCII singleton bytes,
        // then it isn't actually possible to smush them into a single class.
        // (Because classes are either "all codepoints" or "all bytes." You
        // can have a class that both matches non-ASCII but valid UTF-8 and
        // invalid UTF-8.) So we look for all chars and then all bytes, and
        // don't handle anything else.
        if let Some(singletons) = singleton_chars(&new) {
            let it = singletons
                .into_iter()
                .map(|ch| ClassUnicodeRange { start: ch, end: ch });
            return Hir::class(Class::Unicode(ClassUnicode::new(it)));
        }
        if let Some(singletons) = singleton_bytes(&new) {
            let it = singletons
                .into_iter()
                .map(|b| ClassBytesRange { start: b, end: b });
            return Hir::class(Class::Bytes(ClassBytes::new(it)));
        }
        // Similar to singleton chars, we can also look for alternations of
        // classes. Those can be smushed into a single class.
        if let Some(cls) = class_chars(&new) {
            return Hir::class(cls);
        }
        if let Some(cls) = class_bytes(&new) {
            return Hir::class(cls);
        }
        // Factor out a common prefix if we can, which might potentially
        // simplify the expression and unlock other optimizations downstream.
        // It also might generally make NFA matching and DFA construction
        // faster by reducing the scope of branching in the regex.
        new = match lift_common_prefix(new) {
            Ok(hir) => return hir,
            Err(unchanged) => unchanged,
        };
        let props = Properties::alternation(&new);
        Hir { kind: HirKind::Alternation(new), props }
    }

    /// Returns an HIR expression for `.`.
    ///
    /// * [`Dot::AnyChar`] maps to `(?su-R:.)`.
    /// * [`Dot::AnyByte`] maps to `(?s-Ru:.)`.
    /// * [`Dot::AnyCharExceptLF`] maps to `(?u-Rs:.)`.
    /// * [`Dot::AnyCharExceptCRLF`] maps to `(?Ru-s:.)`.
    /// * [`Dot::AnyByteExceptLF`] maps to `(?-Rsu:.)`.
    /// * [`Dot::AnyByteExceptCRLF`] maps to `(?R-su:.)`.
    ///
    /// # Example
    ///
    /// Note that this is a convenience routine for constructing the correct
    /// character class based on the value of `Dot`. There is no explicit "dot"
    /// HIR value. It is just an abbreviation for a common character class.
    ///
    /// ```
    /// use regex_syntax::hir::{Hir, Dot, Class, ClassBytes, ClassBytesRange};
    ///
    /// let hir = Hir::dot(Dot::AnyByte);
    /// let expected = Hir::class(Class::Bytes(ClassBytes::new([
    ///     ClassBytesRange::new(0x00, 0xFF),
    /// ])));
    /// assert_eq!(expected, hir);
    /// ```
    #[inline]
    pub fn dot(dot: Dot) -> Hir {
        match dot {
            Dot::AnyChar => Hir::class(Class::Unicode(ClassUnicode::new([
                ClassUnicodeRange::new('\0', '\u{10FFFF}'),
            ]))),
            Dot::AnyByte => Hir::class(Class::Bytes(ClassBytes::new([
                ClassBytesRange::new(b'\0', b'\xFF'),
            ]))),
            Dot::AnyCharExcept(ch) => {
                let mut cls =
                    ClassUnicode::new([ClassUnicodeRange::new(ch, ch)]);
                cls.negate();
                Hir::class(Class::Unicode(cls))
            }
            Dot::AnyCharExceptLF => {
                Hir::class(Class::Unicode(ClassUnicode::new([
                    ClassUnicodeRange::new('\0', '\x09'),
                    ClassUnicodeRange::new('\x0B', '\u{10FFFF}'),
                ])))
            }
            Dot::AnyCharExceptCRLF => {
                Hir::class(Class::Unicode(ClassUnicode::new([
                    ClassUnicodeRange::new('\0', '\x09'),
                    ClassUnicodeRange::new('\x0B', '\x0C'),
                    ClassUnicodeRange::new('\x0E', '\u{10FFFF}'),
                ])))
            }
            Dot::AnyByteExcept(byte) => {
                let mut cls =
                    ClassBytes::new([ClassBytesRange::new(byte, byte)]);
                cls.negate();
                Hir::class(Class::Bytes(cls))
            }
            Dot::AnyByteExceptLF => {
                Hir::class(Class::Bytes(ClassBytes::new([
                    ClassBytesRange::new(b'\0', b'\x09'),
                    ClassBytesRange::new(b'\x0B', b'\xFF'),
                ])))
            }
            Dot::AnyByteExceptCRLF => {
                Hir::class(Class::Bytes(ClassBytes::new([
                    ClassBytesRange::new(b'\0', b'\x09'),
                    ClassBytesRange::new(b'\x0B', b'\x0C'),
                    ClassBytesRange::new(b'\x0E', b'\xFF'),
                ])))
            }
        }
    }
}

/// The underlying kind of an arbitrary [`Hir`] expression.
///
/// An `HirKind` is principally useful for doing case analysis on the type
/// of a regular expression. If you're looking to build new `Hir` values,
/// then you _must_ use the smart constructors defined on `Hir`, like
/// [`Hir::repetition`], to build new `Hir` values. The API intentionally does
/// not expose any way of building an `Hir` directly from an `HirKind`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirKind {
    /// The empty regular expression, which matches everything, including the
    /// empty string.
    Empty,
    /// A literalstring that matches exactly these bytes.
    Literal(Literal),
    /// A single character class that matches any of the characters in the
    /// class. A class can either consist of Unicode scalar values as
    /// characters, or it can use bytes.
    ///
    /// A class may be empty. In which case, it matches nothing.
    Class(Class),
    /// A look-around assertion. A look-around match always has zero length.
    Look(Look),
    /// A repetition operation applied to a sub-expression.
    Repetition(Repetition),
    /// A capturing group, which contains a sub-expression.
    Capture(Capture),
    /// A concatenation of expressions.
    ///
    /// A concatenation matches only if each of its sub-expressions match one
    /// after the other.
    ///
    /// Concatenations are guaranteed by `Hir`'s smart constructors to always
    /// have at least two sub-expressions.
    Concat(Vec<Hir>),
    /// An alternation of expressions.
    ///
    /// An alternation matches only if at least one of its sub-expressions
    /// match. If multiple sub-expressions match, then the leftmost is
    /// preferred.
    ///
    /// Alternations are guaranteed by `Hir`'s smart constructors to always
    /// have at least two sub-expressions.
    Alternation(Vec<Hir>),
}

impl HirKind {
    /// Returns a slice of this kind's sub-expressions, if any.
    pub fn subs(&self) -> &[Hir] {
        use core::slice::from_ref;

        match *self {
            HirKind::Empty
            | HirKind::Literal(_)
            | HirKind::Class(_)
            | HirKind::Look(_) => &[],
            HirKind::Repetition(Repetition { ref sub, .. }) => from_ref(sub),
            HirKind::Capture(Capture { ref sub, .. }) => from_ref(sub),
            HirKind::Concat(ref subs) => subs,
            HirKind::Alternation(ref subs) => subs,
        }
    }
}

impl core::fmt::Debug for Hir {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.kind.fmt(f)
    }
}

/// Print a display representation of this Hir.
///
/// The result of this is a valid regular expression pattern string.
///
/// This implementation uses constant stack space and heap space proportional
/// to the size of the `Hir`.
impl core::fmt::Display for Hir {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        crate::hir::print::Printer::new().print(self, f)
    }
}

/// The high-level intermediate representation of a literal.
///
/// A literal corresponds to `0` or more bytes that should be matched
/// literally. The smart constructors defined on `Hir` will automatically
/// concatenate adjacent literals into one literal, and will even automatically
/// replace empty literals with `Hir::empty()`.
///
/// Note that despite a literal being represented by a sequence of bytes, its
/// `Debug` implementation will attempt to print it as a normal string. (That
/// is, not a sequence of decimal numbers.)
#[derive(Clone, Eq, PartialEq)]
pub struct Literal(pub Box<[u8]>);

impl core::fmt::Debug for Literal {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        crate::debug::Bytes(&self.0).fmt(f)
    }
}

/// The high-level intermediate representation of a character class.
///
/// A character class corresponds to a set of characters. A character is either
/// defined by a Unicode scalar value or a byte.
///
/// A character class, regardless of its character type, is represented by a
/// sequence of non-overlapping non-adjacent ranges of characters.
///
/// There are no guarantees about which class variant is used. Generally
/// speaking, the Unicode variat is used whenever a class needs to contain
/// non-ASCII Unicode scalar values. But the Unicode variant can be used even
/// when Unicode mode is disabled. For example, at the time of writing, the
/// regex `(?-u:a|\xc2\xa0)` will compile down to HIR for the Unicode class
/// `[a\u00A0]` due to optimizations.
///
/// Note that `Bytes` variant may be produced even when it exclusively matches
/// valid UTF-8. This is because a `Bytes` variant represents an intention by
/// the author of the regular expression to disable Unicode mode, which in turn
/// impacts the semantics of case insensitive matching. For example, `(?i)k`
/// and `(?i-u)k` will not match the same set of strings.
#[derive(Clone, Eq, PartialEq)]
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
    ///
    /// # Panics
    ///
    /// This routine panics when the case mapping data necessary for this
    /// routine to complete is unavailable. This occurs when the `unicode-case`
    /// feature is not enabled and the underlying class is Unicode oriented.
    ///
    /// Callers should prefer using `try_case_fold_simple` instead, which will
    /// return an error instead of panicking.
    pub fn case_fold_simple(&mut self) {
        match *self {
            Class::Unicode(ref mut x) => x.case_fold_simple(),
            Class::Bytes(ref mut x) => x.case_fold_simple(),
        }
    }

    /// Apply Unicode simple case folding to this character class, in place.
    /// The character class will be expanded to include all simple case folded
    /// character variants.
    ///
    /// If this is a byte oriented character class, then this will be limited
    /// to the ASCII ranges `A-Z` and `a-z`.
    ///
    /// # Error
    ///
    /// This routine returns an error when the case mapping data necessary
    /// for this routine to complete is unavailable. This occurs when the
    /// `unicode-case` feature is not enabled and the underlying class is
    /// Unicode oriented.
    pub fn try_case_fold_simple(
        &mut self,
    ) -> core::result::Result<(), CaseFoldError> {
        match *self {
            Class::Unicode(ref mut x) => x.try_case_fold_simple()?,
            Class::Bytes(ref mut x) => x.case_fold_simple(),
        }
        Ok(())
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
    pub fn is_utf8(&self) -> bool {
        match *self {
            Class::Unicode(_) => true,
            Class::Bytes(ref x) => x.is_ascii(),
        }
    }

    /// Returns the length, in bytes, of the smallest string matched by this
    /// character class.
    ///
    /// For non-empty byte oriented classes, this always returns `1`. For
    /// non-empty Unicode oriented classes, this can return `1`, `2`, `3` or
    /// `4`. For empty classes, `None` is returned. It is impossible for `0` to
    /// be returned.
    ///
    /// # Example
    ///
    /// This example shows some examples of regexes and their corresponding
    /// minimum length, if any.
    ///
    /// ```
    /// use regex_syntax::{hir::Properties, parse};
    ///
    /// // The empty string has a min length of 0.
    /// let hir = parse(r"")?;
    /// assert_eq!(Some(0), hir.properties().minimum_len());
    /// // As do other types of regexes that only match the empty string.
    /// let hir = parse(r"^$\b\B")?;
    /// assert_eq!(Some(0), hir.properties().minimum_len());
    /// // A regex that can match the empty string but match more is still 0.
    /// let hir = parse(r"a*")?;
    /// assert_eq!(Some(0), hir.properties().minimum_len());
    /// // A regex that matches nothing has no minimum defined.
    /// let hir = parse(r"[a&&b]")?;
    /// assert_eq!(None, hir.properties().minimum_len());
    /// // Character classes usually have a minimum length of 1.
    /// let hir = parse(r"\w")?;
    /// assert_eq!(Some(1), hir.properties().minimum_len());
    /// // But sometimes Unicode classes might be bigger!
    /// let hir = parse(r"\p{Cyrillic}")?;
    /// assert_eq!(Some(2), hir.properties().minimum_len());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn minimum_len(&self) -> Option<usize> {
        match *self {
            Class::Unicode(ref x) => x.minimum_len(),
            Class::Bytes(ref x) => x.minimum_len(),
        }
    }

    /// Returns the length, in bytes, of the longest string matched by this
    /// character class.
    ///
    /// For non-empty byte oriented classes, this always returns `1`. For
    /// non-empty Unicode oriented classes, this can return `1`, `2`, `3` or
    /// `4`. For empty classes, `None` is returned. It is impossible for `0` to
    /// be returned.
    ///
    /// # Example
    ///
    /// This example shows some examples of regexes and their corresponding
    /// maximum length, if any.
    ///
    /// ```
    /// use regex_syntax::{hir::Properties, parse};
    ///
    /// // The empty string has a max length of 0.
    /// let hir = parse(r"")?;
    /// assert_eq!(Some(0), hir.properties().maximum_len());
    /// // As do other types of regexes that only match the empty string.
    /// let hir = parse(r"^$\b\B")?;
    /// assert_eq!(Some(0), hir.properties().maximum_len());
    /// // A regex that matches nothing has no maximum defined.
    /// let hir = parse(r"[a&&b]")?;
    /// assert_eq!(None, hir.properties().maximum_len());
    /// // Bounded repeats work as you expect.
    /// let hir = parse(r"x{2,10}")?;
    /// assert_eq!(Some(10), hir.properties().maximum_len());
    /// // An unbounded repeat means there is no maximum.
    /// let hir = parse(r"x{2,}")?;
    /// assert_eq!(None, hir.properties().maximum_len());
    /// // With Unicode enabled, \w can match up to 4 bytes!
    /// let hir = parse(r"\w")?;
    /// assert_eq!(Some(4), hir.properties().maximum_len());
    /// // Without Unicode enabled, \w matches at most 1 byte.
    /// let hir = parse(r"(?-u)\w")?;
    /// assert_eq!(Some(1), hir.properties().maximum_len());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn maximum_len(&self) -> Option<usize> {
        match *self {
            Class::Unicode(ref x) => x.maximum_len(),
            Class::Bytes(ref x) => x.maximum_len(),
        }
    }

    /// Returns true if and only if this character class is empty. That is,
    /// it has no elements.
    ///
    /// An empty character can never match anything, including an empty string.
    pub fn is_empty(&self) -> bool {
        match *self {
            Class::Unicode(ref x) => x.ranges().is_empty(),
            Class::Bytes(ref x) => x.ranges().is_empty(),
        }
    }

    /// If this class consists of exactly one element (whether a codepoint or a
    /// byte), then return it as a literal byte string.
    ///
    /// If this class is empty or contains more than one element, then `None`
    /// is returned.
    pub fn literal(&self) -> Option<Vec<u8>> {
        match *self {
            Class::Unicode(ref x) => x.literal(),
            Class::Bytes(ref x) => x.literal(),
        }
    }
}

impl core::fmt::Debug for Class {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use crate::debug::Byte;

        let mut fmter = f.debug_set();
        match *self {
            Class::Unicode(ref cls) => {
                for r in cls.ranges().iter() {
                    fmter.entry(&(r.start..=r.end));
                }
            }
            Class::Bytes(ref cls) => {
                for r in cls.ranges().iter() {
                    fmter.entry(&(Byte(r.start)..=Byte(r.end)));
                }
            }
        }
        fmter.finish()
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
    /// may overlap. Ranges will automatically be sorted into a canonical
    /// non-overlapping order.
    pub fn new<I>(ranges: I) -> ClassUnicode
    where
        I: IntoIterator<Item = ClassUnicodeRange>,
    {
        ClassUnicode { set: IntervalSet::new(ranges) }
    }

    /// Create a new class with no ranges.
    ///
    /// An empty class matches nothing. That is, it is equivalent to
    /// [`Hir::fail`].
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
    ) -> core::result::Result<(), CaseFoldError> {
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
    pub fn is_ascii(&self) -> bool {
        self.set.intervals().last().map_or(true, |r| r.end <= '\x7F')
    }

    /// Returns the length, in bytes, of the smallest string matched by this
    /// character class.
    ///
    /// Returns `None` when the class is empty.
    pub fn minimum_len(&self) -> Option<usize> {
        let first = self.ranges().get(0)?;
        // Correct because c1 < c2 implies c1.len_utf8() < c2.len_utf8().
        Some(first.start.len_utf8())
    }

    /// Returns the length, in bytes, of the longest string matched by this
    /// character class.
    ///
    /// Returns `None` when the class is empty.
    pub fn maximum_len(&self) -> Option<usize> {
        let last = self.ranges().last()?;
        // Correct because c1 < c2 implies c1.len_utf8() < c2.len_utf8().
        Some(last.end.len_utf8())
    }

    /// If this class consists of exactly one codepoint, then return it as
    /// a literal byte string.
    ///
    /// If this class is empty or contains more than one codepoint, then `None`
    /// is returned.
    pub fn literal(&self) -> Option<Vec<u8>> {
        let rs = self.ranges();
        if rs.len() == 1 && rs[0].start == rs[0].end {
            Some(rs[0].start.encode_utf8(&mut [0; 4]).to_string().into_bytes())
        } else {
            None
        }
    }

    /// If this class consists of only ASCII ranges, then return its
    /// corresponding and equivalent byte class.
    pub fn to_byte_class(&self) -> Option<ClassBytes> {
        if !self.is_ascii() {
            return None;
        }
        Some(ClassBytes::new(self.ranges().iter().map(|r| {
            // Since we are guaranteed that our codepoint range is ASCII, the
            // 'u8::try_from' calls below are guaranteed to be correct.
            ClassBytesRange {
                start: u8::try_from(r.start).unwrap(),
                end: u8::try_from(r.end).unwrap(),
            }
        })))
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

impl core::fmt::Debug for ClassUnicodeRange {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let start = if !self.start.is_whitespace() && !self.start.is_control()
        {
            self.start.to_string()
        } else {
            format!("0x{:X}", u32::from(self.start))
        };
        let end = if !self.end.is_whitespace() && !self.end.is_control() {
            self.end.to_string()
        } else {
            format!("0x{:X}", u32::from(self.end))
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
        let mut folder = unicode::SimpleCaseFolder::new()?;
        if !folder.overlaps(self.start, self.end) {
            return Ok(());
        }
        let (start, end) = (u32::from(self.start), u32::from(self.end));
        for cp in (start..=end).filter_map(char::from_u32) {
            for &cp_folded in folder.mapping(cp) {
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

    /// Returns the number of codepoints in this range.
    pub fn len(&self) -> usize {
        let diff = 1 + u32::from(self.end) - u32::from(self.start);
        // This is likely to panic in 16-bit targets since a usize can only fit
        // 2^16. It's not clear what to do here, other than to return an error
        // when building a Unicode class that contains a range whose length
        // overflows usize. (Which, to be honest, is probably quite common on
        // 16-bit targets. For example, this would imply that '.' and '\p{any}'
        // would be impossible to build.)
        usize::try_from(diff).expect("char class len fits in usize")
    }
}

/// A set of characters represented by arbitrary bytes.
///
/// Each byte corresponds to one character.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassBytes {
    set: IntervalSet<ClassBytesRange>,
}

impl ClassBytes {
    /// Create a new class from a sequence of ranges.
    ///
    /// The given ranges do not need to be in any specific order, and ranges
    /// may overlap. Ranges will automatically be sorted into a canonical
    /// non-overlapping order.
    pub fn new<I>(ranges: I) -> ClassBytes
    where
        I: IntoIterator<Item = ClassBytesRange>,
    {
        ClassBytes { set: IntervalSet::new(ranges) }
    }

    /// Create a new class with no ranges.
    ///
    /// An empty class matches nothing. That is, it is equivalent to
    /// [`Hir::fail`].
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
    pub fn is_ascii(&self) -> bool {
        self.set.intervals().last().map_or(true, |r| r.end <= 0x7F)
    }

    /// Returns the length, in bytes, of the smallest string matched by this
    /// character class.
    ///
    /// Returns `None` when the class is empty.
    pub fn minimum_len(&self) -> Option<usize> {
        if self.ranges().is_empty() {
            None
        } else {
            Some(1)
        }
    }

    /// Returns the length, in bytes, of the longest string matched by this
    /// character class.
    ///
    /// Returns `None` when the class is empty.
    pub fn maximum_len(&self) -> Option<usize> {
        if self.ranges().is_empty() {
            None
        } else {
            Some(1)
        }
    }

    /// If this class consists of exactly one byte, then return it as
    /// a literal byte string.
    ///
    /// If this class is empty or contains more than one byte, then `None`
    /// is returned.
    pub fn literal(&self) -> Option<Vec<u8>> {
        let rs = self.ranges();
        if rs.len() == 1 && rs[0].start == rs[0].end {
            Some(vec![rs[0].start])
        } else {
            None
        }
    }

    /// If this class consists of only ASCII ranges, then return its
    /// corresponding and equivalent Unicode class.
    pub fn to_unicode_class(&self) -> Option<ClassUnicode> {
        if !self.is_ascii() {
            return None;
        }
        Some(ClassUnicode::new(self.ranges().iter().map(|r| {
            // Since we are guaranteed that our byte range is ASCII, the
            // 'char::from' calls below are correct and will not erroneously
            // convert a raw byte value into its corresponding codepoint.
            ClassUnicodeRange {
                start: char::from(r.start),
                end: char::from(r.end),
            }
        })))
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

    /// Returns the number of bytes in this range.
    pub fn len(&self) -> usize {
        usize::from(self.end.checked_sub(self.start).unwrap())
            .checked_add(1)
            .unwrap()
    }
}

impl core::fmt::Debug for ClassBytesRange {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ClassBytesRange")
            .field("start", &crate::debug::Byte(self.start))
            .field("end", &crate::debug::Byte(self.end))
            .finish()
    }
}

/// The high-level intermediate representation for a look-around assertion.
///
/// An assertion match is always zero-length. Also called an "empty match."
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
pub struct Capture {
    /// The capture index of the capture.
    pub index: u32,
    /// The name of the capture, if it exists.
    pub name: Option<Box<str>>,
    /// The expression inside the capturing group, which may be empty.
    pub sub: Box<Hir>,
}

/// The high-level intermediate representation of a repetition operator.
///
/// A repetition operator permits the repetition of an arbitrary
/// sub-expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Repetition {
    /// The minimum range of the repetition.
    ///
    /// Note that special cases like `?`, `+` and `*` all get translated into
    /// the ranges `{0,1}`, `{1,}` and `{0,}`, respectively.
    ///
    /// When `min` is zero, this expression can match the empty string
    /// regardless of what its sub-expression is.
    pub min: u32,
    /// The maximum range of the repetition.
    ///
    /// Note that when `max` is `None`, `min` acts as a lower bound but where
    /// there is no upper bound. For something like `x{5}` where the min and
    /// max are equivalent, `min` will be set to `5` and `max` will be set to
    /// `Some(5)`.
    pub max: Option<u32>,
    /// Whether this repetition operator is greedy or not. A greedy operator
    /// will match as much as it can. A non-greedy operator will match as
    /// little as it can.
    ///
    /// Typically, operators are greedy by default and are only non-greedy when
    /// a `?` suffix is used, e.g., `(expr)*` is greedy while `(expr)*?` is
    /// not. However, this can be inverted via the `U` "ungreedy" flag.
    pub greedy: bool,
    /// The expression being repeated.
    pub sub: Box<Hir>,
}

impl Repetition {
    /// Returns a new repetition with the same `min`, `max` and `greedy`
    /// values, but with its sub-expression replaced with the one given.
    pub fn with(&self, sub: Hir) -> Repetition {
        Repetition {
            min: self.min,
            max: self.max,
            greedy: self.greedy,
            sub: Box::new(sub),
        }
    }
}

/// A type describing the different flavors of `.`.
///
/// This type is meant to be used with [`Hir::dot`], which is a convenience
/// routine for building HIR values derived from the `.` regex.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dot {
    /// Matches the UTF-8 encoding of any Unicode scalar value.
    ///
    /// This is equivalent to `(?su:.)` and also `\p{any}`.
    AnyChar,
    /// Matches any byte value.
    ///
    /// This is equivalent to `(?s-u:.)` and also `(?-u:[\x00-\xFF])`.
    AnyByte,
    /// Matches the UTF-8 encoding of any Unicode scalar value except for the
    /// `char` given.
    ///
    /// This is equivalent to using `(?u-s:.)` with the line terminator set
    /// to a particular ASCII byte. (Because of peculiarities in the regex
    /// engines, a line terminator must be a single byte. It follows that when
    /// UTF-8 mode is enabled, this single byte must also be a Unicode scalar
    /// value. That is, ti must be ASCII.)
    ///
    /// (This and `AnyCharExceptLF` both exist because of legacy reasons.
    /// `AnyCharExceptLF` will be dropped in the next breaking change release.)
    AnyCharExcept(char),
    /// Matches the UTF-8 encoding of any Unicode scalar value except for `\n`.
    ///
    /// This is equivalent to `(?u-s:.)` and also `[\p{any}--\n]`.
    AnyCharExceptLF,
    /// Matches the UTF-8 encoding of any Unicode scalar value except for `\r`
    /// and `\n`.
    ///
    /// This is equivalent to `(?uR-s:.)` and also `[\p{any}--\r\n]`.
    AnyCharExceptCRLF,
    /// Matches any byte value except for the `u8` given.
    ///
    /// This is equivalent to using `(?-us:.)` with the line terminator set
    /// to a particular ASCII byte. (Because of peculiarities in the regex
    /// engines, a line terminator must be a single byte. It follows that when
    /// UTF-8 mode is enabled, this single byte must also be a Unicode scalar
    /// value. That is, ti must be ASCII.)
    ///
    /// (This and `AnyByteExceptLF` both exist because of legacy reasons.
    /// `AnyByteExceptLF` will be dropped in the next breaking change release.)
    AnyByteExcept(u8),
    /// Matches any byte value except for `\n`.
    ///
    /// This is equivalent to `(?-su:.)` and also `(?-u:[[\x00-\xFF]--\n])`.
    AnyByteExceptLF,
    /// Matches any byte value except for `\r` and `\n`.
    ///
    /// This is equivalent to `(?R-su:.)` and also `(?-u:[[\x00-\xFF]--\r\n])`.
    AnyByteExceptCRLF,
}

/// A custom `Drop` impl is used for `HirKind` such that it uses constant stack
/// space but heap space proportional to the depth of the total `Hir`.
impl Drop for Hir {
    fn drop(&mut self) {
        use core::mem;

        match *self.kind() {
            HirKind::Empty
            | HirKind::Literal(_)
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
                | HirKind::Literal(_)
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

/// A type that collects various properties of an HIR value.
///
/// Properties are always scalar values and represent meta data that is
/// computed inductively on an HIR value. Properties are defined for all
/// HIR values.
///
/// All methods on a `Properties` value take constant time and are meant to
/// be cheap to call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Properties(Box<PropertiesI>);

/// The property definition. It is split out so that we can box it, and
/// there by make `Properties` use less stack size. This is kind-of important
/// because every HIR value has a `Properties` attached to it.
///
/// This does have the unfortunate consequence that creating any HIR value
/// always leads to at least one alloc for properties, but this is generally
/// true anyway (for pretty much all HirKinds except for look-arounds).
#[derive(Clone, Debug, Eq, PartialEq)]
struct PropertiesI {
    minimum_len: Option<usize>,
    maximum_len: Option<usize>,
    look_set: LookSet,
    look_set_prefix: LookSet,
    look_set_suffix: LookSet,
    look_set_prefix_any: LookSet,
    look_set_suffix_any: LookSet,
    utf8: bool,
    explicit_captures_len: usize,
    static_explicit_captures_len: Option<usize>,
    literal: bool,
    alternation_literal: bool,
}

impl Properties {
    /// Returns the length (in bytes) of the smallest string matched by this
    /// HIR.
    ///
    /// A return value of `0` is possible and occurs when the HIR can match an
    /// empty string.
    ///
    /// `None` is returned when there is no minimum length. This occurs in
    /// precisely the cases where the HIR matches nothing. i.e., The language
    /// the regex matches is empty. An example of such a regex is `\P{any}`.
    #[inline]
    pub fn minimum_len(&self) -> Option<usize> {
        self.0.minimum_len
    }

    /// Returns the length (in bytes) of the longest string matched by this
    /// HIR.
    ///
    /// A return value of `0` is possible and occurs when nothing longer than
    /// the empty string is in the language described by this HIR.
    ///
    /// `None` is returned when there is no longest matching string. This
    /// occurs when the HIR matches nothing or when there is no upper bound on
    /// the length of matching strings. Example of such regexes are `\P{any}`
    /// (matches nothing) and `a+` (has no upper bound).
    #[inline]
    pub fn maximum_len(&self) -> Option<usize> {
        self.0.maximum_len
    }

    /// Returns a set of all look-around assertions that appear at least once
    /// in this HIR value.
    #[inline]
    pub fn look_set(&self) -> LookSet {
        self.0.look_set
    }

    /// Returns a set of all look-around assertions that appear as a prefix for
    /// this HIR value. That is, the set returned corresponds to the set of
    /// assertions that must be passed before matching any bytes in a haystack.
    ///
    /// For example, `hir.look_set_prefix().contains(Look::Start)` returns true
    /// if and only if the HIR is fully anchored at the start.
    #[inline]
    pub fn look_set_prefix(&self) -> LookSet {
        self.0.look_set_prefix
    }

    /// Returns a set of all look-around assertions that appear as a _possible_
    /// prefix for this HIR value. That is, the set returned corresponds to the
    /// set of assertions that _may_ be passed before matching any bytes in a
    /// haystack.
    ///
    /// For example, `hir.look_set_prefix_any().contains(Look::Start)` returns
    /// true if and only if it's possible for the regex to match through a
    /// anchored assertion before consuming any input.
    #[inline]
    pub fn look_set_prefix_any(&self) -> LookSet {
        self.0.look_set_prefix_any
    }

    /// Returns a set of all look-around assertions that appear as a suffix for
    /// this HIR value. That is, the set returned corresponds to the set of
    /// assertions that must be passed in order to be considered a match after
    /// all other consuming HIR expressions.
    ///
    /// For example, `hir.look_set_suffix().contains(Look::End)` returns true
    /// if and only if the HIR is fully anchored at the end.
    #[inline]
    pub fn look_set_suffix(&self) -> LookSet {
        self.0.look_set_suffix
    }

    /// Returns a set of all look-around assertions that appear as a _possible_
    /// suffix for this HIR value. That is, the set returned corresponds to the
    /// set of assertions that _may_ be passed before matching any bytes in a
    /// haystack.
    ///
    /// For example, `hir.look_set_suffix_any().contains(Look::End)` returns
    /// true if and only if it's possible for the regex to match through a
    /// anchored assertion at the end of a match without consuming any input.
    #[inline]
    pub fn look_set_suffix_any(&self) -> LookSet {
        self.0.look_set_suffix_any
    }

    /// Return true if and only if the corresponding HIR will always match
    /// valid UTF-8.
    ///
    /// When this returns false, then it is possible for this HIR expression to
    /// match invalid UTF-8, including by matching between the code units of
    /// a single UTF-8 encoded codepoint.
    ///
    /// Note that this returns true even when the corresponding HIR can match
    /// the empty string. Since an empty string can technically appear between
    /// UTF-8 code units, it is possible for a match to be reported that splits
    /// a codepoint which could in turn be considered matching invalid UTF-8.
    /// However, it is generally assumed that such empty matches are handled
    /// specially by the search routine if it is absolutely required that
    /// matches not split a codepoint.
    ///
    /// # Example
    ///
    /// This code example shows the UTF-8 property of a variety of patterns.
    ///
    /// ```
    /// use regex_syntax::{ParserBuilder, parse};
    ///
    /// // Examples of 'is_utf8() == true'.
    /// assert!(parse(r"a")?.properties().is_utf8());
    /// assert!(parse(r"[^a]")?.properties().is_utf8());
    /// assert!(parse(r".")?.properties().is_utf8());
    /// assert!(parse(r"\W")?.properties().is_utf8());
    /// assert!(parse(r"\b")?.properties().is_utf8());
    /// assert!(parse(r"\B")?.properties().is_utf8());
    /// assert!(parse(r"(?-u)\b")?.properties().is_utf8());
    /// assert!(parse(r"(?-u)\B")?.properties().is_utf8());
    /// // Unicode mode is enabled by default, and in
    /// // that mode, all \x hex escapes are treated as
    /// // codepoints. So this actually matches the UTF-8
    /// // encoding of U+00FF.
    /// assert!(parse(r"\xFF")?.properties().is_utf8());
    ///
    /// // Now we show examples of 'is_utf8() == false'.
    /// // The only way to do this is to force the parser
    /// // to permit invalid UTF-8, otherwise all of these
    /// // would fail to parse!
    /// let parse = |pattern| {
    ///     ParserBuilder::new().utf8(false).build().parse(pattern)
    /// };
    /// assert!(!parse(r"(?-u)[^a]")?.properties().is_utf8());
    /// assert!(!parse(r"(?-u).")?.properties().is_utf8());
    /// assert!(!parse(r"(?-u)\W")?.properties().is_utf8());
    /// // Conversely to the equivalent example above,
    /// // when Unicode mode is disabled, \x hex escapes
    /// // are treated as their raw byte values.
    /// assert!(!parse(r"(?-u)\xFF")?.properties().is_utf8());
    /// // Note that just because we disabled UTF-8 in the
    /// // parser doesn't mean we still can't use Unicode.
    /// // It is enabled by default, so \xFF is still
    /// // equivalent to matching the UTF-8 encoding of
    /// // U+00FF by default.
    /// assert!(parse(r"\xFF")?.properties().is_utf8());
    /// // Even though we use raw bytes that individually
    /// // are not valid UTF-8, when combined together, the
    /// // overall expression *does* match valid UTF-8!
    /// assert!(parse(r"(?-u)\xE2\x98\x83")?.properties().is_utf8());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn is_utf8(&self) -> bool {
        self.0.utf8
    }

    /// Returns the total number of explicit capturing groups in the
    /// corresponding HIR.
    ///
    /// Note that this does not include the implicit capturing group
    /// corresponding to the entire match that is typically included by regex
    /// engines.
    ///
    /// # Example
    ///
    /// This method will return `0` for `a` and `1` for `(a)`:
    ///
    /// ```
    /// use regex_syntax::parse;
    ///
    /// assert_eq!(0, parse("a")?.properties().explicit_captures_len());
    /// assert_eq!(1, parse("(a)")?.properties().explicit_captures_len());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn explicit_captures_len(&self) -> usize {
        self.0.explicit_captures_len
    }

    /// Returns the total number of explicit capturing groups that appear in
    /// every possible match.
    ///
    /// If the number of capture groups can vary depending on the match, then
    /// this returns `None`. That is, a value is only returned when the number
    /// of matching groups is invariant or "static."
    ///
    /// Note that this does not include the implicit capturing group
    /// corresponding to the entire match.
    ///
    /// # Example
    ///
    /// This shows a few cases where a static number of capture groups is
    /// available and a few cases where it is not.
    ///
    /// ```
    /// use regex_syntax::parse;
    ///
    /// let len = |pattern| {
    ///     parse(pattern).map(|h| {
    ///         h.properties().static_explicit_captures_len()
    ///     })
    /// };
    ///
    /// assert_eq!(Some(0), len("a")?);
    /// assert_eq!(Some(1), len("(a)")?);
    /// assert_eq!(Some(1), len("(a)|(b)")?);
    /// assert_eq!(Some(2), len("(a)(b)|(c)(d)")?);
    /// assert_eq!(None, len("(a)|b")?);
    /// assert_eq!(None, len("a|(b)")?);
    /// assert_eq!(None, len("(b)*")?);
    /// assert_eq!(Some(1), len("(b)+")?);
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub fn static_explicit_captures_len(&self) -> Option<usize> {
        self.0.static_explicit_captures_len
    }

    /// Return true if and only if this HIR is a simple literal. This is
    /// only true when this HIR expression is either itself a `Literal` or a
    /// concatenation of only `Literal`s.
    ///
    /// For example, `f` and `foo` are literals, but `f+`, `(foo)`, `foo()` and
    /// the empty string are not (even though they contain sub-expressions that
    /// are literals).
    #[inline]
    pub fn is_literal(&self) -> bool {
        self.0.literal
    }

    /// Return true if and only if this HIR is either a simple literal or an
    /// alternation of simple literals. This is only
    /// true when this HIR expression is either itself a `Literal` or a
    /// concatenation of only `Literal`s or an alternation of only `Literal`s.
    ///
    /// For example, `f`, `foo`, `a|b|c`, and `foo|bar|baz` are alternation
    /// literals, but `f+`, `(foo)`, `foo()`, and the empty pattern are not
    /// (even though that contain sub-expressions that are literals).
    #[inline]
    pub fn is_alternation_literal(&self) -> bool {
        self.0.alternation_literal
    }

    /// Returns the total amount of heap memory usage, in bytes, used by this
    /// `Properties` value.
    #[inline]
    pub fn memory_usage(&self) -> usize {
        core::mem::size_of::<PropertiesI>()
    }

    /// Returns a new set of properties that corresponds to the union of the
    /// iterator of properties given.
    ///
    /// This is useful when one has multiple `Hir` expressions and wants
    /// to combine them into a single alternation without constructing the
    /// corresponding `Hir`. This routine provides a way of combining the
    /// properties of each `Hir` expression into one set of properties
    /// representing the union of those expressions.
    ///
    /// # Example: union with HIRs that never match
    ///
    /// This example shows that unioning properties together with one that
    /// represents a regex that never matches will "poison" certain attributes,
    /// like the minimum and maximum lengths.
    ///
    /// ```
    /// use regex_syntax::{hir::Properties, parse};
    ///
    /// let hir1 = parse("ab?c?")?;
    /// assert_eq!(Some(1), hir1.properties().minimum_len());
    /// assert_eq!(Some(3), hir1.properties().maximum_len());
    ///
    /// let hir2 = parse(r"[a&&b]")?;
    /// assert_eq!(None, hir2.properties().minimum_len());
    /// assert_eq!(None, hir2.properties().maximum_len());
    ///
    /// let hir3 = parse(r"wxy?z?")?;
    /// assert_eq!(Some(2), hir3.properties().minimum_len());
    /// assert_eq!(Some(4), hir3.properties().maximum_len());
    ///
    /// let unioned = Properties::union([
    ///		hir1.properties(),
    ///		hir2.properties(),
    ///		hir3.properties(),
    ///	]);
    /// assert_eq!(None, unioned.minimum_len());
    /// assert_eq!(None, unioned.maximum_len());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// The maximum length can also be "poisoned" by a pattern that has no
    /// upper bound on the length of a match. The minimum length remains
    /// unaffected:
    ///
    /// ```
    /// use regex_syntax::{hir::Properties, parse};
    ///
    /// let hir1 = parse("ab?c?")?;
    /// assert_eq!(Some(1), hir1.properties().minimum_len());
    /// assert_eq!(Some(3), hir1.properties().maximum_len());
    ///
    /// let hir2 = parse(r"a+")?;
    /// assert_eq!(Some(1), hir2.properties().minimum_len());
    /// assert_eq!(None, hir2.properties().maximum_len());
    ///
    /// let hir3 = parse(r"wxy?z?")?;
    /// assert_eq!(Some(2), hir3.properties().minimum_len());
    /// assert_eq!(Some(4), hir3.properties().maximum_len());
    ///
    /// let unioned = Properties::union([
    ///		hir1.properties(),
    ///		hir2.properties(),
    ///		hir3.properties(),
    ///	]);
    /// assert_eq!(Some(1), unioned.minimum_len());
    /// assert_eq!(None, unioned.maximum_len());
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn union<I, P>(props: I) -> Properties
    where
        I: IntoIterator<Item = P>,
        P: core::borrow::Borrow<Properties>,
    {
        let mut it = props.into_iter().peekable();
        // While empty alternations aren't possible, we still behave as if they
        // are. When we have an empty alternate, then clearly the look-around
        // prefix and suffix is empty. Otherwise, it is the intersection of all
        // prefixes and suffixes (respectively) of the branches.
        let fix = if it.peek().is_none() {
            LookSet::empty()
        } else {
            LookSet::full()
        };
        // And also, an empty alternate means we have 0 static capture groups,
        // but we otherwise start with the number corresponding to the first
        // alternate. If any subsequent alternate has a different number of
        // static capture groups, then we overall have a variation and not a
        // static number of groups.
        let static_explicit_captures_len =
            it.peek().and_then(|p| p.borrow().static_explicit_captures_len());
        // The base case is an empty alternation, which matches nothing.
        // Note though that empty alternations aren't possible, because the
        // Hir::alternation smart constructor rewrites those as empty character
        // classes.
        let mut props = PropertiesI {
            minimum_len: None,
            maximum_len: None,
            look_set: LookSet::empty(),
            look_set_prefix: fix,
            look_set_suffix: fix,
            look_set_prefix_any: LookSet::empty(),
            look_set_suffix_any: LookSet::empty(),
            utf8: true,
            explicit_captures_len: 0,
            static_explicit_captures_len,
            literal: false,
            alternation_literal: true,
        };
        let (mut min_poisoned, mut max_poisoned) = (false, false);
        // Handle properties that need to visit every child hir.
        for prop in it {
            let p = prop.borrow();
            props.look_set.set_union(p.look_set());
            props.look_set_prefix.set_intersect(p.look_set_prefix());
            props.look_set_suffix.set_intersect(p.look_set_suffix());
            props.look_set_prefix_any.set_union(p.look_set_prefix_any());
            props.look_set_suffix_any.set_union(p.look_set_suffix_any());
            props.utf8 = props.utf8 && p.is_utf8();
            props.explicit_captures_len = props
                .explicit_captures_len
                .saturating_add(p.explicit_captures_len());
            if props.static_explicit_captures_len
                != p.static_explicit_captures_len()
            {
                props.static_explicit_captures_len = None;
            }
            props.alternation_literal =
                props.alternation_literal && p.is_literal();
            if !min_poisoned {
                if let Some(xmin) = p.minimum_len() {
                    if props.minimum_len.map_or(true, |pmin| xmin < pmin) {
                        props.minimum_len = Some(xmin);
                    }
                } else {
                    props.minimum_len = None;
                    min_poisoned = true;
                }
            }
            if !max_poisoned {
                if let Some(xmax) = p.maximum_len() {
                    if props.maximum_len.map_or(true, |pmax| xmax > pmax) {
                        props.maximum_len = Some(xmax);
                    }
                } else {
                    props.maximum_len = None;
                    max_poisoned = true;
                }
            }
        }
        Properties(Box::new(props))
    }
}

impl Properties {
    /// Create a new set of HIR properties for an empty regex.
    fn empty() -> Properties {
        let inner = PropertiesI {
            minimum_len: Some(0),
            maximum_len: Some(0),
            look_set: LookSet::empty(),
            look_set_prefix: LookSet::empty(),
            look_set_suffix: LookSet::empty(),
            look_set_prefix_any: LookSet::empty(),
            look_set_suffix_any: LookSet::empty(),
            // It is debatable whether an empty regex always matches at valid
            // UTF-8 boundaries. Strictly speaking, at a byte oriented view,
            // it is clearly false. There are, for example, many empty strings
            // between the bytes encoding a '☃'.
            //
            // However, when Unicode mode is enabled, the fundamental atom
            // of matching is really a codepoint. And in that scenario, an
            // empty regex is defined to only match at valid UTF-8 boundaries
            // and to never split a codepoint. It just so happens that this
            // enforcement is somewhat tricky to do for regexes that match
            // the empty string inside regex engines themselves. It usually
            // requires some layer above the regex engine to filter out such
            // matches.
            //
            // In any case, 'true' is really the only coherent option. If it
            // were false, for example, then 'a*' would also need to be false
            // since it too can match the empty string.
            utf8: true,
            explicit_captures_len: 0,
            static_explicit_captures_len: Some(0),
            literal: false,
            alternation_literal: false,
        };
        Properties(Box::new(inner))
    }

    /// Create a new set of HIR properties for a literal regex.
    fn literal(lit: &Literal) -> Properties {
        let inner = PropertiesI {
            minimum_len: Some(lit.0.len()),
            maximum_len: Some(lit.0.len()),
            look_set: LookSet::empty(),
            look_set_prefix: LookSet::empty(),
            look_set_suffix: LookSet::empty(),
            look_set_prefix_any: LookSet::empty(),
            look_set_suffix_any: LookSet::empty(),
            utf8: core::str::from_utf8(&lit.0).is_ok(),
            explicit_captures_len: 0,
            static_explicit_captures_len: Some(0),
            literal: true,
            alternation_literal: true,
        };
        Properties(Box::new(inner))
    }

    /// Create a new set of HIR properties for a character class.
    fn class(class: &Class) -> Properties {
        let inner = PropertiesI {
            minimum_len: class.minimum_len(),
            maximum_len: class.maximum_len(),
            look_set: LookSet::empty(),
            look_set_prefix: LookSet::empty(),
            look_set_suffix: LookSet::empty(),
            look_set_prefix_any: LookSet::empty(),
            look_set_suffix_any: LookSet::empty(),
            utf8: class.is_utf8(),
            explicit_captures_len: 0,
            static_explicit_captures_len: Some(0),
            literal: false,
            alternation_literal: false,
        };
        Properties(Box::new(inner))
    }

    /// Create a new set of HIR properties for a look-around assertion.
    fn look(look: Look) -> Properties {
        let inner = PropertiesI {
            minimum_len: Some(0),
            maximum_len: Some(0),
            look_set: LookSet::singleton(look),
            look_set_prefix: LookSet::singleton(look),
            look_set_suffix: LookSet::singleton(look),
            look_set_prefix_any: LookSet::singleton(look),
            look_set_suffix_any: LookSet::singleton(look),
            // This requires a little explanation. Basically, we don't consider
            // matching an empty string to be equivalent to matching invalid
            // UTF-8, even though technically matching every empty string will
            // split the UTF-8 encoding of a single codepoint when treating a
            // UTF-8 encoded string as a sequence of bytes. Our defense here is
            // that in such a case, a codepoint should logically be treated as
            // the fundamental atom for matching, and thus the only valid match
            // points are between codepoints and not bytes.
            //
            // More practically, this is true here because it's also true
            // for 'Hir::empty()', otherwise something like 'a*' would be
            // considered to match invalid UTF-8. That in turn makes this
            // property borderline useless.
            utf8: true,
            explicit_captures_len: 0,
            static_explicit_captures_len: Some(0),
            literal: false,
            alternation_literal: false,
        };
        Properties(Box::new(inner))
    }

    /// Create a new set of HIR properties for a repetition.
    fn repetition(rep: &Repetition) -> Properties {
        let p = rep.sub.properties();
        let minimum_len = p.minimum_len().map(|child_min| {
            let rep_min = usize::try_from(rep.min).unwrap_or(usize::MAX);
            child_min.saturating_mul(rep_min)
        });
        let maximum_len = rep.max.and_then(|rep_max| {
            let rep_max = usize::try_from(rep_max).ok()?;
            let child_max = p.maximum_len()?;
            child_max.checked_mul(rep_max)
        });

        let mut inner = PropertiesI {
            minimum_len,
            maximum_len,
            look_set: p.look_set(),
            look_set_prefix: LookSet::empty(),
            look_set_suffix: LookSet::empty(),
            look_set_prefix_any: p.look_set_prefix_any(),
            look_set_suffix_any: p.look_set_suffix_any(),
            utf8: p.is_utf8(),
            explicit_captures_len: p.explicit_captures_len(),
            static_explicit_captures_len: p.static_explicit_captures_len(),
            literal: false,
            alternation_literal: false,
        };
        // If the repetition operator can match the empty string, then its
        // lookset prefix and suffixes themselves remain empty since they are
        // no longer required to match.
        if rep.min > 0 {
            inner.look_set_prefix = p.look_set_prefix();
            inner.look_set_suffix = p.look_set_suffix();
        }
        // If the static captures len of the sub-expression is not known or
        // is greater than zero, then it automatically propagates to the
        // repetition, regardless of the repetition. Otherwise, it might
        // change, but only when the repetition can match 0 times.
        if rep.min == 0
            && inner.static_explicit_captures_len.map_or(false, |len| len > 0)
        {
            // If we require a match 0 times, then our captures len is
            // guaranteed to be zero. Otherwise, if we *can* match the empty
            // string, then it's impossible to know how many captures will be
            // in the resulting match.
            if rep.max == Some(0) {
                inner.static_explicit_captures_len = Some(0);
            } else {
                inner.static_explicit_captures_len = None;
            }
        }
        Properties(Box::new(inner))
    }

    /// Create a new set of HIR properties for a capture.
    fn capture(capture: &Capture) -> Properties {
        let p = capture.sub.properties();
        Properties(Box::new(PropertiesI {
            explicit_captures_len: p.explicit_captures_len().saturating_add(1),
            static_explicit_captures_len: p
                .static_explicit_captures_len()
                .map(|len| len.saturating_add(1)),
            literal: false,
            alternation_literal: false,
            ..*p.0.clone()
        }))
    }

    /// Create a new set of HIR properties for a concatenation.
    fn concat(concat: &[Hir]) -> Properties {
        // The base case is an empty concatenation, which matches the empty
        // string. Note though that empty concatenations aren't possible,
        // because the Hir::concat smart constructor rewrites those as
        // Hir::empty.
        let mut props = PropertiesI {
            minimum_len: Some(0),
            maximum_len: Some(0),
            look_set: LookSet::empty(),
            look_set_prefix: LookSet::empty(),
            look_set_suffix: LookSet::empty(),
            look_set_prefix_any: LookSet::empty(),
            look_set_suffix_any: LookSet::empty(),
            utf8: true,
            explicit_captures_len: 0,
            static_explicit_captures_len: Some(0),
            literal: true,
            alternation_literal: true,
        };
        // Handle properties that need to visit every child hir.
        for x in concat.iter() {
            let p = x.properties();
            props.look_set.set_union(p.look_set());
            props.utf8 = props.utf8 && p.is_utf8();
            props.explicit_captures_len = props
                .explicit_captures_len
                .saturating_add(p.explicit_captures_len());
            props.static_explicit_captures_len = p
                .static_explicit_captures_len()
                .and_then(|len1| {
                    Some((len1, props.static_explicit_captures_len?))
                })
                .and_then(|(len1, len2)| Some(len1.saturating_add(len2)));
            props.literal = props.literal && p.is_literal();
            props.alternation_literal =
                props.alternation_literal && p.is_alternation_literal();
            if let Some(minimum_len) = props.minimum_len {
                match p.minimum_len() {
                    None => props.minimum_len = None,
                    Some(len) => {
                        // We use saturating arithmetic here because the
                        // minimum is just a lower bound. We can't go any
                        // higher than what our number types permit.
                        props.minimum_len =
                            Some(minimum_len.saturating_add(len));
                    }
                }
            }
            if let Some(maximum_len) = props.maximum_len {
                match p.maximum_len() {
                    None => props.maximum_len = None,
                    Some(len) => {
                        props.maximum_len = maximum_len.checked_add(len)
                    }
                }
            }
        }
        // Handle the prefix properties, which only requires visiting
        // child exprs until one matches more than the empty string.
        let mut it = concat.iter();
        while let Some(x) = it.next() {
            props.look_set_prefix.set_union(x.properties().look_set_prefix());
            props
                .look_set_prefix_any
                .set_union(x.properties().look_set_prefix_any());
            if x.properties().maximum_len().map_or(true, |x| x > 0) {
                break;
            }
        }
        // Same thing for the suffix properties, but in reverse.
        let mut it = concat.iter().rev();
        while let Some(x) = it.next() {
            props.look_set_suffix.set_union(x.properties().look_set_suffix());
            props
                .look_set_suffix_any
                .set_union(x.properties().look_set_suffix_any());
            if x.properties().maximum_len().map_or(true, |x| x > 0) {
                break;
            }
        }
        Properties(Box::new(props))
    }

    /// Create a new set of HIR properties for a concatenation.
    fn alternation(alts: &[Hir]) -> Properties {
        Properties::union(alts.iter().map(|hir| hir.properties()))
    }
}

/// A set of look-around assertions.
///
/// This is useful for efficiently tracking look-around assertions. For
/// example, an [`Hir`] provides properties that return `LookSet`s.
#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct LookSet {
    /// The underlying representation this set is exposed to make it possible
    /// to store it somewhere efficiently. The representation is that
    /// of a bitset, where each assertion occupies bit `i` where `i =
    /// Look::as_repr()`.
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

/// Given a sequence of HIR values where each value corresponds to a Unicode
/// class (or an all-ASCII byte class), return a single Unicode class
/// corresponding to the union of the classes found.
fn class_chars(hirs: &[Hir]) -> Option<Class> {
    let mut cls = ClassUnicode::new(vec![]);
    for hir in hirs.iter() {
        match *hir.kind() {
            HirKind::Class(Class::Unicode(ref cls2)) => {
                cls.union(cls2);
            }
            HirKind::Class(Class::Bytes(ref cls2)) => {
                cls.union(&cls2.to_unicode_class()?);
            }
            _ => return None,
        };
    }
    Some(Class::Unicode(cls))
}

/// Given a sequence of HIR values where each value corresponds to a byte class
/// (or an all-ASCII Unicode class), return a single byte class corresponding
/// to the union of the classes found.
fn class_bytes(hirs: &[Hir]) -> Option<Class> {
    let mut cls = ClassBytes::new(vec![]);
    for hir in hirs.iter() {
        match *hir.kind() {
            HirKind::Class(Class::Unicode(ref cls2)) => {
                cls.union(&cls2.to_byte_class()?);
            }
            HirKind::Class(Class::Bytes(ref cls2)) => {
                cls.union(cls2);
            }
            _ => return None,
        };
    }
    Some(Class::Bytes(cls))
}

/// Given a sequence of HIR values where each value corresponds to a literal
/// that is a single `char`, return that sequence of `char`s. Otherwise return
/// None. No deduplication is done.
fn singleton_chars(hirs: &[Hir]) -> Option<Vec<char>> {
    let mut singletons = vec![];
    for hir in hirs.iter() {
        let literal = match *hir.kind() {
            HirKind::Literal(Literal(ref bytes)) => bytes,
            _ => return None,
        };
        let ch = match crate::debug::utf8_decode(literal) {
            None => return None,
            Some(Err(_)) => return None,
            Some(Ok(ch)) => ch,
        };
        if literal.len() != ch.len_utf8() {
            return None;
        }
        singletons.push(ch);
    }
    Some(singletons)
}

/// Given a sequence of HIR values where each value corresponds to a literal
/// that is a single byte, return that sequence of bytes. Otherwise return
/// None. No deduplication is done.
fn singleton_bytes(hirs: &[Hir]) -> Option<Vec<u8>> {
    let mut singletons = vec![];
    for hir in hirs.iter() {
        let literal = match *hir.kind() {
            HirKind::Literal(Literal(ref bytes)) => bytes,
            _ => return None,
        };
        if literal.len() != 1 {
            return None;
        }
        singletons.push(literal[0]);
    }
    Some(singletons)
}

/// Looks for a common prefix in the list of alternation branches given. If one
/// is found, then an equivalent but (hopefully) simplified Hir is returned.
/// Otherwise, the original given list of branches is returned unmodified.
///
/// This is not quite as good as it could be. Right now, it requires that
/// all branches are 'Concat' expressions. It also doesn't do well with
/// literals. For example, given 'foofoo|foobar', it will not refactor it to
/// 'foo(?:foo|bar)' because literals are flattened into their own special
/// concatenation. (One wonders if perhaps 'Literal' should be a single atom
/// instead of a string of bytes because of this. Otherwise, handling the
/// current representation in this routine will be pretty gnarly. Sigh.)
fn lift_common_prefix(hirs: Vec<Hir>) -> Result<Hir, Vec<Hir>> {
    if hirs.len() <= 1 {
        return Err(hirs);
    }
    let mut prefix = match hirs[0].kind() {
        HirKind::Concat(ref xs) => &**xs,
        _ => return Err(hirs),
    };
    if prefix.is_empty() {
        return Err(hirs);
    }
    for h in hirs.iter().skip(1) {
        let concat = match h.kind() {
            HirKind::Concat(ref xs) => xs,
            _ => return Err(hirs),
        };
        let common_len = prefix
            .iter()
            .zip(concat.iter())
            .take_while(|(x, y)| x == y)
            .count();
        prefix = &prefix[..common_len];
        if prefix.is_empty() {
            return Err(hirs);
        }
    }
    let len = prefix.len();
    assert_ne!(0, len);
    let mut prefix_concat = vec![];
    let mut suffix_alts = vec![];
    for h in hirs {
        let mut concat = match h.into_kind() {
            HirKind::Concat(xs) => xs,
            // We required all sub-expressions to be
            // concats above, so we're only here if we
            // have a concat.
            _ => unreachable!(),
        };
        suffix_alts.push(Hir::concat(concat.split_off(len)));
        if prefix_concat.is_empty() {
            prefix_concat = concat;
        }
    }
    let mut concat = prefix_concat;
    concat.push(Hir::alternation(suffix_alts));
    Ok(Hir::concat(concat))
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
                expr = Hir::capture(Capture {
                    index: 1,
                    name: None,
                    sub: Box::new(expr),
                });
                expr = Hir::repetition(Repetition {
                    min: 0,
                    max: Some(1),
                    greedy: true,
                    sub: Box::new(expr),
                });

                expr = Hir {
                    kind: HirKind::Concat(vec![expr]),
                    props: Properties::empty(),
                };
                expr = Hir {
                    kind: HirKind::Alternation(vec![expr]),
                    props: Properties::empty(),
                };
            }
            assert!(!matches!(*expr.kind(), HirKind::Empty));
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
    }

    #[test]
    fn look_set_debug() {
        let res = format!("{:?}", LookSet::empty());
        assert_eq!("∅", res);
        let res = format!("{:?}", LookSet::full());
        assert_eq!("Az^$rRbB𝛃𝚩<>〈〉◁▷◀▶", res);
    }
}

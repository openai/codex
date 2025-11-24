#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(elided_lifetimes_in_paths)]
#![deny(unreachable_pub)]

pub mod ascii_str;
pub mod expr;
mod memchr_splitter;
pub mod node;
mod target;
#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::cell::Cell;
use std::env::current_dir;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::sync::Arc;
use std::{fmt, str};

use winnow::ascii::take_escaped;
use winnow::combinator::{alt, cut_err, delimited, fail, not, opt, peek, preceded, repeat};
use winnow::error::FromExternalError;
use winnow::stream::{AsChar, Stream as _};
use winnow::token::{any, one_of, take_till, take_while};
use winnow::{ModalParser, Parser};

use crate::ascii_str::{AsciiChar, AsciiStr};
pub use crate::expr::{Attr, Expr, Filter, TyGenerics};
pub use crate::node::Node;
pub use crate::target::Target;

mod _parsed {
    use std::path::Path;
    use std::sync::Arc;
    use std::{fmt, mem};

    use super::node::Node;
    use super::{Ast, ParseError, Syntax};

    pub struct Parsed {
        // `source` must outlive `ast`, so `ast` must be declared before `source`
        ast: Ast<'static>,
        #[allow(dead_code)]
        source: Arc<str>,
    }

    impl Parsed {
        /// If `file_path` is `None`, it means the `source` is an inline template. Therefore, if
        /// a parsing error occurs, we won't display the path as it wouldn't be useful.
        pub fn new(
            source: Arc<str>,
            file_path: Option<Arc<Path>>,
            syntax: &Syntax<'_>,
        ) -> Result<Self, ParseError> {
            // Self-referential borrowing: `self` will keep the source alive as `String`,
            // internally we will transmute it to `&'static str` to satisfy the compiler.
            // However, we only expose the nodes with a lifetime limited to `self`.
            let src = unsafe { mem::transmute::<&str, &'static str>(source.as_ref()) };
            let ast = Ast::from_str(src, file_path, syntax)?;
            Ok(Self { ast, source })
        }

        // The return value's lifetime must be limited to `self` to uphold the unsafe invariant.
        #[must_use]
        pub fn nodes(&self) -> &[Node<'_>] {
            &self.ast.nodes
        }

        #[must_use]
        pub fn source(&self) -> &str {
            &self.source
        }
    }

    impl fmt::Debug for Parsed {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("Parsed")
                .field("nodes", &self.ast.nodes)
                .finish_non_exhaustive()
        }
    }

    impl PartialEq for Parsed {
        fn eq(&self, other: &Self) -> bool {
            self.ast.nodes == other.ast.nodes
        }
    }

    impl Default for Parsed {
        fn default() -> Self {
            Self {
                ast: Ast::default(),
                source: "".into(),
            }
        }
    }
}

pub use _parsed::Parsed;

#[derive(Debug, Default)]
pub struct Ast<'a> {
    nodes: Vec<Node<'a>>,
}

impl<'a> Ast<'a> {
    /// If `file_path` is `None`, it means the `source` is an inline template. Therefore, if
    /// a parsing error occurs, we won't display the path as it wouldn't be useful.
    pub fn from_str(
        mut src: &'a str,
        file_path: Option<Arc<Path>>,
        syntax: &Syntax<'_>,
    ) -> Result<Self, ParseError> {
        let start = src;
        let level = Cell::new(Level::MAX_DEPTH);
        let state = State {
            syntax,
            loop_depth: Cell::new(0),
            level: Level(&level),
        };
        match Node::parse_template(&mut src, &state) {
            Ok(nodes) if src.is_empty() => Ok(Self { nodes }),
            Ok(_) | Err(winnow::error::ErrMode::Incomplete(_)) => unreachable!(),
            Err(
                winnow::error::ErrMode::Backtrack(ErrorContext { span, message, .. })
                | winnow::error::ErrMode::Cut(ErrorContext { span, message, .. }),
            ) => Err(ParseError {
                message,
                offset: span.offset_from(start).unwrap_or_default(),
                file_path,
            }),
        }
    }

    #[must_use]
    pub fn nodes(&self) -> &[Node<'a>] {
        &self.nodes
    }
}

/// Struct used to wrap types with their associated "span" which is used when generating errors
/// in the code generation.
pub struct WithSpan<'a, T> {
    inner: T,
    span: Span<'a>,
}

/// An location in `&'a str`
#[derive(Debug, Clone, Copy)]
pub struct Span<'a>(&'a [u8; 0]);

impl Default for Span<'static> {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

impl<'a> Span<'a> {
    #[inline]
    pub const fn empty() -> Self {
        Self(&[])
    }

    pub fn offset_from(self, start: &'a str) -> Option<usize> {
        let start_range = start.as_bytes().as_ptr_range();
        let this_ptr = self.0.as_slice().as_ptr();
        match start_range.contains(&this_ptr) {
            // SAFETY: we just checked that `this_ptr` is inside `start_range`
            true => Some(unsafe { this_ptr.offset_from(start_range.start) as usize }),
            false => None,
        }
    }

    pub fn as_suffix_of(self, start: &'a str) -> Option<&'a str> {
        let offset = self.offset_from(start)?;
        match start.is_char_boundary(offset) {
            true => Some(&start[offset..]),
            false => None,
        }
    }
}

impl<'a> From<&'a str> for Span<'a> {
    #[inline]
    fn from(value: &'a str) -> Self {
        Self(value.as_bytes()[..0].try_into().unwrap())
    }
}

impl<'a, T> WithSpan<'a, T> {
    #[inline]
    pub fn new(inner: T, span: impl Into<Span<'a>>) -> Self {
        Self {
            inner,
            span: span.into(),
        }
    }

    #[inline]
    pub const fn new_without_span(inner: T) -> Self {
        Self {
            inner,
            span: Span::empty(),
        }
    }

    #[inline]
    pub fn span(&self) -> Span<'a> {
        self.span
    }

    #[inline]
    pub fn deconstruct(self) -> (T, Span<'a>) {
        let Self { inner, span } = self;
        (inner, span)
    }
}

impl<T> Deref for WithSpan<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for WithSpan<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T: fmt::Debug> fmt::Debug for WithSpan<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}

impl<T: Clone> Clone for WithSpan<'_, T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            span: self.span,
        }
    }
}

impl<T: PartialEq> PartialEq for WithSpan<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        // We never want to compare the span information.
        self.inner == other.inner
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: Option<Cow<'static, str>>,
    pub offset: usize,
    pub file_path: Option<Arc<Path>>,
}

impl std::error::Error for ParseError {}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ParseError {
            message,
            file_path,
            offset,
        } = self;

        if let Some(message) = message {
            writeln!(f, "{message}")?;
        }

        let path = file_path
            .as_ref()
            .and_then(|path| Some(strip_common(&current_dir().ok()?, path)));
        match path {
            Some(path) => write!(f, "failed to parse template source\n  --> {path}@{offset}"),
            None => write!(f, "failed to parse template source near offset {offset}"),
        }
    }
}

pub(crate) type ParseErr<'a> = winnow::error::ErrMode<ErrorContext<'a>>;
pub(crate) type ParseResult<'a, T = &'a str> = Result<T, ParseErr<'a>>;

/// This type is used to handle `nom` errors and in particular to add custom error messages.
/// It used to generate `ParserError`.
///
/// It cannot be used to replace `ParseError` because it expects a generic, which would make
/// `askama`'s users experience less good (since this generic is only needed for `nom`).
#[derive(Debug)]
pub(crate) struct ErrorContext<'a> {
    pub(crate) span: Span<'a>,
    pub(crate) message: Option<Cow<'static, str>>,
}

impl<'a> ErrorContext<'a> {
    fn unclosed(kind: &str, tag: &str, span: impl Into<Span<'a>>) -> Self {
        Self::new(format!("unclosed {kind}, missing {tag:?}"), span)
    }

    fn new(message: impl Into<Cow<'static, str>>, span: impl Into<Span<'a>>) -> Self {
        Self {
            span: span.into(),
            message: Some(message.into()),
        }
    }

    fn backtrack(self) -> winnow::error::ErrMode<Self> {
        winnow::error::ErrMode::Backtrack(self)
    }

    fn cut(self) -> winnow::error::ErrMode<Self> {
        winnow::error::ErrMode::Cut(self)
    }
}

impl<'a> winnow::error::ParserError<&'a str> for ErrorContext<'a> {
    type Inner = Self;

    fn from_input(input: &&'a str) -> Self {
        Self {
            span: (*input).into(),
            message: None,
        }
    }

    #[inline(always)]
    fn into_inner(self) -> Result<Self::Inner, Self> {
        Ok(self)
    }
}

impl<'a, E: std::fmt::Display> FromExternalError<&'a str, E> for ErrorContext<'a> {
    fn from_external_error(input: &&'a str, e: E) -> Self {
        Self {
            span: (*input).into(),
            message: Some(Cow::Owned(e.to_string())),
        }
    }
}

#[inline]
fn skip_ws0<'a>(i: &mut &'a str) -> ParseResult<'a, ()> {
    *i = i.trim_ascii_start();
    Ok(())
}

#[inline]
fn skip_ws1<'a>(i: &mut &'a str) -> ParseResult<'a, ()> {
    let j = i.trim_ascii_start();
    if i.len() != j.len() {
        *i = i.trim_ascii_start();
        Ok(())
    } else {
        fail.parse_next(i)
    }
}

fn ws<'a, O>(
    inner: impl ModalParser<&'a str, O, ErrorContext<'a>>,
) -> impl ModalParser<&'a str, O, ErrorContext<'a>> {
    delimited(skip_ws0, inner, skip_ws0)
}

/// Skips input until `end` was found, but does not consume it.
/// Returns tuple that would be returned when parsing `end`.
fn skip_till<'a, 'b, O>(
    candidate_finder: impl crate::memchr_splitter::Splitter,
    end: impl ModalParser<&'a str, O, ErrorContext<'a>>,
) -> impl ModalParser<&'a str, (&'a str, O), ErrorContext<'a>> {
    let mut next = alt((end.map(Some), any.map(|_| None)));
    move |i: &mut &'a str| loop {
        *i = match candidate_finder.split(i) {
            Some((_, i)) => i,
            None => {
                return Err(winnow::error::ErrMode::Backtrack(ErrorContext::new(
                    "`end` not found`",
                    *i,
                )));
            }
        };
        let exclusive = *i;
        if let Some(lookahead) = next.parse_next(i)? {
            let inclusive = *i;
            *i = exclusive;
            return Ok((inclusive, lookahead));
        }
    }
}

fn keyword(k: &str) -> impl ModalParser<&str, &str, ErrorContext<'_>> {
    identifier.verify(move |v: &str| v == k)
}

fn identifier<'i>(input: &mut &'i str) -> ParseResult<'i> {
    let start = take_while(1.., |c: char| c.is_alpha() || c == '_' || c >= '\u{0080}');

    let tail = take_while(1.., |c: char| {
        c.is_alphanum() || c == '_' || c >= '\u{0080}'
    });

    (start, opt(tail)).take().parse_next(input)
}

fn bool_lit<'i>(i: &mut &'i str) -> ParseResult<'i> {
    alt((keyword("false"), keyword("true"))).parse_next(i)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Num<'a> {
    Int(&'a str, Option<IntKind>),
    Float(&'a str, Option<FloatKind>),
}

fn num_lit<'a>(i: &mut &'a str) -> ParseResult<'a, Num<'a>> {
    fn num_lit_suffix<'a, T: Copy>(
        kind: &'a str,
        list: &[(&str, T)],
        start: &'a str,
        i: &mut &'a str,
    ) -> ParseResult<'a, T> {
        let suffix = identifier.parse_next(i)?;
        if let Some(value) = list
            .iter()
            .copied()
            .find_map(|(name, value)| (name == suffix).then_some(value))
        {
            Ok(value)
        } else {
            Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                format!("unknown {kind} suffix `{suffix}`"),
                start,
            )))
        }
    }

    let start = *i;

    // Equivalent to <https://github.com/rust-lang/rust/blob/e3f909b2bbd0b10db6f164d466db237c582d3045/compiler/rustc_lexer/src/lib.rs#L587-L620>.
    let int_with_base = (opt('-'), |i: &mut _| {
        let (base, kind) = preceded('0', alt(('b'.value(2), 'o'.value(8), 'x'.value(16))))
            .with_taken()
            .parse_next(i)?;
        match opt(separated_digits(base, false)).parse_next(i)? {
            Some(_) => Ok(()),
            None => Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                format!("expected digits after `{kind}`"),
                start,
            ))),
        }
    });

    // Equivalent to <https://github.com/rust-lang/rust/blob/e3f909b2bbd0b10db6f164d466db237c582d3045/compiler/rustc_lexer/src/lib.rs#L626-L653>:
    // no `_` directly after the decimal point `.`, or between `e` and `+/-`.
    let float = |i: &mut &'a str| -> ParseResult<'a, ()> {
        let has_dot = opt(('.', separated_digits(10, true))).parse_next(i)?;
        let has_exp = opt(|i: &mut _| {
            let (kind, op) = (one_of(['e', 'E']), opt(one_of(['+', '-']))).parse_next(i)?;
            match opt(separated_digits(10, op.is_none())).parse_next(i)? {
                Some(_) => Ok(()),
                None => Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                    format!("expected decimal digits, `+` or `-` after exponent `{kind}`"),
                    start,
                ))),
            }
        })
        .parse_next(i)?;
        match (has_dot, has_exp) {
            (Some(_), _) | (_, Some(())) => Ok(()),
            _ => {
                *i = start;
                fail.parse_next(i)
            }
        }
    };

    let num = if let Ok(Some(num)) = opt(int_with_base.take()).parse_next(i) {
        let suffix =
            opt(|i: &mut _| num_lit_suffix("integer", INTEGER_TYPES, start, i)).parse_next(i)?;
        Num::Int(num, suffix)
    } else {
        let (float, num) = preceded((opt('-'), separated_digits(10, true)), opt(float))
            .with_taken()
            .parse_next(i)?;
        if float.is_some() {
            let suffix =
                opt(|i: &mut _| num_lit_suffix("float", FLOAT_TYPES, start, i)).parse_next(i)?;
            Num::Float(num, suffix)
        } else {
            let suffix =
                opt(|i: &mut _| num_lit_suffix("number", NUM_TYPES, start, i)).parse_next(i)?;
            match suffix {
                Some(NumKind::Int(kind)) => Num::Int(num, Some(kind)),
                Some(NumKind::Float(kind)) => Num::Float(num, Some(kind)),
                None => Num::Int(num, None),
            }
        }
    };
    Ok(num)
}

/// Underscore separated digits of the given base, unless `start` is true this may start
/// with an underscore.
fn separated_digits<'a>(
    radix: u32,
    start: bool,
) -> impl ModalParser<&'a str, &'a str, ErrorContext<'a>> {
    (
        move |i: &mut &'a _| match start {
            true => Ok(()),
            false => repeat(0.., '_').parse_next(i),
        },
        one_of(move |ch: char| ch.is_digit(radix)),
        repeat(0.., one_of(move |ch: char| ch == '_' || ch.is_digit(radix))).map(|()| ()),
    )
        .take()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StrPrefix {
    Binary,
    CLike,
}

impl StrPrefix {
    #[must_use]
    pub fn to_char(self) -> char {
        match self {
            Self::Binary => 'b',
            Self::CLike => 'c',
        }
    }
}

impl fmt::Display for StrPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use std::fmt::Write;

        f.write_char(self.to_char())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StrLit<'a> {
    pub prefix: Option<StrPrefix>,
    pub content: &'a str,
}

fn str_lit_without_prefix<'a>(i: &mut &'a str) -> ParseResult<'a> {
    let s = delimited(
        '"',
        opt(take_escaped(take_till(1.., ['\\', '"']), '\\', any)),
        '"',
    )
    .parse_next(i)?;
    Ok(s.unwrap_or_default())
}

fn str_lit<'a>(i: &mut &'a str) -> ParseResult<'a, StrLit<'a>> {
    let (prefix, content) = (opt(alt(('b', 'c'))), str_lit_without_prefix).parse_next(i)?;
    let prefix = match prefix {
        Some('b') => Some(StrPrefix::Binary),
        Some('c') => Some(StrPrefix::CLike),
        _ => None,
    };
    Ok(StrLit { prefix, content })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CharPrefix {
    Binary,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CharLit<'a> {
    pub prefix: Option<CharPrefix>,
    pub content: &'a str,
}

// Information about allowed character escapes is available at:
// <https://doc.rust-lang.org/reference/tokens.html#character-literals>.
fn char_lit<'a>(i: &mut &'a str) -> ParseResult<'a, CharLit<'a>> {
    let start = i.checkpoint();
    let (b_prefix, s) = (
        opt('b'),
        delimited(
            '\'',
            opt(take_escaped(take_till(1.., ['\\', '\'']), '\\', any)),
            '\'',
        ),
    )
        .parse_next(i)?;

    let Some(s) = s else {
        i.reset(&start);
        return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            "empty character literal",
            *i,
        )));
    };
    let mut is = s;
    let Ok(c) = Char::parse(&mut is) else {
        i.reset(&start);
        return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            "invalid character",
            *i,
        )));
    };

    let (nb, max_value, err1, err2) = match c {
        Char::Literal | Char::Escaped => {
            return Ok(CharLit {
                prefix: b_prefix.map(|_| CharPrefix::Binary),
                content: s,
            });
        }
        Char::AsciiEscape(nb) => (
            nb,
            // `0x7F` is the maximum value for a `\x` escaped character.
            0x7F,
            "invalid character in ascii escape",
            "must be a character in the range [\\x00-\\x7f]",
        ),
        Char::UnicodeEscape(nb) => (
            nb,
            // `0x10FFFF` is the maximum value for a `\u` escaped character.
            0x0010_FFFF,
            "invalid character in unicode escape",
            "unicode escape must be at most 10FFFF",
        ),
    };

    let Ok(nb) = u32::from_str_radix(nb, 16) else {
        i.reset(&start);
        return Err(winnow::error::ErrMode::Cut(ErrorContext::new(err1, *i)));
    };
    if nb > max_value {
        i.reset(&start);
        return Err(winnow::error::ErrMode::Cut(ErrorContext::new(err2, *i)));
    }

    Ok(CharLit {
        prefix: b_prefix.map(|_| CharPrefix::Binary),
        content: s,
    })
}

/// Represents the different kinds of char declarations:
#[derive(Copy, Clone)]
enum Char<'a> {
    /// Any character that is not escaped.
    Literal,
    /// An escaped character (like `\n`) which doesn't require any extra check.
    Escaped,
    /// Ascii escape (like `\x12`).
    AsciiEscape(&'a str),
    /// Unicode escape (like `\u{12}`).
    UnicodeEscape(&'a str),
}

impl<'a> Char<'a> {
    fn parse(i: &mut &'a str) -> ParseResult<'a, Self> {
        if i.chars().count() == 1 {
            return any.value(Self::Literal).parse_next(i);
        }
        (
            '\\',
            alt((
                'n'.value(Self::Escaped),
                'r'.value(Self::Escaped),
                't'.value(Self::Escaped),
                '\\'.value(Self::Escaped),
                '0'.value(Self::Escaped),
                '\''.value(Self::Escaped),
                // Not useful but supported by rust.
                '"'.value(Self::Escaped),
                ('x', take_while(2, |c: char| c.is_ascii_hexdigit()))
                    .map(|(_, s)| Self::AsciiEscape(s)),
                (
                    "u{",
                    take_while(1..=6, |c: char| c.is_ascii_hexdigit()),
                    '}',
                )
                    .map(|(_, s, _)| Self::UnicodeEscape(s)),
            )),
        )
            .map(|(_, ch)| ch)
            .parse_next(i)
    }
}

enum PathOrIdentifier<'a> {
    Path(Vec<&'a str>),
    Identifier(&'a str),
}

fn path_or_identifier<'a>(i: &mut &'a str) -> ParseResult<'a, PathOrIdentifier<'a>> {
    let root = ws(opt("::"));
    let tail = opt(repeat(1.., preceded(ws("::"), identifier)).map(|v: Vec<_>| v));

    let (root, start, rest) = (root, identifier, tail).parse_next(i)?;
    let rest = rest.as_deref().unwrap_or_default();

    // The returned identifier can be assumed to be path if:
    // - it is an absolute path (starts with `::`), or
    // - it has multiple components (at least one `::`), or
    // - the first letter is uppercase
    match (root, start, rest) {
        (Some(_), start, tail) => {
            let mut path = Vec::with_capacity(2 + tail.len());
            path.push("");
            path.push(start);
            path.extend(rest);
            Ok(PathOrIdentifier::Path(path))
        }
        (None, name, [])
            if name
                .chars()
                .next()
                .is_none_or(|c| c == '_' || c.is_lowercase()) =>
        {
            Ok(PathOrIdentifier::Identifier(name))
        }
        (None, start, tail) => {
            let mut path = Vec::with_capacity(1 + tail.len());
            path.push(start);
            path.extend(rest);
            Ok(PathOrIdentifier::Path(path))
        }
    }
}

struct State<'a, 'l> {
    syntax: &'l Syntax<'a>,
    loop_depth: Cell<usize>,
    level: Level<'l>,
}

impl State<'_, '_> {
    fn tag_block_start<'i>(&self, i: &mut &'i str) -> ParseResult<'i, ()> {
        self.syntax.block_start.value(()).parse_next(i)
    }

    fn tag_block_end<'i>(&self, i: &mut &'i str) -> ParseResult<'i, ()> {
        let control = alt((
            self.syntax.block_end.value(None),
            peek(delimited('%', alt(('-', '~', '+')).map(Some), '}')),
            fail, // rollback on partial matches in the previous line
        ))
        .parse_next(i)?;
        if let Some(control) = control {
            let message = format!(
                "unclosed block, you likely meant to apply whitespace control: \"{}{}\"",
                control.escape_default(),
                self.syntax.block_end.escape_default(),
            );
            Err(ErrorContext::new(message, *i).backtrack())
        } else {
            Ok(())
        }
    }

    fn tag_comment_start<'i>(&self, i: &mut &'i str) -> ParseResult<'i, ()> {
        self.syntax.comment_start.value(()).parse_next(i)
    }

    fn tag_comment_end<'i>(&self, i: &mut &'i str) -> ParseResult<'i, ()> {
        self.syntax.comment_end.value(()).parse_next(i)
    }

    fn tag_expr_start<'i>(&self, i: &mut &'i str) -> ParseResult<'i, ()> {
        self.syntax.expr_start.value(()).parse_next(i)
    }

    fn tag_expr_end<'i>(&self, i: &mut &'i str) -> ParseResult<'i, ()> {
        self.syntax.expr_end.value(()).parse_next(i)
    }

    fn enter_loop(&self) {
        self.loop_depth.set(self.loop_depth.get() + 1);
    }

    fn leave_loop(&self) {
        self.loop_depth.set(self.loop_depth.get() - 1);
    }

    fn is_in_loop(&self) -> bool {
        self.loop_depth.get() > 0
    }
}

#[derive(Default, Hash, PartialEq, Clone, Copy)]
pub struct Syntax<'a>(InnerSyntax<'a>);

// This abstraction ensures that the fields are readable, but not writable.
#[derive(Hash, PartialEq, Clone, Copy)]
pub struct InnerSyntax<'a> {
    pub block_start: &'a str,
    pub block_end: &'a str,
    pub expr_start: &'a str,
    pub expr_end: &'a str,
    pub comment_start: &'a str,
    pub comment_end: &'a str,
}

impl<'a> Deref for Syntax<'a> {
    type Target = InnerSyntax<'a>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for InnerSyntax<'static> {
    fn default() -> Self {
        Self {
            block_start: "{%",
            block_end: "%}",
            expr_start: "{{",
            expr_end: "}}",
            comment_start: "{#",
            comment_end: "#}",
        }
    }
}

impl fmt::Debug for Syntax<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_syntax("Syntax", self, f)
    }
}

impl fmt::Debug for InnerSyntax<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_syntax("InnerSyntax", self, f)
    }
}

fn fmt_syntax(name: &str, inner: &InnerSyntax<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct(name)
        .field("block_start", &inner.block_start)
        .field("block_end", &inner.block_end)
        .field("expr_start", &inner.expr_start)
        .field("expr_end", &inner.expr_end)
        .field("comment_start", &inner.comment_start)
        .field("comment_end", &inner.comment_end)
        .finish()
}

#[derive(Debug, Default, Clone, Copy, Hash, PartialEq)]
#[cfg_attr(feature = "config", derive(serde_derive::Deserialize))]
pub struct SyntaxBuilder<'a> {
    pub name: &'a str,
    pub block_start: Option<&'a str>,
    pub block_end: Option<&'a str>,
    pub expr_start: Option<&'a str>,
    pub expr_end: Option<&'a str>,
    pub comment_start: Option<&'a str>,
    pub comment_end: Option<&'a str>,
}

impl<'a> SyntaxBuilder<'a> {
    pub fn to_syntax(&self) -> Result<Syntax<'a>, String> {
        let default = InnerSyntax::default();
        let syntax = Syntax(InnerSyntax {
            block_start: self.block_start.unwrap_or(default.block_start),
            block_end: self.block_end.unwrap_or(default.block_end),
            expr_start: self.expr_start.unwrap_or(default.expr_start),
            expr_end: self.expr_end.unwrap_or(default.expr_end),
            comment_start: self.comment_start.unwrap_or(default.comment_start),
            comment_end: self.comment_end.unwrap_or(default.comment_end),
        });

        for (s, k, is_closing) in [
            (syntax.block_start, "opening block", false),
            (syntax.block_end, "closing block", true),
            (syntax.expr_start, "opening expression", false),
            (syntax.expr_end, "closing expression", true),
            (syntax.comment_start, "opening comment", false),
            (syntax.comment_end, "closing comment", true),
        ] {
            if s.len() < 2 {
                return Err(format!(
                    "delimiters must be at least two characters long. \
                        The {k} delimiter ({s:?}) is too short",
                ));
            } else if s.len() > 32 {
                return Err(format!(
                    "delimiters must be at most 32 characters long. \
                        The {k} delimiter ({:?}...) is too long",
                    &s[..(16..=s.len())
                        .find(|&i| s.is_char_boundary(i))
                        .unwrap_or(s.len())],
                ));
            } else if s.chars().any(char::is_whitespace) {
                return Err(format!(
                    "delimiters may not contain white spaces. \
                        The {k} delimiter ({s:?}) contains white spaces",
                ));
            } else if is_closing
                && ['(', '-', '+', '~', '.', '>', '<', '&', '|', '!']
                    .contains(&s.chars().next().unwrap())
            {
                return Err(format!(
                    "closing delimiters may not start with operators. \
                        The {k} delimiter ({s:?}) starts with operator `{}`",
                    s.chars().next().unwrap(),
                ));
            }
        }

        for ((s1, k1), (s2, k2)) in [
            (
                (syntax.block_start, "block"),
                (syntax.expr_start, "expression"),
            ),
            (
                (syntax.block_start, "block"),
                (syntax.comment_start, "comment"),
            ),
            (
                (syntax.expr_start, "expression"),
                (syntax.comment_start, "comment"),
            ),
        ] {
            if s1.starts_with(s2) || s2.starts_with(s1) {
                let (s1, k1, s2, k2) = match s1.len() < s2.len() {
                    true => (s1, k1, s2, k2),
                    false => (s2, k2, s1, k1),
                };
                return Err(format!(
                    "an opening delimiter may not be the prefix of another delimiter. \
                        The {k1} delimiter ({s1:?}) clashes with the {k2} delimiter ({s2:?})",
                ));
            }
        }

        Ok(syntax)
    }
}

/// The nesting level of nodes and expressions.
///
/// The level counts down from [`Level::MAX_DEPTH`] to 0. Once the value would reach below 0,
/// [`Level::nest()`] / [`LevelGuard::nest()`] will return an error. The same [`Level`] instance is
/// shared across all usages in a [`Parsed::new()`] / [`Ast::from_str()`] call, using a reference
/// to an interior mutable counter.
#[derive(Debug, Clone, Copy)]
struct Level<'l>(&'l Cell<usize>);

impl Level<'_> {
    const MAX_DEPTH: usize = 128;

    /// Acquire a [`LevelGuard`] without decrementing the counter, to be used with loops.
    fn guard(&self) -> LevelGuard<'_> {
        LevelGuard {
            level: *self,
            count: 0,
        }
    }

    /// Decrement the remaining level counter, and return a [`LevelGuard`] that increments it again
    /// when it's dropped.
    fn nest<'a>(&self, i: &'a str) -> ParseResult<'a, LevelGuard<'_>> {
        if let Some(new_level) = self.0.get().checked_sub(1) {
            self.0.set(new_level);
            Ok(LevelGuard {
                level: *self,
                count: 1,
            })
        } else {
            Err(Self::_fail(i))
        }
    }

    #[cold]
    #[inline(never)]
    fn _fail(i: &str) -> ParseErr<'_> {
        winnow::error::ErrMode::Cut(ErrorContext::new(
            "your template code is too deeply nested, or the last expression is too complex",
            i,
        ))
    }
}

/// Used to keep track how often [`LevelGuard::nest()`] was called and to re-increment the
/// remaining level counter when it is dropped / falls out of scope.
#[must_use]
struct LevelGuard<'l> {
    level: Level<'l>,
    count: usize,
}

impl Drop for LevelGuard<'_> {
    fn drop(&mut self) {
        self.level.0.set(self.level.0.get() + self.count);
    }
}

impl LevelGuard<'_> {
    /// Used to decrement the level multiple times, e.g. for every iteration of a loop.
    fn nest<'a>(&mut self, i: &'a str) -> ParseResult<'a, ()> {
        if let Some(new_level) = self.level.0.get().checked_sub(1) {
            self.level.0.set(new_level);
            self.count += 1;
            Ok(())
        } else {
            Err(Level::_fail(i))
        }
    }
}

#[allow(clippy::type_complexity)]
fn filter<'a>(
    i: &mut &'a str,
    level: Level<'_>,
) -> ParseResult<
    'a,
    (
        &'a str,
        Vec<WithSpan<'a, TyGenerics<'a>>>,
        Option<Vec<WithSpan<'a, Expr<'a>>>>,
    ),
> {
    ws(('|', not('|'))).parse_next(i)?;

    let _level_guard = level.nest(i)?;
    cut_err((
        ws(identifier),
        opt(|i: &mut _| expr::call_generics(i, level)).map(|generics| generics.unwrap_or_default()),
        opt(|i: &mut _| Expr::arguments(i, level, true)),
    ))
    .parse_next(i)
}

/// Returns the common parts of two paths.
///
/// The goal of this function is to reduce the path length based on the `base` argument
/// (generally the path where the program is running into). For example:
///
/// ```text
/// current dir: /a/b/c
/// path:        /a/b/c/d/e.txt
/// ```
///
/// `strip_common` will return `d/e.txt`.
#[must_use]
pub fn strip_common(base: &Path, path: &Path) -> String {
    let path = match path.canonicalize() {
        Ok(path) => path,
        Err(_) => return path.display().to_string(),
    };
    let mut components_iter = path.components().peekable();

    for current_path_component in base.components() {
        let Some(path_component) = components_iter.peek() else {
            return path.display().to_string();
        };
        if current_path_component != *path_component {
            break;
        }
        components_iter.next();
    }
    let path_parts = components_iter
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>();
    if path_parts.is_empty() {
        path.display().to_string()
    } else {
        path_parts.join(std::path::MAIN_SEPARATOR_STR)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntKind {
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatKind {
    F16,
    F32,
    F64,
    F128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumKind {
    Int(IntKind),
    Float(FloatKind),
}

/// Primitive integer types. Also used as number suffixes.
const INTEGER_TYPES: &[(&str, IntKind)] = &[
    ("i8", IntKind::I8),
    ("i16", IntKind::I16),
    ("i32", IntKind::I32),
    ("i64", IntKind::I64),
    ("i128", IntKind::I128),
    ("isize", IntKind::Isize),
    ("u8", IntKind::U8),
    ("u16", IntKind::U16),
    ("u32", IntKind::U32),
    ("u64", IntKind::U64),
    ("u128", IntKind::U128),
    ("usize", IntKind::Usize),
];

/// Primitive floating point types. Also used as number suffixes.
const FLOAT_TYPES: &[(&str, FloatKind)] = &[
    ("f16", FloatKind::F16),
    ("f32", FloatKind::F32),
    ("f64", FloatKind::F64),
    ("f128", FloatKind::F128),
];

/// Primitive numeric types. Also used as number suffixes.
const NUM_TYPES: &[(&str, NumKind)] = &{
    let mut list = [("", NumKind::Int(IntKind::I8)); INTEGER_TYPES.len() + FLOAT_TYPES.len()];
    let mut i = 0;
    let mut o = 0;
    while i < INTEGER_TYPES.len() {
        let (name, value) = INTEGER_TYPES[i];
        list[o] = (name, NumKind::Int(value));
        i += 1;
        o += 1;
    }
    let mut i = 0;
    while i < FLOAT_TYPES.len() {
        let (name, value) = FLOAT_TYPES[i];
        list[o] = (name, NumKind::Float(value));
        i += 1;
        o += 1;
    }
    list
};

/// Complete list of named primitive types.
const PRIMITIVE_TYPES: &[&str] = &{
    let mut list = [""; NUM_TYPES.len() + 1];
    let mut i = 0;
    let mut o = 0;
    while i < NUM_TYPES.len() {
        list[o] = NUM_TYPES[i].0;
        i += 1;
        o += 1;
    }
    list[o] = "bool";
    list
};

pub const MAX_RUST_KEYWORD_LEN: usize = 8;
pub const MAX_RUST_RAW_KEYWORD_LEN: usize = MAX_RUST_KEYWORD_LEN + 2;

pub const RUST_KEYWORDS: &[&[[AsciiChar; MAX_RUST_RAW_KEYWORD_LEN]]; MAX_RUST_KEYWORD_LEN + 1] = &{
    const NO_KWS: &[[AsciiChar; MAX_RUST_RAW_KEYWORD_LEN]] = &[];
    const KW2: &[[AsciiChar; MAX_RUST_RAW_KEYWORD_LEN]] = &[
        AsciiStr::new_sized("r#as"),
        AsciiStr::new_sized("r#do"),
        AsciiStr::new_sized("r#fn"),
        AsciiStr::new_sized("r#if"),
        AsciiStr::new_sized("r#in"),
    ];
    const KW3: &[[AsciiChar; MAX_RUST_RAW_KEYWORD_LEN]] = &[
        AsciiStr::new_sized("r#box"),
        AsciiStr::new_sized("r#dyn"),
        AsciiStr::new_sized("r#for"),
        AsciiStr::new_sized("r#gen"),
        AsciiStr::new_sized("r#let"),
        AsciiStr::new_sized("r#mod"),
        AsciiStr::new_sized("r#mut"),
        AsciiStr::new_sized("r#pub"),
        AsciiStr::new_sized("r#ref"),
        AsciiStr::new_sized("r#try"),
        AsciiStr::new_sized("r#use"),
    ];
    const KW4: &[[AsciiChar; MAX_RUST_RAW_KEYWORD_LEN]] = &[
        AsciiStr::new_sized("r#else"),
        AsciiStr::new_sized("r#enum"),
        AsciiStr::new_sized("r#impl"),
        AsciiStr::new_sized("r#move"),
        AsciiStr::new_sized("r#priv"),
        AsciiStr::new_sized("r#true"),
        AsciiStr::new_sized("r#type"),
    ];
    const KW5: &[[AsciiChar; MAX_RUST_RAW_KEYWORD_LEN]] = &[
        AsciiStr::new_sized("r#async"),
        AsciiStr::new_sized("r#await"),
        AsciiStr::new_sized("r#break"),
        AsciiStr::new_sized("r#const"),
        AsciiStr::new_sized("r#crate"),
        AsciiStr::new_sized("r#false"),
        AsciiStr::new_sized("r#final"),
        AsciiStr::new_sized("r#macro"),
        AsciiStr::new_sized("r#match"),
        AsciiStr::new_sized("r#trait"),
        AsciiStr::new_sized("r#where"),
        AsciiStr::new_sized("r#while"),
        AsciiStr::new_sized("r#yield"),
    ];
    const KW6: &[[AsciiChar; MAX_RUST_RAW_KEYWORD_LEN]] = &[
        AsciiStr::new_sized("r#become"),
        AsciiStr::new_sized("r#extern"),
        AsciiStr::new_sized("r#return"),
        AsciiStr::new_sized("r#static"),
        AsciiStr::new_sized("r#struct"),
        AsciiStr::new_sized("r#typeof"),
        AsciiStr::new_sized("r#unsafe"),
    ];
    const KW7: &[[AsciiChar; MAX_RUST_RAW_KEYWORD_LEN]] = &[
        AsciiStr::new_sized("r#unsized"),
        AsciiStr::new_sized("r#virtual"),
    ];
    const KW8: &[[AsciiChar; MAX_RUST_RAW_KEYWORD_LEN]] = &[
        AsciiStr::new_sized("r#abstract"),
        AsciiStr::new_sized("r#continue"),
        AsciiStr::new_sized("r#override"),
    ];

    [NO_KWS, NO_KWS, KW2, KW3, KW4, KW5, KW6, KW7, KW8]
};

// These ones are only used in the parser, hence why they're private.
const KWS_PARSER: &[&[[AsciiChar; MAX_RUST_RAW_KEYWORD_LEN]]; MAX_RUST_KEYWORD_LEN + 1] = &{
    const KW4: &[[AsciiChar; MAX_RUST_RAW_KEYWORD_LEN]] = &{
        let mut result = [AsciiStr::new_sized("r#"); RUST_KEYWORDS[4].len() + 3];
        let mut i = 0;
        while i < RUST_KEYWORDS[4].len() {
            result[i] = RUST_KEYWORDS[4][i];
            i += 1;
        }
        result[result.len() - 3] = AsciiStr::new_sized("r#loop");
        result[result.len() - 2] = AsciiStr::new_sized("r#self");
        result[result.len() - 1] = AsciiStr::new_sized("r#Self");
        result
    };
    const KW5: &[[AsciiChar; MAX_RUST_RAW_KEYWORD_LEN]] = &{
        let mut result = [AsciiStr::new_sized("r#"); RUST_KEYWORDS[5].len() + 2];
        let mut i = 0;
        while i < RUST_KEYWORDS[5].len() {
            result[i] = RUST_KEYWORDS[5][i];
            i += 1;
        }
        result[result.len() - 2] = AsciiStr::new_sized("r#super");
        result[result.len() - 1] = AsciiStr::new_sized("r#union");
        result
    };

    [
        RUST_KEYWORDS[0],
        RUST_KEYWORDS[1],
        RUST_KEYWORDS[2],
        RUST_KEYWORDS[3],
        KW4,
        KW5,
        RUST_KEYWORDS[6],
        RUST_KEYWORDS[7],
        RUST_KEYWORDS[8],
    ]
};

fn is_rust_keyword(ident: &str) -> bool {
    let ident_len = ident.len();
    if ident_len > MAX_RUST_KEYWORD_LEN {
        return false;
    }
    let kws = KWS_PARSER[ident.len()];

    let mut padded_ident = [0; MAX_RUST_KEYWORD_LEN];
    padded_ident[..ident_len].copy_from_slice(ident.as_bytes());

    // Since the individual buckets are quite short, a linear search is faster than a binary search.
    for probe in kws {
        if padded_ident == *AsciiChar::slice_as_bytes(probe[2..].try_into().unwrap()) {
            return true;
        }
    }
    false
}

#[cfg(not(windows))]
#[cfg(test)]
mod test {
    use std::path::Path;

    use super::*;

    #[test]
    fn test_strip_common() {
        // Full path is returned instead of empty when the entire path is in common.
        assert_eq!(strip_common(Path::new("home"), Path::new("home")), "home");

        let cwd = std::env::current_dir().expect("current_dir failed");

        // We need actual existing paths for `canonicalize` to work, so let's do that.
        let entry = cwd
            .read_dir()
            .expect("read_dir failed")
            .filter_map(std::result::Result::ok)
            .find(|f| f.path().is_file())
            .expect("no entry");

        // Since they have the complete path in common except for the folder entry name, it should
        // return only the folder entry name.
        assert_eq!(
            strip_common(&cwd, &entry.path()),
            entry.file_name().to_string_lossy()
        );

        // In this case it cannot canonicalize `/a/b/c` so it returns the path as is.
        assert_eq!(strip_common(&cwd, Path::new("/a/b/c")), "/a/b/c");
    }

    #[test]
    fn test_num_lit() {
        // Should fail.
        assert!(num_lit.parse_peek(".").is_err());
        // Should succeed.
        assert_eq!(
            num_lit.parse_peek("1.2E-02").unwrap(),
            ("", Num::Float("1.2E-02", None))
        );
        assert_eq!(
            num_lit.parse_peek("4e3").unwrap(),
            ("", Num::Float("4e3", None)),
        );
        assert_eq!(
            num_lit.parse_peek("4e+_3").unwrap(),
            ("", Num::Float("4e+_3", None)),
        );
        // Not supported because Rust wants a number before the `.`.
        assert!(num_lit.parse_peek(".1").is_err());
        assert!(num_lit.parse_peek(".1E-02").is_err());
        // A `_` directly after the `.` denotes a field.
        assert_eq!(
            num_lit.parse_peek("1._0").unwrap(),
            ("._0", Num::Int("1", None))
        );
        assert_eq!(
            num_lit.parse_peek("1_.0").unwrap(),
            ("", Num::Float("1_.0", None))
        );
        // Not supported (voluntarily because of `1..` syntax).
        assert_eq!(
            num_lit.parse_peek("1.").unwrap(),
            (".", Num::Int("1", None))
        );
        assert_eq!(
            num_lit.parse_peek("1_.").unwrap(),
            (".", Num::Int("1_", None))
        );
        assert_eq!(
            num_lit.parse_peek("1_2.").unwrap(),
            (".", Num::Int("1_2", None))
        );
        // Numbers with suffixes
        assert_eq!(
            num_lit.parse_peek("-1usize").unwrap(),
            ("", Num::Int("-1", Some(IntKind::Usize)))
        );
        assert_eq!(
            num_lit.parse_peek("123_f32").unwrap(),
            ("", Num::Float("123_", Some(FloatKind::F32)))
        );
        assert_eq!(
            num_lit.parse_peek("1_.2_e+_3_f64|into_isize").unwrap(),
            (
                "|into_isize",
                Num::Float("1_.2_e+_3_", Some(FloatKind::F64))
            )
        );
        assert_eq!(
            num_lit.parse_peek("4e3f128").unwrap(),
            ("", Num::Float("4e3", Some(FloatKind::F128))),
        );
    }

    #[test]
    fn test_char_lit() {
        let lit = |s: &'static str| crate::CharLit {
            prefix: None,
            content: s,
        };

        assert_eq!(char_lit.parse_peek("'a'").unwrap(), ("", lit("a")));
        assert_eq!(char_lit.parse_peek("'字'").unwrap(), ("", lit("字")));

        // Escaped single characters.
        assert_eq!(char_lit.parse_peek("'\\\"'").unwrap(), ("", lit("\\\"")));
        assert_eq!(char_lit.parse_peek("'\\''").unwrap(), ("", lit("\\'")));
        assert_eq!(char_lit.parse_peek("'\\t'").unwrap(), ("", lit("\\t")));
        assert_eq!(char_lit.parse_peek("'\\n'").unwrap(), ("", lit("\\n")));
        assert_eq!(char_lit.parse_peek("'\\r'").unwrap(), ("", lit("\\r")));
        assert_eq!(char_lit.parse_peek("'\\0'").unwrap(), ("", lit("\\0")));
        // Escaped ascii characters (up to `0x7F`).
        assert_eq!(char_lit.parse_peek("'\\x12'").unwrap(), ("", lit("\\x12")));
        assert_eq!(char_lit.parse_peek("'\\x02'").unwrap(), ("", lit("\\x02")));
        assert_eq!(char_lit.parse_peek("'\\x6a'").unwrap(), ("", lit("\\x6a")));
        assert_eq!(char_lit.parse_peek("'\\x7F'").unwrap(), ("", lit("\\x7F")));
        // Escaped unicode characters (up to `0x10FFFF`).
        assert_eq!(
            char_lit.parse_peek("'\\u{A}'").unwrap(),
            ("", lit("\\u{A}"))
        );
        assert_eq!(
            char_lit.parse_peek("'\\u{10}'").unwrap(),
            ("", lit("\\u{10}"))
        );
        assert_eq!(
            char_lit.parse_peek("'\\u{aa}'").unwrap(),
            ("", lit("\\u{aa}"))
        );
        assert_eq!(
            char_lit.parse_peek("'\\u{10FFFF}'").unwrap(),
            ("", lit("\\u{10FFFF}"))
        );

        // Check with `b` prefix.
        assert_eq!(
            char_lit.parse_peek("b'a'").unwrap(),
            (
                "",
                crate::CharLit {
                    prefix: Some(crate::CharPrefix::Binary),
                    content: "a"
                }
            )
        );

        // Should fail.
        assert!(char_lit.parse_peek("''").is_err());
        assert!(char_lit.parse_peek("'\\o'").is_err());
        assert!(char_lit.parse_peek("'\\x'").is_err());
        assert!(char_lit.parse_peek("'\\x1'").is_err());
        assert!(char_lit.parse_peek("'\\x80'").is_err());
        assert!(char_lit.parse_peek("'\\u'").is_err());
        assert!(char_lit.parse_peek("'\\u{}'").is_err());
        assert!(char_lit.parse_peek("'\\u{110000}'").is_err());
    }

    #[test]
    fn test_str_lit() {
        assert_eq!(
            str_lit.parse_peek(r#"b"hello""#).unwrap(),
            (
                "",
                StrLit {
                    prefix: Some(StrPrefix::Binary),
                    content: "hello"
                }
            )
        );
        assert_eq!(
            str_lit.parse_peek(r#"c"hello""#).unwrap(),
            (
                "",
                StrLit {
                    prefix: Some(StrPrefix::CLike),
                    content: "hello"
                }
            )
        );
        assert!(str_lit.parse_peek(r#"d"hello""#).is_err());
    }

    #[test]
    fn assert_span_size() {
        assert_eq!(
            std::mem::size_of::<Span<'static>>(),
            std::mem::size_of::<*const ()>()
        );
    }

    #[test]
    fn test_is_rust_keyword() {
        assert!(is_rust_keyword("super"));
        assert!(is_rust_keyword("become"));
        assert!(!is_rust_keyword("supeeeer"));
        assert!(!is_rust_keyword("sur"));
    }
}

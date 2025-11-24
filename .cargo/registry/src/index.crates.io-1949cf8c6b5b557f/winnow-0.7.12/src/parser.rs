//! Basic types to build the parsers

use crate::ascii::Caseless as AsciiCaseless;
use crate::combinator::impls;
#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
use crate::error::FromRecoverableError;
use crate::error::{AddContext, FromExternalError, ParseError, ParserError, Result};
use crate::stream::{Compare, Location, ParseSlice, Stream, StreamIsPartial};
#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
use crate::stream::{Recover, Recoverable};

/// Core trait for parsing
///
/// The simplest way to implement a `Parser` is with a function
/// ```rust
/// use winnow::prelude::*;
///
/// fn empty(input: &mut &str) -> ModalResult<()> {
///     let output = ();
///     Ok(output)
/// }
///
/// let (input, output) = empty.parse_peek("Hello").unwrap();
/// assert_eq!(input, "Hello");  // We didn't consume any input
/// ```
///
/// which can be made stateful by returning a function
/// ```rust
/// use winnow::prelude::*;
///
/// fn empty<O: Clone>(output: O) -> impl FnMut(&mut &str) -> ModalResult<O> {
///     move |input: &mut &str| {
///         let output = output.clone();
///         Ok(output)
///     }
/// }
///
/// let (input, output) = empty("World").parse_peek("Hello").unwrap();
/// assert_eq!(input, "Hello");  // We didn't consume any input
/// assert_eq!(output, "World");
/// ```
///
/// Additionally, some basic types implement `Parser` as well, including
/// - `u8` and `char`, see [`winnow::token::one_of`][crate::token::one_of]
/// - `&[u8]` and `&str`, see [`winnow::token::literal`][crate::token::literal]
pub trait Parser<I, O, E> {
    /// Parse all of `input`, generating `O` from it
    ///
    /// This is intended for integrating your parser into the rest of your application.
    ///
    /// For one [`Parser`] to drive another [`Parser`] forward or for
    /// [incremental parsing][StreamIsPartial], see instead [`Parser::parse_next`].
    ///
    /// This assumes the [`Parser`] intends to read all of `input` and will return an
    /// [`eof`][crate::combinator::eof] error if it does not
    /// To ignore trailing `input`, combine your parser with a [`rest`][crate::token::rest]
    /// (e.g. `(parser, rest).parse(input)`).
    ///
    /// See also the [tutorial][crate::_tutorial::chapter_6].
    #[inline]
    fn parse(&mut self, mut input: I) -> Result<O, ParseError<I, <E as ParserError<I>>::Inner>>
    where
        Self: core::marker::Sized,
        I: Stream,
        // Force users to deal with `Incomplete` when `StreamIsPartial<true>`
        I: StreamIsPartial,
        E: ParserError<I>,
        <E as ParserError<I>>::Inner: ParserError<I>,
    {
        debug_assert!(
            !I::is_partial_supported(),
            "partial streams need to handle `ErrMode::Incomplete`"
        );

        let start = input.checkpoint();
        let (o, _) = (self.by_ref(), crate::combinator::eof)
            .parse_next(&mut input)
            .map_err(|e| {
                let e = e.into_inner().unwrap_or_else(|_err| {
                    panic!("complete parsers should not report `ErrMode::Incomplete(_)`")
                });
                ParseError::new(input, start, e)
            })?;
        Ok(o)
    }

    /// Take tokens from the [`Stream`], turning it into the output
    ///
    /// This includes advancing the input [`Stream`] to the next location.
    ///
    /// On error, `input` will be left pointing at the error location.
    ///
    /// This is intended for a [`Parser`] to drive another [`Parser`] forward or for
    /// [incremental parsing][StreamIsPartial]
    fn parse_next(&mut self, input: &mut I) -> Result<O, E>;

    /// Take tokens from the [`Stream`], turning it into the output
    ///
    /// This returns a copy of the [`Stream`] advanced to the next location.
    ///
    /// <div class="warning">
    ///
    /// Generally, prefer [`Parser::parse_next`].
    /// This is primarily intended for:
    /// - Migrating from older versions / `nom`
    /// - Testing [`Parser`]s
    ///
    /// For look-ahead parsing, see instead [`peek`][crate::combinator::peek].
    ///
    /// </div>
    #[inline(always)]
    fn parse_peek(&mut self, mut input: I) -> Result<(I, O), E> {
        match self.parse_next(&mut input) {
            Ok(o) => Ok((input, o)),
            Err(err) => Err(err),
        }
    }

    /// Treat `&mut Self` as a parser
    ///
    /// This helps when needing to move a `Parser` when all you have is a `&mut Parser`.
    ///
    /// # Example
    ///
    /// Because parsers are `FnMut`, they can be called multiple times. This prevents moving `f`
    /// into [`length_take`][crate::binary::length_take] and `g` into
    /// [`Parser::complete_err`]:
    /// ```rust,compile_fail
    /// # use winnow::prelude::*;
    /// # use winnow::Parser;
    /// # use winnow::error::ParserError;
    /// # use winnow::binary::length_take;
    /// pub fn length_value<'i, O, E: ParserError<&'i [u8]>>(
    ///     mut f: impl Parser<&'i [u8], usize, E>,
    ///     mut g: impl Parser<&'i [u8], O, E>
    /// ) -> impl Parser<&'i [u8], O, E> {
    ///   move |i: &mut &'i [u8]| {
    ///     let mut data = length_take(f).parse_next(i)?;
    ///     let o = g.complete_err().parse_next(&mut data)?;
    ///     Ok(o)
    ///   }
    /// }
    /// ```
    ///
    /// By adding `by_ref`, we can make this work:
    /// ```rust
    /// # use winnow::prelude::*;
    /// # use winnow::Parser;
    /// # use winnow::error::ParserError;
    /// # use winnow::binary::length_take;
    /// pub fn length_value<'i, O, E: ParserError<&'i [u8]>>(
    ///     mut f: impl Parser<&'i [u8], usize, E>,
    ///     mut g: impl Parser<&'i [u8], O, E>
    /// ) -> impl Parser<&'i [u8], O, E> {
    ///   move |i: &mut &'i [u8]| {
    ///     let mut data = length_take(f.by_ref()).parse_next(i)?;
    ///     let o = g.by_ref().complete_err().parse_next(&mut data)?;
    ///     Ok(o)
    ///   }
    /// }
    /// ```
    #[inline(always)]
    fn by_ref(&mut self) -> impls::ByRef<'_, Self, I, O, E>
    where
        Self: core::marker::Sized,
    {
        impls::ByRef {
            p: self,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
        }
    }

    /// Produce the provided value
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::{error::ErrMode, Parser};
    /// # use winnow::prelude::*;
    /// use winnow::ascii::alpha1;
    /// # fn main() {
    ///
    /// fn parser<'i>(input: &mut &'i str) -> ModalResult<i32> {
    ///     alpha1.value(1234).parse_next(input)
    /// }
    ///
    /// assert_eq!(parser.parse_peek("abcd"), Ok(("", 1234)));
    /// assert!(parser.parse_peek("123abcd;").is_err());
    /// # }
    /// ```
    #[doc(alias = "to")]
    #[inline(always)]
    fn value<O2>(self, val: O2) -> impls::Value<Self, I, O, O2, E>
    where
        Self: core::marker::Sized,
        O2: Clone,
    {
        impls::Value {
            parser: self,
            val,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
        }
    }

    /// Produce a type's default value
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::{error::ErrMode, Parser};
    /// # use winnow::prelude::*;
    /// use winnow::ascii::alpha1;
    /// # fn main() {
    ///
    /// fn parser<'i>(input: &mut &'i str) -> ModalResult<u32> {
    ///     alpha1.default_value().parse_next(input)
    /// }
    ///
    /// assert_eq!(parser.parse_peek("abcd"), Ok(("", 0)));
    /// assert!(parser.parse_peek("123abcd;").is_err());
    /// # }
    /// ```
    #[inline(always)]
    fn default_value<O2>(self) -> impls::DefaultValue<Self, I, O, O2, E>
    where
        Self: core::marker::Sized,
        O2: core::default::Default,
    {
        impls::DefaultValue {
            parser: self,
            o2: Default::default(),
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
        }
    }

    /// Discards the output of the `Parser`
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::{error::ErrMode, Parser};
    /// # use winnow::prelude::*;
    /// use winnow::ascii::alpha1;
    /// # fn main() {
    ///
    /// fn parser<'i>(input: &mut &'i str) -> ModalResult<()> {
    ///     alpha1.void().parse_next(input)
    /// }
    ///
    /// assert_eq!(parser.parse_peek("abcd"), Ok(("", ())));
    /// assert!(parser.parse_peek("123abcd;").is_err());
    /// # }
    /// ```
    #[inline(always)]
    fn void(self) -> impls::Void<Self, I, O, E>
    where
        Self: core::marker::Sized,
    {
        impls::Void {
            parser: self,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
        }
    }

    /// Convert the parser's output to another type using [`std::convert::From`]
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::prelude::*;
    /// # use winnow::error::ContextError;
    /// use winnow::ascii::alpha1;
    /// # fn main() {
    ///
    /// fn parser1<'s>(i: &mut &'s str) -> ModalResult<&'s str> {
    ///   alpha1(i)
    /// }
    ///
    /// let mut parser2 = parser1.output_into();
    ///
    /// // the parser converts the &str output of the child parser into a Vec<u8>
    /// let bytes: ModalResult<(_, Vec<u8>), _> = parser2.parse_peek("abcd");
    /// assert_eq!(bytes, Ok(("", vec![97, 98, 99, 100])));
    /// # }
    /// ```
    #[inline(always)]
    fn output_into<O2>(self) -> impls::OutputInto<Self, I, O, O2, E>
    where
        Self: core::marker::Sized,
        O: Into<O2>,
    {
        impls::OutputInto {
            parser: self,
            i: Default::default(),
            o: Default::default(),
            o2: Default::default(),
            e: Default::default(),
        }
    }

    /// Produce the consumed input as produced value.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::{error::ErrMode, Parser};
    /// # use winnow::prelude::*;
    /// use winnow::ascii::{alpha1};
    /// use winnow::combinator::separated_pair;
    /// # fn main() {
    ///
    /// fn parser<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
    ///     separated_pair(alpha1, ',', alpha1).take().parse_next(input)
    /// }
    ///
    /// assert_eq!(parser.parse_peek("abcd,efgh"), Ok(("", "abcd,efgh")));
    /// assert!(parser.parse_peek("abcd;").is_err());
    /// # }
    /// ```
    #[doc(alias = "concat")]
    #[doc(alias = "recognize")]
    #[inline(always)]
    fn take(self) -> impls::Take<Self, I, O, E>
    where
        Self: core::marker::Sized,
        I: Stream,
    {
        impls::Take {
            parser: self,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
        }
    }

    /// Produce the consumed input with the output
    ///
    /// Functions similarly to [take][Parser::take] except it
    /// returns the parser output as well.
    ///
    /// This can be useful especially in cases where the output is not the same type
    /// as the input, or the input is a user defined type.
    ///
    /// Returned tuple is of the format `(produced output, consumed input)`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::prelude::*;
    /// # use winnow::{error::ErrMode};
    /// use winnow::ascii::{alpha1};
    /// use winnow::token::literal;
    /// use winnow::combinator::separated_pair;
    ///
    /// fn parser<'i>(input: &mut &'i str) -> ModalResult<(bool, &'i str)> {
    ///     separated_pair(alpha1, ',', alpha1).value(true).with_taken().parse_next(input)
    /// }
    ///
    /// assert_eq!(parser.parse_peek("abcd,efgh1"), Ok(("1", (true, "abcd,efgh"))));
    /// assert!(parser.parse_peek("abcd;").is_err());
    /// ```
    #[doc(alias = "consumed")]
    #[doc(alias = "with_recognized")]
    #[inline(always)]
    fn with_taken(self) -> impls::WithTaken<Self, I, O, E>
    where
        Self: core::marker::Sized,
        I: Stream,
    {
        impls::WithTaken {
            parser: self,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
        }
    }

    /// Produce the location of the consumed input as produced value.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::prelude::*;
    /// # use winnow::{error::ErrMode, stream::Stream};
    /// # use std::ops::Range;
    /// use winnow::stream::LocatingSlice;
    /// use winnow::ascii::alpha1;
    /// use winnow::combinator::separated_pair;
    ///
    /// fn parser<'i>(input: &mut LocatingSlice<&'i str>) -> ModalResult<(Range<usize>, Range<usize>)> {
    ///     separated_pair(alpha1.span(), ',', alpha1.span()).parse_next(input)
    /// }
    ///
    /// assert_eq!(parser.parse(LocatingSlice::new("abcd,efgh")), Ok((0..4, 5..9)));
    /// assert!(parser.parse_peek(LocatingSlice::new("abcd;")).is_err());
    /// ```
    #[inline(always)]
    fn span(self) -> impls::Span<Self, I, O, E>
    where
        Self: core::marker::Sized,
        I: Stream + Location,
    {
        impls::Span {
            parser: self,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
        }
    }

    /// Produce the location of consumed input with the output
    ///
    /// Functions similarly to [`Parser::span`] except it
    /// returns the parser output as well.
    ///
    /// This can be useful especially in cases where the output is not the same type
    /// as the input, or the input is a user defined type.
    ///
    /// Returned tuple is of the format `(produced output, consumed input)`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::prelude::*;
    /// # use winnow::{error::ErrMode, stream::Stream};
    /// # use std::ops::Range;
    /// use winnow::stream::LocatingSlice;
    /// use winnow::ascii::alpha1;
    /// use winnow::token::literal;
    /// use winnow::combinator::separated_pair;
    ///
    /// fn parser<'i>(input: &mut LocatingSlice<&'i str>) -> ModalResult<((usize, Range<usize>), (usize, Range<usize>))> {
    ///     separated_pair(alpha1.value(1).with_span(), ',', alpha1.value(2).with_span()).parse_next(input)
    /// }
    ///
    /// assert_eq!(parser.parse(LocatingSlice::new("abcd,efgh")), Ok(((1, 0..4), (2, 5..9))));
    /// assert!(parser.parse_peek(LocatingSlice::new("abcd;")).is_err());
    /// ```
    #[inline(always)]
    fn with_span(self) -> impls::WithSpan<Self, I, O, E>
    where
        Self: core::marker::Sized,
        I: Stream + Location,
    {
        impls::WithSpan {
            parser: self,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
        }
    }

    /// Maps a function over the output of a parser
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::prelude::*;
    /// # use winnow::{error::ErrMode, Parser};
    /// # use winnow::ascii::digit1;
    /// # fn main() {
    ///
    /// fn parser<'i>(input: &mut &'i str) -> ModalResult<usize> {
    ///     digit1.map(|s: &str| s.len()).parse_next(input)
    /// }
    ///
    /// // the parser will count how many characters were returned by digit1
    /// assert_eq!(parser.parse_peek("123456"), Ok(("", 6)));
    ///
    /// // this will fail if digit1 fails
    /// assert!(parser.parse_peek("abc").is_err());
    /// # }
    /// ```
    #[inline(always)]
    fn map<G, O2>(self, map: G) -> impls::Map<Self, G, I, O, O2, E>
    where
        G: FnMut(O) -> O2,
        Self: core::marker::Sized,
    {
        impls::Map {
            parser: self,
            map,
            i: Default::default(),
            o: Default::default(),
            o2: Default::default(),
            e: Default::default(),
        }
    }

    /// Applies a function returning a `Result` over the output of a parser.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::{error::ErrMode, Parser};
    /// # use winnow::prelude::*;
    /// use winnow::ascii::digit1;
    /// # fn main() {
    ///
    /// fn parser<'i>(input: &mut &'i str) -> ModalResult<u8> {
    ///     digit1.try_map(|s: &str| s.parse::<u8>()).parse_next(input)
    /// }
    ///
    /// // the parser will convert the result of digit1 to a number
    /// assert_eq!(parser.parse_peek("123"), Ok(("", 123)));
    ///
    /// // this will fail if digit1 fails
    /// assert!(parser.parse_peek("abc").is_err());
    ///
    /// // this will fail if the mapped function fails (a `u8` is too small to hold `123456`)
    /// assert!(parser.parse_peek("123456").is_err());
    /// # }
    /// ```
    #[inline(always)]
    fn try_map<G, O2, E2>(self, map: G) -> impls::TryMap<Self, G, I, O, O2, E, E2>
    where
        Self: core::marker::Sized,
        G: FnMut(O) -> Result<O2, E2>,
        I: Stream,
        E: FromExternalError<I, E2>,
        E: ParserError<I>,
    {
        impls::TryMap {
            parser: self,
            map,
            i: Default::default(),
            o: Default::default(),
            o2: Default::default(),
            e: Default::default(),
            e2: Default::default(),
        }
    }

    /// Apply both [`Parser::verify`] and [`Parser::map`].
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::{error::ErrMode, Parser};
    /// # use winnow::prelude::*;
    /// use winnow::ascii::digit1;
    /// # fn main() {
    ///
    /// fn parser<'i>(input: &mut &'i str) -> ModalResult<u8> {
    ///     digit1.verify_map(|s: &str| s.parse::<u8>().ok()).parse_next(input)
    /// }
    ///
    /// // the parser will convert the result of digit1 to a number
    /// assert_eq!(parser.parse_peek("123"), Ok(("", 123)));
    ///
    /// // this will fail if digit1 fails
    /// assert!(parser.parse_peek("abc").is_err());
    ///
    /// // this will fail if the mapped function fails (a `u8` is too small to hold `123456`)
    /// assert!(parser.parse_peek("123456").is_err());
    /// # }
    /// ```
    #[doc(alias = "satisfy_map")]
    #[doc(alias = "filter_map")]
    #[doc(alias = "map_opt")]
    #[inline(always)]
    fn verify_map<G, O2>(self, map: G) -> impls::VerifyMap<Self, G, I, O, O2, E>
    where
        Self: core::marker::Sized,
        G: FnMut(O) -> Option<O2>,
        I: Stream,
        E: ParserError<I>,
    {
        impls::VerifyMap {
            parser: self,
            map,
            i: Default::default(),
            o: Default::default(),
            o2: Default::default(),
            e: Default::default(),
        }
    }

    /// Creates a parser from the output of this one
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::{error::ErrMode, ModalResult, Parser};
    /// use winnow::token::take;
    /// use winnow::binary::u8;
    ///
    /// fn length_take<'s>(input: &mut &'s [u8]) -> ModalResult<&'s [u8]> {
    ///     u8.flat_map(take).parse_next(input)
    /// }
    ///
    /// assert_eq!(length_take.parse_peek(&[2, 0, 1, 2][..]), Ok((&[2][..], &[0, 1][..])));
    /// assert!(length_take.parse_peek(&[4, 0, 1, 2][..]).is_err());
    /// ```
    ///
    /// which is the same as
    /// ```rust
    /// # use winnow::{error::ErrMode, ModalResult, Parser};
    /// use winnow::token::take;
    /// use winnow::binary::u8;
    ///
    /// fn length_take<'s>(input: &mut &'s [u8]) -> ModalResult<&'s [u8]> {
    ///     let length = u8.parse_next(input)?;
    ///     let data = take(length).parse_next(input)?;
    ///     Ok(data)
    /// }
    ///
    /// assert_eq!(length_take.parse_peek(&[2, 0, 1, 2][..]), Ok((&[2][..], &[0, 1][..])));
    /// assert!(length_take.parse_peek(&[4, 0, 1, 2][..]).is_err());
    /// ```
    #[inline(always)]
    fn flat_map<G, H, O2>(self, map: G) -> impls::FlatMap<Self, G, H, I, O, O2, E>
    where
        Self: core::marker::Sized,
        G: FnMut(O) -> H,
        H: Parser<I, O2, E>,
    {
        impls::FlatMap {
            f: self,
            g: map,
            h: Default::default(),
            i: Default::default(),
            o: Default::default(),
            o2: Default::default(),
            e: Default::default(),
        }
    }

    /// Applies a second parser over the output of the first one
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::{error::ErrMode, Parser};
    /// # use winnow::prelude::*;
    /// use winnow::ascii::digit1;
    /// use winnow::token::take;
    /// # fn main() {
    ///
    /// fn parser<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
    ///     take(5u8).and_then(digit1).parse_next(input)
    /// }
    ///
    /// assert_eq!(parser.parse_peek("12345"), Ok(("", "12345")));
    /// assert_eq!(parser.parse_peek("123ab"), Ok(("", "123")));
    /// assert!(parser.parse_peek("123").is_err());
    /// # }
    /// ```
    #[inline(always)]
    fn and_then<G, O2>(self, inner: G) -> impls::AndThen<Self, G, I, O, O2, E>
    where
        Self: core::marker::Sized,
        G: Parser<O, O2, E>,
        O: StreamIsPartial,
        I: Stream,
    {
        impls::AndThen {
            outer: self,
            inner,
            i: Default::default(),
            o: Default::default(),
            o2: Default::default(),
            e: Default::default(),
        }
    }

    /// Apply [`std::str::FromStr`] to the output of the parser
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::prelude::*;
    /// use winnow::{error::ErrMode, Parser};
    /// use winnow::ascii::digit1;
    ///
    /// fn parser<'s>(input: &mut &'s str) -> ModalResult<u64> {
    ///     digit1.parse_to().parse_next(input)
    /// }
    ///
    /// // the parser will count how many characters were returned by digit1
    /// assert_eq!(parser.parse_peek("123456"), Ok(("", 123456)));
    ///
    /// // this will fail if digit1 fails
    /// assert!(parser.parse_peek("abc").is_err());
    /// ```
    #[doc(alias = "from_str")]
    #[inline(always)]
    fn parse_to<O2>(self) -> impls::ParseTo<Self, I, O, O2, E>
    where
        Self: core::marker::Sized,
        I: Stream,
        O: ParseSlice<O2>,
        E: ParserError<I>,
    {
        impls::ParseTo {
            p: self,
            i: Default::default(),
            o: Default::default(),
            o2: Default::default(),
            e: Default::default(),
        }
    }

    /// Returns the output of the child parser if it satisfies a verification function.
    ///
    /// The verification function takes as argument a reference to the output of the
    /// parser.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::{error::ErrMode, Parser};
    /// # use winnow::ascii::alpha1;
    /// # use winnow::prelude::*;
    /// # fn main() {
    ///
    /// fn parser<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
    ///     alpha1.verify(|s: &str| s.len() == 4).parse_next(input)
    /// }
    ///
    /// assert_eq!(parser.parse_peek("abcd"), Ok(("", "abcd")));
    /// assert!(parser.parse_peek("abcde").is_err());
    /// assert!(parser.parse_peek("123abcd;").is_err());
    /// # }
    /// ```
    #[doc(alias = "satisfy")]
    #[doc(alias = "filter")]
    #[inline(always)]
    fn verify<G, O2>(self, filter: G) -> impls::Verify<Self, G, I, O, O2, E>
    where
        Self: core::marker::Sized,
        G: FnMut(&O2) -> bool,
        I: Stream,
        O: crate::lib::std::borrow::Borrow<O2>,
        O2: ?Sized,
        E: ParserError<I>,
    {
        impls::Verify {
            parser: self,
            filter,
            i: Default::default(),
            o: Default::default(),
            o2: Default::default(),
            e: Default::default(),
        }
    }

    /// If parsing fails, add context to the error
    ///
    /// This is used mainly to add user friendly information
    /// to errors when backtracking through a parse tree.
    ///
    /// See also [tutorial][crate::_tutorial::chapter_7].
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::prelude::*;
    /// # use winnow::{error::ErrMode, Parser};
    /// # use winnow::ascii::digit1;
    /// # use winnow::error::StrContext;
    /// # use winnow::error::StrContextValue;
    /// # fn main() {
    ///
    /// fn parser<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
    ///     digit1
    ///       .context(StrContext::Expected(StrContextValue::Description("digit")))
    ///       .parse_next(input)
    /// }
    ///
    /// assert_eq!(parser.parse_peek("123456"), Ok(("", "123456")));
    /// assert!(parser.parse_peek("abc").is_err());
    /// # }
    /// ```
    #[doc(alias = "labelled")]
    #[inline(always)]
    fn context<C>(self, context: C) -> impls::Context<Self, I, O, E, C>
    where
        Self: core::marker::Sized,
        I: Stream,
        E: AddContext<I, C>,
        E: ParserError<I>,
        C: Clone + crate::lib::std::fmt::Debug,
    {
        impls::Context {
            parser: self,
            context,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
        }
    }

    /// If parsing fails, dynamically add context to the error
    ///
    /// This is used mainly to add user friendly information
    /// to errors when backtracking through a parse tree.
    ///
    /// See also [tutorial][crate::_tutorial::chapter_7].
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::prelude::*;
    /// # use winnow::{error::ErrMode, Parser};
    /// # use winnow::ascii::digit1;
    /// # use winnow::error::StrContext;
    /// # use winnow::error::StrContextValue;
    /// # fn main() {
    ///
    /// fn parser<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
    ///     digit1
    ///       .context_with(|| {
    ///         "0123456789".chars().map(|c| StrContext::Expected(c.into()))
    ///       })
    ///       .parse_next(input)
    /// }
    ///
    /// assert_eq!(parser.parse_peek("123456"), Ok(("", "123456")));
    /// assert!(parser.parse_peek("abc").is_err());
    /// # }
    /// ```
    #[doc(alias = "labelled")]
    #[inline(always)]
    fn context_with<F, C, FI>(self, context: F) -> impls::ContextWith<Self, I, O, E, F, C, FI>
    where
        Self: core::marker::Sized,
        I: Stream,
        E: AddContext<I, C>,
        E: ParserError<I>,
        F: Fn() -> FI + Clone,
        C: crate::lib::std::fmt::Debug,
        FI: Iterator<Item = C>,
    {
        impls::ContextWith {
            parser: self,
            context,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
            c: Default::default(),
            fi: Default::default(),
        }
    }

    /// Maps a function over the error of a parser
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::prelude::*;
    /// # use winnow::Parser;
    /// # use winnow::Result;
    /// # use winnow::ascii::digit1;
    /// # use winnow::error::StrContext;
    /// # use winnow::error::AddContext;
    /// # use winnow::error::ContextError;
    /// # fn main() {
    ///
    /// fn parser<'i>(input: &mut &'i str) -> Result<&'i str> {
    ///     digit1.map_err(|mut e: ContextError| {
    ///         e.extend("0123456789".chars().map(|c| StrContext::Expected(c.into())));
    ///         e
    ///     }).parse_next(input)
    /// }
    ///
    /// assert_eq!(parser.parse_peek("123456"), Ok(("", "123456")));
    /// assert!(parser.parse_peek("abc").is_err());
    /// # }
    /// ```
    #[inline(always)]
    fn map_err<G, E2>(self, map: G) -> impls::MapErr<Self, G, I, O, E, E2>
    where
        G: FnMut(E) -> E2,
        Self: core::marker::Sized,
    {
        impls::MapErr {
            parser: self,
            map,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
            e2: Default::default(),
        }
    }

    /// Transforms [`Incomplete`][crate::error::ErrMode::Incomplete] into [`Backtrack`][crate::error::ErrMode::Backtrack]
    ///
    /// # Example
    ///
    /// ```rust
    /// # use winnow::{error::ErrMode, error::InputError, stream::Partial, Parser};
    /// # use winnow::token::take;
    /// # use winnow::prelude::*;
    /// # fn main() {
    ///
    /// fn parser<'i>(input: &mut Partial<&'i str>) -> ModalResult<&'i str, InputError<Partial<&'i str>>> {
    ///     take(5u8).complete_err().parse_next(input)
    /// }
    ///
    /// assert_eq!(parser.parse_peek(Partial::new("abcdefg")), Ok((Partial::new("fg"), "abcde")));
    /// assert_eq!(parser.parse_peek(Partial::new("abcd")), Err(ErrMode::Backtrack(InputError::at(Partial::new("abcd")))));
    /// # }
    /// ```
    #[inline(always)]
    fn complete_err(self) -> impls::CompleteErr<Self, I, O, E>
    where
        Self: core::marker::Sized,
    {
        impls::CompleteErr {
            p: self,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
        }
    }

    /// Convert the parser's error to another type using [`std::convert::From`]
    #[inline(always)]
    fn err_into<E2>(self) -> impls::ErrInto<Self, I, O, E, E2>
    where
        Self: core::marker::Sized,
        E: Into<E2>,
    {
        impls::ErrInto {
            parser: self,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
            e2: Default::default(),
        }
    }

    /// Recover from an error by skipping everything `recover` consumes and trying again
    ///
    /// If `recover` consumes nothing, the error is returned, allowing an alternative recovery
    /// method.
    ///
    /// This commits the parse result, preventing alternative branch paths like with
    /// [`winnow::combinator::alt`][crate::combinator::alt].
    #[inline(always)]
    #[cfg(feature = "unstable-recover")]
    #[cfg(feature = "std")]
    fn retry_after<R>(self, recover: R) -> impls::RetryAfter<Self, R, I, O, E>
    where
        Self: core::marker::Sized,
        R: Parser<I, (), E>,
        I: Stream,
        I: Recover<E>,
        E: ParserError<I> + FromRecoverableError<I, E>,
    {
        impls::RetryAfter {
            parser: self,
            recover,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
        }
    }

    /// Recover from an error by skipping this parse and everything `recover` consumes
    ///
    /// This commits the parse result, preventing alternative branch paths like with
    /// [`winnow::combinator::alt`][crate::combinator::alt].
    #[inline(always)]
    #[cfg(feature = "unstable-recover")]
    #[cfg(feature = "std")]
    fn resume_after<R>(self, recover: R) -> impls::ResumeAfter<Self, R, I, O, E>
    where
        Self: core::marker::Sized,
        R: Parser<I, (), E>,
        I: Stream,
        I: Recover<E>,
        E: ParserError<I> + FromRecoverableError<I, E>,
    {
        impls::ResumeAfter {
            parser: self,
            recover,
            i: Default::default(),
            o: Default::default(),
            e: Default::default(),
        }
    }
}

impl<I, O, E, F> Parser<I, O, E> for F
where
    F: FnMut(&mut I) -> Result<O, E>,
    I: Stream,
{
    #[inline(always)]
    fn parse_next(&mut self, i: &mut I) -> Result<O, E> {
        self(i)
    }
}

/// This is a shortcut for [`one_of`][crate::token::one_of].
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError};
/// fn parser<'s>(i: &mut &'s [u8]) -> ModalResult<u8>  {
///     b'a'.parse_next(i)
/// }
/// assert_eq!(parser.parse_peek(&b"abc"[..]), Ok((&b"bc"[..], b'a')));
/// assert!(parser.parse_peek(&b" abc"[..]).is_err());
/// assert!(parser.parse_peek(&b"bc"[..]).is_err());
/// assert!(parser.parse_peek(&b""[..]).is_err());
/// ```
impl<I, E> Parser<I, u8, E> for u8
where
    I: StreamIsPartial,
    I: Stream,
    I: Compare<u8>,
    E: ParserError<I>,
{
    #[inline(always)]
    fn parse_next(&mut self, i: &mut I) -> Result<u8, E> {
        crate::token::literal(*self).value(*self).parse_next(i)
    }
}

/// This is a shortcut for [`one_of`][crate::token::one_of].
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError};
/// fn parser<'s>(i: &mut &'s str) -> ModalResult<char> {
///     'a'.parse_next(i)
/// }
/// assert_eq!(parser.parse_peek("abc"), Ok(("bc", 'a')));
/// assert!(parser.parse_peek(" abc").is_err());
/// assert!(parser.parse_peek("bc").is_err());
/// assert!(parser.parse_peek("").is_err());
/// ```
impl<I, E> Parser<I, char, E> for char
where
    I: StreamIsPartial,
    I: Stream,
    I: Compare<char>,
    E: ParserError<I>,
{
    #[inline(always)]
    fn parse_next(&mut self, i: &mut I) -> Result<char, E> {
        crate::token::literal(*self).value(*self).parse_next(i)
    }
}

/// This is a shortcut for [`literal`][crate::token::literal].
///
/// # Example
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::combinator::alt;
/// # use winnow::token::take;
///
/// fn parser<'s>(s: &mut &'s [u8]) -> ModalResult<&'s [u8]> {
///   alt((&"Hello"[..], take(5usize))).parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"Hello, World!"[..]), Ok((&b", World!"[..], &b"Hello"[..])));
/// assert_eq!(parser.parse_peek(&b"Something"[..]), Ok((&b"hing"[..], &b"Somet"[..])));
/// assert!(parser.parse_peek(&b"Some"[..]).is_err());
/// assert!(parser.parse_peek(&b""[..]).is_err());
/// ```
impl<'s, I, E: ParserError<I>> Parser<I, <I as Stream>::Slice, E> for &'s [u8]
where
    I: Compare<&'s [u8]> + StreamIsPartial,
    I: Stream,
{
    #[inline(always)]
    fn parse_next(&mut self, i: &mut I) -> Result<<I as Stream>::Slice, E> {
        crate::token::literal(*self).parse_next(i)
    }
}

/// This is a shortcut for [`literal`][crate::token::literal].
///
/// # Example
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::combinator::alt;
/// # use winnow::token::take;
/// use winnow::ascii::Caseless;
///
/// fn parser<'s>(s: &mut &'s [u8]) -> ModalResult<&'s [u8]> {
///   alt((Caseless(&"hello"[..]), take(5usize))).parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"Hello, World!"[..]), Ok((&b", World!"[..], &b"Hello"[..])));
/// assert_eq!(parser.parse_peek(&b"hello, World!"[..]), Ok((&b", World!"[..], &b"hello"[..])));
/// assert_eq!(parser.parse_peek(&b"HeLlo, World!"[..]), Ok((&b", World!"[..], &b"HeLlo"[..])));
/// assert_eq!(parser.parse_peek(&b"Something"[..]), Ok((&b"hing"[..], &b"Somet"[..])));
/// assert!(parser.parse_peek(&b"Some"[..]).is_err());
/// assert!(parser.parse_peek(&b""[..]).is_err());
/// ```
impl<'s, I, E: ParserError<I>> Parser<I, <I as Stream>::Slice, E> for AsciiCaseless<&'s [u8]>
where
    I: Compare<AsciiCaseless<&'s [u8]>> + StreamIsPartial,
    I: Stream,
{
    #[inline(always)]
    fn parse_next(&mut self, i: &mut I) -> Result<<I as Stream>::Slice, E> {
        crate::token::literal(*self).parse_next(i)
    }
}

/// This is a shortcut for [`literal`][crate::token::literal].
///
/// # Example
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::combinator::alt;
/// # use winnow::token::take;
///
/// fn parser<'s>(s: &mut &'s [u8]) -> ModalResult<&'s [u8]> {
///   alt((b"Hello", take(5usize))).parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"Hello, World!"[..]), Ok((&b", World!"[..], &b"Hello"[..])));
/// assert_eq!(parser.parse_peek(&b"Something"[..]), Ok((&b"hing"[..], &b"Somet"[..])));
/// assert!(parser.parse_peek(&b"Some"[..]).is_err());
/// assert!(parser.parse_peek(&b""[..]).is_err());
/// ```
impl<'s, I, E: ParserError<I>, const N: usize> Parser<I, <I as Stream>::Slice, E> for &'s [u8; N]
where
    I: Compare<&'s [u8; N]> + StreamIsPartial,
    I: Stream,
{
    #[inline(always)]
    fn parse_next(&mut self, i: &mut I) -> Result<<I as Stream>::Slice, E> {
        crate::token::literal(*self).parse_next(i)
    }
}

/// This is a shortcut for [`literal`][crate::token::literal].
///
/// # Example
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::combinator::alt;
/// # use winnow::token::take;
/// use winnow::ascii::Caseless;
///
/// fn parser<'s>(s: &mut &'s [u8]) -> ModalResult<&'s [u8]> {
///   alt((Caseless(b"hello"), take(5usize))).parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"Hello, World!"[..]), Ok((&b", World!"[..], &b"Hello"[..])));
/// assert_eq!(parser.parse_peek(&b"hello, World!"[..]), Ok((&b", World!"[..], &b"hello"[..])));
/// assert_eq!(parser.parse_peek(&b"HeLlo, World!"[..]), Ok((&b", World!"[..], &b"HeLlo"[..])));
/// assert_eq!(parser.parse_peek(&b"Something"[..]), Ok((&b"hing"[..], &b"Somet"[..])));
/// assert!(parser.parse_peek(&b"Some"[..]).is_err());
/// assert!(parser.parse_peek(&b""[..]).is_err());
/// ```
impl<'s, I, E: ParserError<I>, const N: usize> Parser<I, <I as Stream>::Slice, E>
    for AsciiCaseless<&'s [u8; N]>
where
    I: Compare<AsciiCaseless<&'s [u8; N]>> + StreamIsPartial,
    I: Stream,
{
    #[inline(always)]
    fn parse_next(&mut self, i: &mut I) -> Result<<I as Stream>::Slice, E> {
        crate::token::literal(*self).parse_next(i)
    }
}

/// This is a shortcut for [`literal`][crate::token::literal].
///
/// # Example
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError};
/// # use winnow::combinator::alt;
/// # use winnow::token::take;
///
/// fn parser<'s>(s: &mut &'s str) -> ModalResult<&'s str> {
///   alt(("Hello", take(5usize))).parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek("Hello, World!"), Ok((", World!", "Hello")));
/// assert_eq!(parser.parse_peek("Something"), Ok(("hing", "Somet")));
/// assert!(parser.parse_peek("Some").is_err());
/// assert!(parser.parse_peek("").is_err());
/// ```
impl<'s, I, E: ParserError<I>> Parser<I, <I as Stream>::Slice, E> for &'s str
where
    I: Compare<&'s str> + StreamIsPartial,
    I: Stream,
{
    #[inline(always)]
    fn parse_next(&mut self, i: &mut I) -> Result<<I as Stream>::Slice, E> {
        crate::token::literal(*self).parse_next(i)
    }
}

/// This is a shortcut for [`literal`][crate::token::literal].
///
/// # Example
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError};
/// # use winnow::combinator::alt;
/// # use winnow::token::take;
/// # use winnow::ascii::Caseless;
///
/// fn parser<'s>(s: &mut &'s str) -> ModalResult<&'s str> {
///   alt((Caseless("hello"), take(5usize))).parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek("Hello, World!"), Ok((", World!", "Hello")));
/// assert_eq!(parser.parse_peek("hello, World!"), Ok((", World!", "hello")));
/// assert_eq!(parser.parse_peek("HeLlo, World!"), Ok((", World!", "HeLlo")));
/// assert_eq!(parser.parse_peek("Something"), Ok(("hing", "Somet")));
/// assert!(parser.parse_peek("Some").is_err());
/// assert!(parser.parse_peek("").is_err());
/// ```
impl<'s, I, E: ParserError<I>> Parser<I, <I as Stream>::Slice, E> for AsciiCaseless<&'s str>
where
    I: Compare<AsciiCaseless<&'s str>> + StreamIsPartial,
    I: Stream,
{
    #[inline(always)]
    fn parse_next(&mut self, i: &mut I) -> Result<<I as Stream>::Slice, E> {
        crate::token::literal(*self).parse_next(i)
    }
}

impl<I: Stream, E: ParserError<I>> Parser<I, (), E> for () {
    #[inline(always)]
    fn parse_next(&mut self, _i: &mut I) -> Result<(), E> {
        Ok(())
    }
}

macro_rules! impl_parser_for_tuple {
  ($($index:tt $parser:ident $output:ident),+) => (
    #[allow(non_snake_case)]
    impl<I: Stream, $($output),+, E: ParserError<I>, $($parser),+> Parser<I, ($($output),+,), E> for ($($parser),+,)
    where
      $($parser: Parser<I, $output, E>),+
    {
      #[inline(always)]
      fn parse_next(&mut self, i: &mut I) -> Result<($($output),+,), E> {
        $(let $output = self.$index.parse_next(i)?;)+

        Ok(($($output),+,))
      }
    }
  )
}

macro_rules! impl_parser_for_tuples {
    ($index1:tt $parser1:ident $output1:ident, $($index:tt $parser:ident $output:ident),+) => {
        impl_parser_for_tuples!(__impl $index1 $parser1 $output1; $($index $parser $output),+);
    };
    (__impl $($index:tt $parser:ident $output:ident),+; $index1:tt $parser1:ident $output1:ident $(,$index2:tt $parser2:ident $output2:ident)*) => {
        impl_parser_for_tuple!($($index $parser $output),+);
        impl_parser_for_tuples!(__impl $($index $parser $output),+, $index1 $parser1 $output1; $($index2 $parser2 $output2),*);
    };
    (__impl $($index:tt $parser:ident $output:ident),+;) => {
        impl_parser_for_tuple!($($index $parser $output),+);
    }
}

impl_parser_for_tuples!(
  0 P0 O0,
  1 P1 O1,
  2 P2 O2,
  3 P3 O3,
  4 P4 O4,
  5 P5 O5,
  6 P6 O6,
  7 P7 O7,
  8 P8 O8,
  9 P9 O9,
  10 P10 O10,
  11 P11 O11,
  12 P12 O12,
  13 P13 O13,
  14 P14 O14,
  15 P15 O15,
  16 P16 O16,
  17 P17 O17,
  18 P18 O18,
  19 P19 O19,
  20 P20 O20,
  21 P21 O21
);

#[cfg(feature = "alloc")]
use crate::lib::std::boxed::Box;

#[cfg(feature = "alloc")]
impl<I, O, E> Parser<I, O, E> for Box<dyn Parser<I, O, E> + '_> {
    #[inline(always)]
    fn parse_next(&mut self, i: &mut I) -> Result<O, E> {
        (**self).parse_next(i)
    }
}

/// Trait alias for [`Parser`] to be used with [`ModalResult`][crate::error::ModalResult]
pub trait ModalParser<I, O, E>: Parser<I, O, crate::error::ErrMode<E>> {}

impl<I, O, E, P> ModalParser<I, O, E> for P where P: Parser<I, O, crate::error::ErrMode<E>> {}

/// Collect all errors when parsing the input
///
/// [`Parser`]s will need to use [`Recoverable<I, _>`] for their input.
#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
pub trait RecoverableParser<I, O, R, E> {
    /// Collect all errors when parsing the input
    ///
    /// If `self` fails, this acts like [`Parser::resume_after`] and returns `Ok(None)`.
    /// Generally, this should be avoided by using
    /// [`Parser::retry_after`] and [`Parser::resume_after`] throughout your parser.
    ///
    /// The empty `input` is returned to allow turning the errors into [`ParserError`]s.
    fn recoverable_parse(&mut self, input: I) -> (I, Option<O>, Vec<R>);
}

#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
impl<P, I, O, R, E> RecoverableParser<I, O, R, E> for P
where
    P: Parser<Recoverable<I, R>, O, E>,
    I: Stream,
    I: StreamIsPartial,
    R: FromRecoverableError<Recoverable<I, R>, E>,
    R: crate::lib::std::fmt::Debug,
    E: FromRecoverableError<Recoverable<I, R>, E>,
    E: ParserError<Recoverable<I, R>>,
    E: crate::lib::std::fmt::Debug,
{
    fn recoverable_parse(&mut self, input: I) -> (I, Option<O>, Vec<R>) {
        debug_assert!(
            !I::is_partial_supported(),
            "partial streams need to handle `ErrMode::Incomplete`"
        );

        let start = input.checkpoint();
        let mut input = Recoverable::new(input);
        let start_token = input.checkpoint();
        let result = (
            self.by_ref(),
            crate::combinator::eof.resume_after(crate::token::rest.void()),
        )
            .parse_next(&mut input);

        let (o, err) = match result {
            Ok((o, _)) => (Some(o), None),
            Err(err) => {
                let err_start = input.checkpoint();
                let err = R::from_recoverable_error(&start_token, &err_start, &input, err);
                (None, Some(err))
            }
        };

        let (mut input, mut errs) = input.into_parts();
        input.reset(&start);
        if let Some(err) = err {
            errs.push(err);
        }

        (input, o, errs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use snapbox::prelude::*;
    use snapbox::str;

    use crate::binary::be_u16;
    use crate::error::ErrMode;
    use crate::error::Needed;
    use crate::error::TestResult;
    use crate::token::take;
    use crate::Partial;

    #[doc(hidden)]
    #[macro_export]
    macro_rules! assert_size (
    ($t:ty, $sz:expr) => (
      assert!($crate::lib::std::mem::size_of::<$t>() <= $sz, "{} <= {} failed", $crate::lib::std::mem::size_of::<$t>(), $sz);
    );
  );

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn size_test() {
        assert_size!(Result<&[u8], (&[u8], u32)>, 40);
        assert_size!(Result<&str, u32>, 40);
        assert_size!(Needed, 8);
        assert_size!(ErrMode<u32>, 16);
    }

    #[test]
    fn err_map_test() {
        let e = ErrMode::Backtrack(1);
        assert_eq!(e.map(|v| v + 1), ErrMode::Backtrack(2));
    }

    #[test]
    fn single_element_tuples() {
        use crate::ascii::alpha1;

        let mut parser = (alpha1,);
        assert_parse!(
            parser.parse_peek("abc123def"),
            str![[r#"
Ok(
    (
        "123def",
        (
            "abc",
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            parser.parse_peek("123def"),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: "123def",
        },
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn tuple_test() {
        #[allow(clippy::type_complexity)]
        fn tuple_3<'i>(
            i: &mut Partial<&'i [u8]>,
        ) -> TestResult<Partial<&'i [u8]>, (u16, &'i [u8], &'i [u8])> {
            (be_u16, take(3u8), "fg").parse_next(i)
        }

        assert_parse!(
            tuple_3.parse_peek(Partial::new(&b"abcdefgh"[..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                104,
            ],
            partial: true,
        },
        (
            24930,
            [
                99,
                100,
                101,
            ],
            [
                102,
                103,
            ],
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            tuple_3.parse_peek(Partial::new(&b"abcd"[..])),
            str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            tuple_3.parse_peek(Partial::new(&b"abcde"[..])),
            str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            tuple_3.parse_peek(Partial::new(&b"abcdejk"[..])),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    106,
                    107,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn unit_type() {
        fn parser<'i>(i: &mut &'i str) -> TestResult<&'i str, ()> {
            ().parse_next(i)
        }
        assert_parse!(
            parser.parse_peek("abxsbsh"),
            str![[r#"
Ok(
    (
        "abxsbsh",
        (),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            parser.parse_peek("sdfjakdsas"),
            str![[r#"
Ok(
    (
        "sdfjakdsas",
        (),
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            parser.parse_peek(""),
            str![[r#"
Ok(
    (
        "",
        (),
    ),
)

"#]]
            .raw()
        );
    }
}

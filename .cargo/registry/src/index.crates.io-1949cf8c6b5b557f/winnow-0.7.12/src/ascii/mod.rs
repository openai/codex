//! Character specific parsers and combinators
//!
//! Functions recognizing specific characters

#[cfg(test)]
mod tests;

use crate::lib::std::ops::{Add, Shl};

use crate::combinator::alt;
use crate::combinator::dispatch;
use crate::combinator::empty;
use crate::combinator::fail;
use crate::combinator::opt;
use crate::combinator::peek;
use crate::combinator::trace;
use crate::error::Needed;
use crate::error::ParserError;
use crate::stream::FindSlice;
use crate::stream::{AsBStr, AsChar, ParseSlice, Stream, StreamIsPartial};
use crate::stream::{Compare, CompareResult};
use crate::token::any;
use crate::token::one_of;
use crate::token::take_until;
use crate::token::take_while;
use crate::Parser;
use crate::Result;

/// Mark a value as case-insensitive for ASCII characters
///
/// # Example
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::Caseless;
///
/// fn parser<'s>(s: &mut &'s str) -> ModalResult<&'s str> {
///   Caseless("hello").parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek("Hello, World!"), Ok((", World!", "Hello")));
/// assert_eq!(parser.parse_peek("hello, World!"), Ok((", World!", "hello")));
/// assert_eq!(parser.parse_peek("HeLlo, World!"), Ok((", World!", "HeLlo")));
/// assert!(parser.parse_peek("Some").is_err());
/// assert!(parser.parse_peek("").is_err());
/// ```
#[derive(Copy, Clone, Debug)]
pub struct Caseless<T>(pub T);

impl Caseless<&str> {
    /// Get the byte-representation of this case-insensitive value
    #[inline(always)]
    pub fn as_bytes(&self) -> Caseless<&[u8]> {
        Caseless(self.0.as_bytes())
    }
}

/// Recognizes the string `"\r\n"`.
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data.
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn crlf<'i>(input: &mut &'i str) -> ModalResult<&'i str>
/// # {
/// #     winnow::ascii::crlf.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::crlf;
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
///     crlf.parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("\r\nc"), Ok(("c", "\r\n")));
/// assert!(parser.parse_peek("ab\r\nc").is_err());
/// assert!(parser.parse_peek("").is_err());
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::ascii::crlf;
/// assert_eq!(crlf::<_, ErrMode<ContextError>>.parse_peek(Partial::new("\r\nc")), Ok((Partial::new("c"), "\r\n")));
/// assert!(crlf::<_, ErrMode<ContextError>>.parse_peek(Partial::new("ab\r\nc")).is_err());
/// assert_eq!(crlf::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// ```
#[inline(always)]
pub fn crlf<Input, Error>(input: &mut Input) -> Result<<Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream + Compare<&'static str>,
    Error: ParserError<Input>,
{
    trace("crlf", "\r\n").parse_next(input)
}

/// Recognizes a string of 0+ characters until `"\r\n"`, `"\n"`, or eof.
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data.
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn till_line_ending<'i>(input: &mut &'i str) -> ModalResult<&'i str>
/// # {
/// #     winnow::ascii::till_line_ending.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::till_line_ending;
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
///     till_line_ending.parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("ab\r\nc"), Ok(("\r\nc", "ab")));
/// assert_eq!(parser.parse_peek("ab\nc"), Ok(("\nc", "ab")));
/// assert_eq!(parser.parse_peek("abc"), Ok(("", "abc")));
/// assert_eq!(parser.parse_peek(""), Ok(("", "")));
/// assert!(parser.parse_peek("a\rb\nc").is_err());
/// assert!(parser.parse_peek("a\rbc").is_err());
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::ascii::till_line_ending;
/// assert_eq!(till_line_ending::<_, ErrMode<ContextError>>.parse_peek(Partial::new("ab\r\nc")), Ok((Partial::new("\r\nc"), "ab")));
/// assert_eq!(till_line_ending::<_, ErrMode<ContextError>>.parse_peek(Partial::new("abc")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// assert_eq!(till_line_ending::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// assert!(till_line_ending::<_, ErrMode<ContextError>>.parse_peek(Partial::new("a\rb\nc")).is_err());
/// assert!(till_line_ending::<_, ErrMode<ContextError>>.parse_peek(Partial::new("a\rbc")).is_err());
/// ```
#[inline(always)]
pub fn till_line_ending<Input, Error>(input: &mut Input) -> Result<<Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream + Compare<&'static str> + FindSlice<(char, char)>,
    <Input as Stream>::Token: AsChar + Clone,
    Error: ParserError<Input>,
{
    trace("till_line_ending", move |input: &mut Input| {
        if <Input as StreamIsPartial>::is_partial_supported() {
            till_line_ending_::<_, _, true>(input)
        } else {
            till_line_ending_::<_, _, false>(input)
        }
    })
    .parse_next(input)
}

fn till_line_ending_<I, E: ParserError<I>, const PARTIAL: bool>(
    input: &mut I,
) -> Result<<I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream,
    I: Compare<&'static str>,
    I: FindSlice<(char, char)>,
    <I as Stream>::Token: AsChar + Clone,
{
    let res = match take_until(0.., ('\r', '\n'))
        .parse_next(input)
        .map_err(|e: E| e)
    {
        Ok(slice) => slice,
        Err(err) if err.is_backtrack() => input.finish(),
        Err(err) => {
            return Err(err);
        }
    };
    if matches!(input.compare("\r"), CompareResult::Ok(_)) {
        let comp = input.compare("\r\n");
        match comp {
            CompareResult::Ok(_) => {}
            CompareResult::Incomplete if PARTIAL && input.is_partial() => {
                return Err(ParserError::incomplete(input, Needed::Unknown));
            }
            CompareResult::Incomplete | CompareResult::Error => {
                return Err(ParserError::from_input(input));
            }
        }
    }
    Ok(res)
}

/// Recognizes an end of line (both `"\n"` and `"\r\n"`).
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data.
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn line_ending<'i>(input: &mut &'i str) -> ModalResult<&'i str>
/// # {
/// #     winnow::ascii::line_ending.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::line_ending;
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
///     line_ending.parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("\r\nc"), Ok(("c", "\r\n")));
/// assert!(parser.parse_peek("ab\r\nc").is_err());
/// assert!(parser.parse_peek("").is_err());
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::ascii::line_ending;
/// assert_eq!(line_ending::<_, ErrMode<ContextError>>.parse_peek(Partial::new("\r\nc")), Ok((Partial::new("c"), "\r\n")));
/// assert!(line_ending::<_, ErrMode<ContextError>>.parse_peek(Partial::new("ab\r\nc")).is_err());
/// assert_eq!(line_ending::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// ```
#[inline(always)]
pub fn line_ending<Input, Error>(input: &mut Input) -> Result<<Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream + Compare<&'static str>,
    Error: ParserError<Input>,
{
    trace("line_ending", alt(("\n", "\r\n"))).parse_next(input)
}

/// Matches a newline character `'\n'`.
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data.
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn newline(input: &mut &str) -> ModalResult<char>
/// # {
/// #     winnow::ascii::newline.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::newline;
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<char> {
///     newline.parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("\nc"), Ok(("c", '\n')));
/// assert!(parser.parse_peek("\r\nc").is_err());
/// assert!(parser.parse_peek("").is_err());
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::ascii::newline;
/// assert_eq!(newline::<_, ErrMode<ContextError>>.parse_peek(Partial::new("\nc")), Ok((Partial::new("c"), '\n')));
/// assert!(newline::<_, ErrMode<ContextError>>.parse_peek(Partial::new("\r\nc")).is_err());
/// assert_eq!(newline::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// ```
#[inline(always)]
pub fn newline<I, Error: ParserError<I>>(input: &mut I) -> Result<char, Error>
where
    I: StreamIsPartial,
    I: Stream,
    I: Compare<char>,
{
    trace("newline", '\n').parse_next(input)
}

/// Matches a tab character `'\t'`.
///
/// *Complete version*: Will return an error if there's not enough input data.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data.
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn tab(input: &mut &str) -> ModalResult<char>
/// # {
/// #     winnow::ascii::tab.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::tab;
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<char> {
///     tab.parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("\tc"), Ok(("c", '\t')));
/// assert!(parser.parse_peek("\r\nc").is_err());
/// assert!(parser.parse_peek("").is_err());
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::ascii::tab;
/// assert_eq!(tab::<_, ErrMode<ContextError>>.parse_peek(Partial::new("\tc")), Ok((Partial::new("c"), '\t')));
/// assert!(tab::<_, ErrMode<ContextError>>.parse_peek(Partial::new("\r\nc")).is_err());
/// assert_eq!(tab::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// ```
#[inline(always)]
pub fn tab<Input, Error>(input: &mut Input) -> Result<char, Error>
where
    Input: StreamIsPartial + Stream + Compare<char>,
    Error: ParserError<Input>,
{
    trace("tab", '\t').parse_next(input)
}

/// Recognizes zero or more lowercase and uppercase ASCII alphabetic characters: `'a'..='z'`, `'A'..='Z'`
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non
/// alphabetic character).
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non alphabetic character).
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn alpha0<'i>(input: &mut &'i str) -> ModalResult<&'i str>
/// # {
/// #     winnow::ascii::alpha0.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::alpha0;
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
///     alpha0.parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("ab1c"), Ok(("1c", "ab")));
/// assert_eq!(parser.parse_peek("1c"), Ok(("1c", "")));
/// assert_eq!(parser.parse_peek(""), Ok(("", "")));
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::ascii::alpha0;
/// assert_eq!(alpha0::<_, ErrMode<ContextError>>.parse_peek(Partial::new("ab1c")), Ok((Partial::new("1c"), "ab")));
/// assert_eq!(alpha0::<_, ErrMode<ContextError>>.parse_peek(Partial::new("1c")), Ok((Partial::new("1c"), "")));
/// assert_eq!(alpha0::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn alpha0<Input, Error>(input: &mut Input) -> Result<<Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream,
    <Input as Stream>::Token: AsChar,
    Error: ParserError<Input>,
{
    trace("alpha0", take_while(0.., AsChar::is_alpha)).parse_next(input)
}

/// Recognizes one or more lowercase and uppercase ASCII alphabetic characters: `'a'..='z'`, `'A'..='Z'`
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found  (a non alphabetic character).
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non alphabetic character).
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn alpha1<'i>(input: &mut &'i str) -> ModalResult<&'i str>
/// # {
/// #     winnow::ascii::alpha1.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::alpha1;
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
///     alpha1.parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("aB1c"), Ok(("1c", "aB")));
/// assert!(parser.parse_peek("1c").is_err());
/// assert!(parser.parse_peek("").is_err());
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::ascii::alpha1;
/// assert_eq!(alpha1::<_, ErrMode<ContextError>>.parse_peek(Partial::new("aB1c")), Ok((Partial::new("1c"), "aB")));
/// assert!(alpha1::<_, ErrMode<ContextError>>.parse_peek(Partial::new("1c")).is_err());
/// assert_eq!(alpha1::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn alpha1<Input, Error>(input: &mut Input) -> Result<<Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream,
    <Input as Stream>::Token: AsChar,
    Error: ParserError<Input>,
{
    trace("alpha1", take_while(1.., AsChar::is_alpha)).parse_next(input)
}

/// Recognizes zero or more ASCII numerical characters: `'0'..='9'`
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non digit character).
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non digit character).
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn digit0<'i>(input: &mut &'i str) -> ModalResult<&'i str>
/// # {
/// #     winnow::ascii::digit0.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::digit0;
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
///     digit0.parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("21c"), Ok(("c", "21")));
/// assert_eq!(parser.parse_peek("21"), Ok(("", "21")));
/// assert_eq!(parser.parse_peek("a21c"), Ok(("a21c", "")));
/// assert_eq!(parser.parse_peek(""), Ok(("", "")));
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::ascii::digit0;
/// assert_eq!(digit0::<_, ErrMode<ContextError>>.parse_peek(Partial::new("21c")), Ok((Partial::new("c"), "21")));
/// assert_eq!(digit0::<_, ErrMode<ContextError>>.parse_peek(Partial::new("a21c")), Ok((Partial::new("a21c"), "")));
/// assert_eq!(digit0::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn digit0<Input, Error>(input: &mut Input) -> Result<<Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream,
    <Input as Stream>::Token: AsChar,
    Error: ParserError<Input>,
{
    trace("digit0", take_while(0.., AsChar::is_dec_digit)).parse_next(input)
}

/// Recognizes one or more ASCII numerical characters: `'0'..='9'`
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non digit character).
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non digit character).
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn digit1<'i>(input: &mut &'i str) -> ModalResult<&'i str>
/// # {
/// #     winnow::ascii::digit1.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::digit1;
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
///     digit1.parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("21c"), Ok(("c", "21")));
/// assert!(parser.parse_peek("c1").is_err());
/// assert!(parser.parse_peek("").is_err());
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::ascii::digit1;
/// assert_eq!(digit1::<_, ErrMode<ContextError>>.parse_peek(Partial::new("21c")), Ok((Partial::new("c"), "21")));
/// assert!(digit1::<_, ErrMode<ContextError>>.parse_peek(Partial::new("c1")).is_err());
/// assert_eq!(digit1::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
///
/// ## Parsing an integer
///
/// You can use `digit1` in combination with [`Parser::try_map`] to parse an integer:
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::digit1;
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<u32> {
///   digit1.try_map(str::parse).parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("416"), Ok(("", 416)));
/// assert_eq!(parser.parse_peek("12b"), Ok(("b", 12)));
/// assert!(parser.parse_peek("b").is_err());
/// ```
#[inline(always)]
pub fn digit1<Input, Error>(input: &mut Input) -> Result<<Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream,
    <Input as Stream>::Token: AsChar,
    Error: ParserError<Input>,
{
    trace("digit1", take_while(1.., AsChar::is_dec_digit)).parse_next(input)
}

/// Recognizes zero or more ASCII hexadecimal numerical characters: `'0'..='9'`, `'A'..='F'`,
/// `'a'..='f'`
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non hexadecimal digit character).
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non hexadecimal digit character).
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn hex_digit0<'i>(input: &mut &'i str) -> ModalResult<&'i str>
/// # {
/// #     winnow::ascii::hex_digit0.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::hex_digit0;
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
///     hex_digit0.parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("21cZ"), Ok(("Z", "21c")));
/// assert_eq!(parser.parse_peek("Z21c"), Ok(("Z21c", "")));
/// assert_eq!(parser.parse_peek(""), Ok(("", "")));
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::ascii::hex_digit0;
/// assert_eq!(hex_digit0::<_, ErrMode<ContextError>>.parse_peek(Partial::new("21cZ")), Ok((Partial::new("Z"), "21c")));
/// assert_eq!(hex_digit0::<_, ErrMode<ContextError>>.parse_peek(Partial::new("Z21c")), Ok((Partial::new("Z21c"), "")));
/// assert_eq!(hex_digit0::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn hex_digit0<Input, Error>(input: &mut Input) -> Result<<Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream,
    <Input as Stream>::Token: AsChar,
    Error: ParserError<Input>,
{
    trace("hex_digit0", take_while(0.., AsChar::is_hex_digit)).parse_next(input)
}

/// Recognizes one or more ASCII hexadecimal numerical characters: `'0'..='9'`, `'A'..='F'`,
/// `'a'..='f'`
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non hexadecimal digit character).
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non hexadecimal digit character).
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn hex_digit1<'i>(input: &mut &'i str) -> ModalResult<&'i str>
/// # {
/// #     winnow::ascii::hex_digit1.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::hex_digit1;
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
///     hex_digit1.parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("21cZ"), Ok(("Z", "21c")));
/// assert!(parser.parse_peek("H2").is_err());
/// assert!(parser.parse_peek("").is_err());
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::ascii::hex_digit1;
/// assert_eq!(hex_digit1::<_, ErrMode<ContextError>>.parse_peek(Partial::new("21cZ")), Ok((Partial::new("Z"), "21c")));
/// assert!(hex_digit1::<_, ErrMode<ContextError>>.parse_peek(Partial::new("H2")).is_err());
/// assert_eq!(hex_digit1::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn hex_digit1<Input, Error>(input: &mut Input) -> Result<<Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream,
    <Input as Stream>::Token: AsChar,
    Error: ParserError<Input>,
{
    trace("hex_digit1", take_while(1.., AsChar::is_hex_digit)).parse_next(input)
}

/// Recognizes zero or more octal characters: `'0'..='7'`
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non octal
/// digit character).
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non octal digit character).
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn oct_digit0<'i>(input: &mut &'i str) -> ModalResult<&'i str>
/// # {
/// #     winnow::ascii::oct_digit0.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::oct_digit0;
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
///     oct_digit0.parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("21cZ"), Ok(("cZ", "21")));
/// assert_eq!(parser.parse_peek("Z21c"), Ok(("Z21c", "")));
/// assert_eq!(parser.parse_peek(""), Ok(("", "")));
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::ascii::oct_digit0;
/// assert_eq!(oct_digit0::<_, ErrMode<ContextError>>.parse_peek(Partial::new("21cZ")), Ok((Partial::new("cZ"), "21")));
/// assert_eq!(oct_digit0::<_, ErrMode<ContextError>>.parse_peek(Partial::new("Z21c")), Ok((Partial::new("Z21c"), "")));
/// assert_eq!(oct_digit0::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn oct_digit0<Input, Error>(input: &mut Input) -> Result<<Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial,
    Input: Stream,
    <Input as Stream>::Token: AsChar,
    Error: ParserError<Input>,
{
    trace("oct_digit0", take_while(0.., AsChar::is_oct_digit)).parse_next(input)
}

/// Recognizes one or more octal characters: `'0'..='7'`
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non octal digit character).
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non octal digit character).
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn oct_digit1<'i>(input: &mut &'i str) -> ModalResult<&'i str>
/// # {
/// #     winnow::ascii::oct_digit1.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::oct_digit1;
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
///     oct_digit1.parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("21cZ"), Ok(("cZ", "21")));
/// assert!(parser.parse_peek("H2").is_err());
/// assert!(parser.parse_peek("").is_err());
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::ascii::oct_digit1;
/// assert_eq!(oct_digit1::<_, ErrMode<ContextError>>.parse_peek(Partial::new("21cZ")), Ok((Partial::new("cZ"), "21")));
/// assert!(oct_digit1::<_, ErrMode<ContextError>>.parse_peek(Partial::new("H2")).is_err());
/// assert_eq!(oct_digit1::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn oct_digit1<Input, Error>(input: &mut Input) -> Result<<Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream,
    <Input as Stream>::Token: AsChar,
    Error: ParserError<Input>,
{
    trace("oct_digit0", take_while(1.., AsChar::is_oct_digit)).parse_next(input)
}

/// Recognizes zero or more ASCII numerical and alphabetic characters: `'a'..='z'`, `'A'..='Z'`, `'0'..='9'`
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non
/// alphanumerical character).
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non alphanumerical character).
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn alphanumeric0<'i>(input: &mut &'i str) -> ModalResult<&'i str>
/// # {
/// #     winnow::ascii::alphanumeric0.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::alphanumeric0;
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
///     alphanumeric0.parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("21cZ%1"), Ok(("%1", "21cZ")));
/// assert_eq!(parser.parse_peek("&Z21c"), Ok(("&Z21c", "")));
/// assert_eq!(parser.parse_peek(""), Ok(("", "")));
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::ascii::alphanumeric0;
/// assert_eq!(alphanumeric0::<_, ErrMode<ContextError>>.parse_peek(Partial::new("21cZ%1")), Ok((Partial::new("%1"), "21cZ")));
/// assert_eq!(alphanumeric0::<_, ErrMode<ContextError>>.parse_peek(Partial::new("&Z21c")), Ok((Partial::new("&Z21c"), "")));
/// assert_eq!(alphanumeric0::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn alphanumeric0<Input, Error>(input: &mut Input) -> Result<<Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream,
    <Input as Stream>::Token: AsChar,
    Error: ParserError<Input>,
{
    trace("alphanumeric0", take_while(0.., AsChar::is_alphanum)).parse_next(input)
}

/// Recognizes one or more ASCII numerical and alphabetic characters: `'a'..='z'`, `'A'..='Z'`, `'0'..='9'`
///
/// *Complete version*: Will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non alphanumerical character).
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non alphanumerical character).
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn alphanumeric1<'i>(input: &mut &'i str) -> ModalResult<&'i str>
/// # {
/// #     winnow::ascii::alphanumeric1.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::alphanumeric1;
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
///     alphanumeric1.parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("21cZ%1"), Ok(("%1", "21cZ")));
/// assert!(parser.parse_peek("&H2").is_err());
/// assert!(parser.parse_peek("").is_err());
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::ascii::alphanumeric1;
/// assert_eq!(alphanumeric1::<_, ErrMode<ContextError>>.parse_peek(Partial::new("21cZ%1")), Ok((Partial::new("%1"), "21cZ")));
/// assert!(alphanumeric1::<_, ErrMode<ContextError>>.parse_peek(Partial::new("&H2")).is_err());
/// assert_eq!(alphanumeric1::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn alphanumeric1<Input, Error>(input: &mut Input) -> Result<<Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream,
    <Input as Stream>::Token: AsChar,
    Error: ParserError<Input>,
{
    trace("alphanumeric1", take_while(1.., AsChar::is_alphanum)).parse_next(input)
}

/// Recognizes zero or more spaces and tabs.
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non space
/// character).
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non space character).
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn space0<'i>(input: &mut &'i str) -> ModalResult<&'i str>
/// # {
/// #     winnow::ascii::space0.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::ascii::space0;
/// assert_eq!(space0::<_, ErrMode<ContextError>>.parse_peek(Partial::new(" \t21c")), Ok((Partial::new("21c"), " \t")));
/// assert_eq!(space0::<_, ErrMode<ContextError>>.parse_peek(Partial::new("Z21c")), Ok((Partial::new("Z21c"), "")));
/// assert_eq!(space0::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn space0<Input, Error>(input: &mut Input) -> Result<<Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream,
    <Input as Stream>::Token: AsChar,
    Error: ParserError<Input>,
{
    trace("space0", take_while(0.., AsChar::is_space)).parse_next(input)
}

/// Recognizes one or more spaces and tabs.
///
/// *Complete version*: Will return the whole input if no terminating token is found (a non space
/// character).
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non space character).
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn space1<'i>(input: &mut &'i str) -> ModalResult<&'i str>
/// # {
/// #     winnow::ascii::space1.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::space1;
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
///     space1.parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek(" \t21c"), Ok(("21c", " \t")));
/// assert!(parser.parse_peek("H2").is_err());
/// assert!(parser.parse_peek("").is_err());
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::ascii::space1;
/// assert_eq!(space1::<_, ErrMode<ContextError>>.parse_peek(Partial::new(" \t21c")), Ok((Partial::new("21c"), " \t")));
/// assert!(space1::<_, ErrMode<ContextError>>.parse_peek(Partial::new("H2")).is_err());
/// assert_eq!(space1::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn space1<Input, Error>(input: &mut Input) -> Result<<Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream,
    <Input as Stream>::Token: AsChar,
    Error: ParserError<Input>,
{
    trace("space1", take_while(1.., AsChar::is_space)).parse_next(input)
}

/// Recognizes zero or more spaces, tabs, carriage returns and line feeds.
///
/// *Complete version*: will return the whole input if no terminating token is found (a non space
/// character).
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non space character).
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn multispace0<'i>(input: &mut &'i str) -> ModalResult<&'i str>
/// # {
/// #     winnow::ascii::multispace0.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::multispace0;
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
///     multispace0.parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek(" \t\n\r21c"), Ok(("21c", " \t\n\r")));
/// assert_eq!(parser.parse_peek("Z21c"), Ok(("Z21c", "")));
/// assert_eq!(parser.parse_peek(""), Ok(("", "")));
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::ascii::multispace0;
/// assert_eq!(multispace0::<_, ErrMode<ContextError>>.parse_peek(Partial::new(" \t\n\r21c")), Ok((Partial::new("21c"), " \t\n\r")));
/// assert_eq!(multispace0::<_, ErrMode<ContextError>>.parse_peek(Partial::new("Z21c")), Ok((Partial::new("Z21c"), "")));
/// assert_eq!(multispace0::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn multispace0<Input, Error>(input: &mut Input) -> Result<<Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream,
    <Input as Stream>::Token: AsChar + Clone,
    Error: ParserError<Input>,
{
    trace("multispace0", take_while(0.., (' ', '\t', '\r', '\n'))).parse_next(input)
}

/// Recognizes one or more spaces, tabs, carriage returns and line feeds.
///
/// *Complete version*: will return an error if there's not enough input data,
/// or the whole input if no terminating token is found (a non space character).
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data,
/// or if no terminating token is found (a non space character).
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn multispace1<'i>(input: &mut &'i str) -> ModalResult<&'i str>
/// # {
/// #     winnow::ascii::multispace1.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::multispace1;
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
///     multispace1.parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek(" \t\n\r21c"), Ok(("21c", " \t\n\r")));
/// assert!(parser.parse_peek("H2").is_err());
/// assert!(parser.parse_peek("").is_err());
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::ascii::multispace1;
/// assert_eq!(multispace1::<_, ErrMode<ContextError>>.parse_peek(Partial::new(" \t\n\r21c")), Ok((Partial::new("21c"), " \t\n\r")));
/// assert!(multispace1::<_, ErrMode<ContextError>>.parse_peek(Partial::new("H2")).is_err());
/// assert_eq!(multispace1::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn multispace1<Input, Error>(input: &mut Input) -> Result<<Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream,
    <Input as Stream>::Token: AsChar + Clone,
    Error: ParserError<Input>,
{
    trace("multispace1", take_while(1.., (' ', '\t', '\r', '\n'))).parse_next(input)
}

/// Decode a decimal unsigned integer (e.g. [`u32`])
///
/// *Complete version*: can parse until the end of input.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data.
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream] into a `u32`:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn dec_uint(input: &mut &str) -> ModalResult<u32>
/// # {
/// #     winnow::ascii::dec_uint.parse_next(input)
/// # }
/// ```
#[doc(alias = "u8")]
#[doc(alias = "u16")]
#[doc(alias = "u32")]
#[doc(alias = "u64")]
#[doc(alias = "u128")]
pub fn dec_uint<Input, Output, Error>(input: &mut Input) -> Result<Output, Error>
where
    Input: StreamIsPartial + Stream,
    <Input as Stream>::Slice: AsBStr,
    <Input as Stream>::Token: AsChar + Clone,
    Output: Uint,
    Error: ParserError<Input>,
{
    trace("dec_uint", move |input: &mut Input| {
        alt(((one_of('1'..='9'), digit0).void(), one_of('0').void()))
            .take()
            .verify_map(|s: <Input as Stream>::Slice| {
                let s = s.as_bstr();
                // SAFETY: Only 7-bit ASCII characters are parsed
                let s = unsafe { crate::lib::std::str::from_utf8_unchecked(s) };
                Output::try_from_dec_uint(s)
            })
            .parse_next(input)
    })
    .parse_next(input)
}

/// Metadata for parsing unsigned integers, see [`dec_uint`]
pub trait Uint: Sized {
    #[doc(hidden)]
    fn try_from_dec_uint(slice: &str) -> Option<Self>;
}

impl Uint for u8 {
    fn try_from_dec_uint(slice: &str) -> Option<Self> {
        slice.parse().ok()
    }
}

impl Uint for u16 {
    fn try_from_dec_uint(slice: &str) -> Option<Self> {
        slice.parse().ok()
    }
}

impl Uint for u32 {
    fn try_from_dec_uint(slice: &str) -> Option<Self> {
        slice.parse().ok()
    }
}

impl Uint for u64 {
    fn try_from_dec_uint(slice: &str) -> Option<Self> {
        slice.parse().ok()
    }
}

impl Uint for u128 {
    fn try_from_dec_uint(slice: &str) -> Option<Self> {
        slice.parse().ok()
    }
}

impl Uint for usize {
    fn try_from_dec_uint(slice: &str) -> Option<Self> {
        slice.parse().ok()
    }
}

/// Decode a decimal signed integer (e.g. [`i32`])
///
/// *Complete version*: can parse until the end of input.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there's not enough input data.
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream] into an `i32`:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn dec_int(input: &mut &str) -> ModalResult<i32>
/// # {
/// #     winnow::ascii::dec_int.parse_next(input)
/// # }
/// ```
#[doc(alias = "i8")]
#[doc(alias = "i16")]
#[doc(alias = "i32")]
#[doc(alias = "i64")]
#[doc(alias = "i128")]
pub fn dec_int<Input, Output, Error>(input: &mut Input) -> Result<Output, Error>
where
    Input: StreamIsPartial + Stream,
    <Input as Stream>::Slice: AsBStr,
    <Input as Stream>::Token: AsChar + Clone,
    Output: Int,
    Error: ParserError<Input>,
{
    trace("dec_int", move |input: &mut Input| {
        let sign = opt(dispatch! {any.map(AsChar::as_char);
            '+' => empty.value(true),
            '-' => empty.value(false),
            _ => fail,
        });
        alt(((sign, one_of('1'..='9'), digit0).void(), one_of('0').void()))
            .take()
            .verify_map(|s: <Input as Stream>::Slice| {
                let s = s.as_bstr();
                // SAFETY: Only 7-bit ASCII characters are parsed
                let s = unsafe { crate::lib::std::str::from_utf8_unchecked(s) };
                Output::try_from_dec_int(s)
            })
            .parse_next(input)
    })
    .parse_next(input)
}

/// Metadata for parsing signed integers, see [`dec_int`]
pub trait Int: Sized {
    #[doc(hidden)]
    fn try_from_dec_int(slice: &str) -> Option<Self>;
}

impl Int for i8 {
    fn try_from_dec_int(slice: &str) -> Option<Self> {
        slice.parse().ok()
    }
}

impl Int for i16 {
    fn try_from_dec_int(slice: &str) -> Option<Self> {
        slice.parse().ok()
    }
}

impl Int for i32 {
    fn try_from_dec_int(slice: &str) -> Option<Self> {
        slice.parse().ok()
    }
}

impl Int for i64 {
    fn try_from_dec_int(slice: &str) -> Option<Self> {
        slice.parse().ok()
    }
}

impl Int for i128 {
    fn try_from_dec_int(slice: &str) -> Option<Self> {
        slice.parse().ok()
    }
}

impl Int for isize {
    fn try_from_dec_int(slice: &str) -> Option<Self> {
        slice.parse().ok()
    }
}

/// Decode a variable-width hexadecimal integer (e.g. [`u32`])
///
/// *Complete version*: Will parse until the end of input if it has fewer characters than the type
/// supports.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if end-of-input
/// is hit before a hard boundary (non-hex character, more characters than supported).
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream] into a `u32`:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn hex_uint(input: &mut &str) -> ModalResult<u32>
/// # {
/// #     winnow::ascii::hex_uint.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// use winnow::ascii::hex_uint;
///
/// fn parser<'s>(s: &mut &'s [u8]) -> ModalResult<u32> {
///   hex_uint(s)
/// }
///
/// assert_eq!(parser.parse_peek(&b"01AE"[..]), Ok((&b""[..], 0x01AE)));
/// assert_eq!(parser.parse_peek(&b"abc"[..]), Ok((&b""[..], 0x0ABC)));
/// assert!(parser.parse_peek(&b"ggg"[..]).is_err());
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::Partial;
/// use winnow::ascii::hex_uint;
///
/// fn parser<'s>(s: &mut Partial<&'s [u8]>) -> ModalResult<u32> {
///   hex_uint(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new(&b"01AE;"[..])), Ok((Partial::new(&b";"[..]), 0x01AE)));
/// assert_eq!(parser.parse_peek(Partial::new(&b"abc"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert!(parser.parse_peek(Partial::new(&b"ggg"[..])).is_err());
/// ```
#[inline]
pub fn hex_uint<Input, Output, Error>(input: &mut Input) -> Result<Output, Error>
where
    Input: StreamIsPartial + Stream,
    <Input as Stream>::Token: AsChar,
    <Input as Stream>::Slice: AsBStr,
    Output: HexUint,
    Error: ParserError<Input>,
{
    trace("hex_uint", move |input: &mut Input| {
        let invalid_offset = input
            .offset_for(|c| {
                let c = c.as_char();
                !"0123456789abcdefABCDEF".contains(c)
            })
            .unwrap_or_else(|| input.eof_offset());
        let max_nibbles = Output::max_nibbles(sealed::SealedMarker);
        let max_offset = input.offset_at(max_nibbles);
        let offset = match max_offset {
            Ok(max_offset) => {
                if max_offset < invalid_offset {
                    // Overflow
                    return Err(ParserError::from_input(input));
                } else {
                    invalid_offset
                }
            }
            Err(_) => {
                if <Input as StreamIsPartial>::is_partial_supported()
                    && input.is_partial()
                    && invalid_offset == input.eof_offset()
                {
                    // Only the next byte is guaranteed required
                    return Err(ParserError::incomplete(input, Needed::new(1)));
                } else {
                    invalid_offset
                }
            }
        };
        if offset == 0 {
            // Must be at least one digit
            return Err(ParserError::from_input(input));
        }
        let parsed = input.next_slice(offset);

        let mut res = Output::default();
        for c in parsed.as_bstr() {
            let nibble = *c as char;
            let nibble = nibble.to_digit(16).unwrap_or(0) as u8;
            let nibble = Output::from(nibble);
            res = (res << Output::from(4)) + nibble;
        }

        Ok(res)
    })
    .parse_next(input)
}

/// Metadata for parsing hex numbers, see [`hex_uint`]
pub trait HexUint:
    Default + Shl<Self, Output = Self> + Add<Self, Output = Self> + From<u8>
{
    #[doc(hidden)]
    fn max_nibbles(_: sealed::SealedMarker) -> usize;
}

impl HexUint for u8 {
    #[inline(always)]
    fn max_nibbles(_: sealed::SealedMarker) -> usize {
        2
    }
}

impl HexUint for u16 {
    #[inline(always)]
    fn max_nibbles(_: sealed::SealedMarker) -> usize {
        4
    }
}

impl HexUint for u32 {
    #[inline(always)]
    fn max_nibbles(_: sealed::SealedMarker) -> usize {
        8
    }
}

impl HexUint for u64 {
    #[inline(always)]
    fn max_nibbles(_: sealed::SealedMarker) -> usize {
        16
    }
}

impl HexUint for u128 {
    #[inline(always)]
    fn max_nibbles(_: sealed::SealedMarker) -> usize {
        32
    }
}

/// Recognizes floating point number in text format and returns a [`f32`] or [`f64`].
///
/// *Complete version*: Can parse until the end of input.
///
/// *[Partial version][crate::_topic::partial]*: Will return `Err(winnow::error::ErrMode::Incomplete(_))` if there is not enough data.
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream] into an `f64`:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn float(input: &mut &str) -> ModalResult<f64>
/// # {
/// #     winnow::ascii::float.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::ascii::float;
///
/// fn parser<'s>(s: &mut &'s str) -> ModalResult<f64> {
///   float(s)
/// }
///
/// assert_eq!(parser.parse_peek("11e-1"), Ok(("", 1.1)));
/// assert_eq!(parser.parse_peek("123E-02"), Ok(("", 1.23)));
/// assert_eq!(parser.parse_peek("123K-01"), Ok(("K-01", 123.0)));
/// assert!(parser.parse_peek("abc").is_err());
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::error::Needed::Size;
/// # use winnow::Partial;
/// use winnow::ascii::float;
///
/// fn parser<'s>(s: &mut Partial<&'s str>) -> ModalResult<f64> {
///   float(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new("11e-1 ")), Ok((Partial::new(" "), 1.1)));
/// assert_eq!(parser.parse_peek(Partial::new("11e-1")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(parser.parse_peek(Partial::new("123E-02")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(parser.parse_peek(Partial::new("123K-01")), Ok((Partial::new("K-01"), 123.0)));
/// assert!(parser.parse_peek(Partial::new("abc")).is_err());
/// ```
#[inline(always)]
#[doc(alias = "f32")]
#[doc(alias = "double")]
#[allow(clippy::trait_duplication_in_bounds)] // HACK: clippy 1.64.0 bug
pub fn float<Input, Output, Error>(input: &mut Input) -> Result<Output, Error>
where
    Input: StreamIsPartial + Stream + Compare<Caseless<&'static str>> + Compare<char> + AsBStr,
    <Input as Stream>::Slice: ParseSlice<Output>,
    <Input as Stream>::Token: AsChar + Clone,
    <Input as Stream>::IterOffsets: Clone,
    Error: ParserError<Input>,
{
    trace("float", move |input: &mut Input| {
        let s = take_float_or_exceptions(input)?;
        s.parse_slice()
            .ok_or_else(|| ParserError::from_input(input))
    })
    .parse_next(input)
}

#[allow(clippy::trait_duplication_in_bounds)] // HACK: clippy 1.64.0 bug
fn take_float_or_exceptions<I, E: ParserError<I>>(input: &mut I) -> Result<<I as Stream>::Slice, E>
where
    I: StreamIsPartial,
    I: Stream,
    I: Compare<Caseless<&'static str>>,
    I: Compare<char>,
    <I as Stream>::Token: AsChar + Clone,
    <I as Stream>::IterOffsets: Clone,
    I: AsBStr,
{
    dispatch! {opt(peek(any).map(AsChar::as_char));
        Some('N') | Some('n') => Caseless("nan").void(),
        Some('+') | Some('-') => (any, take_unsigned_float_or_exceptions).void(),
        _ => take_unsigned_float_or_exceptions,
    }
    .take()
    .parse_next(input)
}

#[allow(clippy::trait_duplication_in_bounds)] // HACK: clippy 1.64.0 bug
fn take_unsigned_float_or_exceptions<I, E: ParserError<I>>(input: &mut I) -> Result<(), E>
where
    I: StreamIsPartial,
    I: Stream,
    I: Compare<Caseless<&'static str>>,
    I: Compare<char>,
    <I as Stream>::Token: AsChar + Clone,
    <I as Stream>::IterOffsets: Clone,
    I: AsBStr,
{
    dispatch! {opt(peek(any).map(AsChar::as_char));
        Some('I') | Some('i') => (Caseless("inf"), opt(Caseless("inity"))).void(),
        Some('.') => ('.', digit1, take_exp).void(),
        _ => (digit1, opt(('.', opt(digit1))), take_exp).void(),
    }
    .parse_next(input)
}

#[allow(clippy::trait_duplication_in_bounds)] // HACK: clippy 1.64.0 bug
fn take_exp<I, E: ParserError<I>>(input: &mut I) -> Result<(), E>
where
    I: StreamIsPartial,
    I: Stream,
    I: Compare<char>,
    <I as Stream>::Token: AsChar + Clone,
    <I as Stream>::IterOffsets: Clone,
    I: AsBStr,
{
    dispatch! {opt(peek(any).map(AsChar::as_char));
        Some('E') | Some('e') => (one_of(['e', 'E']), opt(one_of(['+', '-'])), digit1).void(),
        _ => empty,
    }
    .parse_next(input)
}

/// Recognize the input slice with escaped characters.
///
/// Arguments:
/// - `normal`: unescapeable characters
///   - Must not include `control`
/// - `control_char`: e.g. `\` for strings in most languages
/// - `escape`: parse and transform the escaped character
///
/// Parsing ends when:
/// - `alt(normal, control._char)` [`Backtrack`s][crate::error::ErrMode::Backtrack]
/// - `normal` doesn't advance the input stream
/// - *(complete)* input stream is exhausted
///
/// See also [`escaped_transform`]
///
/// <div class="warning">
///
/// **Warning:** If the `normal` parser passed to `take_escaped` accepts empty inputs
/// (like `alpha0` or `digit0`), `take_escaped` will return an error,
/// to prevent going into an infinite loop.
///
/// </div>
///
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::ascii::digit1;
/// # use winnow::prelude::*;
/// use winnow::ascii::take_escaped;
/// use winnow::token::one_of;
///
/// fn esc<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
///   take_escaped(digit1, '\\', one_of(['"', 'n', '\\'])).parse_next(input)
/// }
///
/// assert_eq!(esc.parse_peek("123;"), Ok((";", "123")));
/// assert_eq!(esc.parse_peek(r#"12\"34;"#), Ok((";", r#"12\"34"#)));
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::ascii::digit1;
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::ascii::take_escaped;
/// use winnow::token::one_of;
///
/// fn esc<'i>(input: &mut Partial<&'i str>) -> ModalResult<&'i str> {
///   take_escaped(digit1, '\\', one_of(['"', 'n', '\\'])).parse_next(input)
/// }
///
/// assert_eq!(esc.parse_peek(Partial::new("123;")), Ok((Partial::new(";"), "123")));
/// assert_eq!(esc.parse_peek(Partial::new("12\\\"34;")), Ok((Partial::new(";"), "12\\\"34")));
/// ```
#[inline(always)]
pub fn take_escaped<Input, Error, Normal, Escapable, NormalOutput, EscapableOutput>(
    mut normal: Normal,
    control_char: char,
    mut escapable: Escapable,
) -> impl Parser<Input, <Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream + Compare<char>,
    Normal: Parser<Input, NormalOutput, Error>,
    Escapable: Parser<Input, EscapableOutput, Error>,
    Error: ParserError<Input>,
{
    trace("take_escaped", move |input: &mut Input| {
        if <Input as StreamIsPartial>::is_partial_supported() && input.is_partial() {
            escaped_internal::<_, _, _, _, _, _, true>(
                input,
                &mut normal,
                control_char,
                &mut escapable,
            )
        } else {
            escaped_internal::<_, _, _, _, _, _, false>(
                input,
                &mut normal,
                control_char,
                &mut escapable,
            )
        }
    })
}

fn escaped_internal<I, Error, F, G, O1, O2, const PARTIAL: bool>(
    input: &mut I,
    normal: &mut F,
    control_char: char,
    escapable: &mut G,
) -> Result<<I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream,
    I: Compare<char>,
    F: Parser<I, O1, Error>,
    G: Parser<I, O2, Error>,
    Error: ParserError<I>,
{
    let start = input.checkpoint();

    while input.eof_offset() > 0 {
        let current_len = input.eof_offset();

        match opt(normal.by_ref()).parse_next(input)? {
            Some(_) => {
                // infinite loop check: the parser must always consume
                if input.eof_offset() == current_len {
                    return Err(ParserError::assert(
                        input,
                        "`take_escaped` parsers must always consume",
                    ));
                }
            }
            None => {
                if opt(control_char).parse_next(input)?.is_some() {
                    let _ = escapable.parse_next(input)?;
                } else {
                    let offset = input.offset_from(&start);
                    input.reset(&start);
                    return Ok(input.next_slice(offset));
                }
            }
        }
    }

    if PARTIAL && input.is_partial() {
        Err(ParserError::incomplete(input, Needed::Unknown))
    } else {
        input.reset(&start);
        Ok(input.finish())
    }
}

/// Deprecated, replaed with [`escaped`]
#[inline(always)]
#[deprecated(since = "7.0.0", note = "replaced with `escaped`")]
pub fn escaped_transform<Input, Error, Normal, NormalOutput, Escape, EscapeOutput, Output>(
    normal: Normal,
    control_char: char,
    escape: Escape,
) -> impl Parser<Input, Output, Error>
where
    Input: StreamIsPartial + Stream + Compare<char>,
    Normal: Parser<Input, NormalOutput, Error>,
    Escape: Parser<Input, EscapeOutput, Error>,
    Output: crate::stream::Accumulate<NormalOutput>,
    Output: crate::stream::Accumulate<EscapeOutput>,
    Error: ParserError<Input>,
{
    escaped(normal, control_char, escape)
}

/// Parse escaped characters, unescaping them
///
/// Arguments:
/// - `normal`: unescapeable characters
///   - Must not include `control`
/// - `control_char`: e.g. `\` for strings in most languages
/// - `escape`: parse and transform the escaped character
///
/// Parsing ends when:
/// - `alt(normal, control._char)` [`Backtrack`s][crate::error::ErrMode::Backtrack]
/// - `normal` doesn't advance the input stream
/// - *(complete)* input stream is exhausted
///
/// <div class="warning">
///
/// **Warning:** If the `normal` parser passed to `escaped_transform` accepts empty inputs
/// (like `alpha0` or `digit0`), `escaped_transform` will return an error,
/// to prevent going into an infinite loop.
///
/// </div>
///
/// # Example
///
/// ```rust
/// # #[cfg(feature = "std")] {
/// # use winnow::prelude::*;
/// # use std::str::from_utf8;
/// use winnow::token::literal;
/// use winnow::ascii::escaped_transform;
/// use winnow::ascii::alpha1;
/// use winnow::combinator::alt;
///
/// fn parser<'s>(input: &mut &'s str) -> ModalResult<String> {
///   escaped_transform(
///     alpha1,
///     '\\',
///     alt((
///       "\\".value("\\"),
///       "\"".value("\""),
///       "n".value("\n"),
///     ))
///   ).parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("ab\\\"cd"), Ok(("", String::from("ab\"cd"))));
/// assert_eq!(parser.parse_peek("ab\\ncd"), Ok(("", String::from("ab\ncd"))));
/// # }
/// ```
///
/// ```rust
/// # #[cfg(feature = "std")] {
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::Needed};
/// # use std::str::from_utf8;
/// # use winnow::Partial;
/// use winnow::token::literal;
/// use winnow::ascii::escaped_transform;
/// use winnow::ascii::alpha1;
/// use winnow::combinator::alt;
///
/// fn parser<'s>(input: &mut Partial<&'s str>) -> ModalResult<String> {
///   escaped_transform(
///     alpha1,
///     '\\',
///     alt((
///       "\\".value("\\"),
///       "\"".value("\""),
///       "n".value("\n"),
///     ))
///   ).parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new("ab\\\"cd\"")), Ok((Partial::new("\""), String::from("ab\"cd"))));
/// # }
/// ```
#[inline(always)]
pub fn escaped<Input, Error, Normal, NormalOutput, Escape, EscapeOutput, Output>(
    mut normal: Normal,
    control_char: char,
    mut escape: Escape,
) -> impl Parser<Input, Output, Error>
where
    Input: StreamIsPartial + Stream + Compare<char>,
    Normal: Parser<Input, NormalOutput, Error>,
    Escape: Parser<Input, EscapeOutput, Error>,
    Output: crate::stream::Accumulate<NormalOutput>,
    Output: crate::stream::Accumulate<EscapeOutput>,
    Error: ParserError<Input>,
{
    trace("escaped", move |input: &mut Input| {
        if <Input as StreamIsPartial>::is_partial_supported() && input.is_partial() {
            escaped_transform_internal::<_, _, _, _, _, _, _, true>(
                input,
                &mut normal,
                control_char,
                &mut escape,
            )
        } else {
            escaped_transform_internal::<_, _, _, _, _, _, _, false>(
                input,
                &mut normal,
                control_char,
                &mut escape,
            )
        }
    })
}

fn escaped_transform_internal<
    I,
    Error,
    F,
    NormalOutput,
    G,
    EscapeOutput,
    Output,
    const PARTIAL: bool,
>(
    input: &mut I,
    normal: &mut F,
    control_char: char,
    transform: &mut G,
) -> Result<Output, Error>
where
    I: StreamIsPartial,
    I: Stream,
    I: Compare<char>,
    Output: crate::stream::Accumulate<NormalOutput>,
    Output: crate::stream::Accumulate<EscapeOutput>,
    F: Parser<I, NormalOutput, Error>,
    G: Parser<I, EscapeOutput, Error>,
    Error: ParserError<I>,
{
    let mut res =
        <Output as crate::stream::Accumulate<NormalOutput>>::initial(Some(input.eof_offset()));

    while input.eof_offset() > 0 {
        let current_len = input.eof_offset();

        match opt(normal.by_ref()).parse_next(input)? {
            Some(o) => {
                res.accumulate(o);
                // infinite loop check: the parser must always consume
                if input.eof_offset() == current_len {
                    return Err(ParserError::assert(
                        input,
                        "`escaped_transform` parsers must always consume",
                    ));
                }
            }
            None => {
                if opt(control_char).parse_next(input)?.is_some() {
                    let o = transform.parse_next(input)?;
                    res.accumulate(o);
                } else {
                    return Ok(res);
                }
            }
        }
    }

    if PARTIAL && input.is_partial() {
        Err(ParserError::incomplete(input, Needed::Unknown))
    } else {
        Ok(res)
    }
}

mod sealed {
    #[allow(unnameable_types)]
    pub struct SealedMarker;
}

//! Parsers extracting tokens from the stream

#[cfg(test)]
mod tests;

use crate::combinator::trace;
use crate::combinator::DisplayDebug;
use crate::error::Needed;
use crate::error::ParserError;
use crate::lib::std::result::Result::Ok;
use crate::stream::Range;
use crate::stream::{Compare, CompareResult, ContainsToken, FindSlice, Stream};
use crate::stream::{StreamIsPartial, ToUsize};
use crate::Parser;
use crate::Result;

/// Matches one token
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
/// pub fn any(input: &mut &str) -> ModalResult<char>
/// # {
/// #     winnow::token::any.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::{token::any, error::ErrMode, error::ContextError};
/// # use winnow::prelude::*;
/// fn parser(input: &mut &str) -> ModalResult<char> {
///     any.parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("abc"), Ok(("bc",'a')));
/// assert!(parser.parse_peek("").is_err());
/// ```
///
/// ```rust
/// # use winnow::{token::any, error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// assert_eq!(any::<_, ErrMode<ContextError>>.parse_peek(Partial::new("abc")), Ok((Partial::new("bc"),'a')));
/// assert_eq!(any::<_, ErrMode<ContextError>>.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
#[doc(alias = "token")]
pub fn any<Input, Error>(input: &mut Input) -> Result<<Input as Stream>::Token, Error>
where
    Input: StreamIsPartial + Stream,
    Error: ParserError<Input>,
{
    trace("any", move |input: &mut Input| {
        if <Input as StreamIsPartial>::is_partial_supported() {
            any_::<_, _, true>(input)
        } else {
            any_::<_, _, false>(input)
        }
    })
    .parse_next(input)
}

fn any_<I, E: ParserError<I>, const PARTIAL: bool>(input: &mut I) -> Result<<I as Stream>::Token, E>
where
    I: StreamIsPartial,
    I: Stream,
{
    input.next_token().ok_or_else(|| {
        if PARTIAL && input.is_partial() {
            ParserError::incomplete(input, Needed::new(1))
        } else {
            ParserError::from_input(input)
        }
    })
}

/// Recognizes a literal
///
/// The input data will be compared to the literal combinator's argument and will return the part of
/// the input that matches the argument
///
/// It will return `Err(ErrMode::Backtrack(_))` if the input doesn't match the literal
///
/// <div class="warning">
///
/// **Note:** [`Parser`] is implemented for strings and byte strings as a convenience (complete
/// only)
///
/// </div>
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// # use winnow::error::ContextError;
/// pub fn literal(literal: &str) -> impl Parser<&str, &str, ContextError>
/// # {
/// #     winnow::token::literal(literal)
/// # }
/// ```
///
/// # Example
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// #
/// fn parser<'i>(s: &mut &'i str) -> ModalResult<&'i str> {
///   "Hello".parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek("Hello, World!"), Ok((", World!", "Hello")));
/// assert!(parser.parse_peek("Something").is_err());
/// assert!(parser.parse_peek("").is_err());
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
///
/// fn parser<'i>(s: &mut Partial<&'i str>) -> ModalResult<&'i str> {
///   "Hello".parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek(Partial::new("Hello, World!")), Ok((Partial::new(", World!"), "Hello")));
/// assert!(parser.parse_peek(Partial::new("Something")).is_err());
/// assert!(parser.parse_peek(Partial::new("S")).is_err());
/// assert_eq!(parser.parse_peek(Partial::new("H")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::token::literal;
/// use winnow::ascii::Caseless;
///
/// fn parser<'i>(s: &mut &'i str) -> ModalResult<&'i str> {
///   literal(Caseless("hello")).parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek("Hello, World!"), Ok((", World!", "Hello")));
/// assert_eq!(parser.parse_peek("hello, World!"), Ok((", World!", "hello")));
/// assert_eq!(parser.parse_peek("HeLlO, World!"), Ok((", World!", "HeLlO")));
/// assert!(parser.parse_peek("Something").is_err());
/// assert!(parser.parse_peek("").is_err());
/// ```
#[inline(always)]
#[doc(alias = "tag")]
#[doc(alias = "bytes")]
#[doc(alias = "just")]
pub fn literal<Literal, Input, Error>(
    literal: Literal,
) -> impl Parser<Input, <Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream + Compare<Literal>,
    Literal: Clone + crate::lib::std::fmt::Debug,
    Error: ParserError<Input>,
{
    trace(DisplayDebug(literal.clone()), move |i: &mut Input| {
        let t = literal.clone();
        if <Input as StreamIsPartial>::is_partial_supported() {
            literal_::<_, _, _, true>(i, t)
        } else {
            literal_::<_, _, _, false>(i, t)
        }
    })
}

fn literal_<T, I, Error: ParserError<I>, const PARTIAL: bool>(
    i: &mut I,
    t: T,
) -> Result<<I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream + Compare<T>,
    T: crate::lib::std::fmt::Debug,
{
    match i.compare(t) {
        CompareResult::Ok(len) => Ok(i.next_slice(len)),
        CompareResult::Incomplete if PARTIAL && i.is_partial() => {
            Err(ParserError::incomplete(i, Needed::Unknown))
        }
        CompareResult::Incomplete | CompareResult::Error => Err(ParserError::from_input(i)),
    }
}

/// Recognize a token that matches a [set of tokens][ContainsToken]
///
/// <div class="warning">
///
/// **Note:** [`Parser`] is implemented as a convenience (complete
/// only) for
/// - `u8`
/// - `char`
///
/// </div>
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
/// # use winnow::stream::ContainsToken;
/// # use winnow::error::ContextError;
/// pub fn one_of<'i>(set: impl ContainsToken<char>) -> impl Parser<&'i str, char, ContextError>
/// # {
/// #     winnow::token::one_of(set)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError};
/// # use winnow::token::one_of;
/// assert_eq!(one_of::<_, _, ContextError>(['a', 'b', 'c']).parse_peek("b"), Ok(("", 'b')));
/// assert!(one_of::<_, _, ContextError>('a').parse_peek("bc").is_err());
/// assert!(one_of::<_, _, ContextError>('a').parse_peek("").is_err());
///
/// fn parser_fn(i: &mut &str) -> ModalResult<char> {
///     one_of(|c| c == 'a' || c == 'b').parse_next(i)
/// }
/// assert_eq!(parser_fn.parse_peek("abc"), Ok(("bc", 'a')));
/// assert!(parser_fn.parse_peek("cd").is_err());
/// assert!(parser_fn.parse_peek("").is_err());
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::Partial;
/// # use winnow::token::one_of;
/// assert_eq!(one_of::<_, _, ErrMode<ContextError>>(['a', 'b', 'c']).parse_peek(Partial::new("b")), Ok((Partial::new(""), 'b')));
/// assert!(one_of::<_, _, ErrMode<ContextError>>('a').parse_peek(Partial::new("bc")).is_err());
/// assert_eq!(one_of::<_, _, ErrMode<ContextError>>('a').parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
///
/// fn parser_fn(i: &mut Partial<&str>) -> ModalResult<char> {
///     one_of(|c| c == 'a' || c == 'b').parse_next(i)
/// }
/// assert_eq!(parser_fn.parse_peek(Partial::new("abc")), Ok((Partial::new("bc"), 'a')));
/// assert!(parser_fn.parse_peek(Partial::new("cd")).is_err());
/// assert_eq!(parser_fn.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
#[doc(alias = "char")]
#[doc(alias = "token")]
#[doc(alias = "satisfy")]
pub fn one_of<Input, Set, Error>(set: Set) -> impl Parser<Input, <Input as Stream>::Token, Error>
where
    Input: StreamIsPartial + Stream,
    <Input as Stream>::Token: Clone,
    Set: ContainsToken<<Input as Stream>::Token>,
    Error: ParserError<Input>,
{
    trace(
        "one_of",
        any.verify(move |t: &<Input as Stream>::Token| set.contains_token(t.clone())),
    )
}

/// Recognize a token that does not match a [set of tokens][ContainsToken]
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
/// # use winnow::stream::ContainsToken;
/// # use winnow::error::ContextError;
/// pub fn none_of<'i>(set: impl ContainsToken<char>) -> impl Parser<&'i str, char, ContextError>
/// # {
/// #     winnow::token::none_of(set)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ContextError};
/// # use winnow::prelude::*;
/// # use winnow::token::none_of;
/// assert_eq!(none_of::<_, _, ContextError>(['a', 'b', 'c']).parse_peek("z"), Ok(("", 'z')));
/// assert!(none_of::<_, _, ContextError>(['a', 'b']).parse_peek("a").is_err());
/// assert!(none_of::<_, _, ContextError>('a').parse_peek("").is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// # use winnow::token::none_of;
/// assert_eq!(none_of::<_, _, ErrMode<ContextError>>(['a', 'b', 'c']).parse_peek(Partial::new("z")), Ok((Partial::new(""), 'z')));
/// assert!(none_of::<_, _, ErrMode<ContextError>>(['a', 'b']).parse_peek(Partial::new("a")).is_err());
/// assert_eq!(none_of::<_, _, ErrMode<ContextError>>('a').parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
pub fn none_of<Input, Set, Error>(set: Set) -> impl Parser<Input, <Input as Stream>::Token, Error>
where
    Input: StreamIsPartial + Stream,
    <Input as Stream>::Token: Clone,
    Set: ContainsToken<<Input as Stream>::Token>,
    Error: ParserError<Input>,
{
    trace(
        "none_of",
        any.verify(move |t: &<Input as Stream>::Token| !set.contains_token(t.clone())),
    )
}

/// Recognize the longest (m <= len <= n) input slice that matches a [set of tokens][ContainsToken]
///
/// It will return an `ErrMode::Backtrack(_)` if the set of tokens wasn't met or is out
/// of range (m <= len <= n).
///
/// *[Partial version][crate::_topic::partial]* will return a `ErrMode::Incomplete(Needed::new(1))` if a member of the set of tokens reaches the end of the input or is too short.
///
/// To take a series of tokens, use [`repeat`][crate::combinator::repeat] to [`Accumulate`][crate::stream::Accumulate] into a `()` and then [`Parser::take`].
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream] with `0..` or `1..` [ranges][Range]:
/// ```rust
/// # use std::ops::RangeFrom;
/// # use winnow::prelude::*;
/// # use winnow::stream::ContainsToken;
/// # use winnow::error::ContextError;
/// pub fn take_while<'i>(occurrences: RangeFrom<usize>, set: impl ContainsToken<char>) -> impl Parser<&'i str, &'i str, ContextError>
/// # {
/// #     winnow::token::take_while(occurrences, set)
/// # }
/// ```
///
/// # Example
///
/// Zero or more tokens:
/// ```rust
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::token::take_while;
/// use winnow::stream::AsChar;
///
/// fn alpha<'i>(s: &mut &'i [u8]) -> ModalResult<&'i [u8]> {
///   take_while(0.., AsChar::is_alpha).parse_next(s)
/// }
///
/// assert_eq!(alpha.parse_peek(b"latin123"), Ok((&b"123"[..], &b"latin"[..])));
/// assert_eq!(alpha.parse_peek(b"12345"), Ok((&b"12345"[..], &b""[..])));
/// assert_eq!(alpha.parse_peek(b"latin"), Ok((&b""[..], &b"latin"[..])));
/// assert_eq!(alpha.parse_peek(b""), Ok((&b""[..], &b""[..])));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::token::take_while;
/// use winnow::stream::AsChar;
///
/// fn alpha<'i>(s: &mut Partial<&'i [u8]>) -> ModalResult<&'i [u8]> {
///   take_while(0.., AsChar::is_alpha).parse_next(s)
/// }
///
/// assert_eq!(alpha.parse_peek(Partial::new(b"latin123")), Ok((Partial::new(&b"123"[..]), &b"latin"[..])));
/// assert_eq!(alpha.parse_peek(Partial::new(b"12345")), Ok((Partial::new(&b"12345"[..]), &b""[..])));
/// assert_eq!(alpha.parse_peek(Partial::new(b"latin")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(alpha.parse_peek(Partial::new(b"")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
///
/// One or more tokens:
/// ```rust
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::token::take_while;
/// use winnow::stream::AsChar;
///
/// fn alpha<'i>(s: &mut &'i [u8]) -> ModalResult<&'i [u8]> {
///   take_while(1.., AsChar::is_alpha).parse_next(s)
/// }
///
/// assert_eq!(alpha.parse_peek(b"latin123"), Ok((&b"123"[..], &b"latin"[..])));
/// assert_eq!(alpha.parse_peek(b"latin"), Ok((&b""[..], &b"latin"[..])));
/// assert!(alpha.parse_peek(b"12345").is_err());
///
/// fn hex<'i>(s: &mut &'i str) -> ModalResult<&'i str> {
///   take_while(1.., ('0'..='9', 'A'..='F')).parse_next(s)
/// }
///
/// assert_eq!(hex.parse_peek("123 and voila"), Ok((" and voila", "123")));
/// assert_eq!(hex.parse_peek("DEADBEEF and others"), Ok((" and others", "DEADBEEF")));
/// assert_eq!(hex.parse_peek("BADBABEsomething"), Ok(("something", "BADBABE")));
/// assert_eq!(hex.parse_peek("D15EA5E"), Ok(("", "D15EA5E")));
/// assert!(hex.parse_peek("").is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::token::take_while;
/// use winnow::stream::AsChar;
///
/// fn alpha<'i>(s: &mut Partial<&'i [u8]>) -> ModalResult<&'i [u8]> {
///   take_while(1.., AsChar::is_alpha).parse_next(s)
/// }
///
/// assert_eq!(alpha.parse_peek(Partial::new(b"latin123")), Ok((Partial::new(&b"123"[..]), &b"latin"[..])));
/// assert_eq!(alpha.parse_peek(Partial::new(b"latin")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert!(alpha.parse_peek(Partial::new(b"12345")).is_err());
///
/// fn hex<'i>(s: &mut Partial<&'i str>) -> ModalResult<&'i str> {
///   take_while(1.., ('0'..='9', 'A'..='F')).parse_next(s)
/// }
///
/// assert_eq!(hex.parse_peek(Partial::new("123 and voila")), Ok((Partial::new(" and voila"), "123")));
/// assert_eq!(hex.parse_peek(Partial::new("DEADBEEF and others")), Ok((Partial::new(" and others"), "DEADBEEF")));
/// assert_eq!(hex.parse_peek(Partial::new("BADBABEsomething")), Ok((Partial::new("something"), "BADBABE")));
/// assert_eq!(hex.parse_peek(Partial::new("D15EA5E")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(hex.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
///
/// Arbitrary amount of tokens:
/// ```rust
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::token::take_while;
/// use winnow::stream::AsChar;
///
/// fn short_alpha<'i>(s: &mut &'i [u8]) -> ModalResult<&'i [u8]> {
///   take_while(3..=6, AsChar::is_alpha).parse_next(s)
/// }
///
/// assert_eq!(short_alpha.parse_peek(b"latin123"), Ok((&b"123"[..], &b"latin"[..])));
/// assert_eq!(short_alpha.parse_peek(b"lengthy"), Ok((&b"y"[..], &b"length"[..])));
/// assert_eq!(short_alpha.parse_peek(b"latin"), Ok((&b""[..], &b"latin"[..])));
/// assert!(short_alpha.parse_peek(b"ed").is_err());
/// assert!(short_alpha.parse_peek(b"12345").is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::token::take_while;
/// use winnow::stream::AsChar;
///
/// fn short_alpha<'i>(s: &mut Partial<&'i [u8]>) -> ModalResult<&'i [u8]> {
///   take_while(3..=6, AsChar::is_alpha).parse_next(s)
/// }
///
/// assert_eq!(short_alpha.parse_peek(Partial::new(b"latin123")), Ok((Partial::new(&b"123"[..]), &b"latin"[..])));
/// assert_eq!(short_alpha.parse_peek(Partial::new(b"lengthy")), Ok((Partial::new(&b"y"[..]), &b"length"[..])));
/// assert_eq!(short_alpha.parse_peek(Partial::new(b"latin")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(short_alpha.parse_peek(Partial::new(b"ed")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert!(short_alpha.parse_peek(Partial::new(b"12345")).is_err());
/// ```
#[inline(always)]
#[doc(alias = "is_a")]
#[doc(alias = "take_while0")]
#[doc(alias = "take_while1")]
pub fn take_while<Set, Input, Error>(
    occurrences: impl Into<Range>,
    set: Set,
) -> impl Parser<Input, <Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream,
    Set: ContainsToken<<Input as Stream>::Token>,
    Error: ParserError<Input>,
{
    let Range {
        start_inclusive,
        end_inclusive,
    } = occurrences.into();
    trace("take_while", move |i: &mut Input| {
        match (start_inclusive, end_inclusive) {
            (0, None) => {
                if <Input as StreamIsPartial>::is_partial_supported() {
                    take_till0::<_, _, _, true>(i, |c| !set.contains_token(c))
                } else {
                    take_till0::<_, _, _, false>(i, |c| !set.contains_token(c))
                }
            }
            (1, None) => {
                if <Input as StreamIsPartial>::is_partial_supported() {
                    take_till1::<_, _, _, true>(i, |c| !set.contains_token(c))
                } else {
                    take_till1::<_, _, _, false>(i, |c| !set.contains_token(c))
                }
            }
            (start, end) => {
                let end = end.unwrap_or(usize::MAX);
                if <Input as StreamIsPartial>::is_partial_supported() {
                    take_till_m_n::<_, _, _, true>(i, start, end, |c| !set.contains_token(c))
                } else {
                    take_till_m_n::<_, _, _, false>(i, start, end, |c| !set.contains_token(c))
                }
            }
        }
    })
}

fn take_till0<P, I: StreamIsPartial + Stream, E: ParserError<I>, const PARTIAL: bool>(
    input: &mut I,
    predicate: P,
) -> Result<<I as Stream>::Slice, E>
where
    P: Fn(I::Token) -> bool,
{
    let offset = match input.offset_for(predicate) {
        Some(offset) => offset,
        None if PARTIAL && input.is_partial() => {
            return Err(ParserError::incomplete(input, Needed::new(1)));
        }
        None => input.eof_offset(),
    };
    Ok(input.next_slice(offset))
}

fn take_till1<P, I: StreamIsPartial + Stream, E: ParserError<I>, const PARTIAL: bool>(
    input: &mut I,
    predicate: P,
) -> Result<<I as Stream>::Slice, E>
where
    P: Fn(I::Token) -> bool,
{
    let offset = match input.offset_for(predicate) {
        Some(offset) => offset,
        None if PARTIAL && input.is_partial() => {
            return Err(ParserError::incomplete(input, Needed::new(1)));
        }
        None => input.eof_offset(),
    };
    if offset == 0 {
        Err(ParserError::from_input(input))
    } else {
        Ok(input.next_slice(offset))
    }
}

fn take_till_m_n<P, I, Error: ParserError<I>, const PARTIAL: bool>(
    input: &mut I,
    m: usize,
    n: usize,
    predicate: P,
) -> Result<<I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream,
    P: Fn(I::Token) -> bool,
{
    if n < m {
        return Err(ParserError::assert(
            input,
            "`occurrences` should be ascending, rather than descending",
        ));
    }

    let mut final_count = 0;
    for (processed, (offset, token)) in input.iter_offsets().enumerate() {
        if predicate(token) {
            if processed < m {
                return Err(ParserError::from_input(input));
            } else {
                return Ok(input.next_slice(offset));
            }
        } else {
            if processed == n {
                return Ok(input.next_slice(offset));
            }
            final_count = processed + 1;
        }
    }
    if PARTIAL && input.is_partial() {
        if final_count == n {
            Ok(input.finish())
        } else {
            let needed = if m > input.eof_offset() {
                m - input.eof_offset()
            } else {
                1
            };
            Err(ParserError::incomplete(input, Needed::new(needed)))
        }
    } else {
        if m <= final_count {
            Ok(input.finish())
        } else {
            Err(ParserError::from_input(input))
        }
    }
}

/// Recognize the longest input slice (if any) till a member of a [set of tokens][ContainsToken] is found.
///
/// It doesn't consume the terminating token from the set.
///
/// *[Partial version][crate::_topic::partial]* will return a `ErrMode::Incomplete(Needed::new(1))` if the match reaches the
/// end of input or if there was not match.
///
/// See also
/// - [`take_until`] for recognizing up-to a [`literal`] (w/ optional simd optimizations)
/// - [`repeat_till`][crate::combinator::repeat_till] with [`Parser::take`] for taking tokens up to a [`Parser`]
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream] with `0..` or `1..` [ranges][Range]:
/// ```rust
/// # use std::ops::RangeFrom;
/// # use winnow::prelude::*;
/// # use winnow::stream::ContainsToken;
/// # use winnow::error::ContextError;
/// pub fn take_till<'i>(occurrences: RangeFrom<usize>, set: impl ContainsToken<char>) -> impl Parser<&'i str, &'i str, ContextError>
/// # {
/// #     winnow::token::take_till(occurrences, set)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::token::take_till;
///
/// fn till_colon<'i>(s: &mut &'i str) -> ModalResult<&'i str> {
///   take_till(0.., |c| c == ':').parse_next(s)
/// }
///
/// assert_eq!(till_colon.parse_peek("latin:123"), Ok((":123", "latin")));
/// assert_eq!(till_colon.parse_peek(":empty matched"), Ok((":empty matched", ""))); //allowed
/// assert_eq!(till_colon.parse_peek("12345"), Ok(("", "12345")));
/// assert_eq!(till_colon.parse_peek(""), Ok(("", "")));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::token::take_till;
///
/// fn till_colon<'i>(s: &mut Partial<&'i str>) -> ModalResult<&'i str> {
///   take_till(0.., |c| c == ':').parse_next(s)
/// }
///
/// assert_eq!(till_colon.parse_peek(Partial::new("latin:123")), Ok((Partial::new(":123"), "latin")));
/// assert_eq!(till_colon.parse_peek(Partial::new(":empty matched")), Ok((Partial::new(":empty matched"), ""))); //allowed
/// assert_eq!(till_colon.parse_peek(Partial::new("12345")), Err(ErrMode::Incomplete(Needed::new(1))));
/// assert_eq!(till_colon.parse_peek(Partial::new("")), Err(ErrMode::Incomplete(Needed::new(1))));
/// ```
#[inline(always)]
#[doc(alias = "is_not")]
pub fn take_till<Set, Input, Error>(
    occurrences: impl Into<Range>,
    set: Set,
) -> impl Parser<Input, <Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream,
    Set: ContainsToken<<Input as Stream>::Token>,
    Error: ParserError<Input>,
{
    let Range {
        start_inclusive,
        end_inclusive,
    } = occurrences.into();
    trace("take_till", move |i: &mut Input| {
        match (start_inclusive, end_inclusive) {
            (0, None) => {
                if <Input as StreamIsPartial>::is_partial_supported() {
                    take_till0::<_, _, _, true>(i, |c| set.contains_token(c))
                } else {
                    take_till0::<_, _, _, false>(i, |c| set.contains_token(c))
                }
            }
            (1, None) => {
                if <Input as StreamIsPartial>::is_partial_supported() {
                    take_till1::<_, _, _, true>(i, |c| set.contains_token(c))
                } else {
                    take_till1::<_, _, _, false>(i, |c| set.contains_token(c))
                }
            }
            (start, end) => {
                let end = end.unwrap_or(usize::MAX);
                if <Input as StreamIsPartial>::is_partial_supported() {
                    take_till_m_n::<_, _, _, true>(i, start, end, |c| set.contains_token(c))
                } else {
                    take_till_m_n::<_, _, _, false>(i, start, end, |c| set.contains_token(c))
                }
            }
        }
    })
}

/// Recognize an input slice containing the first N input elements (I[..N]).
///
/// *Complete version*: It will return `Err(ErrMode::Backtrack(_))` if the input is shorter than the argument.
///
/// *[Partial version][crate::_topic::partial]*: if the input has less than N elements, `take` will
/// return a `ErrMode::Incomplete(Needed::new(M))` where M is the number of
/// additional bytes the parser would need to succeed.
/// It is well defined for `&[u8]` as the number of elements is the byte size,
/// but for types like `&str`, we cannot know how many bytes correspond for
/// the next few chars, so the result will be `ErrMode::Incomplete(Needed::Unknown)`
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream] with `0..` or `1..` ranges:
/// ```rust
/// # use std::ops::RangeFrom;
/// # use winnow::prelude::*;
/// # use winnow::stream::ContainsToken;
/// # use winnow::error::ContextError;
/// pub fn take<'i>(token_count: usize) -> impl Parser<&'i str, &'i str, ContextError>
/// # {
/// #     winnow::token::take(token_count)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::token::take;
///
/// fn take6<'i>(s: &mut &'i str) -> ModalResult<&'i str> {
///   take(6usize).parse_next(s)
/// }
///
/// assert_eq!(take6.parse_peek("1234567"), Ok(("7", "123456")));
/// assert_eq!(take6.parse_peek("things"), Ok(("", "things")));
/// assert!(take6.parse_peek("short").is_err());
/// assert!(take6.parse_peek("").is_err());
/// ```
///
/// The units that are taken will depend on the input type. For example, for a
/// `&str` it will take a number of `char`'s, whereas for a `&[u8]` it will
/// take that many `u8`'s:
///
/// ```rust
/// # use winnow::prelude::*;
/// use winnow::error::ContextError;
/// use winnow::token::take;
///
/// assert_eq!(take::<_, _, ContextError>(1usize).parse_peek("💙"), Ok(("", "💙")));
/// assert_eq!(take::<_, _, ContextError>(1usize).parse_peek("💙".as_bytes()), Ok((b"\x9F\x92\x99".as_ref(), b"\xF0".as_ref())));
/// ```
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::error::{ErrMode, ContextError, Needed};
/// # use winnow::Partial;
/// use winnow::token::take;
///
/// fn take6<'i>(s: &mut Partial<&'i str>) -> ModalResult<&'i str> {
///   take(6usize).parse_next(s)
/// }
///
/// assert_eq!(take6.parse_peek(Partial::new("1234567")), Ok((Partial::new("7"), "123456")));
/// assert_eq!(take6.parse_peek(Partial::new("things")), Ok((Partial::new(""), "things")));
/// // `Unknown` as we don't know the number of bytes that `count` corresponds to
/// assert_eq!(take6.parse_peek(Partial::new("short")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// ```
#[inline(always)]
pub fn take<UsizeLike, Input, Error>(
    token_count: UsizeLike,
) -> impl Parser<Input, <Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream,
    UsizeLike: ToUsize,
    Error: ParserError<Input>,
{
    let c = token_count.to_usize();
    trace("take", move |i: &mut Input| {
        if <Input as StreamIsPartial>::is_partial_supported() {
            take_::<_, _, true>(i, c)
        } else {
            take_::<_, _, false>(i, c)
        }
    })
}

fn take_<I, Error: ParserError<I>, const PARTIAL: bool>(
    i: &mut I,
    c: usize,
) -> Result<<I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream,
{
    match i.offset_at(c) {
        Ok(offset) => Ok(i.next_slice(offset)),
        Err(e) if PARTIAL && i.is_partial() => Err(ParserError::incomplete(i, e)),
        Err(_needed) => Err(ParserError::from_input(i)),
    }
}

/// Recognize the input slice up to the first occurrence of a [literal].
///
/// Feature `simd` will enable the use of [`memchr`](https://docs.rs/memchr/latest/memchr/).
///
/// It doesn't consume the literal.
///
/// *Complete version*: It will return `Err(ErrMode::Backtrack(_))`
/// if the literal wasn't met.
///
/// *[Partial version][crate::_topic::partial]*: will return a `ErrMode::Incomplete(Needed::new(N))` if the input doesn't
/// contain the literal or if the input is smaller than the literal.
///
/// See also
/// - [`take_till`] for recognizing up-to a [set of tokens][ContainsToken]
/// - [`repeat_till`][crate::combinator::repeat_till] with [`Parser::take`] for taking tokens up to a [`Parser`]
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream] with `0..` or `1..` [ranges][Range]:
/// ```rust
/// # use std::ops::RangeFrom;
/// # use winnow::prelude::*;;
/// # use winnow::error::ContextError;
/// pub fn take_until(occurrences: RangeFrom<usize>, literal: &str) -> impl Parser<&str, &str, ContextError>
/// # {
/// #     winnow::token::take_until(occurrences, literal)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::token::take_until;
///
/// fn until_eof<'i>(s: &mut &'i str) -> ModalResult<&'i str> {
///   take_until(0.., "eof").parse_next(s)
/// }
///
/// assert_eq!(until_eof.parse_peek("hello, worldeof"), Ok(("eof", "hello, world")));
/// assert!(until_eof.parse_peek("hello, world").is_err());
/// assert!(until_eof.parse_peek("").is_err());
/// assert_eq!(until_eof.parse_peek("1eof2eof"), Ok(("eof2eof", "1")));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::token::take_until;
///
/// fn until_eof<'i>(s: &mut Partial<&'i str>) -> ModalResult<&'i str> {
///   take_until(0.., "eof").parse_next(s)
/// }
///
/// assert_eq!(until_eof.parse_peek(Partial::new("hello, worldeof")), Ok((Partial::new("eof"), "hello, world")));
/// assert_eq!(until_eof.parse_peek(Partial::new("hello, world")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// assert_eq!(until_eof.parse_peek(Partial::new("hello, worldeo")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// assert_eq!(until_eof.parse_peek(Partial::new("1eof2eof")), Ok((Partial::new("eof2eof"), "1")));
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::token::take_until;
///
/// fn until_eof<'i>(s: &mut &'i str) -> ModalResult<&'i str> {
///   take_until(1.., "eof").parse_next(s)
/// }
///
/// assert_eq!(until_eof.parse_peek("hello, worldeof"), Ok(("eof", "hello, world")));
/// assert!(until_eof.parse_peek("hello, world").is_err());
/// assert!(until_eof.parse_peek("").is_err());
/// assert_eq!(until_eof.parse_peek("1eof2eof"), Ok(("eof2eof", "1")));
/// assert!(until_eof.parse_peek("eof").is_err());
/// ```
///
/// ```rust
/// # use winnow::{error::ErrMode, error::ContextError, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::Partial;
/// use winnow::token::take_until;
///
/// fn until_eof<'i>(s: &mut Partial<&'i str>) -> ModalResult<&'i str> {
///   take_until(1.., "eof").parse_next(s)
/// }
///
/// assert_eq!(until_eof.parse_peek(Partial::new("hello, worldeof")), Ok((Partial::new("eof"), "hello, world")));
/// assert_eq!(until_eof.parse_peek(Partial::new("hello, world")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// assert_eq!(until_eof.parse_peek(Partial::new("hello, worldeo")), Err(ErrMode::Incomplete(Needed::Unknown)));
/// assert_eq!(until_eof.parse_peek(Partial::new("1eof2eof")), Ok((Partial::new("eof2eof"), "1")));
/// assert!(until_eof.parse_peek(Partial::new("eof")).is_err());
/// ```
#[inline(always)]
pub fn take_until<Literal, Input, Error>(
    occurrences: impl Into<Range>,
    literal: Literal,
) -> impl Parser<Input, <Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream + FindSlice<Literal>,
    Literal: Clone,
    Error: ParserError<Input>,
{
    let Range {
        start_inclusive,
        end_inclusive,
    } = occurrences.into();
    trace("take_until", move |i: &mut Input| {
        match (start_inclusive, end_inclusive) {
            (0, None) => {
                if <Input as StreamIsPartial>::is_partial_supported() {
                    take_until0_::<_, _, _, true>(i, literal.clone())
                } else {
                    take_until0_::<_, _, _, false>(i, literal.clone())
                }
            }
            (1, None) => {
                if <Input as StreamIsPartial>::is_partial_supported() {
                    take_until1_::<_, _, _, true>(i, literal.clone())
                } else {
                    take_until1_::<_, _, _, false>(i, literal.clone())
                }
            }
            (start, end) => {
                let end = end.unwrap_or(usize::MAX);
                if <Input as StreamIsPartial>::is_partial_supported() {
                    take_until_m_n_::<_, _, _, true>(i, start, end, literal.clone())
                } else {
                    take_until_m_n_::<_, _, _, false>(i, start, end, literal.clone())
                }
            }
        }
    })
}

fn take_until0_<T, I, Error: ParserError<I>, const PARTIAL: bool>(
    i: &mut I,
    t: T,
) -> Result<<I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream + FindSlice<T>,
{
    match i.find_slice(t) {
        Some(range) => Ok(i.next_slice(range.start)),
        None if PARTIAL && i.is_partial() => Err(ParserError::incomplete(i, Needed::Unknown)),
        None => Err(ParserError::from_input(i)),
    }
}

fn take_until1_<T, I, Error: ParserError<I>, const PARTIAL: bool>(
    i: &mut I,
    t: T,
) -> Result<<I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream + FindSlice<T>,
{
    match i.find_slice(t) {
        None if PARTIAL && i.is_partial() => Err(ParserError::incomplete(i, Needed::Unknown)),
        None => Err(ParserError::from_input(i)),
        Some(range) => {
            if range.start == 0 {
                Err(ParserError::from_input(i))
            } else {
                Ok(i.next_slice(range.start))
            }
        }
    }
}

fn take_until_m_n_<T, I, Error: ParserError<I>, const PARTIAL: bool>(
    i: &mut I,
    start: usize,
    end: usize,
    t: T,
) -> Result<<I as Stream>::Slice, Error>
where
    I: StreamIsPartial,
    I: Stream + FindSlice<T>,
{
    if end < start {
        return Err(ParserError::assert(
            i,
            "`occurrences` should be ascending, rather than descending",
        ));
    }

    match i.find_slice(t) {
        Some(range) => {
            let start_offset = i.offset_at(start);
            let end_offset = i.offset_at(end).unwrap_or_else(|_err| i.eof_offset());
            if start_offset.map(|s| range.start < s).unwrap_or(true) {
                if PARTIAL && i.is_partial() {
                    return Err(ParserError::incomplete(i, Needed::Unknown));
                } else {
                    return Err(ParserError::from_input(i));
                }
            }
            if end_offset < range.start {
                return Err(ParserError::from_input(i));
            }
            Ok(i.next_slice(range.start))
        }
        None if PARTIAL && i.is_partial() => Err(ParserError::incomplete(i, Needed::Unknown)),
        None => Err(ParserError::from_input(i)),
    }
}

/// Return the remaining input.
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn rest<'i>(input: &mut &'i str) -> ModalResult<&'i str>
/// # {
/// #     winnow::token::rest.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::error::ContextError;
/// use winnow::token::rest;
/// assert_eq!(rest::<_,ContextError>.parse_peek("abc"), Ok(("", "abc")));
/// assert_eq!(rest::<_,ContextError>.parse_peek(""), Ok(("", "")));
/// ```
#[inline]
pub fn rest<Input, Error>(input: &mut Input) -> Result<<Input as Stream>::Slice, Error>
where
    Input: Stream,
    Error: ParserError<Input>,
{
    trace("rest", move |input: &mut Input| Ok(input.finish())).parse_next(input)
}

/// Return the length of the remaining input.
///
/// <div class="warning">
///
/// Note: this does not advance the [`Stream`]
///
/// </div>
///
/// # Effective Signature
///
/// Assuming you are parsing a `&str` [Stream]:
/// ```rust
/// # use winnow::prelude::*;;
/// pub fn rest_len(input: &mut &str) -> ModalResult<usize>
/// # {
/// #     winnow::token::rest_len.parse_next(input)
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use winnow::prelude::*;
/// # use winnow::error::ContextError;
/// use winnow::token::rest_len;
/// assert_eq!(rest_len::<_,ContextError>.parse_peek("abc"), Ok(("abc", 3)));
/// assert_eq!(rest_len::<_,ContextError>.parse_peek(""), Ok(("", 0)));
/// ```
#[inline]
pub fn rest_len<Input, Error>(input: &mut Input) -> Result<usize, Error>
where
    Input: Stream,
    Error: ParserError<Input>,
{
    trace("rest_len", move |input: &mut Input| {
        let len = input.eof_offset();
        Ok(len)
    })
    .parse_next(input)
}

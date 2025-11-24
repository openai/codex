use crate::combinator::trace;
use crate::error::ParserError;
use crate::stream::Stream;
use crate::*;

#[doc(inline)]
pub use crate::seq;

/// Sequence two parsers, only returning the output from the second.
///
/// See also [`seq`] to generalize this across any number of fields.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::combinator::preceded;
///
/// fn parser<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
///     preceded("abc", "efg").parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("abcefg"), Ok(("", "efg")));
/// assert_eq!(parser.parse_peek("abcefghij"), Ok(("hij", "efg")));
/// assert!(parser.parse_peek("").is_err());
/// assert!(parser.parse_peek("123").is_err());
/// ```
#[doc(alias = "ignore_then")]
pub fn preceded<Input, Ignored, Output, Error, IgnoredParser, ParseNext>(
    mut ignored: IgnoredParser,
    mut parser: ParseNext,
) -> impl Parser<Input, Output, Error>
where
    Input: Stream,
    Error: ParserError<Input>,
    IgnoredParser: Parser<Input, Ignored, Error>,
    ParseNext: Parser<Input, Output, Error>,
{
    trace("preceded", move |input: &mut Input| {
        let _ = ignored.parse_next(input)?;
        parser.parse_next(input)
    })
}

/// Sequence two parsers, only returning the output of the first.
///
/// See also [`seq`] to generalize this across any number of fields.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::prelude::*;
/// # use winnow::error::Needed::Size;
/// use winnow::combinator::terminated;
///
/// fn parser<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
///     terminated("abc", "efg").parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("abcefg"), Ok(("", "abc")));
/// assert_eq!(parser.parse_peek("abcefghij"), Ok(("hij", "abc")));
/// assert!(parser.parse_peek("").is_err());
/// assert!(parser.parse_peek("123").is_err());
/// ```
#[doc(alias = "then_ignore")]
pub fn terminated<Input, Output, Ignored, Error, ParseNext, IgnoredParser>(
    mut parser: ParseNext,
    mut ignored: IgnoredParser,
) -> impl Parser<Input, Output, Error>
where
    Input: Stream,
    Error: ParserError<Input>,
    ParseNext: Parser<Input, Output, Error>,
    IgnoredParser: Parser<Input, Ignored, Error>,
{
    trace("terminated", move |input: &mut Input| {
        let o = parser.parse_next(input)?;
        ignored.parse_next(input).map(|_| o)
    })
}

/// Sequence three parsers, only returning the values of the first and third.
///
/// See also [`seq`] to generalize this across any number of fields.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::error::Needed::Size;
/// # use winnow::prelude::*;
/// use winnow::combinator::separated_pair;
///
/// fn parser<'i>(input: &mut &'i str) -> ModalResult<(&'i str, &'i str)> {
///     separated_pair("abc", "|", "efg").parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("abc|efg"), Ok(("", ("abc", "efg"))));
/// assert_eq!(parser.parse_peek("abc|efghij"), Ok(("hij", ("abc", "efg"))));
/// assert!(parser.parse_peek("").is_err());
/// assert!(parser.parse_peek("123").is_err());
/// ```
pub fn separated_pair<Input, O1, Sep, O2, Error, P1, SepParser, P2>(
    mut first: P1,
    mut sep: SepParser,
    mut second: P2,
) -> impl Parser<Input, (O1, O2), Error>
where
    Input: Stream,
    Error: ParserError<Input>,
    P1: Parser<Input, O1, Error>,
    SepParser: Parser<Input, Sep, Error>,
    P2: Parser<Input, O2, Error>,
{
    trace("separated_pair", move |input: &mut Input| {
        let o1 = first.parse_next(input)?;
        let _ = sep.parse_next(input)?;
        second.parse_next(input).map(|o2| (o1, o2))
    })
}

/// Sequence three parsers, only returning the output of the second.
///
/// See also [`seq`] to generalize this across any number of fields.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::error::Needed::Size;
/// # use winnow::prelude::*;
/// use winnow::combinator::delimited;
///
/// fn parser<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
///     delimited("(", "abc", ")").parse_next(input)
/// }
///
/// assert_eq!(parser.parse_peek("(abc)"), Ok(("", "abc")));
/// assert_eq!(parser.parse_peek("(abc)def"), Ok(("def", "abc")));
/// assert!(parser.parse_peek("").is_err());
/// assert!(parser.parse_peek("123").is_err());
/// ```
#[doc(alias = "between")]
#[doc(alias = "padded")]
pub fn delimited<
    Input,
    Ignored1,
    Output,
    Ignored2,
    Error,
    IgnoredParser1,
    ParseNext,
    IgnoredParser2,
>(
    mut ignored1: IgnoredParser1,
    mut parser: ParseNext,
    mut ignored2: IgnoredParser2,
) -> impl Parser<Input, Output, Error>
where
    Input: Stream,
    Error: ParserError<Input>,
    IgnoredParser1: Parser<Input, Ignored1, Error>,
    ParseNext: Parser<Input, Output, Error>,
    IgnoredParser2: Parser<Input, Ignored2, Error>,
{
    trace("delimited", move |input: &mut Input| {
        let _ = ignored1.parse_next(input)?;
        let o2 = parser.parse_next(input)?;
        ignored2.parse_next(input).map(|_| o2)
    })
}

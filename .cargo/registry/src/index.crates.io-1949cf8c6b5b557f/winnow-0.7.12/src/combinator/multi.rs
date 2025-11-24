//! Combinators applying their child parser multiple times

use crate::combinator::trace;
use crate::error::FromExternalError;
use crate::error::ParserError;
use crate::stream::Accumulate;
use crate::stream::Range;
use crate::stream::Stream;
use crate::Parser;
use crate::Result;

/// [`Accumulate`] the output of a parser into a container, like `Vec`
///
/// This stops before `n` when the parser returns [`ErrMode::Backtrack`][crate::error::ErrMode::Backtrack]. To instead chain an error up, see
/// [`cut_err`][crate::combinator::cut_err].
///
/// To take a series of tokens, [`Accumulate`] into a `()`
/// (e.g. with [`.map(|()| ())`][Parser::map])
/// and then [`Parser::take`].
///
/// <div class="warning">
///
/// **Warning:** If the parser passed to `repeat` accepts empty inputs
/// (like `alpha0` or `digit0`), `repeat` will return an error,
/// to prevent going into an infinite loop.
///
/// </div>
///
/// # Example
///
/// Zero or more repetitions:
/// ```rust
/// # #[cfg(feature = "std")] {
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::combinator::repeat;
///
/// fn parser<'i>(s: &mut &'i str) -> ModalResult<Vec<&'i str>> {
///   repeat(0.., "abc").parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek("abcabc"), Ok(("", vec!["abc", "abc"])));
/// assert_eq!(parser.parse_peek("abc123"), Ok(("123", vec!["abc"])));
/// assert_eq!(parser.parse_peek("123123"), Ok(("123123", vec![])));
/// assert_eq!(parser.parse_peek(""), Ok(("", vec![])));
/// # }
/// ```
///
/// One or more repetitions:
/// ```rust
/// # #[cfg(feature = "std")] {
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::combinator::repeat;
///
/// fn parser<'i>(s: &mut &'i str) -> ModalResult<Vec<&'i str>> {
///   repeat(1.., "abc").parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek("abcabc"), Ok(("", vec!["abc", "abc"])));
/// assert_eq!(parser.parse_peek("abc123"), Ok(("123", vec!["abc"])));
/// assert!(parser.parse_peek("123123").is_err());
/// assert!(parser.parse_peek("").is_err());
/// # }
/// ```
///
/// Fixed number of repetitions:
/// ```rust
/// # #[cfg(feature = "std")] {
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::combinator::repeat;
///
/// fn parser<'i>(s: &mut &'i str) -> ModalResult<Vec<&'i str>> {
///   repeat(2, "abc").parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek("abcabc"), Ok(("", vec!["abc", "abc"])));
/// assert!(parser.parse_peek("abc123").is_err());
/// assert!(parser.parse_peek("123123").is_err());
/// assert!(parser.parse_peek("").is_err());
/// assert_eq!(parser.parse_peek("abcabcabc"), Ok(("abc", vec!["abc", "abc"])));
/// # }
/// ```
///
/// Arbitrary repetitions:
/// ```rust
/// # #[cfg(feature = "std")] {
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::combinator::repeat;
///
/// fn parser<'i>(s: &mut &'i str) -> ModalResult<Vec<&'i str>> {
///   repeat(0..=2, "abc").parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek("abcabc"), Ok(("", vec!["abc", "abc"])));
/// assert_eq!(parser.parse_peek("abc123"), Ok(("123", vec!["abc"])));
/// assert_eq!(parser.parse_peek("123123"), Ok(("123123", vec![])));
/// assert_eq!(parser.parse_peek(""), Ok(("", vec![])));
/// assert_eq!(parser.parse_peek("abcabcabc"), Ok(("abc", vec!["abc", "abc"])));
/// # }
/// ```
#[doc(alias = "many0")]
#[doc(alias = "count")]
#[doc(alias = "many0_count")]
#[doc(alias = "many1")]
#[doc(alias = "many1_count")]
#[doc(alias = "many_m_n")]
#[doc(alias = "repeated")]
#[doc(alias = "skip_many")]
#[doc(alias = "skip_many1")]
#[inline(always)]
pub fn repeat<Input, Output, Accumulator, Error, ParseNext>(
    occurrences: impl Into<Range>,
    parser: ParseNext,
) -> Repeat<ParseNext, Input, Output, Accumulator, Error>
where
    Input: Stream,
    Accumulator: Accumulate<Output>,
    ParseNext: Parser<Input, Output, Error>,
    Error: ParserError<Input>,
{
    Repeat {
        occurrences: occurrences.into(),
        parser,
        i: Default::default(),
        o: Default::default(),
        c: Default::default(),
        e: Default::default(),
    }
}

/// Customizable [`Parser`] implementation for [`repeat`]
pub struct Repeat<P, I, O, C, E>
where
    P: Parser<I, O, E>,
    I: Stream,
    C: Accumulate<O>,
    E: ParserError<I>,
{
    occurrences: Range,
    parser: P,
    i: core::marker::PhantomData<I>,
    o: core::marker::PhantomData<O>,
    c: core::marker::PhantomData<C>,
    e: core::marker::PhantomData<E>,
}

impl<ParseNext, Input, Output, Error> Repeat<ParseNext, Input, Output, (), Error>
where
    ParseNext: Parser<Input, Output, Error>,
    Input: Stream,
    Error: ParserError<Input>,
{
    /// Repeats the embedded parser, calling `op` to gather the results
    ///
    /// This stops before `n` when the parser returns [`ErrMode::Backtrack`][crate::error::ErrMode::Backtrack]. To instead chain an error up, see
    /// [`cut_err`][crate::combinator::cut_err].
    ///
    /// # Arguments
    /// * `init` A function returning the initial value.
    /// * `op` The function that combines a result of `f` with
    ///   the current accumulator.
    ///
    /// <div class="warning">
    ///
    /// **Warning:** If the parser passed to [`repeat`] accepts empty inputs
    /// (like `alpha0` or `digit0`), `fold` will return an error,
    /// to prevent going into an infinite loop.
    ///
    /// </div>
    ///
    /// # Example
    ///
    /// Zero or more repetitions:
    /// ```rust
    /// # use winnow::{error::ErrMode, error::Needed};
    /// # use winnow::prelude::*;
    /// use winnow::combinator::repeat;
    ///
    /// fn parser<'i>(s: &mut &'i str) -> ModalResult<Vec<&'i str>> {
    ///   repeat(
    ///     0..,
    ///     "abc"
    ///   ).fold(
    ///     Vec::new,
    ///     |mut acc: Vec<_>, item| {
    ///       acc.push(item);
    ///       acc
    ///     }
    ///   ).parse_next(s)
    /// }
    ///
    /// assert_eq!(parser.parse_peek("abcabc"), Ok(("", vec!["abc", "abc"])));
    /// assert_eq!(parser.parse_peek("abc123"), Ok(("123", vec!["abc"])));
    /// assert_eq!(parser.parse_peek("123123"), Ok(("123123", vec![])));
    /// assert_eq!(parser.parse_peek(""), Ok(("", vec![])));
    /// ```
    ///
    /// One or more repetitions:
    /// ```rust
    /// # use winnow::{error::ErrMode, error::Needed};
    /// # use winnow::prelude::*;
    /// use winnow::combinator::repeat;
    ///
    /// fn parser<'i>(s: &mut &'i str) -> ModalResult<Vec<&'i str>> {
    ///   repeat(
    ///     1..,
    ///     "abc",
    ///   ).fold(
    ///     Vec::new,
    ///     |mut acc: Vec<_>, item| {
    ///       acc.push(item);
    ///       acc
    ///     }
    ///   ).parse_next(s)
    /// }
    ///
    /// assert_eq!(parser.parse_peek("abcabc"), Ok(("", vec!["abc", "abc"])));
    /// assert_eq!(parser.parse_peek("abc123"), Ok(("123", vec!["abc"])));
    /// assert!(parser.parse_peek("123123").is_err());
    /// assert!(parser.parse_peek("").is_err());
    /// ```
    ///
    /// Arbitrary number of repetitions:
    /// ```rust
    /// # use winnow::{error::ErrMode, error::Needed};
    /// # use winnow::prelude::*;
    /// use winnow::combinator::repeat;
    ///
    /// fn parser<'i>(s: &mut &'i str) -> ModalResult<Vec<&'i str>> {
    ///   repeat(
    ///     0..=2,
    ///     "abc",
    ///   ).fold(
    ///     Vec::new,
    ///     |mut acc: Vec<_>, item| {
    ///       acc.push(item);
    ///       acc
    ///     }
    ///   ).parse_next(s)
    /// }
    ///
    /// assert_eq!(parser.parse_peek("abcabc"), Ok(("", vec!["abc", "abc"])));
    /// assert_eq!(parser.parse_peek("abc123"), Ok(("123", vec!["abc"])));
    /// assert_eq!(parser.parse_peek("123123"), Ok(("123123", vec![])));
    /// assert_eq!(parser.parse_peek(""), Ok(("", vec![])));
    /// assert_eq!(parser.parse_peek("abcabcabc"), Ok(("abc", vec!["abc", "abc"])));
    /// ```
    #[doc(alias = "fold_many0")]
    #[doc(alias = "fold_many1")]
    #[doc(alias = "fold_many_m_n")]
    #[doc(alias = "fold_repeat")]
    #[inline(always)]
    pub fn fold<Init, Op, Result>(
        mut self,
        mut init: Init,
        mut op: Op,
    ) -> impl Parser<Input, Result, Error>
    where
        Init: FnMut() -> Result,
        Op: FnMut(Result, Output) -> Result,
    {
        let Range {
            start_inclusive,
            end_inclusive,
        } = self.occurrences;
        trace("repeat_fold", move |i: &mut Input| {
            match (start_inclusive, end_inclusive) {
                (0, None) => fold_repeat0_(&mut self.parser, &mut init, &mut op, i),
                (1, None) => fold_repeat1_(&mut self.parser, &mut init, &mut op, i),
                (start, end) if Some(start) == end => {
                    fold_repeat_n_(start, &mut self.parser, &mut init, &mut op, i)
                }
                (start, end) => fold_repeat_m_n_(
                    start,
                    end.unwrap_or(usize::MAX),
                    &mut self.parser,
                    &mut init,
                    &mut op,
                    i,
                ),
            }
        })
    }

    /// Akin to [`Repeat::fold`], but for containers that can reject an element.
    ///
    /// This stops before `n` when the parser returns [`ErrMode::Backtrack`][crate::error::ErrMode::Backtrack]. To instead chain an error up, see
    /// [`cut_err`][crate::combinator::cut_err]. Additionally, if the fold function returns `None`, the parser will
    /// stop and return an error.
    ///
    /// # Arguments
    /// * `init` A function returning the initial value.
    /// * `op` The function that combines a result of `f` with
    ///   the current accumulator.
    ///
    /// <div class="warning">
    ///
    /// **Warning:** If the parser passed to [`repeat`] accepts empty inputs
    /// (like `alpha0` or `digit0`), `verify_fold` will return an error,
    /// to prevent going into an infinite loop.
    ///
    /// </div>
    ///
    /// # Example
    ///
    /// Guaranteeing that the input had unique elements:
    /// ```rust
    /// # use winnow::{error::ErrMode, error::Needed};
    /// # use winnow::prelude::*;
    /// use winnow::combinator::repeat;
    /// use std::collections::HashSet;
    ///
    /// fn parser<'i>(s: &mut &'i str) -> ModalResult<HashSet<&'i str>> {
    ///   repeat(
    ///     0..,
    ///     "abc"
    ///   ).verify_fold(
    ///     HashSet::new,
    ///     |mut acc: HashSet<_>, item| {
    ///       if acc.insert(item) {
    ///          Some(acc)
    ///       } else {
    ///          None
    ///       }
    ///     }
    ///   ).parse_next(s)
    /// }
    ///
    /// assert_eq!(parser.parse_peek("abc"), Ok(("", HashSet::from(["abc"]))));
    /// assert!(parser.parse_peek("abcabc").is_err());
    /// assert_eq!(parser.parse_peek("abc123"), Ok(("123", HashSet::from(["abc"]))));
    /// assert_eq!(parser.parse_peek("123123"), Ok(("123123", HashSet::from([]))));
    /// assert_eq!(parser.parse_peek(""), Ok(("", HashSet::from([]))));
    /// ```
    #[inline(always)]
    pub fn verify_fold<Init, Op, Result>(
        mut self,
        mut init: Init,
        mut op: Op,
    ) -> impl Parser<Input, Result, Error>
    where
        Init: FnMut() -> Result,
        Op: FnMut(Result, Output) -> Option<Result>,
    {
        let Range {
            start_inclusive,
            end_inclusive,
        } = self.occurrences;
        trace("repeat_verify_fold", move |input: &mut Input| {
            verify_fold_m_n(
                start_inclusive,
                end_inclusive.unwrap_or(usize::MAX),
                &mut self.parser,
                &mut init,
                &mut op,
                input,
            )
        })
    }

    /// Akin to [`Repeat::fold`], but for containers that can error when an element is accumulated.
    ///
    /// This stops before `n` when the parser returns [`ErrMode::Backtrack`][crate::error::ErrMode::Backtrack]. To instead chain an error up, see
    /// [`cut_err`][crate::combinator::cut_err]. Additionally, if the fold function returns an error, the parser will
    /// stop and return it.
    ///
    /// # Arguments
    /// * `init` A function returning the initial value.
    /// * `op` The function that combines a result of `f` with
    ///   the current accumulator.
    ///
    /// <div class="warning">
    ///
    /// **Warning:** If the parser passed to [`repeat`] accepts empty inputs
    /// (like `alpha0` or `digit0`), `try_fold` will return an error,
    /// to prevent going into an infinite loop.
    ///
    /// </div>
    ///
    /// # Example
    ///
    /// Writing the output to a vector of bytes:
    /// ```rust
    /// # use winnow::{error::ErrMode, error::Needed};
    /// # use winnow::prelude::*;
    /// use winnow::combinator::repeat;
    /// use std::io::Write;
    /// use std::io::Error;
    ///
    /// fn parser(s: &mut &str) -> ModalResult<Vec<u8>> {
    ///   repeat(
    ///     0..,
    ///     "abc"
    ///   ).try_fold(
    ///     Vec::new,
    ///     |mut acc, item: &str| -> Result<_, Error> {
    ///       acc.write(item.as_bytes())?;
    ///       Ok(acc)
    ///     }
    ///   ).parse_next(s)
    /// }
    ///
    /// assert_eq!(parser.parse_peek("abc"), Ok(("", b"abc".to_vec())));
    /// assert_eq!(parser.parse_peek("abc123"), Ok(("123", b"abc".to_vec())));
    /// assert_eq!(parser.parse_peek("123123"), Ok(("123123", vec![])));
    /// assert_eq!(parser.parse_peek(""), Ok(("", vec![])));
    #[inline(always)]
    pub fn try_fold<Init, Op, OpError, Result>(
        mut self,
        mut init: Init,
        mut op: Op,
    ) -> impl Parser<Input, Result, Error>
    where
        Init: FnMut() -> Result,
        Op: FnMut(Result, Output) -> core::result::Result<Result, OpError>,
        Error: FromExternalError<Input, OpError>,
    {
        let Range {
            start_inclusive,
            end_inclusive,
        } = self.occurrences;
        trace("repeat_try_fold", move |input: &mut Input| {
            try_fold_m_n(
                start_inclusive,
                end_inclusive.unwrap_or(usize::MAX),
                &mut self.parser,
                &mut init,
                &mut op,
                input,
            )
        })
    }
}

impl<P, I, O, C, E> Parser<I, C, E> for Repeat<P, I, O, C, E>
where
    P: Parser<I, O, E>,
    I: Stream,
    C: Accumulate<O>,
    E: ParserError<I>,
{
    #[inline(always)]
    fn parse_next(&mut self, i: &mut I) -> Result<C, E> {
        let Range {
            start_inclusive,
            end_inclusive,
        } = self.occurrences;
        trace("repeat", move |i: &mut I| {
            match (start_inclusive, end_inclusive) {
                (0, None) => fold_repeat0_(
                    &mut self.parser,
                    &mut || C::initial(None),
                    &mut |mut acc, o| {
                        acc.accumulate(o);
                        acc
                    },
                    i,
                ),
                (1, None) => fold_repeat1_(
                    &mut self.parser,
                    &mut || C::initial(None),
                    &mut |mut acc, o| {
                        acc.accumulate(o);
                        acc
                    },
                    i,
                ),
                (min, end) if Some(min) == end => fold_repeat_n_(
                    min,
                    &mut self.parser,
                    &mut || C::initial(Some(min)),
                    &mut |mut acc, o| {
                        acc.accumulate(o);
                        acc
                    },
                    i,
                ),
                (min, end) => fold_repeat_m_n_(
                    min,
                    end.unwrap_or(usize::MAX),
                    &mut self.parser,
                    &mut || C::initial(Some(min)),
                    &mut |mut acc, o| {
                        acc.accumulate(o);
                        acc
                    },
                    i,
                ),
            }
        })
        .parse_next(i)
    }
}

fn fold_repeat0_<I, O, E, P, N, F, R>(
    parser: &mut P,
    init: &mut N,
    fold: &mut F,
    input: &mut I,
) -> Result<R, E>
where
    I: Stream,
    P: Parser<I, O, E>,
    N: FnMut() -> R,
    F: FnMut(R, O) -> R,
    E: ParserError<I>,
{
    let mut res = init();

    loop {
        let start = input.checkpoint();
        let len = input.eof_offset();
        match parser.parse_next(input) {
            Ok(output) => {
                // infinite loop check: the parser must always consume
                if input.eof_offset() == len {
                    return Err(ParserError::assert(
                        input,
                        "`repeat` parsers must always consume",
                    ));
                }

                res = fold(res, output);
            }
            Err(err) if err.is_backtrack() => {
                input.reset(&start);
                return Ok(res);
            }
            Err(err) => {
                return Err(err);
            }
        }
    }
}

fn fold_repeat1_<I, O, E, P, N, F, R>(
    parser: &mut P,
    init: &mut N,
    fold: &mut F,
    input: &mut I,
) -> Result<R, E>
where
    I: Stream,
    P: Parser<I, O, E>,
    N: FnMut() -> R,
    F: FnMut(R, O) -> R,
    E: ParserError<I>,
{
    let start = input.checkpoint();
    match parser.parse_next(input) {
        Err(err) => Err(err.append(input, &start)),
        Ok(output) => {
            let init = init();
            let mut res = fold(init, output);

            loop {
                let start = input.checkpoint();
                let len = input.eof_offset();
                match parser.parse_next(input) {
                    Err(err) if err.is_backtrack() => {
                        input.reset(&start);
                        break;
                    }
                    Err(err) => return Err(err),
                    Ok(output) => {
                        // infinite loop check: the parser must always consume
                        if input.eof_offset() == len {
                            return Err(ParserError::assert(
                                input,
                                "`repeat` parsers must always consume",
                            ));
                        }

                        res = fold(res, output);
                    }
                }
            }

            Ok(res)
        }
    }
}

fn fold_repeat_n_<I, O, E, P, N, F, R>(
    count: usize,
    parse: &mut P,
    init: &mut N,
    fold: &mut F,
    input: &mut I,
) -> Result<R, E>
where
    I: Stream,
    P: Parser<I, O, E>,
    N: FnMut() -> R,
    F: FnMut(R, O) -> R,
    E: ParserError<I>,
{
    let mut res = init();

    for _ in 0..count {
        let start = input.checkpoint();
        let len = input.eof_offset();
        match parse.parse_next(input) {
            Ok(output) => {
                // infinite loop check: the parser must always consume
                if input.eof_offset() == len {
                    return Err(ParserError::assert(
                        input,
                        "`repeat` parsers must always consume",
                    ));
                }

                res = fold(res, output);
            }
            Err(err) => {
                return Err(err.append(input, &start));
            }
        }
    }

    Ok(res)
}

fn fold_repeat_m_n_<I, O, E, P, N, F, R>(
    min: usize,
    max: usize,
    parse: &mut P,
    init: &mut N,
    fold: &mut F,
    input: &mut I,
) -> Result<R, E>
where
    I: Stream,
    P: Parser<I, O, E>,
    N: FnMut() -> R,
    F: FnMut(R, O) -> R,
    E: ParserError<I>,
{
    if min > max {
        return Err(ParserError::assert(
            input,
            "range should be ascending, rather than descending",
        ));
    }

    let mut res = init();
    for count in 0..max {
        let start = input.checkpoint();
        let len = input.eof_offset();
        match parse.parse_next(input) {
            Ok(output) => {
                // infinite loop check: the parser must always consume
                if input.eof_offset() == len {
                    return Err(ParserError::assert(
                        input,
                        "`repeat` parsers must always consume",
                    ));
                }

                res = fold(res, output);
            }
            //FInputXMError: handle failure properly
            Err(err) if err.is_backtrack() => {
                if count < min {
                    return Err(err.append(input, &start));
                } else {
                    input.reset(&start);
                    break;
                }
            }
            Err(err) => return Err(err),
        }
    }

    Ok(res)
}

fn verify_fold_m_n<I, O, E, P, N, F, R>(
    min: usize,
    max: usize,
    parse: &mut P,
    init: &mut N,
    fold: &mut F,
    input: &mut I,
) -> Result<R, E>
where
    I: Stream,
    P: Parser<I, O, E>,
    N: FnMut() -> R,
    F: FnMut(R, O) -> Option<R>,
    E: ParserError<I>,
{
    if min > max {
        return Err(ParserError::assert(
            input,
            "range should be ascending, rather than descending",
        ));
    }

    let mut res = init();
    for count in 0..max {
        let start = input.checkpoint();
        let len = input.eof_offset();
        match parse.parse_next(input) {
            Ok(output) => {
                // infinite loop check: the parser must always consume
                if input.eof_offset() == len {
                    return Err(ParserError::assert(
                        input,
                        "`repeat` parsers must always consume",
                    ));
                }

                let Some(res_) = fold(res, output) else {
                    input.reset(&start);
                    let res = Err(ParserError::from_input(input));
                    super::debug::trace_result("verify_fold", &res);
                    return res;
                };
                res = res_;
            }
            //FInputXMError: handle failure properly
            Err(err) if err.is_backtrack() => {
                if count < min {
                    return Err(err.append(input, &start));
                } else {
                    input.reset(&start);
                    break;
                }
            }
            Err(err) => return Err(err),
        }
    }

    Ok(res)
}

fn try_fold_m_n<I, O, E, P, N, F, R, RE>(
    min: usize,
    max: usize,
    parse: &mut P,
    init: &mut N,
    fold: &mut F,
    input: &mut I,
) -> Result<R, E>
where
    I: Stream,
    P: Parser<I, O, E>,
    N: FnMut() -> R,
    F: FnMut(R, O) -> Result<R, RE>,
    E: ParserError<I> + FromExternalError<I, RE>,
{
    if min > max {
        return Err(ParserError::assert(
            input,
            "range should be ascending, rather than descending",
        ));
    }

    let mut res = init();
    for count in 0..max {
        let start = input.checkpoint();
        let len = input.eof_offset();
        match parse.parse_next(input) {
            Ok(output) => {
                // infinite loop check: the parser must always consume
                if input.eof_offset() == len {
                    return Err(ParserError::assert(
                        input,
                        "`repeat` parsers must always consume",
                    ));
                }

                match fold(res, output) {
                    Ok(res_) => res = res_,
                    Err(err) => {
                        input.reset(&start);
                        let res = Err(E::from_external_error(input, err));
                        super::debug::trace_result("try_fold", &res);
                        return res;
                    }
                }
            }
            //FInputXMError: handle failure properly
            Err(err) if err.is_backtrack() => {
                if count < min {
                    return Err(err.append(input, &start));
                } else {
                    input.reset(&start);
                    break;
                }
            }
            Err(err) => return Err(err),
        }
    }

    Ok(res)
}

/// [`Accumulate`] the output of parser `f` into a container, like `Vec`, until the parser `g`
/// produces a result.
///
/// Returns a tuple of the results of `f` in a `Vec` and the result of `g`.
///
/// `f` keeps going so long as `g` produces [`ErrMode::Backtrack`][crate::error::ErrMode::Backtrack]. To instead chain an error up, see [`cut_err`][crate::combinator::cut_err].
///
/// To take a series of tokens, [`Accumulate`] into a `()`
/// (e.g. with [`.map(|((), _)| ())`][Parser::map])
/// and then [`Parser::take`].
///
/// See also
/// - [`take_till`][crate::token::take_till] for recognizing up-to a member of a [set of tokens][crate::stream::ContainsToken]
/// - [`take_until`][crate::token::take_until] for recognizing up-to a [`literal`][crate::token::literal] (w/ optional simd optimizations)
///
/// # Example
///
/// ```rust
/// # #[cfg(feature = "std")] {
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::combinator::repeat_till;
///
/// fn parser<'i>(s: &mut &'i str) -> ModalResult<(Vec<&'i str>, &'i str)> {
///   repeat_till(0.., "abc", "end").parse_next(s)
/// };
///
/// assert_eq!(parser.parse_peek("abcabcend"), Ok(("", (vec!["abc", "abc"], "end"))));
/// assert!(parser.parse_peek("abc123end").is_err());
/// assert!(parser.parse_peek("123123end").is_err());
/// assert!(parser.parse_peek("").is_err());
/// assert_eq!(parser.parse_peek("abcendefg"), Ok(("efg", (vec!["abc"], "end"))));
/// # }
/// ```
#[doc(alias = "many_till0")]
pub fn repeat_till<Input, Output, Accumulator, Terminator, Error, ParseNext, TerminatorParser>(
    occurrences: impl Into<Range>,
    mut parse: ParseNext,
    mut terminator: TerminatorParser,
) -> impl Parser<Input, (Accumulator, Terminator), Error>
where
    Input: Stream,
    Accumulator: Accumulate<Output>,
    ParseNext: Parser<Input, Output, Error>,
    TerminatorParser: Parser<Input, Terminator, Error>,
    Error: ParserError<Input>,
{
    let Range {
        start_inclusive,
        end_inclusive,
    } = occurrences.into();
    trace("repeat_till", move |i: &mut Input| {
        match (start_inclusive, end_inclusive) {
            (0, None) => repeat_till0_(&mut parse, &mut terminator, i),
            (start, end) => repeat_till_m_n_(
                start,
                end.unwrap_or(usize::MAX),
                &mut parse,
                &mut terminator,
                i,
            ),
        }
    })
}

fn repeat_till0_<I, O, C, P, E, F, G>(f: &mut F, g: &mut G, i: &mut I) -> Result<(C, P), E>
where
    I: Stream,
    C: Accumulate<O>,
    F: Parser<I, O, E>,
    G: Parser<I, P, E>,
    E: ParserError<I>,
{
    let mut res = C::initial(None);
    loop {
        let start = i.checkpoint();
        let len = i.eof_offset();
        match g.parse_next(i) {
            Ok(o) => return Ok((res, o)),
            Err(e) if e.is_backtrack() => {
                i.reset(&start);
                match f.parse_next(i) {
                    Err(e) => return Err(e.append(i, &start)),
                    Ok(o) => {
                        // infinite loop check: the parser must always consume
                        if i.eof_offset() == len {
                            return Err(ParserError::assert(
                                i,
                                "`repeat` parsers must always consume",
                            ));
                        }

                        res.accumulate(o);
                    }
                }
            }
            Err(e) => return Err(e),
        }
    }
}

fn repeat_till_m_n_<I, O, C, P, E, F, G>(
    min: usize,
    max: usize,
    f: &mut F,
    g: &mut G,
    i: &mut I,
) -> Result<(C, P), E>
where
    I: Stream,
    C: Accumulate<O>,
    F: Parser<I, O, E>,
    G: Parser<I, P, E>,
    E: ParserError<I>,
{
    if min > max {
        return Err(ParserError::assert(
            i,
            "range should be ascending, rather than descending",
        ));
    }

    let mut res = C::initial(Some(min));

    let start = i.checkpoint();
    for _ in 0..min {
        match f.parse_next(i) {
            Ok(o) => {
                res.accumulate(o);
            }
            Err(e) => {
                return Err(e.append(i, &start));
            }
        }
    }
    for count in min..=max {
        let start = i.checkpoint();
        let len = i.eof_offset();
        match g.parse_next(i) {
            Ok(o) => return Ok((res, o)),
            Err(err) if err.is_backtrack() => {
                if count == max {
                    return Err(err);
                }
                i.reset(&start);
                match f.parse_next(i) {
                    Err(e) => {
                        return Err(e.append(i, &start));
                    }
                    Ok(o) => {
                        // infinite loop check: the parser must always consume
                        if i.eof_offset() == len {
                            return Err(ParserError::assert(
                                i,
                                "`repeat` parsers must always consume",
                            ));
                        }

                        res.accumulate(o);
                    }
                }
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}

/// [`Accumulate`] the output of a parser, interleaved with `sep`
///
/// This stops when either parser returns [`ErrMode::Backtrack`][crate::error::ErrMode::Backtrack]. To instead chain an error up, see
/// [`cut_err`][crate::combinator::cut_err].
///
/// To take a series of tokens, [`Accumulate`] into a `()`
/// (e.g. with [`.map(|()| ())`][Parser::map])
/// and then [`Parser::take`].
///
/// <div class="warning">
///
/// **Warning:** If the separator parser accepts empty inputs
/// (like `alpha0` or `digit0`), `separated` will return an error,
/// to prevent going into an infinite loop.
///
/// </div>
///
/// # Example
///
/// Zero or more repetitions:
/// ```rust
/// # #[cfg(feature = "std")] {
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::combinator::separated;
///
/// fn parser<'i>(s: &mut &'i str) -> ModalResult<Vec<&'i str>> {
///   separated(0.., "abc", "|").parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek("abc|abc|abc"), Ok(("", vec!["abc", "abc", "abc"])));
/// assert_eq!(parser.parse_peek("abc123abc"), Ok(("123abc", vec!["abc"])));
/// assert_eq!(parser.parse_peek("abc|def"), Ok(("|def", vec!["abc"])));
/// assert_eq!(parser.parse_peek(""), Ok(("", vec![])));
/// assert_eq!(parser.parse_peek("def|abc"), Ok(("def|abc", vec![])));
/// # }
/// ```
///
/// One or more repetitions:
/// ```rust
/// # #[cfg(feature = "std")] {
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::combinator::separated;
///
/// fn parser<'i>(s: &mut &'i str) -> ModalResult<Vec<&'i str>> {
///   separated(1.., "abc", "|").parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek("abc|abc|abc"), Ok(("", vec!["abc", "abc", "abc"])));
/// assert_eq!(parser.parse_peek("abc123abc"), Ok(("123abc", vec!["abc"])));
/// assert_eq!(parser.parse_peek("abc|def"), Ok(("|def", vec!["abc"])));
/// assert!(parser.parse_peek("").is_err());
/// assert!(parser.parse_peek("def|abc").is_err());
/// # }
/// ```
///
/// Fixed number of repetitions:
/// ```rust
/// # #[cfg(feature = "std")] {
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::combinator::separated;
///
/// fn parser<'i>(s: &mut &'i str) -> ModalResult<Vec<&'i str>> {
///   separated(2, "abc", "|").parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek("abc|abc|abc"), Ok(("|abc", vec!["abc", "abc"])));
/// assert!(parser.parse_peek("abc123abc").is_err());
/// assert!(parser.parse_peek("abc|def").is_err());
/// assert!(parser.parse_peek("").is_err());
/// assert!(parser.parse_peek("def|abc").is_err());
/// # }
/// ```
///
/// Arbitrary repetitions:
/// ```rust
/// # #[cfg(feature = "std")] {
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::combinator::separated;
///
/// fn parser<'i>(s: &mut &'i str) -> ModalResult<Vec<&'i str>> {
///   separated(0..=2, "abc", "|").parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek("abc|abc|abc"), Ok(("|abc", vec!["abc", "abc"])));
/// assert_eq!(parser.parse_peek("abc123abc"), Ok(("123abc", vec!["abc"])));
/// assert_eq!(parser.parse_peek("abc|def"), Ok(("|def", vec!["abc"])));
/// assert_eq!(parser.parse_peek(""), Ok(("", vec![])));
/// assert_eq!(parser.parse_peek("def|abc"), Ok(("def|abc", vec![])));
/// # }
/// ```
#[doc(alias = "sep_by")]
#[doc(alias = "sep_by1")]
#[doc(alias = "separated_list0")]
#[doc(alias = "separated_list1")]
#[doc(alias = "separated_m_n")]
#[inline(always)]
pub fn separated<Input, Output, Accumulator, Sep, Error, ParseNext, SepParser>(
    occurrences: impl Into<Range>,
    mut parser: ParseNext,
    mut separator: SepParser,
) -> impl Parser<Input, Accumulator, Error>
where
    Input: Stream,
    Accumulator: Accumulate<Output>,
    ParseNext: Parser<Input, Output, Error>,
    SepParser: Parser<Input, Sep, Error>,
    Error: ParserError<Input>,
{
    let Range {
        start_inclusive,
        end_inclusive,
    } = occurrences.into();
    trace("separated", move |input: &mut Input| {
        match (start_inclusive, end_inclusive) {
            (0, None) => separated0_(&mut parser, &mut separator, input),
            (1, None) => separated1_(&mut parser, &mut separator, input),
            (start, end) if Some(start) == end => {
                separated_n_(start, &mut parser, &mut separator, input)
            }
            (start, end) => separated_m_n_(
                start,
                end.unwrap_or(usize::MAX),
                &mut parser,
                &mut separator,
                input,
            ),
        }
    })
}

fn separated0_<I, O, C, O2, E, P, S>(
    parser: &mut P,
    separator: &mut S,
    input: &mut I,
) -> Result<C, E>
where
    I: Stream,
    C: Accumulate<O>,
    P: Parser<I, O, E>,
    S: Parser<I, O2, E>,
    E: ParserError<I>,
{
    let mut acc = C::initial(None);

    let start = input.checkpoint();
    match parser.parse_next(input) {
        Err(e) if e.is_backtrack() => {
            input.reset(&start);
            return Ok(acc);
        }
        Err(e) => return Err(e),
        Ok(o) => {
            acc.accumulate(o);
        }
    }

    loop {
        let start = input.checkpoint();
        let len = input.eof_offset();
        match separator.parse_next(input) {
            Err(e) if e.is_backtrack() => {
                input.reset(&start);
                return Ok(acc);
            }
            Err(e) => return Err(e),
            Ok(_) => {
                // infinite loop check
                if input.eof_offset() == len {
                    return Err(ParserError::assert(
                        input,
                        "`separated` separator parser must always consume",
                    ));
                }

                match parser.parse_next(input) {
                    Err(e) if e.is_backtrack() => {
                        input.reset(&start);
                        return Ok(acc);
                    }
                    Err(e) => return Err(e),
                    Ok(o) => {
                        acc.accumulate(o);
                    }
                }
            }
        }
    }
}

fn separated1_<I, O, C, O2, E, P, S>(
    parser: &mut P,
    separator: &mut S,
    input: &mut I,
) -> Result<C, E>
where
    I: Stream,
    C: Accumulate<O>,
    P: Parser<I, O, E>,
    S: Parser<I, O2, E>,
    E: ParserError<I>,
{
    let mut acc = C::initial(None);

    // Parse the first element
    match parser.parse_next(input) {
        Err(e) => return Err(e),
        Ok(o) => {
            acc.accumulate(o);
        }
    }

    loop {
        let start = input.checkpoint();
        let len = input.eof_offset();
        match separator.parse_next(input) {
            Err(e) if e.is_backtrack() => {
                input.reset(&start);
                return Ok(acc);
            }
            Err(e) => return Err(e),
            Ok(_) => {
                // infinite loop check
                if input.eof_offset() == len {
                    return Err(ParserError::assert(
                        input,
                        "`separated` separator parser must always consume",
                    ));
                }

                match parser.parse_next(input) {
                    Err(e) if e.is_backtrack() => {
                        input.reset(&start);
                        return Ok(acc);
                    }
                    Err(e) => return Err(e),
                    Ok(o) => {
                        acc.accumulate(o);
                    }
                }
            }
        }
    }
}

fn separated_n_<I, O, C, O2, E, P, S>(
    count: usize,
    parser: &mut P,
    separator: &mut S,
    input: &mut I,
) -> Result<C, E>
where
    I: Stream,
    C: Accumulate<O>,
    P: Parser<I, O, E>,
    S: Parser<I, O2, E>,
    E: ParserError<I>,
{
    let mut acc = C::initial(Some(count));

    if count == 0 {
        return Ok(acc);
    }

    let start = input.checkpoint();
    match parser.parse_next(input) {
        Err(e) => {
            return Err(e.append(input, &start));
        }
        Ok(o) => {
            acc.accumulate(o);
        }
    }

    for _ in 1..count {
        let start = input.checkpoint();
        let len = input.eof_offset();
        match separator.parse_next(input) {
            Err(e) => {
                return Err(e.append(input, &start));
            }
            Ok(_) => {
                // infinite loop check
                if input.eof_offset() == len {
                    return Err(ParserError::assert(
                        input,
                        "`separated` separator parser must always consume",
                    ));
                }

                match parser.parse_next(input) {
                    Err(e) => {
                        return Err(e.append(input, &start));
                    }
                    Ok(o) => {
                        acc.accumulate(o);
                    }
                }
            }
        }
    }

    Ok(acc)
}

fn separated_m_n_<I, O, C, O2, E, P, S>(
    min: usize,
    max: usize,
    parser: &mut P,
    separator: &mut S,
    input: &mut I,
) -> Result<C, E>
where
    I: Stream,
    C: Accumulate<O>,
    P: Parser<I, O, E>,
    S: Parser<I, O2, E>,
    E: ParserError<I>,
{
    if min > max {
        return Err(ParserError::assert(
            input,
            "range should be ascending, rather than descending",
        ));
    }

    let mut acc = C::initial(Some(min));

    let start = input.checkpoint();
    match parser.parse_next(input) {
        Err(e) if e.is_backtrack() => {
            if min == 0 {
                input.reset(&start);
                return Ok(acc);
            } else {
                return Err(e.append(input, &start));
            }
        }
        Err(e) => return Err(e),
        Ok(o) => {
            acc.accumulate(o);
        }
    }

    for index in 1..max {
        let start = input.checkpoint();
        let len = input.eof_offset();
        match separator.parse_next(input) {
            Err(e) if e.is_backtrack() => {
                if index < min {
                    return Err(e.append(input, &start));
                } else {
                    input.reset(&start);
                    return Ok(acc);
                }
            }
            Err(e) => {
                return Err(e);
            }
            Ok(_) => {
                // infinite loop check
                if input.eof_offset() == len {
                    return Err(ParserError::assert(
                        input,
                        "`separated` separator parser must always consume",
                    ));
                }

                match parser.parse_next(input) {
                    Err(e) if e.is_backtrack() => {
                        if index < min {
                            return Err(e.append(input, &start));
                        } else {
                            input.reset(&start);
                            return Ok(acc);
                        }
                    }
                    Err(e) => {
                        return Err(e);
                    }
                    Ok(o) => {
                        acc.accumulate(o);
                    }
                }
            }
        }
    }

    Ok(acc)
}

/// Alternates between two parsers, merging the results (left associative)
///
/// This stops when either parser returns [`ErrMode::Backtrack`][crate::error::ErrMode::Backtrack]. To instead chain an error up, see
/// [`cut_err`][crate::combinator::cut_err].
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::combinator::separated_foldl1;
/// use winnow::ascii::dec_int;
///
/// fn parser(s: &mut &str) -> ModalResult<i32> {
///   separated_foldl1(dec_int, "-", |l, _, r| l - r).parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek("9-3-5"), Ok(("", 1)));
/// assert!(parser.parse_peek("").is_err());
/// assert!(parser.parse_peek("def|abc").is_err());
/// ```
pub fn separated_foldl1<Input, Output, Sep, Error, ParseNext, SepParser, Op>(
    mut parser: ParseNext,
    mut sep: SepParser,
    mut op: Op,
) -> impl Parser<Input, Output, Error>
where
    Input: Stream,
    ParseNext: Parser<Input, Output, Error>,
    SepParser: Parser<Input, Sep, Error>,
    Error: ParserError<Input>,
    Op: FnMut(Output, Sep, Output) -> Output,
{
    trace("separated_foldl1", move |i: &mut Input| {
        let mut ol = parser.parse_next(i)?;

        loop {
            let start = i.checkpoint();
            let len = i.eof_offset();
            match sep.parse_next(i) {
                Err(e) if e.is_backtrack() => {
                    i.reset(&start);
                    return Ok(ol);
                }
                Err(e) => return Err(e),
                Ok(s) => {
                    // infinite loop check: the parser must always consume
                    if i.eof_offset() == len {
                        return Err(ParserError::assert(
                            i,
                            "`repeat` parsers must always consume",
                        ));
                    }

                    match parser.parse_next(i) {
                        Err(e) if e.is_backtrack() => {
                            i.reset(&start);
                            return Ok(ol);
                        }
                        Err(e) => return Err(e),
                        Ok(or) => {
                            ol = op(ol, s, or);
                        }
                    }
                }
            }
        }
    })
}

/// Alternates between two parsers, merging the results (right associative)
///
/// This stops when either parser returns [`ErrMode::Backtrack`][crate::error::ErrMode::Backtrack]. To instead chain an error up, see
/// [`cut_err`][crate::combinator::cut_err].
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::combinator::separated_foldr1;
/// use winnow::ascii::dec_uint;
///
/// fn parser(s: &mut &str) -> ModalResult<u32> {
///   separated_foldr1(dec_uint, "^", |l: u32, _, r: u32| l.pow(r)).parse_next(s)
/// }
///
/// assert_eq!(parser.parse_peek("2^3^2"), Ok(("", 512)));
/// assert_eq!(parser.parse_peek("2"), Ok(("", 2)));
/// assert!(parser.parse_peek("").is_err());
/// assert!(parser.parse_peek("def|abc").is_err());
/// ```
#[cfg(feature = "alloc")]
pub fn separated_foldr1<Input, Output, Sep, Error, ParseNext, SepParser, Op>(
    mut parser: ParseNext,
    mut sep: SepParser,
    mut op: Op,
) -> impl Parser<Input, Output, Error>
where
    Input: Stream,
    ParseNext: Parser<Input, Output, Error>,
    SepParser: Parser<Input, Sep, Error>,
    Error: ParserError<Input>,
    Op: FnMut(Output, Sep, Output) -> Output,
{
    trace("separated_foldr1", move |i: &mut Input| {
        let ol = parser.parse_next(i)?;
        let all: crate::lib::std::vec::Vec<(Sep, Output)> =
            repeat(0.., (sep.by_ref(), parser.by_ref())).parse_next(i)?;
        if let Some((s, or)) = all
            .into_iter()
            .rev()
            .reduce(|(sr, or), (sl, ol)| (sl, op(ol, sr, or)))
        {
            let merged = op(ol, s, or);
            Ok(merged)
        } else {
            Ok(ol)
        }
    })
}

/// Repeats the embedded parser, filling the given slice with results.
///
/// This parser fails if the input runs out before the given slice is full.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::combinator::fill;
///
/// fn parser<'i>(s: &mut &'i str) -> ModalResult<[&'i str; 2]> {
///   let mut buf = ["", ""];
///   fill("abc", &mut buf).parse_next(s)?;
///   Ok(buf)
/// }
///
/// assert_eq!(parser.parse_peek("abcabc"), Ok(("", ["abc", "abc"])));
/// assert!(parser.parse_peek("abc123").is_err());
/// assert!(parser.parse_peek("123123").is_err());
/// assert!(parser.parse_peek("").is_err());
/// assert_eq!(parser.parse_peek("abcabcabc"), Ok(("abc", ["abc", "abc"])));
/// ```
pub fn fill<'i, Input, Output, Error, ParseNext>(
    mut parser: ParseNext,
    buf: &'i mut [Output],
) -> impl Parser<Input, (), Error> + 'i
where
    Input: Stream + 'i,
    ParseNext: Parser<Input, Output, Error> + 'i,
    Error: ParserError<Input> + 'i,
{
    trace("fill", move |i: &mut Input| {
        for elem in buf.iter_mut() {
            let start = i.checkpoint();
            match parser.parse_next(i) {
                Ok(o) => {
                    *elem = o;
                }
                Err(e) => {
                    return Err(e.append(i, &start));
                }
            }
        }

        Ok(())
    })
}

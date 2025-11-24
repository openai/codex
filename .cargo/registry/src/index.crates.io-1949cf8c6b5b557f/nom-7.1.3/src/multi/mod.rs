//! Combinators applying their child parser multiple times

#[cfg(test)]
mod tests;

use crate::error::ErrorKind;
use crate::error::ParseError;
use crate::internal::{Err, IResult, Needed, Parser};
#[cfg(feature = "alloc")]
use crate::lib::std::vec::Vec;
use crate::traits::{InputLength, InputTake, ToUsize};
use core::num::NonZeroUsize;

/// Don't pre-allocate more than 64KiB when calling `Vec::with_capacity`.
///
/// Pre-allocating memory is a nice optimization but count fields can't
/// always be trusted. We should clamp initial capacities to some reasonable
/// amount. This reduces the risk of a bogus count value triggering a panic
/// due to an OOM error.
///
/// This does not affect correctness. Nom will always read the full number
/// of elements regardless of the capacity cap.
#[cfg(feature = "alloc")]
const MAX_INITIAL_CAPACITY_BYTES: usize = 65536;

/// Repeats the embedded parser, gathering the results in a `Vec`.
///
/// This stops on [`Err::Error`] and returns the results that were accumulated. To instead chain an error up, see
/// [`cut`][crate::combinator::cut].
///
/// # Arguments
/// * `f` The parser to apply.
///
/// *Note*: if the parser passed in accepts empty inputs (like `alpha0` or `digit0`), `many0` will
/// return an error, to prevent going into an infinite loop
///
/// ```rust
/// # use nom::{Err, error::ErrorKind, Needed, IResult};
/// use nom::multi::many0;
/// use nom::bytes::complete::tag;
///
/// fn parser(s: &str) -> IResult<&str, Vec<&str>> {
///   many0(tag("abc"))(s)
/// }
///
/// assert_eq!(parser("abcabc"), Ok(("", vec!["abc", "abc"])));
/// assert_eq!(parser("abc123"), Ok(("123", vec!["abc"])));
/// assert_eq!(parser("123123"), Ok(("123123", vec![])));
/// assert_eq!(parser(""), Ok(("", vec![])));
/// ```
#[cfg(feature = "alloc")]
#[cfg_attr(feature = "docsrs", doc(cfg(feature = "alloc")))]
pub fn many0<I, O, E, F>(mut f: F) -> impl FnMut(I) -> IResult<I, Vec<O>, E>
where
  I: Clone + InputLength,
  F: Parser<I, O, E>,
  E: ParseError<I>,
{
  move |mut i: I| {
    let mut acc = crate::lib::std::vec::Vec::with_capacity(4);
    loop {
      let len = i.input_len();
      match f.parse(i.clone()) {
        Err(Err::Error(_)) => return Ok((i, acc)),
        Err(e) => return Err(e),
        Ok((i1, o)) => {
          // infinite loop check: the parser must always consume
          if i1.input_len() == len {
            return Err(Err::Error(E::from_error_kind(i, ErrorKind::Many0)));
          }

          i = i1;
          acc.push(o);
        }
      }
    }
  }
}

/// Runs the embedded parser, gathering the results in a `Vec`.
///
/// This stops on [`Err::Error`] if there is at least one result,  and returns the results that were accumulated. To instead chain an error up,
/// see [`cut`][crate::combinator::cut].
///
/// # Arguments
/// * `f` The parser to apply.
///
/// *Note*: If the parser passed to `many1` accepts empty inputs
/// (like `alpha0` or `digit0`), `many1` will return an error,
/// to prevent going into an infinite loop.
///
/// ```rust
/// # use nom::{Err, error::{Error, ErrorKind}, Needed, IResult};
/// use nom::multi::many1;
/// use nom::bytes::complete::tag;
///
/// fn parser(s: &str) -> IResult<&str, Vec<&str>> {
///   many1(tag("abc"))(s)
/// }
///
/// assert_eq!(parser("abcabc"), Ok(("", vec!["abc", "abc"])));
/// assert_eq!(parser("abc123"), Ok(("123", vec!["abc"])));
/// assert_eq!(parser("123123"), Err(Err::Error(Error::new("123123", ErrorKind::Tag))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Tag))));
/// ```
#[cfg(feature = "alloc")]
#[cfg_attr(feature = "docsrs", doc(cfg(feature = "alloc")))]
pub fn many1<I, O, E, F>(mut f: F) -> impl FnMut(I) -> IResult<I, Vec<O>, E>
where
  I: Clone + InputLength,
  F: Parser<I, O, E>,
  E: ParseError<I>,
{
  move |mut i: I| match f.parse(i.clone()) {
    Err(Err::Error(err)) => Err(Err::Error(E::append(i, ErrorKind::Many1, err))),
    Err(e) => Err(e),
    Ok((i1, o)) => {
      let mut acc = crate::lib::std::vec::Vec::with_capacity(4);
      acc.push(o);
      i = i1;

      loop {
        let len = i.input_len();
        match f.parse(i.clone()) {
          Err(Err::Error(_)) => return Ok((i, acc)),
          Err(e) => return Err(e),
          Ok((i1, o)) => {
            // infinite loop check: the parser must always consume
            if i1.input_len() == len {
              return Err(Err::Error(E::from_error_kind(i, ErrorKind::Many1)));
            }

            i = i1;
            acc.push(o);
          }
        }
      }
    }
  }
}

/// Applies the parser `f` until the parser `g` produces a result.
///
/// Returns a tuple of the results of `f` in a `Vec` and the result of `g`.
///
/// `f` keeps going so long as `g` produces [`Err::Error`]. To instead chain an error up, see [`cut`][crate::combinator::cut].
///
/// ```rust
/// # use nom::{Err, error::{Error, ErrorKind}, Needed, IResult};
/// use nom::multi::many_till;
/// use nom::bytes::complete::tag;
///
/// fn parser(s: &str) -> IResult<&str, (Vec<&str>, &str)> {
///   many_till(tag("abc"), tag("end"))(s)
/// };
///
/// assert_eq!(parser("abcabcend"), Ok(("", (vec!["abc", "abc"], "end"))));
/// assert_eq!(parser("abc123end"), Err(Err::Error(Error::new("123end", ErrorKind::Tag))));
/// assert_eq!(parser("123123end"), Err(Err::Error(Error::new("123123end", ErrorKind::Tag))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Tag))));
/// assert_eq!(parser("abcendefg"), Ok(("efg", (vec!["abc"], "end"))));
/// ```
#[cfg(feature = "alloc")]
#[cfg_attr(feature = "docsrs", doc(cfg(feature = "alloc")))]
pub fn many_till<I, O, P, E, F, G>(
  mut f: F,
  mut g: G,
) -> impl FnMut(I) -> IResult<I, (Vec<O>, P), E>
where
  I: Clone + InputLength,
  F: Parser<I, O, E>,
  G: Parser<I, P, E>,
  E: ParseError<I>,
{
  move |mut i: I| {
    let mut res = crate::lib::std::vec::Vec::new();
    loop {
      let len = i.input_len();
      match g.parse(i.clone()) {
        Ok((i1, o)) => return Ok((i1, (res, o))),
        Err(Err::Error(_)) => {
          match f.parse(i.clone()) {
            Err(Err::Error(err)) => return Err(Err::Error(E::append(i, ErrorKind::ManyTill, err))),
            Err(e) => return Err(e),
            Ok((i1, o)) => {
              // infinite loop check: the parser must always consume
              if i1.input_len() == len {
                return Err(Err::Error(E::from_error_kind(i1, ErrorKind::ManyTill)));
              }

              res.push(o);
              i = i1;
            }
          }
        }
        Err(e) => return Err(e),
      }
    }
  }
}

/// Alternates between two parsers to produce a list of elements.
///
/// This stops when either parser returns [`Err::Error`]  and returns the results that were accumulated. To instead chain an error up, see
/// [`cut`][crate::combinator::cut].
///
/// # Arguments
/// * `sep` Parses the separator between list elements.
/// * `f` Parses the elements of the list.
///
/// ```rust
/// # use nom::{Err, error::ErrorKind, Needed, IResult};
/// use nom::multi::separated_list0;
/// use nom::bytes::complete::tag;
///
/// fn parser(s: &str) -> IResult<&str, Vec<&str>> {
///   separated_list0(tag("|"), tag("abc"))(s)
/// }
///
/// assert_eq!(parser("abc|abc|abc"), Ok(("", vec!["abc", "abc", "abc"])));
/// assert_eq!(parser("abc123abc"), Ok(("123abc", vec!["abc"])));
/// assert_eq!(parser("abc|def"), Ok(("|def", vec!["abc"])));
/// assert_eq!(parser(""), Ok(("", vec![])));
/// assert_eq!(parser("def|abc"), Ok(("def|abc", vec![])));
/// ```
#[cfg(feature = "alloc")]
#[cfg_attr(feature = "docsrs", doc(cfg(feature = "alloc")))]
pub fn separated_list0<I, O, O2, E, F, G>(
  mut sep: G,
  mut f: F,
) -> impl FnMut(I) -> IResult<I, Vec<O>, E>
where
  I: Clone + InputLength,
  F: Parser<I, O, E>,
  G: Parser<I, O2, E>,
  E: ParseError<I>,
{
  move |mut i: I| {
    let mut res = Vec::new();

    match f.parse(i.clone()) {
      Err(Err::Error(_)) => return Ok((i, res)),
      Err(e) => return Err(e),
      Ok((i1, o)) => {
        res.push(o);
        i = i1;
      }
    }

    loop {
      let len = i.input_len();
      match sep.parse(i.clone()) {
        Err(Err::Error(_)) => return Ok((i, res)),
        Err(e) => return Err(e),
        Ok((i1, _)) => {
          // infinite loop check: the parser must always consume
          if i1.input_len() == len {
            return Err(Err::Error(E::from_error_kind(i1, ErrorKind::SeparatedList)));
          }

          match f.parse(i1.clone()) {
            Err(Err::Error(_)) => return Ok((i, res)),
            Err(e) => return Err(e),
            Ok((i2, o)) => {
              res.push(o);
              i = i2;
            }
          }
        }
      }
    }
  }
}

/// Alternates between two parsers to produce a list of elements until [`Err::Error`].
///
/// Fails if the element parser does not produce at least one element.$
///
/// This stops when either parser returns [`Err::Error`]  and returns the results that were accumulated. To instead chain an error up, see
/// [`cut`][crate::combinator::cut].
///
/// # Arguments
/// * `sep` Parses the separator between list elements.
/// * `f` Parses the elements of the list.
/// ```rust
/// # use nom::{Err, error::{Error, ErrorKind}, Needed, IResult};
/// use nom::multi::separated_list1;
/// use nom::bytes::complete::tag;
///
/// fn parser(s: &str) -> IResult<&str, Vec<&str>> {
///   separated_list1(tag("|"), tag("abc"))(s)
/// }
///
/// assert_eq!(parser("abc|abc|abc"), Ok(("", vec!["abc", "abc", "abc"])));
/// assert_eq!(parser("abc123abc"), Ok(("123abc", vec!["abc"])));
/// assert_eq!(parser("abc|def"), Ok(("|def", vec!["abc"])));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Tag))));
/// assert_eq!(parser("def|abc"), Err(Err::Error(Error::new("def|abc", ErrorKind::Tag))));
/// ```
#[cfg(feature = "alloc")]
#[cfg_attr(feature = "docsrs", doc(cfg(feature = "alloc")))]
pub fn separated_list1<I, O, O2, E, F, G>(
  mut sep: G,
  mut f: F,
) -> impl FnMut(I) -> IResult<I, Vec<O>, E>
where
  I: Clone + InputLength,
  F: Parser<I, O, E>,
  G: Parser<I, O2, E>,
  E: ParseError<I>,
{
  move |mut i: I| {
    let mut res = Vec::new();

    // Parse the first element
    match f.parse(i.clone()) {
      Err(e) => return Err(e),
      Ok((i1, o)) => {
        res.push(o);
        i = i1;
      }
    }

    loop {
      let len = i.input_len();
      match sep.parse(i.clone()) {
        Err(Err::Error(_)) => return Ok((i, res)),
        Err(e) => return Err(e),
        Ok((i1, _)) => {
          // infinite loop check: the parser must always consume
          if i1.input_len() == len {
            return Err(Err::Error(E::from_error_kind(i1, ErrorKind::SeparatedList)));
          }

          match f.parse(i1.clone()) {
            Err(Err::Error(_)) => return Ok((i, res)),
            Err(e) => return Err(e),
            Ok((i2, o)) => {
              res.push(o);
              i = i2;
            }
          }
        }
      }
    }
  }
}

/// Repeats the embedded parser `m..=n` times
///
/// This stops before `n` when the parser returns [`Err::Error`]  and returns the results that were accumulated. To instead chain an error up, see
/// [`cut`][crate::combinator::cut].
///
/// # Arguments
/// * `m` The minimum number of iterations.
/// * `n` The maximum number of iterations.
/// * `f` The parser to apply.
///
/// *Note*: If the parser passed to `many1` accepts empty inputs
/// (like `alpha0` or `digit0`), `many1` will return an error,
/// to prevent going into an infinite loop.
///
/// ```rust
/// # use nom::{Err, error::ErrorKind, Needed, IResult};
/// use nom::multi::many_m_n;
/// use nom::bytes::complete::tag;
///
/// fn parser(s: &str) -> IResult<&str, Vec<&str>> {
///   many_m_n(0, 2, tag("abc"))(s)
/// }
///
/// assert_eq!(parser("abcabc"), Ok(("", vec!["abc", "abc"])));
/// assert_eq!(parser("abc123"), Ok(("123", vec!["abc"])));
/// assert_eq!(parser("123123"), Ok(("123123", vec![])));
/// assert_eq!(parser(""), Ok(("", vec![])));
/// assert_eq!(parser("abcabcabc"), Ok(("abc", vec!["abc", "abc"])));
/// ```
#[cfg(feature = "alloc")]
#[cfg_attr(feature = "docsrs", doc(cfg(feature = "alloc")))]
pub fn many_m_n<I, O, E, F>(
  min: usize,
  max: usize,
  mut parse: F,
) -> impl FnMut(I) -> IResult<I, Vec<O>, E>
where
  I: Clone + InputLength,
  F: Parser<I, O, E>,
  E: ParseError<I>,
{
  move |mut input: I| {
    if min > max {
      return Err(Err::Failure(E::from_error_kind(input, ErrorKind::ManyMN)));
    }

    let max_initial_capacity =
      MAX_INITIAL_CAPACITY_BYTES / crate::lib::std::mem::size_of::<O>().max(1);
    let mut res = crate::lib::std::vec::Vec::with_capacity(min.min(max_initial_capacity));
    for count in 0..max {
      let len = input.input_len();
      match parse.parse(input.clone()) {
        Ok((tail, value)) => {
          // infinite loop check: the parser must always consume
          if tail.input_len() == len {
            return Err(Err::Error(E::from_error_kind(input, ErrorKind::ManyMN)));
          }

          res.push(value);
          input = tail;
        }
        Err(Err::Error(e)) => {
          if count < min {
            return Err(Err::Error(E::append(input, ErrorKind::ManyMN, e)));
          } else {
            return Ok((input, res));
          }
        }
        Err(e) => {
          return Err(e);
        }
      }
    }

    Ok((input, res))
  }
}

/// Repeats the embedded parser, counting the results
///
/// This stops on [`Err::Error`]. To instead chain an error up, see
/// [`cut`][crate::combinator::cut].
///
/// # Arguments
/// * `f` The parser to apply.
///
/// *Note*: if the parser passed in accepts empty inputs (like `alpha0` or `digit0`), `many0` will
/// return an error, to prevent going into an infinite loop
///
/// ```rust
/// # use nom::{Err, error::ErrorKind, Needed, IResult};
/// use nom::multi::many0_count;
/// use nom::bytes::complete::tag;
///
/// fn parser(s: &str) -> IResult<&str, usize> {
///   many0_count(tag("abc"))(s)
/// }
///
/// assert_eq!(parser("abcabc"), Ok(("", 2)));
/// assert_eq!(parser("abc123"), Ok(("123", 1)));
/// assert_eq!(parser("123123"), Ok(("123123", 0)));
/// assert_eq!(parser(""), Ok(("", 0)));
/// ```
pub fn many0_count<I, O, E, F>(mut f: F) -> impl FnMut(I) -> IResult<I, usize, E>
where
  I: Clone + InputLength,
  F: Parser<I, O, E>,
  E: ParseError<I>,
{
  move |i: I| {
    let mut input = i;
    let mut count = 0;

    loop {
      let input_ = input.clone();
      let len = input.input_len();
      match f.parse(input_) {
        Ok((i, _)) => {
          // infinite loop check: the parser must always consume
          if i.input_len() == len {
            return Err(Err::Error(E::from_error_kind(input, ErrorKind::Many0Count)));
          }

          input = i;
          count += 1;
        }

        Err(Err::Error(_)) => return Ok((input, count)),

        Err(e) => return Err(e),
      }
    }
  }
}

/// Runs the embedded parser, counting the results.
///
/// This stops on [`Err::Error`] if there is at least one result. To instead chain an error up,
/// see [`cut`][crate::combinator::cut].
///
/// # Arguments
/// * `f` The parser to apply.
///
/// *Note*: If the parser passed to `many1` accepts empty inputs
/// (like `alpha0` or `digit0`), `many1` will return an error,
/// to prevent going into an infinite loop.
///
/// ```rust
/// # use nom::{Err, error::{Error, ErrorKind}, Needed, IResult};
/// use nom::multi::many1_count;
/// use nom::bytes::complete::tag;
///
/// fn parser(s: &str) -> IResult<&str, usize> {
///   many1_count(tag("abc"))(s)
/// }
///
/// assert_eq!(parser("abcabc"), Ok(("", 2)));
/// assert_eq!(parser("abc123"), Ok(("123", 1)));
/// assert_eq!(parser("123123"), Err(Err::Error(Error::new("123123", ErrorKind::Many1Count))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Many1Count))));
/// ```
pub fn many1_count<I, O, E, F>(mut f: F) -> impl FnMut(I) -> IResult<I, usize, E>
where
  I: Clone + InputLength,
  F: Parser<I, O, E>,
  E: ParseError<I>,
{
  move |i: I| {
    let i_ = i.clone();
    match f.parse(i_) {
      Err(Err::Error(_)) => Err(Err::Error(E::from_error_kind(i, ErrorKind::Many1Count))),
      Err(i) => Err(i),
      Ok((i1, _)) => {
        let mut count = 1;
        let mut input = i1;

        loop {
          let len = input.input_len();
          let input_ = input.clone();
          match f.parse(input_) {
            Err(Err::Error(_)) => return Ok((input, count)),
            Err(e) => return Err(e),
            Ok((i, _)) => {
              // infinite loop check: the parser must always consume
              if i.input_len() == len {
                return Err(Err::Error(E::from_error_kind(i, ErrorKind::Many1Count)));
              }

              count += 1;
              input = i;
            }
          }
        }
      }
    }
  }
}

/// Runs the embedded parser `count` times, gathering the results in a `Vec`
///
/// # Arguments
/// * `f` The parser to apply.
/// * `count` How often to apply the parser.
/// ```rust
/// # use nom::{Err, error::{Error, ErrorKind}, Needed, IResult};
/// use nom::multi::count;
/// use nom::bytes::complete::tag;
///
/// fn parser(s: &str) -> IResult<&str, Vec<&str>> {
///   count(tag("abc"), 2)(s)
/// }
///
/// assert_eq!(parser("abcabc"), Ok(("", vec!["abc", "abc"])));
/// assert_eq!(parser("abc123"), Err(Err::Error(Error::new("123", ErrorKind::Tag))));
/// assert_eq!(parser("123123"), Err(Err::Error(Error::new("123123", ErrorKind::Tag))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Tag))));
/// assert_eq!(parser("abcabcabc"), Ok(("abc", vec!["abc", "abc"])));
/// ```
#[cfg(feature = "alloc")]
#[cfg_attr(feature = "docsrs", doc(cfg(feature = "alloc")))]
pub fn count<I, O, E, F>(mut f: F, count: usize) -> impl FnMut(I) -> IResult<I, Vec<O>, E>
where
  I: Clone + PartialEq,
  F: Parser<I, O, E>,
  E: ParseError<I>,
{
  move |i: I| {
    let mut input = i.clone();
    let max_initial_capacity =
      MAX_INITIAL_CAPACITY_BYTES / crate::lib::std::mem::size_of::<O>().max(1);
    let mut res = crate::lib::std::vec::Vec::with_capacity(count.min(max_initial_capacity));

    for _ in 0..count {
      let input_ = input.clone();
      match f.parse(input_) {
        Ok((i, o)) => {
          res.push(o);
          input = i;
        }
        Err(Err::Error(e)) => {
          return Err(Err::Error(E::append(i, ErrorKind::Count, e)));
        }
        Err(e) => {
          return Err(e);
        }
      }
    }

    Ok((input, res))
  }
}

/// Runs the embedded parser repeatedly, filling the given slice with results.
///
/// This parser fails if the input runs out before the given slice is full.
///
/// # Arguments
/// * `f` The parser to apply.
/// * `buf` The slice to fill
/// ```rust
/// # use nom::{Err, error::{Error, ErrorKind}, Needed, IResult};
/// use nom::multi::fill;
/// use nom::bytes::complete::tag;
///
/// fn parser(s: &str) -> IResult<&str, [&str; 2]> {
///   let mut buf = ["", ""];
///   let (rest, ()) = fill(tag("abc"), &mut buf)(s)?;
///   Ok((rest, buf))
/// }
///
/// assert_eq!(parser("abcabc"), Ok(("", ["abc", "abc"])));
/// assert_eq!(parser("abc123"), Err(Err::Error(Error::new("123", ErrorKind::Tag))));
/// assert_eq!(parser("123123"), Err(Err::Error(Error::new("123123", ErrorKind::Tag))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Tag))));
/// assert_eq!(parser("abcabcabc"), Ok(("abc", ["abc", "abc"])));
/// ```
pub fn fill<'a, I, O, E, F>(f: F, buf: &'a mut [O]) -> impl FnMut(I) -> IResult<I, (), E> + 'a
where
  I: Clone + PartialEq,
  F: Fn(I) -> IResult<I, O, E> + 'a,
  E: ParseError<I>,
{
  move |i: I| {
    let mut input = i.clone();

    for elem in buf.iter_mut() {
      let input_ = input.clone();
      match f(input_) {
        Ok((i, o)) => {
          *elem = o;
          input = i;
        }
        Err(Err::Error(e)) => {
          return Err(Err::Error(E::append(i, ErrorKind::Count, e)));
        }
        Err(e) => {
          return Err(e);
        }
      }
    }

    Ok((input, ()))
  }
}

/// Repeats the embedded parser, calling `g` to gather the results.
///
/// This stops on [`Err::Error`]. To instead chain an error up, see
/// [`cut`][crate::combinator::cut].
///
/// # Arguments
/// * `f` The parser to apply.
/// * `init` A function returning the initial value.
/// * `g` The function that combines a result of `f` with
///       the current accumulator.
///
/// *Note*: if the parser passed in accepts empty inputs (like `alpha0` or `digit0`), `many0` will
/// return an error, to prevent going into an infinite loop
///
/// ```rust
/// # use nom::{Err, error::ErrorKind, Needed, IResult};
/// use nom::multi::fold_many0;
/// use nom::bytes::complete::tag;
///
/// fn parser(s: &str) -> IResult<&str, Vec<&str>> {
///   fold_many0(
///     tag("abc"),
///     Vec::new,
///     |mut acc: Vec<_>, item| {
///       acc.push(item);
///       acc
///     }
///   )(s)
/// }
///
/// assert_eq!(parser("abcabc"), Ok(("", vec!["abc", "abc"])));
/// assert_eq!(parser("abc123"), Ok(("123", vec!["abc"])));
/// assert_eq!(parser("123123"), Ok(("123123", vec![])));
/// assert_eq!(parser(""), Ok(("", vec![])));
/// ```
pub fn fold_many0<I, O, E, F, G, H, R>(
  mut f: F,
  mut init: H,
  mut g: G,
) -> impl FnMut(I) -> IResult<I, R, E>
where
  I: Clone + InputLength,
  F: Parser<I, O, E>,
  G: FnMut(R, O) -> R,
  H: FnMut() -> R,
  E: ParseError<I>,
{
  move |i: I| {
    let mut res = init();
    let mut input = i;

    loop {
      let i_ = input.clone();
      let len = input.input_len();
      match f.parse(i_) {
        Ok((i, o)) => {
          // infinite loop check: the parser must always consume
          if i.input_len() == len {
            return Err(Err::Error(E::from_error_kind(input, ErrorKind::Many0)));
          }

          res = g(res, o);
          input = i;
        }
        Err(Err::Error(_)) => {
          return Ok((input, res));
        }
        Err(e) => {
          return Err(e);
        }
      }
    }
  }
}

/// Repeats the embedded parser, calling `g` to gather the results.
///
/// This stops on [`Err::Error`] if there is at least one result. To instead chain an error up,
/// see [`cut`][crate::combinator::cut].
///
/// # Arguments
/// * `f` The parser to apply.
/// * `init` A function returning the initial value.
/// * `g` The function that combines a result of `f` with
///       the current accumulator.
///
/// *Note*: If the parser passed to `many1` accepts empty inputs
/// (like `alpha0` or `digit0`), `many1` will return an error,
/// to prevent going into an infinite loop.
///
/// ```rust
/// # use nom::{Err, error::{Error, ErrorKind}, Needed, IResult};
/// use nom::multi::fold_many1;
/// use nom::bytes::complete::tag;
///
/// fn parser(s: &str) -> IResult<&str, Vec<&str>> {
///   fold_many1(
///     tag("abc"),
///     Vec::new,
///     |mut acc: Vec<_>, item| {
///       acc.push(item);
///       acc
///     }
///   )(s)
/// }
///
/// assert_eq!(parser("abcabc"), Ok(("", vec!["abc", "abc"])));
/// assert_eq!(parser("abc123"), Ok(("123", vec!["abc"])));
/// assert_eq!(parser("123123"), Err(Err::Error(Error::new("123123", ErrorKind::Many1))));
/// assert_eq!(parser(""), Err(Err::Error(Error::new("", ErrorKind::Many1))));
/// ```
pub fn fold_many1<I, O, E, F, G, H, R>(
  mut f: F,
  mut init: H,
  mut g: G,
) -> impl FnMut(I) -> IResult<I, R, E>
where
  I: Clone + InputLength,
  F: Parser<I, O, E>,
  G: FnMut(R, O) -> R,
  H: FnMut() -> R,
  E: ParseError<I>,
{
  move |i: I| {
    let _i = i.clone();
    let init = init();
    match f.parse(_i) {
      Err(Err::Error(_)) => Err(Err::Error(E::from_error_kind(i, ErrorKind::Many1))),
      Err(e) => Err(e),
      Ok((i1, o1)) => {
        let mut acc = g(init, o1);
        let mut input = i1;

        loop {
          let _input = input.clone();
          let len = input.input_len();
          match f.parse(_input) {
            Err(Err::Error(_)) => {
              break;
            }
            Err(e) => return Err(e),
            Ok((i, o)) => {
              // infinite loop check: the parser must always consume
              if i.input_len() == len {
                return Err(Err::Failure(E::from_error_kind(i, ErrorKind::Many1)));
              }

              acc = g(acc, o);
              input = i;
            }
          }
        }

        Ok((input, acc))
      }
    }
  }
}

/// Repeats the embedded parser `m..=n` times, calling `g` to gather the results
///
/// This stops before `n` when the parser returns [`Err::Error`]. To instead chain an error up, see
/// [`cut`][crate::combinator::cut].
///
/// # Arguments
/// * `m` The minimum number of iterations.
/// * `n` The maximum number of iterations.
/// * `f` The parser to apply.
/// * `init` A function returning the initial value.
/// * `g` The function that combines a result of `f` with
///       the current accumulator.
///
/// *Note*: If the parser passed to `many1` accepts empty inputs
/// (like `alpha0` or `digit0`), `many1` will return an error,
/// to prevent going into an infinite loop.
///
/// ```rust
/// # use nom::{Err, error::ErrorKind, Needed, IResult};
/// use nom::multi::fold_many_m_n;
/// use nom::bytes::complete::tag;
///
/// fn parser(s: &str) -> IResult<&str, Vec<&str>> {
///   fold_many_m_n(
///     0,
///     2,
///     tag("abc"),
///     Vec::new,
///     |mut acc: Vec<_>, item| {
///       acc.push(item);
///       acc
///     }
///   )(s)
/// }
///
/// assert_eq!(parser("abcabc"), Ok(("", vec!["abc", "abc"])));
/// assert_eq!(parser("abc123"), Ok(("123", vec!["abc"])));
/// assert_eq!(parser("123123"), Ok(("123123", vec![])));
/// assert_eq!(parser(""), Ok(("", vec![])));
/// assert_eq!(parser("abcabcabc"), Ok(("abc", vec!["abc", "abc"])));
/// ```
pub fn fold_many_m_n<I, O, E, F, G, H, R>(
  min: usize,
  max: usize,
  mut parse: F,
  mut init: H,
  mut fold: G,
) -> impl FnMut(I) -> IResult<I, R, E>
where
  I: Clone + InputLength,
  F: Parser<I, O, E>,
  G: FnMut(R, O) -> R,
  H: FnMut() -> R,
  E: ParseError<I>,
{
  move |mut input: I| {
    if min > max {
      return Err(Err::Failure(E::from_error_kind(input, ErrorKind::ManyMN)));
    }

    let mut acc = init();
    for count in 0..max {
      let len = input.input_len();
      match parse.parse(input.clone()) {
        Ok((tail, value)) => {
          // infinite loop check: the parser must always consume
          if tail.input_len() == len {
            return Err(Err::Error(E::from_error_kind(tail, ErrorKind::ManyMN)));
          }

          acc = fold(acc, value);
          input = tail;
        }
        //FInputXMError: handle failure properly
        Err(Err::Error(err)) => {
          if count < min {
            return Err(Err::Error(E::append(input, ErrorKind::ManyMN, err)));
          } else {
            break;
          }
        }
        Err(e) => return Err(e),
      }
    }

    Ok((input, acc))
  }
}

/// Gets a number from the parser and returns a
/// subslice of the input of that size.
/// If the parser returns `Incomplete`,
/// `length_data` will return an error.
/// # Arguments
/// * `f` The parser to apply.
/// ```rust
/// # use nom::{Err, error::ErrorKind, Needed, IResult};
/// use nom::number::complete::be_u16;
/// use nom::multi::length_data;
/// use nom::bytes::complete::tag;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], &[u8]> {
///   length_data(be_u16)(s)
/// }
///
/// assert_eq!(parser(b"\x00\x03abcefg"), Ok((&b"efg"[..], &b"abc"[..])));
/// assert_eq!(parser(b"\x00\x03a"), Err(Err::Incomplete(Needed::new(2))));
/// ```
pub fn length_data<I, N, E, F>(mut f: F) -> impl FnMut(I) -> IResult<I, I, E>
where
  I: InputLength + InputTake,
  N: ToUsize,
  F: Parser<I, N, E>,
  E: ParseError<I>,
{
  move |i: I| {
    let (i, length) = f.parse(i)?;

    let length: usize = length.to_usize();

    if let Some(needed) = length
      .checked_sub(i.input_len())
      .and_then(NonZeroUsize::new)
    {
      Err(Err::Incomplete(Needed::Size(needed)))
    } else {
      Ok(i.take_split(length))
    }
  }
}

/// Gets a number from the first parser,
/// takes a subslice of the input of that size,
/// then applies the second parser on that subslice.
/// If the second parser returns `Incomplete`,
/// `length_value` will return an error.
/// # Arguments
/// * `f` The parser to apply.
/// * `g` The parser to apply on the subslice.
/// ```rust
/// # use nom::{Err, error::{Error, ErrorKind}, Needed, IResult};
/// use nom::number::complete::be_u16;
/// use nom::multi::length_value;
/// use nom::bytes::complete::tag;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], &[u8]> {
///   length_value(be_u16, tag("abc"))(s)
/// }
///
/// assert_eq!(parser(b"\x00\x03abcefg"), Ok((&b"efg"[..], &b"abc"[..])));
/// assert_eq!(parser(b"\x00\x03123123"), Err(Err::Error(Error::new(&b"123"[..], ErrorKind::Tag))));
/// assert_eq!(parser(b"\x00\x03a"), Err(Err::Incomplete(Needed::new(2))));
/// ```
pub fn length_value<I, O, N, E, F, G>(mut f: F, mut g: G) -> impl FnMut(I) -> IResult<I, O, E>
where
  I: Clone + InputLength + InputTake,
  N: ToUsize,
  F: Parser<I, N, E>,
  G: Parser<I, O, E>,
  E: ParseError<I>,
{
  move |i: I| {
    let (i, length) = f.parse(i)?;

    let length: usize = length.to_usize();

    if let Some(needed) = length
      .checked_sub(i.input_len())
      .and_then(NonZeroUsize::new)
    {
      Err(Err::Incomplete(Needed::Size(needed)))
    } else {
      let (rest, i) = i.take_split(length);
      match g.parse(i.clone()) {
        Err(Err::Incomplete(_)) => Err(Err::Error(E::from_error_kind(i, ErrorKind::Complete))),
        Err(e) => Err(e),
        Ok((_, o)) => Ok((rest, o)),
      }
    }
  }
}

/// Gets a number from the first parser,
/// then applies the second parser that many times.
/// # Arguments
/// * `f` The parser to apply to obtain the count.
/// * `g` The parser to apply repeatedly.
/// ```rust
/// # use nom::{Err, error::{Error, ErrorKind}, Needed, IResult};
/// use nom::number::complete::u8;
/// use nom::multi::length_count;
/// use nom::bytes::complete::tag;
/// use nom::combinator::map;
///
/// fn parser(s: &[u8]) -> IResult<&[u8], Vec<&[u8]>> {
///   length_count(map(u8, |i| {
///      println!("got number: {}", i);
///      i
///   }), tag("abc"))(s)
/// }
///
/// assert_eq!(parser(&b"\x02abcabcabc"[..]), Ok(((&b"abc"[..], vec![&b"abc"[..], &b"abc"[..]]))));
/// assert_eq!(parser(b"\x03123123123"), Err(Err::Error(Error::new(&b"123123123"[..], ErrorKind::Tag))));
/// ```
#[cfg(feature = "alloc")]
pub fn length_count<I, O, N, E, F, G>(mut f: F, mut g: G) -> impl FnMut(I) -> IResult<I, Vec<O>, E>
where
  I: Clone,
  N: ToUsize,
  F: Parser<I, N, E>,
  G: Parser<I, O, E>,
  E: ParseError<I>,
{
  move |i: I| {
    let (i, count) = f.parse(i)?;
    let mut input = i.clone();
    let mut res = Vec::new();

    for _ in 0..count.to_usize() {
      let input_ = input.clone();
      match g.parse(input_) {
        Ok((i, o)) => {
          res.push(o);
          input = i;
        }
        Err(Err::Error(e)) => {
          return Err(Err::Error(E::append(i, ErrorKind::Count, e)));
        }
        Err(e) => {
          return Err(e);
        }
      }
    }

    Ok((input, res))
  }
}

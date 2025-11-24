use crate::combinator::trace;
use crate::error::ParserError;
use crate::stream::Stream;
use crate::*;

#[doc(inline)]
pub use crate::dispatch;

/// Helper trait for the [`alt()`] combinator.
///
/// This trait is implemented for tuples of up to 21 elements
pub trait Alt<I, O, E> {
    /// Tests each parser in the tuple and returns the result of the first one that succeeds
    fn choice(&mut self, input: &mut I) -> Result<O, E>;
}

/// Pick the first successful parser
///
/// To stop on an error, rather than trying further cases, see
/// [`cut_err`][crate::combinator::cut_err] ([example][crate::_tutorial::chapter_7]).
///
/// For tight control over the error when no match is found, add a final case using [`fail`][crate::combinator::fail].
/// Alternatively, with a [custom error type][crate::_topic::error], it is possible to track all
/// errors or return the error of the parser that went the farthest in the input data.
///
/// When the alternative cases have unique prefixes, [`dispatch`] can offer better performance.
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::ascii::{alpha1, digit1};
/// use winnow::combinator::alt;
/// # fn main() {
/// fn parser<'i>(input: &mut &'i str) -> ModalResult<&'i str> {
///   alt((alpha1, digit1)).parse_next(input)
/// };
///
/// // the first parser, alpha1, takes the input
/// assert_eq!(parser.parse_peek("abc"), Ok(("", "abc")));
///
/// // the first parser returns an error, so alt tries the second one
/// assert_eq!(parser.parse_peek("123456"), Ok(("", "123456")));
///
/// // both parsers failed, and with the default error type, alt will return the last error
/// assert!(parser.parse_peek(" ").is_err());
/// # }
/// ```
#[doc(alias = "choice")]
#[inline(always)]
pub fn alt<Input: Stream, Output, Error, Alternatives>(
    mut alternatives: Alternatives,
) -> impl Parser<Input, Output, Error>
where
    Alternatives: Alt<Input, Output, Error>,
    Error: ParserError<Input>,
{
    trace("alt", move |i: &mut Input| alternatives.choice(i))
}

/// Helper trait for the [`permutation()`] combinator.
///
/// This trait is implemented for tuples of up to 21 elements
pub trait Permutation<I, O, E> {
    /// Tries to apply all parsers in the tuple in various orders until all of them succeed
    fn permutation(&mut self, input: &mut I) -> Result<O, E>;
}

/// Applies a list of parsers in any order.
///
/// Permutation will succeed if all of the child parsers succeeded.
/// It takes as argument a tuple of parsers, and returns a
/// tuple of the parser results.
///
/// To stop on an error, rather than trying further permutations, see
/// [`cut_err`][crate::combinator::cut_err] ([example][crate::_tutorial::chapter_7]).
///
/// # Example
///
/// ```rust
/// # use winnow::{error::ErrMode, error::Needed};
/// # use winnow::prelude::*;
/// use winnow::ascii::{alpha1, digit1};
/// use winnow::combinator::permutation;
/// # fn main() {
/// fn parser<'i>(input: &mut &'i str) -> ModalResult<(&'i str, &'i str)> {
///   permutation((alpha1, digit1)).parse_next(input)
/// }
///
/// // permutation takes alphabetic characters then digit
/// assert_eq!(parser.parse_peek("abc123"), Ok(("", ("abc", "123"))));
///
/// // but also in inverse order
/// assert_eq!(parser.parse_peek("123abc"), Ok(("", ("abc", "123"))));
///
/// // it will fail if one of the parsers failed
/// assert!(parser.parse_peek("abc;").is_err());
/// # }
/// ```
///
/// The parsers are applied greedily: if there are multiple unapplied parsers
/// that could parse the next slice of input, the first one is used.
/// ```rust
/// # use winnow::error::ErrMode;
/// # use winnow::prelude::*;
/// use winnow::combinator::permutation;
/// use winnow::token::any;
///
/// fn parser(input: &mut &str) -> ModalResult<(char, char)> {
///   permutation((any, 'a')).parse_next(input)
/// }
///
/// // any parses 'b', then char('a') parses 'a'
/// assert_eq!(parser.parse_peek("ba"), Ok(("", ('b', 'a'))));
///
/// // any parses 'a', then char('a') fails on 'b',
/// // even though char('a') followed by any would succeed
/// assert!(parser.parse_peek("ab").is_err());
/// ```
///
#[inline(always)]
pub fn permutation<I: Stream, O, E: ParserError<I>, List: Permutation<I, O, E>>(
    mut l: List,
) -> impl Parser<I, O, E> {
    trace("permutation", move |i: &mut I| l.permutation(i))
}

impl<const N: usize, I: Stream, O, E: ParserError<I>, P: Parser<I, O, E>> Alt<I, O, E> for [P; N] {
    fn choice(&mut self, input: &mut I) -> Result<O, E> {
        let mut error: Option<E> = None;

        let start = input.checkpoint();
        for branch in self {
            input.reset(&start);
            match branch.parse_next(input) {
                Err(e) if e.is_backtrack() => {
                    error = match error {
                        Some(error) => Some(error.or(e)),
                        None => Some(e),
                    };
                }
                res => return res,
            }
        }

        match error {
            Some(e) => Err(e.append(input, &start)),
            None => Err(ParserError::assert(
                input,
                "`alt` needs at least one parser",
            )),
        }
    }
}

impl<I: Stream, O, E: ParserError<I>, P: Parser<I, O, E>> Alt<I, O, E> for &mut [P] {
    fn choice(&mut self, input: &mut I) -> Result<O, E> {
        let mut error: Option<E> = None;

        let start = input.checkpoint();
        for branch in self.iter_mut() {
            input.reset(&start);
            match branch.parse_next(input) {
                Err(e) if e.is_backtrack() => {
                    error = match error {
                        Some(error) => Some(error.or(e)),
                        None => Some(e),
                    };
                }
                res => return res,
            }
        }

        match error {
            Some(e) => Err(e.append(input, &start)),
            None => Err(ParserError::assert(
                input,
                "`alt` needs at least one parser",
            )),
        }
    }
}

macro_rules! alt_trait(
  ($first:ident $second:ident $($id: ident)+) => (
    alt_trait!(__impl $first $second; $($id)+);
  );
  (__impl $($current:ident)*; $head:ident $($id: ident)+) => (
    alt_trait_impl!($($current)*);

    alt_trait!(__impl $($current)* $head; $($id)+);
  );
  (__impl $($current:ident)*; $head:ident) => (
    alt_trait_impl!($($current)*);
    alt_trait_impl!($($current)* $head);
  );
);

macro_rules! alt_trait_impl(
  ($($id:ident)+) => (
    impl<
      I: Stream, Output, Error: ParserError<I>,
      $($id: Parser<I, Output, Error>),+
    > Alt<I, Output, Error> for ( $($id),+ ) {

      fn choice(&mut self, input: &mut I) -> Result<Output, Error> {
        let start = input.checkpoint();
        match self.0.parse_next(input) {
          Err(e) if e.is_backtrack() => alt_trait_inner!(1, self, input, start, e, $($id)+),
          res => res,
        }
      }
    }
  );
);

macro_rules! succ (
  (0, $submac:ident ! ($($rest:tt)*)) => ($submac!(1, $($rest)*));
  (1, $submac:ident ! ($($rest:tt)*)) => ($submac!(2, $($rest)*));
  (2, $submac:ident ! ($($rest:tt)*)) => ($submac!(3, $($rest)*));
  (3, $submac:ident ! ($($rest:tt)*)) => ($submac!(4, $($rest)*));
  (4, $submac:ident ! ($($rest:tt)*)) => ($submac!(5, $($rest)*));
  (5, $submac:ident ! ($($rest:tt)*)) => ($submac!(6, $($rest)*));
  (6, $submac:ident ! ($($rest:tt)*)) => ($submac!(7, $($rest)*));
  (7, $submac:ident ! ($($rest:tt)*)) => ($submac!(8, $($rest)*));
  (8, $submac:ident ! ($($rest:tt)*)) => ($submac!(9, $($rest)*));
  (9, $submac:ident ! ($($rest:tt)*)) => ($submac!(10, $($rest)*));
  (10, $submac:ident ! ($($rest:tt)*)) => ($submac!(11, $($rest)*));
  (11, $submac:ident ! ($($rest:tt)*)) => ($submac!(12, $($rest)*));
  (12, $submac:ident ! ($($rest:tt)*)) => ($submac!(13, $($rest)*));
  (13, $submac:ident ! ($($rest:tt)*)) => ($submac!(14, $($rest)*));
  (14, $submac:ident ! ($($rest:tt)*)) => ($submac!(15, $($rest)*));
  (15, $submac:ident ! ($($rest:tt)*)) => ($submac!(16, $($rest)*));
  (16, $submac:ident ! ($($rest:tt)*)) => ($submac!(17, $($rest)*));
  (17, $submac:ident ! ($($rest:tt)*)) => ($submac!(18, $($rest)*));
  (18, $submac:ident ! ($($rest:tt)*)) => ($submac!(19, $($rest)*));
  (19, $submac:ident ! ($($rest:tt)*)) => ($submac!(20, $($rest)*));
  (20, $submac:ident ! ($($rest:tt)*)) => ($submac!(21, $($rest)*));
);

macro_rules! alt_trait_inner(
  ($it:tt, $self:expr, $input:expr, $start:ident, $err:expr, $head:ident $($id:ident)+) => ({
    $input.reset(&$start);
    match $self.$it.parse_next($input) {
      Err(e) if e.is_backtrack() => {
        let err = $err.or(e);
        succ!($it, alt_trait_inner!($self, $input, $start, err, $($id)+))
      }
      res => res,
    }
  });
  ($it:tt, $self:expr, $input:expr, $start:ident, $err:expr, $head:ident) => ({
    Err($err.append($input, &$start))
  });
);

alt_trait!(Alt2 Alt3 Alt4 Alt5 Alt6 Alt7 Alt8 Alt9 Alt10 Alt11 Alt12 Alt13 Alt14 Alt15 Alt16 Alt17 Alt18 Alt19 Alt20 Alt21 Alt22);

// Manually implement Alt for (A,), the 1-tuple type
impl<I: Stream, O, E: ParserError<I>, A: Parser<I, O, E>> Alt<I, O, E> for (A,) {
    fn choice(&mut self, input: &mut I) -> Result<O, E> {
        self.0.parse_next(input)
    }
}

macro_rules! permutation_trait(
  (
    $name1:ident $ty1:ident $item1:ident
    $name2:ident $ty2:ident $item2:ident
    $($name3:ident $ty3:ident $item3:ident)*
  ) => (
    permutation_trait!(__impl $name1 $ty1 $item1, $name2 $ty2 $item2; $($name3 $ty3 $item3)*);
  );
  (
    __impl $($name:ident $ty:ident $item:ident),+;
    $name1:ident $ty1:ident $item1:ident $($name2:ident $ty2:ident $item2:ident)*
  ) => (
    permutation_trait_impl!($($name $ty $item),+);
    permutation_trait!(__impl $($name $ty $item),+ , $name1 $ty1 $item1; $($name2 $ty2 $item2)*);
  );
  (__impl $($name:ident $ty:ident $item:ident),+;) => (
    permutation_trait_impl!($($name $ty $item),+);
  );
);

macro_rules! permutation_trait_impl(
  ($($name:ident $ty:ident $item:ident),+) => (
    impl<
      I: Stream, $($ty),+ , Error: ParserError<I>,
      $($name: Parser<I, $ty, Error>),+
    > Permutation<I, ( $($ty),+ ), Error> for ( $($name),+ ) {

      fn permutation(&mut self, input: &mut I) -> Result<( $($ty),+ ), Error> {
        let mut res = ($(Option::<$ty>::None),+);

        loop {
          let mut err: Option<Error> = None;
          let start = input.checkpoint();
          permutation_trait_inner!(0, self, input, start, res, err, $($name)+);

          // If we reach here, every iterator has either been applied before,
          // or errored on the remaining input
          if let Some(err) = err {
            // There are remaining parsers, and all errored on the remaining input
            input.reset(&start);
            return Err(err.append(input, &start));
          }

          // All parsers were applied
          match res {
            ($(Some($item)),+) => return Ok(($($item),+)),
            _ => unreachable!(),
          }
        }
      }
    }
  );
);

macro_rules! permutation_trait_inner(
  ($it:tt, $self:expr, $input:ident, $start:ident, $res:expr, $err:expr, $head:ident $($id:ident)*) => (
    if $res.$it.is_none() {
      $input.reset(&$start);
      match $self.$it.parse_next($input) {
        Ok(o) => {
          $res.$it = Some(o);
          continue;
        }
        Err(e) if e.is_backtrack() => {
          $err = Some(match $err {
            Some(err) => err.or(e),
            None => e,
          });
        }
        Err(e) => return Err(e),
      };
    }
    succ!($it, permutation_trait_inner!($self, $input, $start, $res, $err, $($id)*));
  );
  ($it:tt, $self:expr, $input:ident, $start:ident, $res:expr, $err:expr,) => ();
);

permutation_trait!(
  P1 O1 o1
  P2 O2 o2
  P3 O3 o3
  P4 O4 o4
  P5 O5 o5
  P6 O6 o6
  P7 O7 o7
  P8 O8 o8
  P9 O9 o9
  P10 O10 o10
  P11 O11 o11
  P12 O12 o12
  P13 O13 o13
  P14 O14 o14
  P15 O15 o15
  P16 O16 o16
  P17 O17 o17
  P18 O18 o18
  P19 O19 o19
  P20 O20 o20
  P21 O21 o21
);

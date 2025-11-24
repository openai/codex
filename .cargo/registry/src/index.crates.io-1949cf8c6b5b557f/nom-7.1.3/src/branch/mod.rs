//! Choice combinators

#[cfg(test)]
mod tests;

use crate::error::ErrorKind;
use crate::error::ParseError;
use crate::internal::{Err, IResult, Parser};

/// Helper trait for the [alt()] combinator.
///
/// This trait is implemented for tuples of up to 21 elements
pub trait Alt<I, O, E> {
  /// Tests each parser in the tuple and returns the result of the first one that succeeds
  fn choice(&mut self, input: I) -> IResult<I, O, E>;
}

/// Tests a list of parsers one by one until one succeeds.
///
/// It takes as argument a tuple of parsers. There is a maximum of 21
/// parsers. If you need more, it is possible to nest them in other `alt` calls,
/// like this: `alt(parser_a, alt(parser_b, parser_c))`
///
/// ```rust
/// # use nom::error_position;
/// # use nom::{Err,error::ErrorKind, Needed, IResult};
/// use nom::character::complete::{alpha1, digit1};
/// use nom::branch::alt;
/// # fn main() {
/// fn parser(input: &str) -> IResult<&str, &str> {
///   alt((alpha1, digit1))(input)
/// };
///
/// // the first parser, alpha1, recognizes the input
/// assert_eq!(parser("abc"), Ok(("", "abc")));
///
/// // the first parser returns an error, so alt tries the second one
/// assert_eq!(parser("123456"), Ok(("", "123456")));
///
/// // both parsers failed, and with the default error type, alt will return the last error
/// assert_eq!(parser(" "), Err(Err::Error(error_position!(" ", ErrorKind::Digit))));
/// # }
/// ```
///
/// With a custom error type, it is possible to have alt return the error of the parser
/// that went the farthest in the input data
pub fn alt<I: Clone, O, E: ParseError<I>, List: Alt<I, O, E>>(
  mut l: List,
) -> impl FnMut(I) -> IResult<I, O, E> {
  move |i: I| l.choice(i)
}

/// Helper trait for the [permutation()] combinator.
///
/// This trait is implemented for tuples of up to 21 elements
pub trait Permutation<I, O, E> {
  /// Tries to apply all parsers in the tuple in various orders until all of them succeed
  fn permutation(&mut self, input: I) -> IResult<I, O, E>;
}

/// Applies a list of parsers in any order.
///
/// Permutation will succeed if all of the child parsers succeeded.
/// It takes as argument a tuple of parsers, and returns a
/// tuple of the parser results.
///
/// ```rust
/// # use nom::{Err,error::{Error, ErrorKind}, Needed, IResult};
/// use nom::character::complete::{alpha1, digit1};
/// use nom::branch::permutation;
/// # fn main() {
/// fn parser(input: &str) -> IResult<&str, (&str, &str)> {
///   permutation((alpha1, digit1))(input)
/// }
///
/// // permutation recognizes alphabetic characters then digit
/// assert_eq!(parser("abc123"), Ok(("", ("abc", "123"))));
///
/// // but also in inverse order
/// assert_eq!(parser("123abc"), Ok(("", ("abc", "123"))));
///
/// // it will fail if one of the parsers failed
/// assert_eq!(parser("abc;"), Err(Err::Error(Error::new(";", ErrorKind::Digit))));
/// # }
/// ```
///
/// The parsers are applied greedily: if there are multiple unapplied parsers
/// that could parse the next slice of input, the first one is used.
/// ```rust
/// # use nom::{Err, error::{Error, ErrorKind}, IResult};
/// use nom::branch::permutation;
/// use nom::character::complete::{anychar, char};
///
/// fn parser(input: &str) -> IResult<&str, (char, char)> {
///   permutation((anychar, char('a')))(input)
/// }
///
/// // anychar parses 'b', then char('a') parses 'a'
/// assert_eq!(parser("ba"), Ok(("", ('b', 'a'))));
///
/// // anychar parses 'a', then char('a') fails on 'b',
/// // even though char('a') followed by anychar would succeed
/// assert_eq!(parser("ab"), Err(Err::Error(Error::new("b", ErrorKind::Char))));
/// ```
///
pub fn permutation<I: Clone, O, E: ParseError<I>, List: Permutation<I, O, E>>(
  mut l: List,
) -> impl FnMut(I) -> IResult<I, O, E> {
  move |i: I| l.permutation(i)
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
      Input: Clone, Output, Error: ParseError<Input>,
      $($id: Parser<Input, Output, Error>),+
    > Alt<Input, Output, Error> for ( $($id),+ ) {

      fn choice(&mut self, input: Input) -> IResult<Input, Output, Error> {
        match self.0.parse(input.clone()) {
          Err(Err::Error(e)) => alt_trait_inner!(1, self, input, e, $($id)+),
          res => res,
        }
      }
    }
  );
);

macro_rules! alt_trait_inner(
  ($it:tt, $self:expr, $input:expr, $err:expr, $head:ident $($id:ident)+) => (
    match $self.$it.parse($input.clone()) {
      Err(Err::Error(e)) => {
        let err = $err.or(e);
        succ!($it, alt_trait_inner!($self, $input, err, $($id)+))
      }
      res => res,
    }
  );
  ($it:tt, $self:expr, $input:expr, $err:expr, $head:ident) => (
    Err(Err::Error(Error::append($input, ErrorKind::Alt, $err)))
  );
);

alt_trait!(A B C D E F G H I J K L M N O P Q R S T U);

// Manually implement Alt for (A,), the 1-tuple type
impl<Input, Output, Error: ParseError<Input>, A: Parser<Input, Output, Error>>
  Alt<Input, Output, Error> for (A,)
{
  fn choice(&mut self, input: Input) -> IResult<Input, Output, Error> {
    self.0.parse(input)
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
      Input: Clone, $($ty),+ , Error: ParseError<Input>,
      $($name: Parser<Input, $ty, Error>),+
    > Permutation<Input, ( $($ty),+ ), Error> for ( $($name),+ ) {

      fn permutation(&mut self, mut input: Input) -> IResult<Input, ( $($ty),+ ), Error> {
        let mut res = ($(Option::<$ty>::None),+);

        loop {
          let mut err: Option<Error> = None;
          permutation_trait_inner!(0, self, input, res, err, $($name)+);

          // If we reach here, every iterator has either been applied before,
          // or errored on the remaining input
          if let Some(err) = err {
            // There are remaining parsers, and all errored on the remaining input
            return Err(Err::Error(Error::append(input, ErrorKind::Permutation, err)));
          }

          // All parsers were applied
          match res {
            ($(Some($item)),+) => return Ok((input, ($($item),+))),
            _ => unreachable!(),
          }
        }
      }
    }
  );
);

macro_rules! permutation_trait_inner(
  ($it:tt, $self:expr, $input:ident, $res:expr, $err:expr, $head:ident $($id:ident)*) => (
    if $res.$it.is_none() {
      match $self.$it.parse($input.clone()) {
        Ok((i, o)) => {
          $input = i;
          $res.$it = Some(o);
          continue;
        }
        Err(Err::Error(e)) => {
          $err = Some(match $err {
            Some(err) => err.or(e),
            None => e,
          });
        }
        Err(e) => return Err(e),
      };
    }
    succ!($it, permutation_trait_inner!($self, $input, $res, $err, $($id)*));
  );
  ($it:tt, $self:expr, $input:ident, $res:expr, $err:expr,) => ();
);

permutation_trait!(
  FnA A a
  FnB B b
  FnC C c
  FnD D d
  FnE E e
  FnF F f
  FnG G g
  FnH H h
  FnI I i
  FnJ J j
  FnK K k
  FnL L l
  FnM M m
  FnN N n
  FnO O o
  FnP P p
  FnQ Q q
  FnR R r
  FnS S s
  FnT T t
  FnU U u
);

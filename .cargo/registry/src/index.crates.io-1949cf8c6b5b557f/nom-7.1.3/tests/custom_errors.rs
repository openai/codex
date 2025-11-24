#![allow(dead_code)]

use nom::bytes::streaming::tag;
use nom::character::streaming::digit1 as digit;
use nom::combinator::verify;
use nom::error::{ErrorKind, ParseError};
#[cfg(feature = "alloc")]
use nom::multi::count;
use nom::sequence::terminated;
use nom::IResult;

#[derive(Debug)]
pub struct CustomError(String);

impl<'a> From<(&'a str, ErrorKind)> for CustomError {
  fn from(error: (&'a str, ErrorKind)) -> Self {
    CustomError(format!("error code was: {:?}", error))
  }
}

impl<'a> ParseError<&'a str> for CustomError {
  fn from_error_kind(_: &'a str, kind: ErrorKind) -> Self {
    CustomError(format!("error code was: {:?}", kind))
  }

  fn append(_: &'a str, kind: ErrorKind, other: CustomError) -> Self {
    CustomError(format!("{:?}\nerror code was: {:?}", other, kind))
  }
}

fn test1(input: &str) -> IResult<&str, &str, CustomError> {
  //fix_error!(input, CustomError, tag!("abcd"))
  tag("abcd")(input)
}

fn test2(input: &str) -> IResult<&str, &str, CustomError> {
  //terminated!(input, test1, fix_error!(CustomError, digit))
  terminated(test1, digit)(input)
}

fn test3(input: &str) -> IResult<&str, &str, CustomError> {
  verify(test1, |s: &str| s.starts_with("abcd"))(input)
}

#[cfg(feature = "alloc")]
fn test4(input: &str) -> IResult<&str, Vec<&str>, CustomError> {
  count(test1, 4)(input)
}

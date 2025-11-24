use super::streaming::*;
use crate::error::ErrorKind;
use crate::internal::{Err, IResult};

#[test]
fn one_of_test() {
  fn f(i: &[u8]) -> IResult<&[u8], char> {
    one_of("ab")(i)
  }

  let a = &b"abcd"[..];
  assert_eq!(f(a), Ok((&b"bcd"[..], 'a')));

  let b = &b"cde"[..];
  assert_eq!(f(b), Err(Err::Error(error_position!(b, ErrorKind::OneOf))));

  fn utf8(i: &str) -> IResult<&str, char> {
    one_of("+\u{FF0B}")(i)
  }

  assert!(utf8("+").is_ok());
  assert!(utf8("\u{FF0B}").is_ok());
}

#[test]
fn none_of_test() {
  fn f(i: &[u8]) -> IResult<&[u8], char> {
    none_of("ab")(i)
  }

  let a = &b"abcd"[..];
  assert_eq!(f(a), Err(Err::Error(error_position!(a, ErrorKind::NoneOf))));

  let b = &b"cde"[..];
  assert_eq!(f(b), Ok((&b"de"[..], 'c')));
}

#[test]
fn char_byteslice() {
  fn f(i: &[u8]) -> IResult<&[u8], char> {
    char('c')(i)
  }

  let a = &b"abcd"[..];
  assert_eq!(f(a), Err(Err::Error(error_position!(a, ErrorKind::Char))));

  let b = &b"cde"[..];
  assert_eq!(f(b), Ok((&b"de"[..], 'c')));
}

#[test]
fn char_str() {
  fn f(i: &str) -> IResult<&str, char> {
    char('c')(i)
  }

  let a = &"abcd"[..];
  assert_eq!(f(a), Err(Err::Error(error_position!(a, ErrorKind::Char))));

  let b = &"cde"[..];
  assert_eq!(f(b), Ok((&"de"[..], 'c')));
}

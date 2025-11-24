use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::streaming::digit1 as digit;
use nom::combinator::{map, map_res, opt, recognize};
use nom::sequence::{delimited, pair};
use nom::IResult;

use std::str;
use std::str::FromStr;

fn unsigned_float(i: &[u8]) -> IResult<&[u8], f32> {
  let float_bytes = recognize(alt((
    delimited(digit, tag("."), opt(digit)),
    delimited(opt(digit), tag("."), digit),
  )));
  let float_str = map_res(float_bytes, str::from_utf8);
  map_res(float_str, FromStr::from_str)(i)
}

fn float(i: &[u8]) -> IResult<&[u8], f32> {
  map(
    pair(opt(alt((tag("+"), tag("-")))), unsigned_float),
    |(sign, value)| {
      sign
        .and_then(|s| if s[0] == b'-' { Some(-1f32) } else { None })
        .unwrap_or(1f32)
        * value
    },
  )(i)
}

#[test]
fn unsigned_float_test() {
  assert_eq!(unsigned_float(&b"123.456;"[..]), Ok((&b";"[..], 123.456)));
  assert_eq!(unsigned_float(&b"0.123;"[..]), Ok((&b";"[..], 0.123)));
  assert_eq!(unsigned_float(&b"123.0;"[..]), Ok((&b";"[..], 123.0)));
  assert_eq!(unsigned_float(&b"123.;"[..]), Ok((&b";"[..], 123.0)));
  assert_eq!(unsigned_float(&b".123;"[..]), Ok((&b";"[..], 0.123)));
}

#[test]
fn float_test() {
  assert_eq!(float(&b"123.456;"[..]), Ok((&b";"[..], 123.456)));
  assert_eq!(float(&b"+123.456;"[..]), Ok((&b";"[..], 123.456)));
  assert_eq!(float(&b"-123.456;"[..]), Ok((&b";"[..], -123.456)));
}

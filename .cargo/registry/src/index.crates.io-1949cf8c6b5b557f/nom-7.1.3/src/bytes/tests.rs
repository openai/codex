use crate::character::is_alphabetic;
use crate::character::streaming::{
  alpha1 as alpha, alphanumeric1 as alphanumeric, digit1 as digit, hex_digit1 as hex_digit,
  multispace1 as multispace, oct_digit1 as oct_digit, space1 as space,
};
use crate::error::ErrorKind;
use crate::internal::{Err, IResult, Needed};
#[cfg(feature = "alloc")]
use crate::{
  branch::alt,
  bytes::complete::{escaped, escaped_transform, tag},
  combinator::{map, value},
  lib::std::string::String,
  lib::std::vec::Vec,
};

#[test]
fn is_a() {
  use crate::bytes::streaming::is_a;

  fn a_or_b(i: &[u8]) -> IResult<&[u8], &[u8]> {
    is_a("ab")(i)
  }

  let a = &b"abcd"[..];
  assert_eq!(a_or_b(a), Ok((&b"cd"[..], &b"ab"[..])));

  let b = &b"bcde"[..];
  assert_eq!(a_or_b(b), Ok((&b"cde"[..], &b"b"[..])));

  let c = &b"cdef"[..];
  assert_eq!(
    a_or_b(c),
    Err(Err::Error(error_position!(c, ErrorKind::IsA)))
  );

  let d = &b"bacdef"[..];
  assert_eq!(a_or_b(d), Ok((&b"cdef"[..], &b"ba"[..])));
}

#[test]
fn is_not() {
  use crate::bytes::streaming::is_not;

  fn a_or_b(i: &[u8]) -> IResult<&[u8], &[u8]> {
    is_not("ab")(i)
  }

  let a = &b"cdab"[..];
  assert_eq!(a_or_b(a), Ok((&b"ab"[..], &b"cd"[..])));

  let b = &b"cbde"[..];
  assert_eq!(a_or_b(b), Ok((&b"bde"[..], &b"c"[..])));

  let c = &b"abab"[..];
  assert_eq!(
    a_or_b(c),
    Err(Err::Error(error_position!(c, ErrorKind::IsNot)))
  );

  let d = &b"cdefba"[..];
  assert_eq!(a_or_b(d), Ok((&b"ba"[..], &b"cdef"[..])));

  let e = &b"e"[..];
  assert_eq!(a_or_b(e), Err(Err::Incomplete(Needed::new(1))));
}

#[cfg(feature = "alloc")]
#[allow(unused_variables)]
#[test]
fn escaping() {
  use crate::character::streaming::one_of;

  fn esc(i: &[u8]) -> IResult<&[u8], &[u8]> {
    escaped(alpha, '\\', one_of("\"n\\"))(i)
  }
  assert_eq!(esc(&b"abcd;"[..]), Ok((&b";"[..], &b"abcd"[..])));
  assert_eq!(esc(&b"ab\\\"cd;"[..]), Ok((&b";"[..], &b"ab\\\"cd"[..])));
  assert_eq!(esc(&b"\\\"abcd;"[..]), Ok((&b";"[..], &b"\\\"abcd"[..])));
  assert_eq!(esc(&b"\\n;"[..]), Ok((&b";"[..], &b"\\n"[..])));
  assert_eq!(esc(&b"ab\\\"12"[..]), Ok((&b"12"[..], &b"ab\\\""[..])));
  assert_eq!(
    esc(&b"AB\\"[..]),
    Err(Err::Error(error_position!(
      &b"AB\\"[..],
      ErrorKind::Escaped
    )))
  );
  assert_eq!(
    esc(&b"AB\\A"[..]),
    Err(Err::Error(error_node_position!(
      &b"AB\\A"[..],
      ErrorKind::Escaped,
      error_position!(&b"A"[..], ErrorKind::OneOf)
    )))
  );

  fn esc2(i: &[u8]) -> IResult<&[u8], &[u8]> {
    escaped(digit, '\\', one_of("\"n\\"))(i)
  }
  assert_eq!(esc2(&b"12\\nnn34"[..]), Ok((&b"nn34"[..], &b"12\\n"[..])));
}

#[cfg(feature = "alloc")]
#[test]
fn escaping_str() {
  use crate::character::streaming::one_of;

  fn esc(i: &str) -> IResult<&str, &str> {
    escaped(alpha, '\\', one_of("\"n\\"))(i)
  }
  assert_eq!(esc("abcd;"), Ok((";", "abcd")));
  assert_eq!(esc("ab\\\"cd;"), Ok((";", "ab\\\"cd")));
  assert_eq!(esc("\\\"abcd;"), Ok((";", "\\\"abcd")));
  assert_eq!(esc("\\n;"), Ok((";", "\\n")));
  assert_eq!(esc("ab\\\"12"), Ok(("12", "ab\\\"")));
  assert_eq!(
    esc("AB\\"),
    Err(Err::Error(error_position!("AB\\", ErrorKind::Escaped)))
  );
  assert_eq!(
    esc("AB\\A"),
    Err(Err::Error(error_node_position!(
      "AB\\A",
      ErrorKind::Escaped,
      error_position!("A", ErrorKind::OneOf)
    )))
  );

  fn esc2(i: &str) -> IResult<&str, &str> {
    escaped(digit, '\\', one_of("\"n\\"))(i)
  }
  assert_eq!(esc2("12\\nnn34"), Ok(("nn34", "12\\n")));

  fn esc3(i: &str) -> IResult<&str, &str> {
    escaped(alpha, '\u{241b}', one_of("\"n"))(i)
  }
  assert_eq!(esc3("ab␛ncd;"), Ok((";", "ab␛ncd")));
}

#[cfg(feature = "alloc")]
fn to_s(i: Vec<u8>) -> String {
  String::from_utf8_lossy(&i).into_owned()
}

#[cfg(feature = "alloc")]
#[test]
fn escape_transform() {
  fn esc(i: &[u8]) -> IResult<&[u8], String> {
    map(
      escaped_transform(
        alpha,
        '\\',
        alt((
          value(&b"\\"[..], tag("\\")),
          value(&b"\""[..], tag("\"")),
          value(&b"\n"[..], tag("n")),
        )),
      ),
      to_s,
    )(i)
  }

  assert_eq!(esc(&b"abcd;"[..]), Ok((&b";"[..], String::from("abcd"))));
  assert_eq!(
    esc(&b"ab\\\"cd;"[..]),
    Ok((&b";"[..], String::from("ab\"cd")))
  );
  assert_eq!(
    esc(&b"\\\"abcd;"[..]),
    Ok((&b";"[..], String::from("\"abcd")))
  );
  assert_eq!(esc(&b"\\n;"[..]), Ok((&b";"[..], String::from("\n"))));
  assert_eq!(
    esc(&b"ab\\\"12"[..]),
    Ok((&b"12"[..], String::from("ab\"")))
  );
  assert_eq!(
    esc(&b"AB\\"[..]),
    Err(Err::Error(error_position!(
      &b"\\"[..],
      ErrorKind::EscapedTransform
    )))
  );
  assert_eq!(
    esc(&b"AB\\A"[..]),
    Err(Err::Error(error_node_position!(
      &b"AB\\A"[..],
      ErrorKind::EscapedTransform,
      error_position!(&b"A"[..], ErrorKind::Tag)
    )))
  );

  fn esc2(i: &[u8]) -> IResult<&[u8], String> {
    map(
      escaped_transform(
        alpha,
        '&',
        alt((
          value("è".as_bytes(), tag("egrave;")),
          value("à".as_bytes(), tag("agrave;")),
        )),
      ),
      to_s,
    )(i)
  }
  assert_eq!(
    esc2(&b"ab&egrave;DEF;"[..]),
    Ok((&b";"[..], String::from("abèDEF")))
  );
  assert_eq!(
    esc2(&b"ab&egrave;D&agrave;EF;"[..]),
    Ok((&b";"[..], String::from("abèDàEF")))
  );
}

#[cfg(feature = "std")]
#[test]
fn escape_transform_str() {
  fn esc(i: &str) -> IResult<&str, String> {
    escaped_transform(
      alpha,
      '\\',
      alt((
        value("\\", tag("\\")),
        value("\"", tag("\"")),
        value("\n", tag("n")),
      )),
    )(i)
  }

  assert_eq!(esc("abcd;"), Ok((";", String::from("abcd"))));
  assert_eq!(esc("ab\\\"cd;"), Ok((";", String::from("ab\"cd"))));
  assert_eq!(esc("\\\"abcd;"), Ok((";", String::from("\"abcd"))));
  assert_eq!(esc("\\n;"), Ok((";", String::from("\n"))));
  assert_eq!(esc("ab\\\"12"), Ok(("12", String::from("ab\""))));
  assert_eq!(
    esc("AB\\"),
    Err(Err::Error(error_position!(
      "\\",
      ErrorKind::EscapedTransform
    )))
  );
  assert_eq!(
    esc("AB\\A"),
    Err(Err::Error(error_node_position!(
      "AB\\A",
      ErrorKind::EscapedTransform,
      error_position!("A", ErrorKind::Tag)
    )))
  );

  fn esc2(i: &str) -> IResult<&str, String> {
    escaped_transform(
      alpha,
      '&',
      alt((value("è", tag("egrave;")), value("à", tag("agrave;")))),
    )(i)
  }
  assert_eq!(esc2("ab&egrave;DEF;"), Ok((";", String::from("abèDEF"))));
  assert_eq!(
    esc2("ab&egrave;D&agrave;EF;"),
    Ok((";", String::from("abèDàEF")))
  );

  fn esc3(i: &str) -> IResult<&str, String> {
    escaped_transform(
      alpha,
      '␛',
      alt((value("\0", tag("0")), value("\n", tag("n")))),
    )(i)
  }
  assert_eq!(esc3("a␛0bc␛n"), Ok(("", String::from("a\0bc\n"))));
}

#[test]
fn take_until_incomplete() {
  use crate::bytes::streaming::take_until;
  fn y(i: &[u8]) -> IResult<&[u8], &[u8]> {
    take_until("end")(i)
  }
  assert_eq!(y(&b"nd"[..]), Err(Err::Incomplete(Needed::Unknown)));
  assert_eq!(y(&b"123"[..]), Err(Err::Incomplete(Needed::Unknown)));
  assert_eq!(y(&b"123en"[..]), Err(Err::Incomplete(Needed::Unknown)));
}

#[test]
fn take_until_incomplete_s() {
  use crate::bytes::streaming::take_until;
  fn ys(i: &str) -> IResult<&str, &str> {
    take_until("end")(i)
  }
  assert_eq!(ys("123en"), Err(Err::Incomplete(Needed::Unknown)));
}

#[test]
fn recognize() {
  use crate::bytes::streaming::{tag, take};
  use crate::combinator::recognize;
  use crate::sequence::delimited;

  fn x(i: &[u8]) -> IResult<&[u8], &[u8]> {
    recognize(delimited(tag("<!--"), take(5_usize), tag("-->")))(i)
  }
  let r = x(&b"<!-- abc --> aaa"[..]);
  assert_eq!(r, Ok((&b" aaa"[..], &b"<!-- abc -->"[..])));

  let semicolon = &b";"[..];

  fn ya(i: &[u8]) -> IResult<&[u8], &[u8]> {
    recognize(alpha)(i)
  }
  let ra = ya(&b"abc;"[..]);
  assert_eq!(ra, Ok((semicolon, &b"abc"[..])));

  fn yd(i: &[u8]) -> IResult<&[u8], &[u8]> {
    recognize(digit)(i)
  }
  let rd = yd(&b"123;"[..]);
  assert_eq!(rd, Ok((semicolon, &b"123"[..])));

  fn yhd(i: &[u8]) -> IResult<&[u8], &[u8]> {
    recognize(hex_digit)(i)
  }
  let rhd = yhd(&b"123abcDEF;"[..]);
  assert_eq!(rhd, Ok((semicolon, &b"123abcDEF"[..])));

  fn yod(i: &[u8]) -> IResult<&[u8], &[u8]> {
    recognize(oct_digit)(i)
  }
  let rod = yod(&b"1234567;"[..]);
  assert_eq!(rod, Ok((semicolon, &b"1234567"[..])));

  fn yan(i: &[u8]) -> IResult<&[u8], &[u8]> {
    recognize(alphanumeric)(i)
  }
  let ran = yan(&b"123abc;"[..]);
  assert_eq!(ran, Ok((semicolon, &b"123abc"[..])));

  fn ys(i: &[u8]) -> IResult<&[u8], &[u8]> {
    recognize(space)(i)
  }
  let rs = ys(&b" \t;"[..]);
  assert_eq!(rs, Ok((semicolon, &b" \t"[..])));

  fn yms(i: &[u8]) -> IResult<&[u8], &[u8]> {
    recognize(multispace)(i)
  }
  let rms = yms(&b" \t\r\n;"[..]);
  assert_eq!(rms, Ok((semicolon, &b" \t\r\n"[..])));
}

#[test]
fn take_while() {
  use crate::bytes::streaming::take_while;

  fn f(i: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while(is_alphabetic)(i)
  }
  let a = b"";
  let b = b"abcd";
  let c = b"abcd123";
  let d = b"123";

  assert_eq!(f(&a[..]), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(f(&b[..]), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(f(&c[..]), Ok((&d[..], &b[..])));
  assert_eq!(f(&d[..]), Ok((&d[..], &a[..])));
}

#[test]
fn take_while1() {
  use crate::bytes::streaming::take_while1;

  fn f(i: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while1(is_alphabetic)(i)
  }
  let a = b"";
  let b = b"abcd";
  let c = b"abcd123";
  let d = b"123";

  assert_eq!(f(&a[..]), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(f(&b[..]), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(f(&c[..]), Ok((&b"123"[..], &b[..])));
  assert_eq!(
    f(&d[..]),
    Err(Err::Error(error_position!(&d[..], ErrorKind::TakeWhile1)))
  );
}

#[test]
fn take_while_m_n() {
  use crate::bytes::streaming::take_while_m_n;

  fn x(i: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while_m_n(2, 4, is_alphabetic)(i)
  }
  let a = b"";
  let b = b"a";
  let c = b"abc";
  let d = b"abc123";
  let e = b"abcde";
  let f = b"123";

  assert_eq!(x(&a[..]), Err(Err::Incomplete(Needed::new(2))));
  assert_eq!(x(&b[..]), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(x(&c[..]), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(x(&d[..]), Ok((&b"123"[..], &c[..])));
  assert_eq!(x(&e[..]), Ok((&b"e"[..], &b"abcd"[..])));
  assert_eq!(
    x(&f[..]),
    Err(Err::Error(error_position!(&f[..], ErrorKind::TakeWhileMN)))
  );
}

#[test]
fn take_till() {
  use crate::bytes::streaming::take_till;

  fn f(i: &[u8]) -> IResult<&[u8], &[u8]> {
    take_till(is_alphabetic)(i)
  }
  let a = b"";
  let b = b"abcd";
  let c = b"123abcd";
  let d = b"123";

  assert_eq!(f(&a[..]), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(f(&b[..]), Ok((&b"abcd"[..], &b""[..])));
  assert_eq!(f(&c[..]), Ok((&b"abcd"[..], &b"123"[..])));
  assert_eq!(f(&d[..]), Err(Err::Incomplete(Needed::new(1))));
}

#[test]
fn take_till1() {
  use crate::bytes::streaming::take_till1;

  fn f(i: &[u8]) -> IResult<&[u8], &[u8]> {
    take_till1(is_alphabetic)(i)
  }
  let a = b"";
  let b = b"abcd";
  let c = b"123abcd";
  let d = b"123";

  assert_eq!(f(&a[..]), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(
    f(&b[..]),
    Err(Err::Error(error_position!(&b[..], ErrorKind::TakeTill1)))
  );
  assert_eq!(f(&c[..]), Ok((&b"abcd"[..], &b"123"[..])));
  assert_eq!(f(&d[..]), Err(Err::Incomplete(Needed::new(1))));
}

#[test]
fn take_while_utf8() {
  use crate::bytes::streaming::take_while;

  fn f(i: &str) -> IResult<&str, &str> {
    take_while(|c| c != '點')(i)
  }

  assert_eq!(f(""), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(f("abcd"), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(f("abcd點"), Ok(("點", "abcd")));
  assert_eq!(f("abcd點a"), Ok(("點a", "abcd")));

  fn g(i: &str) -> IResult<&str, &str> {
    take_while(|c| c == '點')(i)
  }

  assert_eq!(g(""), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(g("點abcd"), Ok(("abcd", "點")));
  assert_eq!(g("點點點a"), Ok(("a", "點點點")));
}

#[test]
fn take_till_utf8() {
  use crate::bytes::streaming::take_till;

  fn f(i: &str) -> IResult<&str, &str> {
    take_till(|c| c == '點')(i)
  }

  assert_eq!(f(""), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(f("abcd"), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(f("abcd點"), Ok(("點", "abcd")));
  assert_eq!(f("abcd點a"), Ok(("點a", "abcd")));

  fn g(i: &str) -> IResult<&str, &str> {
    take_till(|c| c != '點')(i)
  }

  assert_eq!(g(""), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(g("點abcd"), Ok(("abcd", "點")));
  assert_eq!(g("點點點a"), Ok(("a", "點點點")));
}

#[test]
fn take_utf8() {
  use crate::bytes::streaming::{take, take_while};

  fn f(i: &str) -> IResult<&str, &str> {
    take(3_usize)(i)
  }

  assert_eq!(f(""), Err(Err::Incomplete(Needed::Unknown)));
  assert_eq!(f("ab"), Err(Err::Incomplete(Needed::Unknown)));
  assert_eq!(f("點"), Err(Err::Incomplete(Needed::Unknown)));
  assert_eq!(f("ab點cd"), Ok(("cd", "ab點")));
  assert_eq!(f("a點bcd"), Ok(("cd", "a點b")));
  assert_eq!(f("a點b"), Ok(("", "a點b")));

  fn g(i: &str) -> IResult<&str, &str> {
    take_while(|c| c == '點')(i)
  }

  assert_eq!(g(""), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(g("點abcd"), Ok(("abcd", "點")));
  assert_eq!(g("點點點a"), Ok(("a", "點點點")));
}

#[test]
fn take_while_m_n_utf8() {
  use crate::bytes::streaming::take_while_m_n;

  fn parser(i: &str) -> IResult<&str, &str> {
    take_while_m_n(1, 1, |c| c == 'A' || c == '😃')(i)
  }
  assert_eq!(parser("A!"), Ok(("!", "A")));
  assert_eq!(parser("😃!"), Ok(("!", "😃")));
}

#[test]
fn take_while_m_n_utf8_full_match() {
  use crate::bytes::streaming::take_while_m_n;

  fn parser(i: &str) -> IResult<&str, &str> {
    take_while_m_n(1, 1, |c: char| c.is_alphabetic())(i)
  }
  assert_eq!(parser("øn"), Ok(("n", "ø")));
}

#[test]
#[cfg(feature = "std")]
fn recognize_take_while() {
  use crate::bytes::streaming::take_while;
  use crate::character::is_alphanumeric;
  use crate::combinator::recognize;

  fn x(i: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while(is_alphanumeric)(i)
  }
  fn y(i: &[u8]) -> IResult<&[u8], &[u8]> {
    recognize(x)(i)
  }
  assert_eq!(x(&b"ab."[..]), Ok((&b"."[..], &b"ab"[..])));
  println!("X: {:?}", x(&b"ab"[..]));
  assert_eq!(y(&b"ab."[..]), Ok((&b"."[..], &b"ab"[..])));
}

#[test]
fn length_bytes() {
  use crate::{bytes::streaming::tag, multi::length_data, number::streaming::le_u8};

  fn x(i: &[u8]) -> IResult<&[u8], &[u8]> {
    length_data(le_u8)(i)
  }
  assert_eq!(x(b"\x02..>>"), Ok((&b">>"[..], &b".."[..])));
  assert_eq!(x(b"\x02.."), Ok((&[][..], &b".."[..])));
  assert_eq!(x(b"\x02."), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(x(b"\x02"), Err(Err::Incomplete(Needed::new(2))));

  fn y(i: &[u8]) -> IResult<&[u8], &[u8]> {
    let (i, _) = tag("magic")(i)?;
    length_data(le_u8)(i)
  }
  assert_eq!(y(b"magic\x02..>>"), Ok((&b">>"[..], &b".."[..])));
  assert_eq!(y(b"magic\x02.."), Ok((&[][..], &b".."[..])));
  assert_eq!(y(b"magic\x02."), Err(Err::Incomplete(Needed::new(1))));
  assert_eq!(y(b"magic\x02"), Err(Err::Incomplete(Needed::new(2))));
}

#[cfg(feature = "alloc")]
#[test]
fn case_insensitive() {
  use crate::bytes::streaming::tag_no_case;

  fn test(i: &[u8]) -> IResult<&[u8], &[u8]> {
    tag_no_case("ABcd")(i)
  }
  assert_eq!(test(&b"aBCdefgh"[..]), Ok((&b"efgh"[..], &b"aBCd"[..])));
  assert_eq!(test(&b"abcdefgh"[..]), Ok((&b"efgh"[..], &b"abcd"[..])));
  assert_eq!(test(&b"ABCDefgh"[..]), Ok((&b"efgh"[..], &b"ABCD"[..])));
  assert_eq!(test(&b"ab"[..]), Err(Err::Incomplete(Needed::new(2))));
  assert_eq!(
    test(&b"Hello"[..]),
    Err(Err::Error(error_position!(&b"Hello"[..], ErrorKind::Tag)))
  );
  assert_eq!(
    test(&b"Hel"[..]),
    Err(Err::Error(error_position!(&b"Hel"[..], ErrorKind::Tag)))
  );

  fn test2(i: &str) -> IResult<&str, &str> {
    tag_no_case("ABcd")(i)
  }
  assert_eq!(test2("aBCdefgh"), Ok(("efgh", "aBCd")));
  assert_eq!(test2("abcdefgh"), Ok(("efgh", "abcd")));
  assert_eq!(test2("ABCDefgh"), Ok(("efgh", "ABCD")));
  assert_eq!(test2("ab"), Err(Err::Incomplete(Needed::new(2))));
  assert_eq!(
    test2("Hello"),
    Err(Err::Error(error_position!(&"Hello"[..], ErrorKind::Tag)))
  );
  assert_eq!(
    test2("Hel"),
    Err(Err::Error(error_position!(&"Hel"[..], ErrorKind::Tag)))
  );
}

#[test]
fn tag_fixed_size_array() {
  use crate::bytes::streaming::tag;

  fn test(i: &[u8]) -> IResult<&[u8], &[u8]> {
    tag([0x42])(i)
  }
  fn test2(i: &[u8]) -> IResult<&[u8], &[u8]> {
    tag(&[0x42])(i)
  }
  let input = [0x42, 0x00];
  assert_eq!(test(&input), Ok((&b"\x00"[..], &b"\x42"[..])));
  assert_eq!(test2(&input), Ok((&b"\x00"[..], &b"\x42"[..])));
}

use nom::{
  bytes::complete::take_while,
  character::complete::{
    alphanumeric1 as alphanumeric, char, multispace0 as multispace, space0 as space,
  },
  combinator::{map, map_res, opt},
  multi::many0,
  sequence::{delimited, pair, separated_pair, terminated, tuple},
  IResult,
};

use std::collections::HashMap;
use std::str;

fn category(i: &[u8]) -> IResult<&[u8], &str> {
  map_res(
    delimited(char('['), take_while(|c| c != b']'), char(']')),
    str::from_utf8,
  )(i)
}

fn key_value(i: &[u8]) -> IResult<&[u8], (&str, &str)> {
  let (i, key) = map_res(alphanumeric, str::from_utf8)(i)?;
  let (i, _) = tuple((opt(space), char('='), opt(space)))(i)?;
  let (i, val) = map_res(take_while(|c| c != b'\n' && c != b';'), str::from_utf8)(i)?;
  let (i, _) = opt(pair(char(';'), take_while(|c| c != b'\n')))(i)?;
  Ok((i, (key, val)))
}

fn keys_and_values(i: &[u8]) -> IResult<&[u8], HashMap<&str, &str>> {
  map(many0(terminated(key_value, opt(multispace))), |vec| {
    vec.into_iter().collect()
  })(i)
}

fn category_and_keys(i: &[u8]) -> IResult<&[u8], (&str, HashMap<&str, &str>)> {
  let (i, category) = terminated(category, opt(multispace))(i)?;
  let (i, keys) = keys_and_values(i)?;
  Ok((i, (category, keys)))
}

fn categories(i: &[u8]) -> IResult<&[u8], HashMap<&str, HashMap<&str, &str>>> {
  map(
    many0(separated_pair(
      category,
      opt(multispace),
      map(
        many0(terminated(key_value, opt(multispace))),
        |vec: Vec<_>| vec.into_iter().collect(),
      ),
    )),
    |vec: Vec<_>| vec.into_iter().collect(),
  )(i)
}

#[test]
fn parse_category_test() {
  let ini_file = &b"[category]

parameter=value
key = value2"[..];

  let ini_without_category = &b"\n\nparameter=value
key = value2"[..];

  let res = category(ini_file);
  println!("{:?}", res);
  match res {
    Ok((i, o)) => println!("i: {:?} | o: {:?}", str::from_utf8(i), o),
    _ => println!("error"),
  }

  assert_eq!(res, Ok((ini_without_category, "category")));
}

#[test]
fn parse_key_value_test() {
  let ini_file = &b"parameter=value
key = value2"[..];

  let ini_without_key_value = &b"\nkey = value2"[..];

  let res = key_value(ini_file);
  println!("{:?}", res);
  match res {
    Ok((i, (o1, o2))) => println!("i: {:?} | o: ({:?},{:?})", str::from_utf8(i), o1, o2),
    _ => println!("error"),
  }

  assert_eq!(res, Ok((ini_without_key_value, ("parameter", "value"))));
}

#[test]
fn parse_key_value_with_space_test() {
  let ini_file = &b"parameter = value
key = value2"[..];

  let ini_without_key_value = &b"\nkey = value2"[..];

  let res = key_value(ini_file);
  println!("{:?}", res);
  match res {
    Ok((i, (o1, o2))) => println!("i: {:?} | o: ({:?},{:?})", str::from_utf8(i), o1, o2),
    _ => println!("error"),
  }

  assert_eq!(res, Ok((ini_without_key_value, ("parameter", "value"))));
}

#[test]
fn parse_key_value_with_comment_test() {
  let ini_file = &b"parameter=value;abc
key = value2"[..];

  let ini_without_key_value = &b"\nkey = value2"[..];

  let res = key_value(ini_file);
  println!("{:?}", res);
  match res {
    Ok((i, (o1, o2))) => println!("i: {:?} | o: ({:?},{:?})", str::from_utf8(i), o1, o2),
    _ => println!("error"),
  }

  assert_eq!(res, Ok((ini_without_key_value, ("parameter", "value"))));
}

#[test]
fn parse_multiple_keys_and_values_test() {
  let ini_file = &b"parameter=value;abc

key = value2

[category]"[..];

  let ini_without_key_value = &b"[category]"[..];

  let res = keys_and_values(ini_file);
  println!("{:?}", res);
  match res {
    Ok((i, ref o)) => println!("i: {:?} | o: {:?}", str::from_utf8(i), o),
    _ => println!("error"),
  }

  let mut expected: HashMap<&str, &str> = HashMap::new();
  expected.insert("parameter", "value");
  expected.insert("key", "value2");
  assert_eq!(res, Ok((ini_without_key_value, expected)));
}

#[test]
fn parse_category_then_multiple_keys_and_values_test() {
  //FIXME: there can be an empty line or a comment line after a category
  let ini_file = &b"[abcd]
parameter=value;abc

key = value2

[category]"[..];

  let ini_after_parser = &b"[category]"[..];

  let res = category_and_keys(ini_file);
  println!("{:?}", res);
  match res {
    Ok((i, ref o)) => println!("i: {:?} | o: {:?}", str::from_utf8(i), o),
    _ => println!("error"),
  }

  let mut expected_h: HashMap<&str, &str> = HashMap::new();
  expected_h.insert("parameter", "value");
  expected_h.insert("key", "value2");
  assert_eq!(res, Ok((ini_after_parser, ("abcd", expected_h))));
}

#[test]
fn parse_multiple_categories_test() {
  let ini_file = &b"[abcd]

parameter=value;abc

key = value2

[category]
parameter3=value3
key4 = value4
"[..];

  let ini_after_parser = &b""[..];

  let res = categories(ini_file);
  //println!("{:?}", res);
  match res {
    Ok((i, ref o)) => println!("i: {:?} | o: {:?}", str::from_utf8(i), o),
    _ => println!("error"),
  }

  let mut expected_1: HashMap<&str, &str> = HashMap::new();
  expected_1.insert("parameter", "value");
  expected_1.insert("key", "value2");
  let mut expected_2: HashMap<&str, &str> = HashMap::new();
  expected_2.insert("parameter3", "value3");
  expected_2.insert("key4", "value4");
  let mut expected_h: HashMap<&str, HashMap<&str, &str>> = HashMap::new();
  expected_h.insert("abcd", expected_1);
  expected_h.insert("category", expected_2);
  assert_eq!(res, Ok((ini_after_parser, expected_h)));
}

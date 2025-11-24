use std::collections::HashMap;

use winnow::prelude::*;
use winnow::Result;
use winnow::{
    ascii::{alphanumeric1 as alphanumeric, space0 as space},
    combinator::opt,
    combinator::repeat,
    combinator::{delimited, terminated},
    token::{take_till, take_while},
};

pub(crate) type Stream<'i> = &'i str;

pub(crate) fn categories<'s>(
    input: &mut Stream<'s>,
) -> Result<HashMap<&'s str, HashMap<&'s str, &'s str>>> {
    repeat(0.., category_and_keys).parse_next(input)
}

fn category_and_keys<'s>(i: &mut Stream<'s>) -> Result<(&'s str, HashMap<&'s str, &'s str>)> {
    (category, keys_and_values).parse_next(i)
}

fn category<'s>(i: &mut Stream<'s>) -> Result<&'s str> {
    terminated(
        delimited('[', take_while(0.., |c| c != ']'), ']'),
        opt(take_while(1.., [' ', '\r', '\n'])),
    )
    .parse_next(i)
}

fn keys_and_values<'s>(input: &mut Stream<'s>) -> Result<HashMap<&'s str, &'s str>> {
    repeat(0.., key_value).parse_next(input)
}

fn key_value<'s>(i: &mut Stream<'s>) -> Result<(&'s str, &'s str)> {
    let key = alphanumeric.parse_next(i)?;
    let _ = (opt(space), "=", opt(space)).parse_next(i)?;
    let val = take_till(0.., is_line_ending_or_comment).parse_next(i)?;
    let _ = opt(space).parse_next(i)?;
    let _ = opt((";", till_line_ending)).parse_next(i)?;
    let _ = opt(space_or_line_ending).parse_next(i)?;

    Ok((key, val))
}

fn is_line_ending_or_comment(chr: char) -> bool {
    chr == ';' || chr == '\n'
}

fn till_line_ending<'s>(i: &mut Stream<'s>) -> Result<&'s str> {
    take_while(0.., |c| c != '\r' && c != '\n').parse_next(i)
}

fn space_or_line_ending<'s>(i: &mut Stream<'s>) -> Result<&'s str> {
    take_while(1.., [' ', '\r', '\n']).parse_next(i)
}

#[test]
fn parse_category_test() {
    let ini_file = "[category]

parameter=value
key = value2";

    let ini_without_category = "parameter=value
key = value2";

    let res = category.parse_peek(ini_file);
    println!("{res:?}");
    match res {
        Ok((i, o)) => println!("i: {i} | o: {o:?}"),
        _ => println!("error"),
    }

    assert_eq!(res, Ok((ini_without_category, "category")));
}

#[test]
fn parse_key_value_test() {
    let ini_file = "parameter=value
key = value2";

    let ini_without_key_value = "key = value2";

    let res = key_value.parse_peek(ini_file);
    println!("{res:?}");
    match res {
        Ok((i, (o1, o2))) => println!("i: {i} | o: ({o1:?},{o2:?})"),
        _ => println!("error"),
    }

    assert_eq!(res, Ok((ini_without_key_value, ("parameter", "value"))));
}

#[test]
fn parse_key_value_with_space_test() {
    let ini_file = "parameter = value
key = value2";

    let ini_without_key_value = "key = value2";

    let res = key_value.parse_peek(ini_file);
    println!("{res:?}");
    match res {
        Ok((i, (o1, o2))) => println!("i: {i} | o: ({o1:?},{o2:?})"),
        _ => println!("error"),
    }

    assert_eq!(res, Ok((ini_without_key_value, ("parameter", "value"))));
}

#[test]
fn parse_key_value_with_comment_test() {
    let ini_file = "parameter=value;abc
key = value2";

    let ini_without_key_value = "key = value2";

    let res = key_value.parse_peek(ini_file);
    println!("{res:?}");
    match res {
        Ok((i, (o1, o2))) => println!("i: {i} | o: ({o1:?},{o2:?})"),
        _ => println!("error"),
    }

    assert_eq!(res, Ok((ini_without_key_value, ("parameter", "value"))));
}

#[test]
fn parse_multiple_keys_and_values_test() {
    let ini_file = "parameter=value;abc

key = value2

[category]";

    let ini_without_key_value = "[category]";

    let res = keys_and_values.parse_peek(ini_file);
    println!("{res:?}");
    match res {
        Ok((i, ref o)) => println!("i: {i} | o: {o:?}"),
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
    let ini_file = "[abcd]
parameter=value;abc

key = value2

[category]";

    let ini_after_parser = "[category]";

    let res = category_and_keys.parse_peek(ini_file);
    println!("{res:?}");
    match res {
        Ok((i, ref o)) => println!("i: {i} | o: {o:?}"),
        _ => println!("error"),
    }

    let mut expected_h: HashMap<&str, &str> = HashMap::new();
    expected_h.insert("parameter", "value");
    expected_h.insert("key", "value2");
    assert_eq!(res, Ok((ini_after_parser, ("abcd", expected_h))));
}

#[test]
fn parse_multiple_categories_test() {
    let ini_file = "[abcd]

parameter=value;abc

key = value2

[category]
parameter3=value3
key4 = value4
";

    let res = categories.parse_peek(ini_file);
    //println!("{:?}", res);
    match res {
        Ok((i, ref o)) => println!("i: {i} | o: {o:?}"),
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
    assert_eq!(res, Ok(("", expected_h)));
}

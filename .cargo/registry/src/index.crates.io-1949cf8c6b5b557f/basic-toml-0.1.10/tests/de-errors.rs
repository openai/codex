#![allow(
    clippy::elidable_lifetime_names,
    clippy::needless_lifetimes,
    clippy::too_many_lines
)]

use serde::{de, Deserialize};
use std::fmt;

macro_rules! bad {
    ($toml:expr, $ty:ty, $msg:expr) => {
        match basic_toml::from_str::<$ty>($toml) {
            Ok(s) => panic!("parsed to: {:#?}", s),
            Err(e) => assert_eq!(e.to_string(), $msg),
        }
    };
}

#[derive(Debug, Deserialize, PartialEq)]
struct Parent<T> {
    p_a: T,
    p_b: Vec<Child<T>>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Child<T> {
    c_a: T,
    c_b: T,
}

#[derive(Debug, PartialEq)]
enum CasedString {
    Lowercase(String),
    Uppercase(String),
}

impl<'de> de::Deserialize<'de> for CasedString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct CasedStringVisitor;

        impl<'de> de::Visitor<'de> for CasedStringVisitor {
            type Value = CasedString;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if s.is_empty() {
                    Err(de::Error::invalid_length(0, &"a non-empty string"))
                } else if s.chars().all(|x| x.is_ascii_lowercase()) {
                    Ok(CasedString::Lowercase(s.to_string()))
                } else if s.chars().all(|x| x.is_ascii_uppercase()) {
                    Ok(CasedString::Uppercase(s.to_string()))
                } else {
                    Err(de::Error::invalid_value(
                        de::Unexpected::Str(s),
                        &"all lowercase or all uppercase",
                    ))
                }
            }
        }

        deserializer.deserialize_any(CasedStringVisitor)
    }
}

#[test]
fn custom_errors() {
    basic_toml::from_str::<Parent<CasedString>>(
        "
            p_a = 'a'
            p_b = [{c_a = 'a', c_b = 'c'}]
        ",
    )
    .unwrap();

    // Custom error at p_b value.
    bad!(
        "
            p_a = ''
                # ^
        ",
        Parent<CasedString>,
        "invalid length 0, expected a non-empty string for key `p_a` at line 2 column 19"
    );

    // Missing field in table.
    bad!(
        "
            p_a = 'a'
          # ^
        ",
        Parent<CasedString>,
        "missing field `p_b` at line 1 column 1"
    );

    // Invalid type in p_b.
    bad!(
        "
            p_a = 'a'
            p_b = 1
                # ^
        ",
        Parent<CasedString>,
        "invalid type: integer `1`, expected a sequence for key `p_b` at line 3 column 19"
    );

    // Sub-table in Vec is missing a field.
    bad!(
        "
            p_a = 'a'
            p_b = [
                {c_a = 'a'}
              # ^
            ]
        ",
        Parent<CasedString>,
        "missing field `c_b` for key `p_b` at line 4 column 17"
    );

    // Sub-table in Vec has a field with a bad value.
    bad!(
        "
            p_a = 'a'
            p_b = [
                {c_a = 'a', c_b = '*'}
                                # ^
            ]
        ",
        Parent<CasedString>,
        "invalid value: string \"*\", expected all lowercase or all uppercase for key `p_b` at line 4 column 35"
    );

    // Sub-table in Vec is missing a field.
    bad!(
        "
            p_a = 'a'
            p_b = [
                {c_a = 'a', c_b = 'b'},
                {c_a = 'aa'}
              # ^
            ]
        ",
        Parent<CasedString>,
        "missing field `c_b` for key `p_b` at line 5 column 17"
    );

    // Sub-table in the middle of a Vec is missing a field.
    bad!(
        "
            p_a = 'a'
            p_b = [
                {c_a = 'a', c_b = 'b'},
                {c_a = 'aa'},
              # ^
                {c_a = 'aaa', c_b = 'bbb'},
            ]
        ",
        Parent<CasedString>,
        "missing field `c_b` for key `p_b` at line 5 column 17"
    );

    // Sub-table in the middle of a Vec has a field with a bad value.
    bad!(
        "
            p_a = 'a'
            p_b = [
                {c_a = 'a', c_b = 'b'},
                {c_a = 'aa', c_b = 1},
                                 # ^
                {c_a = 'aaa', c_b = 'bbb'},
            ]
        ",
        Parent<CasedString>,
        "invalid type: integer `1`, expected a string for key `p_b` at line 5 column 36"
    );

    // Sub-table in the middle of a Vec has an extra field.
    // FIXME: This location could be better.
    bad!(
        "
            p_a = 'a'
            p_b = [
                {c_a = 'a', c_b = 'b'},
                {c_a = 'aa', c_b = 'bb', c_d = 'd'},
              # ^
                {c_a = 'aaa', c_b = 'bbb'},
                {c_a = 'aaaa', c_b = 'bbbb'},
            ]
        ",
        Parent<CasedString>,
        "unknown field `c_d`, expected `c_a` or `c_b` for key `p_b` at line 5 column 17"
    );

    // Sub-table in the middle of a Vec is missing a field.
    // FIXME: This location is pretty off.
    bad!(
        "
            p_a = 'a'
            [[p_b]]
            c_a = 'a'
            c_b = 'b'
            [[p_b]]
            c_a = 'aa'
            # c_b = 'bb' # <- missing field
            [[p_b]]
            c_a = 'aaa'
            c_b = 'bbb'
            [[p_b]]
          # ^
            c_a = 'aaaa'
            c_b = 'bbbb'
        ",
        Parent<CasedString>,
        "missing field `c_b` for key `p_b` at line 12 column 13"
    );

    // Sub-table in the middle of a Vec has a field with a bad value.
    bad!(
        "
            p_a = 'a'
            [[p_b]]
            c_a = 'a'
            c_b = 'b'
            [[p_b]]
            c_a = 'aa'
            c_b = '*'
                # ^
            [[p_b]]
            c_a = 'aaa'
            c_b = 'bbb'
        ",
        Parent<CasedString>,
        "invalid value: string \"*\", expected all lowercase or all uppercase for key `p_b.c_b` at line 8 column 19"
    );

    // Sub-table in the middle of a Vec has an extra field.
    // FIXME: This location is pretty off.
    bad!(
        "
            p_a = 'a'
            [[p_b]]
            c_a = 'a'
            c_b = 'b'
            [[p_b]]
            c_a = 'aa'
            c_d = 'dd' # unknown field
            [[p_b]]
            c_a = 'aaa'
            c_b = 'bbb'
            [[p_b]]
          # ^
            c_a = 'aaaa'
            c_b = 'bbbb'
        ",
        Parent<CasedString>,
        "unknown field `c_d`, expected `c_a` or `c_b` for key `p_b` at line 12 column 13"
    );
}

#[test]
fn serde_derive_deserialize_errors() {
    bad!(
        "
            p_a = ''
          # ^
        ",
        Parent<String>,
        "missing field `p_b` at line 1 column 1"
    );

    bad!(
        "
            p_a = ''
            p_b = [
                {c_a = ''}
              # ^
            ]
        ",
        Parent<String>,
        "missing field `c_b` for key `p_b` at line 4 column 17"
    );

    bad!(
        "
            p_a = ''
            p_b = [
                {c_a = '', c_b = 1}
                               # ^
            ]
        ",
        Parent<String>,
        "invalid type: integer `1`, expected a string for key `p_b` at line 4 column 34"
    );

    // FIXME: This location could be better.
    bad!(
        "
            p_a = ''
            p_b = [
                {c_a = '', c_b = '', c_d = ''},
              # ^
            ]
        ",
        Parent<String>,
        "unknown field `c_d`, expected `c_a` or `c_b` for key `p_b` at line 4 column 17"
    );

    bad!(
        "
            p_a = 'a'
            p_b = [
                {c_a = '', c_b = 1, c_d = ''},
                               # ^
            ]
        ",
        Parent<String>,
        "invalid type: integer `1`, expected a string for key `p_b` at line 4 column 34"
    );
}

#[test]
fn error_handles_crlf() {
    bad!(
        "\r\n\
         [t1]\r\n\
         [t2]\r\n\
         a = 1\r\n\
         . = 2\r\n\
         ",
        serde_json::Value,
        "expected a table key, found a period at line 5 column 1"
    );

    // Should be the same as above.
    bad!(
        "\n\
         [t1]\n\
         [t2]\n\
         a = 1\n\
         . = 2\n\
         ",
        serde_json::Value,
        "expected a table key, found a period at line 5 column 1"
    );
}

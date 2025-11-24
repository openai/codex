#![allow(
    clippy::assertions_on_result_states,
    clippy::byte_char_slices,
    clippy::cast_precision_loss,
    clippy::derive_partial_eq_without_eq,
    clippy::excessive_precision,
    clippy::float_cmp,
    clippy::incompatible_msrv, // https://github.com/rust-lang/rust-clippy/issues/12257
    clippy::items_after_statements,
    clippy::large_digit_groups,
    clippy::let_underscore_untyped,
    clippy::shadow_unrelated,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    clippy::unreadable_literal,
    clippy::unseparated_literal_suffix,
    clippy::vec_init_then_push,
    clippy::zero_sized_map_values
)]

#[macro_use]
mod macros;

#[cfg(feature = "raw_value")]
use ref_cast::RefCast;
use serde::de::{self, IgnoredAny, IntoDeserializer};
use serde::ser::{self, SerializeMap, SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};
use serde_bytes::{ByteBuf, Bytes};
#[cfg(feature = "raw_value")]
use serde_json::value::RawValue;
use serde_json::{
    from_reader, from_slice, from_str, from_value, json, to_string, to_string_pretty, to_value,
    to_vec, Deserializer, Number, Value,
};
use std::collections::BTreeMap;
#[cfg(feature = "raw_value")]
use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::hash::BuildHasher;
#[cfg(feature = "raw_value")]
use std::hash::{Hash, Hasher};
use std::io;
use std::iter;
use std::marker::PhantomData;
use std::mem;
use std::str::FromStr;
use std::{f32, f64};

macro_rules! treemap {
    () => {
        BTreeMap::new()
    };
    ($($k:expr => $v:expr),+ $(,)?) => {
        {
            let mut m = BTreeMap::new();
            $(
                m.insert($k, $v);
            )+
            m
        }
    };
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum Animal {
    Dog,
    Frog(String, Vec<isize>),
    Cat { age: usize, name: String },
    AntHive(Vec<String>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Inner {
    a: (),
    b: usize,
    c: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Outer {
    inner: Vec<Inner>,
}

fn test_encode_ok<T>(errors: &[(T, &str)])
where
    T: PartialEq + Debug + ser::Serialize,
{
    for &(ref value, out) in errors {
        let out = out.to_string();

        let s = to_string(value).unwrap();
        assert_eq!(s, out);

        let v = to_value(value).unwrap();
        let s = to_string(&v).unwrap();
        assert_eq!(s, out);
    }
}

fn test_pretty_encode_ok<T>(errors: &[(T, &str)])
where
    T: PartialEq + Debug + ser::Serialize,
{
    for &(ref value, out) in errors {
        let out = out.to_string();

        let s = to_string_pretty(value).unwrap();
        assert_eq!(s, out);

        let v = to_value(value).unwrap();
        let s = to_string_pretty(&v).unwrap();
        assert_eq!(s, out);
    }
}

#[test]
fn test_write_null() {
    let tests = &[((), "null")];
    test_encode_ok(tests);
    test_pretty_encode_ok(tests);
}

#[test]
fn test_write_u64() {
    let tests = &[(3u64, "3"), (u64::MAX, &u64::MAX.to_string())];
    test_encode_ok(tests);
    test_pretty_encode_ok(tests);
}

#[test]
fn test_write_i64() {
    let tests = &[
        (3i64, "3"),
        (-2i64, "-2"),
        (-1234i64, "-1234"),
        (i64::MIN, &i64::MIN.to_string()),
    ];
    test_encode_ok(tests);
    test_pretty_encode_ok(tests);
}

#[test]
fn test_write_f64() {
    let tests = &[
        (3.0, "3.0"),
        (3.1, "3.1"),
        (-1.5, "-1.5"),
        (0.5, "0.5"),
        (f64::MIN, "-1.7976931348623157e308"),
        (f64::MAX, "1.7976931348623157e308"),
        (f64::EPSILON, "2.220446049250313e-16"),
    ];
    test_encode_ok(tests);
    test_pretty_encode_ok(tests);
}

#[test]
fn test_encode_nonfinite_float_yields_null() {
    let v = to_value(f64::NAN.copysign(1.0)).unwrap();
    assert!(v.is_null());

    let v = to_value(f64::NAN.copysign(-1.0)).unwrap();
    assert!(v.is_null());

    let v = to_value(f64::INFINITY).unwrap();
    assert!(v.is_null());

    let v = to_value(-f64::INFINITY).unwrap();
    assert!(v.is_null());

    let v = to_value(f32::NAN.copysign(1.0)).unwrap();
    assert!(v.is_null());

    let v = to_value(f32::NAN.copysign(-1.0)).unwrap();
    assert!(v.is_null());

    let v = to_value(f32::INFINITY).unwrap();
    assert!(v.is_null());

    let v = to_value(-f32::INFINITY).unwrap();
    assert!(v.is_null());
}

#[test]
fn test_write_str() {
    let tests = &[("", "\"\""), ("foo", "\"foo\"")];
    test_encode_ok(tests);
    test_pretty_encode_ok(tests);
}

#[test]
fn test_write_bool() {
    let tests = &[(true, "true"), (false, "false")];
    test_encode_ok(tests);
    test_pretty_encode_ok(tests);
}

#[test]
fn test_write_char() {
    let tests = &[
        ('n', "\"n\""),
        ('"', "\"\\\"\""),
        ('\\', "\"\\\\\""),
        ('/', "\"/\""),
        ('\x08', "\"\\b\""),
        ('\x0C', "\"\\f\""),
        ('\n', "\"\\n\""),
        ('\r', "\"\\r\""),
        ('\t', "\"\\t\""),
        ('\x0B', "\"\\u000b\""),
        ('\u{3A3}', "\"\u{3A3}\""),
    ];
    test_encode_ok(tests);
    test_pretty_encode_ok(tests);
}

#[test]
fn test_write_list() {
    test_encode_ok(&[
        (vec![], "[]"),
        (vec![true], "[true]"),
        (vec![true, false], "[true,false]"),
    ]);

    test_encode_ok(&[
        (vec![vec![], vec![], vec![]], "[[],[],[]]"),
        (vec![vec![1, 2, 3], vec![], vec![]], "[[1,2,3],[],[]]"),
        (vec![vec![], vec![1, 2, 3], vec![]], "[[],[1,2,3],[]]"),
        (vec![vec![], vec![], vec![1, 2, 3]], "[[],[],[1,2,3]]"),
    ]);

    test_pretty_encode_ok(&[
        (vec![vec![], vec![], vec![]], pretty_str!([[], [], []])),
        (
            vec![vec![1, 2, 3], vec![], vec![]],
            pretty_str!([[1, 2, 3], [], []]),
        ),
        (
            vec![vec![], vec![1, 2, 3], vec![]],
            pretty_str!([[], [1, 2, 3], []]),
        ),
        (
            vec![vec![], vec![], vec![1, 2, 3]],
            pretty_str!([[], [], [1, 2, 3]]),
        ),
    ]);

    test_pretty_encode_ok(&[
        (vec![], "[]"),
        (vec![true], pretty_str!([true])),
        (vec![true, false], pretty_str!([true, false])),
    ]);

    let long_test_list = json!([false, null, ["foo\nbar", 3.5]]);

    test_encode_ok(&[(
        long_test_list.clone(),
        json_str!([false, null, ["foo\nbar", 3.5]]),
    )]);

    test_pretty_encode_ok(&[(
        long_test_list,
        pretty_str!([false, null, ["foo\nbar", 3.5]]),
    )]);
}

#[test]
fn test_write_object() {
    test_encode_ok(&[
        (treemap!(), "{}"),
        (treemap!("a".to_owned() => true), "{\"a\":true}"),
        (
            treemap!(
                "a".to_owned() => true,
                "b".to_owned() => false,
            ),
            "{\"a\":true,\"b\":false}",
        ),
    ]);

    test_encode_ok(&[
        (
            treemap![
                "a".to_owned() => treemap![],
                "b".to_owned() => treemap![],
                "c".to_owned() => treemap![],
            ],
            "{\"a\":{},\"b\":{},\"c\":{}}",
        ),
        (
            treemap![
                "a".to_owned() => treemap![
                    "a".to_owned() => treemap!["a" => vec![1,2,3]],
                    "b".to_owned() => treemap![],
                    "c".to_owned() => treemap![],
                ],
                "b".to_owned() => treemap![],
                "c".to_owned() => treemap![],
            ],
            "{\"a\":{\"a\":{\"a\":[1,2,3]},\"b\":{},\"c\":{}},\"b\":{},\"c\":{}}",
        ),
        (
            treemap![
                "a".to_owned() => treemap![],
                "b".to_owned() => treemap![
                    "a".to_owned() => treemap!["a" => vec![1,2,3]],
                    "b".to_owned() => treemap![],
                    "c".to_owned() => treemap![],
                ],
                "c".to_owned() => treemap![],
            ],
            "{\"a\":{},\"b\":{\"a\":{\"a\":[1,2,3]},\"b\":{},\"c\":{}},\"c\":{}}",
        ),
        (
            treemap![
                "a".to_owned() => treemap![],
                "b".to_owned() => treemap![],
                "c".to_owned() => treemap![
                    "a".to_owned() => treemap!["a" => vec![1,2,3]],
                    "b".to_owned() => treemap![],
                    "c".to_owned() => treemap![],
                ],
            ],
            "{\"a\":{},\"b\":{},\"c\":{\"a\":{\"a\":[1,2,3]},\"b\":{},\"c\":{}}}",
        ),
    ]);

    test_encode_ok(&[(treemap!['c' => ()], "{\"c\":null}")]);

    test_pretty_encode_ok(&[
        (
            treemap![
                "a".to_owned() => treemap![],
                "b".to_owned() => treemap![],
                "c".to_owned() => treemap![],
            ],
            pretty_str!({
                "a": {},
                "b": {},
                "c": {}
            }),
        ),
        (
            treemap![
                "a".to_owned() => treemap![
                    "a".to_owned() => treemap!["a" => vec![1,2,3]],
                    "b".to_owned() => treemap![],
                    "c".to_owned() => treemap![],
                ],
                "b".to_owned() => treemap![],
                "c".to_owned() => treemap![],
            ],
            pretty_str!({
                "a": {
                    "a": {
                        "a": [
                            1,
                            2,
                            3
                        ]
                    },
                    "b": {},
                    "c": {}
                },
                "b": {},
                "c": {}
            }),
        ),
        (
            treemap![
                "a".to_owned() => treemap![],
                "b".to_owned() => treemap![
                    "a".to_owned() => treemap!["a" => vec![1,2,3]],
                    "b".to_owned() => treemap![],
                    "c".to_owned() => treemap![],
                ],
                "c".to_owned() => treemap![],
            ],
            pretty_str!({
                "a": {},
                "b": {
                    "a": {
                        "a": [
                            1,
                            2,
                            3
                        ]
                    },
                    "b": {},
                    "c": {}
                },
                "c": {}
            }),
        ),
        (
            treemap![
                "a".to_owned() => treemap![],
                "b".to_owned() => treemap![],
                "c".to_owned() => treemap![
                    "a".to_owned() => treemap!["a" => vec![1,2,3]],
                    "b".to_owned() => treemap![],
                    "c".to_owned() => treemap![],
                ],
            ],
            pretty_str!({
                "a": {},
                "b": {},
                "c": {
                    "a": {
                        "a": [
                            1,
                            2,
                            3
                        ]
                    },
                    "b": {},
                    "c": {}
                }
            }),
        ),
    ]);

    test_pretty_encode_ok(&[
        (treemap!(), "{}"),
        (
            treemap!("a".to_owned() => true),
            pretty_str!({
                "a": true
            }),
        ),
        (
            treemap!(
                "a".to_owned() => true,
                "b".to_owned() => false,
            ),
            pretty_str!( {
                "a": true,
                "b": false
            }),
        ),
    ]);

    let complex_obj = json!({
        "b": [
            {"c": "\x0c\x1f\r"},
            {"d": ""}
        ]
    });

    test_encode_ok(&[(
        complex_obj.clone(),
        json_str!({
            "b": [
                {
                    "c": (r#""\f\u001f\r""#)
                },
                {
                    "d": ""
                }
            ]
        }),
    )]);

    test_pretty_encode_ok(&[(
        complex_obj,
        pretty_str!({
            "b": [
                {
                    "c": (r#""\f\u001f\r""#)
                },
                {
                    "d": ""
                }
            ]
        }),
    )]);
}

#[test]
fn test_write_tuple() {
    test_encode_ok(&[((5,), "[5]")]);

    test_pretty_encode_ok(&[((5,), pretty_str!([5]))]);

    test_encode_ok(&[((5, (6, "abc")), "[5,[6,\"abc\"]]")]);

    test_pretty_encode_ok(&[((5, (6, "abc")), pretty_str!([5, [6, "abc"]]))]);
}

#[test]
fn test_write_enum() {
    test_encode_ok(&[
        (Animal::Dog, "\"Dog\""),
        (
            Animal::Frog("Henry".to_owned(), vec![]),
            "{\"Frog\":[\"Henry\",[]]}",
        ),
        (
            Animal::Frog("Henry".to_owned(), vec![349]),
            "{\"Frog\":[\"Henry\",[349]]}",
        ),
        (
            Animal::Frog("Henry".to_owned(), vec![349, 102]),
            "{\"Frog\":[\"Henry\",[349,102]]}",
        ),
        (
            Animal::Cat {
                age: 5,
                name: "Kate".to_owned(),
            },
            "{\"Cat\":{\"age\":5,\"name\":\"Kate\"}}",
        ),
        (
            Animal::AntHive(vec!["Bob".to_owned(), "Stuart".to_owned()]),
            "{\"AntHive\":[\"Bob\",\"Stuart\"]}",
        ),
    ]);

    test_pretty_encode_ok(&[
        (Animal::Dog, "\"Dog\""),
        (
            Animal::Frog("Henry".to_owned(), vec![]),
            pretty_str!({
                "Frog": [
                    "Henry",
                    []
                ]
            }),
        ),
        (
            Animal::Frog("Henry".to_owned(), vec![349]),
            pretty_str!({
                "Frog": [
                    "Henry",
                    [
                        349
                    ]
                ]
            }),
        ),
        (
            Animal::Frog("Henry".to_owned(), vec![349, 102]),
            pretty_str!({
                "Frog": [
                    "Henry",
                    [
                      349,
                      102
                    ]
                ]
            }),
        ),
    ]);
}

#[test]
fn test_write_option() {
    test_encode_ok(&[(None, "null"), (Some("jodhpurs"), "\"jodhpurs\"")]);

    test_encode_ok(&[
        (None, "null"),
        (Some(vec!["foo", "bar"]), "[\"foo\",\"bar\"]"),
    ]);

    test_pretty_encode_ok(&[(None, "null"), (Some("jodhpurs"), "\"jodhpurs\"")]);

    test_pretty_encode_ok(&[
        (None, "null"),
        (Some(vec!["foo", "bar"]), pretty_str!(["foo", "bar"])),
    ]);
}

#[test]
fn test_write_newtype_struct() {
    #[derive(Serialize, PartialEq, Debug)]
    struct Newtype(BTreeMap<String, i32>);

    let inner = Newtype(treemap!(String::from("inner") => 123));
    let outer = treemap!(String::from("outer") => to_value(&inner).unwrap());

    test_encode_ok(&[(inner, r#"{"inner":123}"#)]);

    test_encode_ok(&[(outer, r#"{"outer":{"inner":123}}"#)]);
}

#[test]
fn test_deserialize_number_to_untagged_enum() {
    #[derive(Eq, PartialEq, Deserialize, Debug)]
    #[serde(untagged)]
    enum E {
        N(i64),
    }

    assert_eq!(E::N(0), E::deserialize(Number::from(0)).unwrap());
}

fn test_parse_ok<T>(tests: Vec<(&str, T)>)
where
    T: Clone + Debug + PartialEq + ser::Serialize + de::DeserializeOwned,
{
    for (s, value) in tests {
        let v: T = from_str(s).unwrap();
        assert_eq!(v, value.clone());

        let v: T = from_slice(s.as_bytes()).unwrap();
        assert_eq!(v, value.clone());

        // Make sure we can deserialize into a `Value`.
        let json_value: Value = from_str(s).unwrap();
        assert_eq!(json_value, to_value(&value).unwrap());

        // Make sure we can deserialize from a `&Value`.
        let v = T::deserialize(&json_value).unwrap();
        assert_eq!(v, value);

        // Make sure we can deserialize from a `Value`.
        let v: T = from_value(json_value.clone()).unwrap();
        assert_eq!(v, value);

        // Make sure we can round trip back to `Value`.
        let json_value2: Value = from_value(json_value.clone()).unwrap();
        assert_eq!(json_value2, json_value);

        // Make sure we can fully ignore.
        let twoline = s.to_owned() + "\n3735928559";
        let mut de = Deserializer::from_str(&twoline);
        IgnoredAny::deserialize(&mut de).unwrap();
        assert_eq!(0xDEAD_BEEF, u64::deserialize(&mut de).unwrap());

        // Make sure every prefix is an EOF error, except that a prefix of a
        // number may be a valid number.
        if !json_value.is_number() {
            for (i, _) in s.trim_end().char_indices() {
                assert!(from_str::<Value>(&s[..i]).unwrap_err().is_eof());
                assert!(from_str::<IgnoredAny>(&s[..i]).unwrap_err().is_eof());
            }
        }
    }
}

// For testing representations that the deserializer accepts but the serializer
// never generates. These do not survive a round-trip through Value.
fn test_parse_unusual_ok<T>(tests: Vec<(&str, T)>)
where
    T: Clone + Debug + PartialEq + ser::Serialize + de::DeserializeOwned,
{
    for (s, value) in tests {
        let v: T = from_str(s).unwrap();
        assert_eq!(v, value.clone());

        let v: T = from_slice(s.as_bytes()).unwrap();
        assert_eq!(v, value.clone());
    }
}

macro_rules! test_parse_err {
    ($name:ident::<$($ty:ty),*>($arg:expr) => $expected:expr) => {
        let actual = $name::<$($ty),*>($arg).unwrap_err().to_string();
        assert_eq!(actual, $expected, "unexpected {} error", stringify!($name));
    };
}

fn test_parse_err<T>(errors: &[(&str, &'static str)])
where
    T: Debug + PartialEq + de::DeserializeOwned,
{
    for &(s, err) in errors {
        test_parse_err!(from_str::<T>(s) => err);
        test_parse_err!(from_slice::<T>(s.as_bytes()) => err);
    }
}

fn test_parse_slice_err<T>(errors: &[(&[u8], &'static str)])
where
    T: Debug + PartialEq + de::DeserializeOwned,
{
    for &(s, err) in errors {
        test_parse_err!(from_slice::<T>(s) => err);
    }
}

fn test_fromstr_parse_err<T>(errors: &[(&str, &'static str)])
where
    T: Debug + PartialEq + FromStr,
    <T as FromStr>::Err: ToString,
{
    for &(s, err) in errors {
        let actual = s.parse::<T>().unwrap_err().to_string();
        assert_eq!(actual, err, "unexpected parsing error");
    }
}

#[test]
fn test_parse_null() {
    test_parse_err::<()>(&[
        ("n", "EOF while parsing a value at line 1 column 1"),
        ("nul", "EOF while parsing a value at line 1 column 3"),
        ("nulla", "trailing characters at line 1 column 5"),
    ]);

    test_parse_ok(vec![("null", ())]);
}

#[test]
fn test_parse_bool() {
    test_parse_err::<bool>(&[
        ("t", "EOF while parsing a value at line 1 column 1"),
        ("truz", "expected ident at line 1 column 4"),
        ("f", "EOF while parsing a value at line 1 column 1"),
        ("faz", "expected ident at line 1 column 3"),
        ("truea", "trailing characters at line 1 column 5"),
        ("falsea", "trailing characters at line 1 column 6"),
    ]);

    test_parse_ok(vec![
        ("true", true),
        (" true ", true),
        ("false", false),
        (" false ", false),
    ]);
}

#[test]
fn test_parse_char() {
    test_parse_err::<char>(&[
        (
            "\"ab\"",
            "invalid value: string \"ab\", expected a character at line 1 column 4",
        ),
        (
            "10",
            "invalid type: integer `10`, expected a character at line 1 column 2",
        ),
    ]);

    test_parse_ok(vec![
        ("\"n\"", 'n'),
        ("\"\\\"\"", '"'),
        ("\"\\\\\"", '\\'),
        ("\"/\"", '/'),
        ("\"\\b\"", '\x08'),
        ("\"\\f\"", '\x0C'),
        ("\"\\n\"", '\n'),
        ("\"\\r\"", '\r'),
        ("\"\\t\"", '\t'),
        ("\"\\u000b\"", '\x0B'),
        ("\"\\u000B\"", '\x0B'),
        ("\"\u{3A3}\"", '\u{3A3}'),
    ]);
}

#[test]
fn test_parse_number_errors() {
    test_parse_err::<f64>(&[
        ("+", "expected value at line 1 column 1"),
        (".", "expected value at line 1 column 1"),
        ("-", "EOF while parsing a value at line 1 column 1"),
        ("00", "invalid number at line 1 column 2"),
        ("0x80", "trailing characters at line 1 column 2"),
        ("\\0", "expected value at line 1 column 1"),
        (".0", "expected value at line 1 column 1"),
        ("0.", "EOF while parsing a value at line 1 column 2"),
        ("1.", "EOF while parsing a value at line 1 column 2"),
        ("1.a", "invalid number at line 1 column 3"),
        ("1.e1", "invalid number at line 1 column 3"),
        ("1e", "EOF while parsing a value at line 1 column 2"),
        ("1e+", "EOF while parsing a value at line 1 column 3"),
        ("1a", "trailing characters at line 1 column 2"),
        (
            "100e777777777777777777777777777",
            "number out of range at line 1 column 14",
        ),
        (
            "-100e777777777777777777777777777",
            "number out of range at line 1 column 15",
        ),
        (
            "1000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000", // 1e309
            "number out of range at line 1 column 310",
        ),
        (
            "1000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             .0e9", // 1e309
            "number out of range at line 1 column 305",
        ),
        (
            "1000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             e9", // 1e309
            "number out of range at line 1 column 303",
        ),
    ]);
}

#[test]
fn test_parse_i64() {
    test_parse_ok(vec![
        ("-2", -2),
        ("-1234", -1234),
        (" -1234 ", -1234),
        (&i64::MIN.to_string(), i64::MIN),
        (&i64::MAX.to_string(), i64::MAX),
    ]);
}

#[test]
fn test_parse_u64() {
    test_parse_ok(vec![
        ("0", 0u64),
        ("3", 3u64),
        ("1234", 1234),
        (&u64::MAX.to_string(), u64::MAX),
    ]);
}

#[test]
fn test_parse_negative_zero() {
    for negative_zero in &[
        "-0",
        "-0.0",
        "-0e2",
        "-0.0e2",
        "-1e-400",
        "-1e-4000000000000000000000000000000000000000000000000",
    ] {
        assert!(
            from_str::<f32>(negative_zero).unwrap().is_sign_negative(),
            "should have been negative: {:?}",
            negative_zero,
        );
        assert!(
            from_str::<f64>(negative_zero).unwrap().is_sign_negative(),
            "should have been negative: {:?}",
            negative_zero,
        );
    }
}

#[test]
fn test_parse_f64() {
    test_parse_ok(vec![
        ("0.0", 0.0f64),
        ("3.0", 3.0f64),
        ("3.1", 3.1),
        ("-1.2", -1.2),
        ("0.4", 0.4),
        // Edge case from:
        // https://github.com/serde-rs/json/issues/536#issuecomment-583714900
        ("2.638344616030823e-256", 2.638344616030823e-256),
    ]);

    #[cfg(not(feature = "arbitrary_precision"))]
    test_parse_ok(vec![
        // With arbitrary-precision enabled, this parses as Number{"3.00"}
        // but the float is Number{"3.0"}
        ("3.00", 3.0f64),
        ("0.4e5", 0.4e5),
        ("0.4e+5", 0.4e5),
        ("0.4e15", 0.4e15),
        ("0.4e+15", 0.4e15),
        ("0.4e-01", 0.4e-1),
        (" 0.4e-01 ", 0.4e-1),
        ("0.4e-001", 0.4e-1),
        ("0.4e-0", 0.4e0),
        ("0.00e00", 0.0),
        ("0.00e+00", 0.0),
        ("0.00e-00", 0.0),
        ("3.5E-2147483647", 0.0),
        ("0.0100000000000000000001", 0.01),
        (
            &format!("{}", (i64::MIN as f64) - 1.0),
            (i64::MIN as f64) - 1.0,
        ),
        (
            &format!("{}", (u64::MAX as f64) + 1.0),
            (u64::MAX as f64) + 1.0,
        ),
        (&format!("{}", f64::EPSILON), f64::EPSILON),
        (
            "0.0000000000000000000000000000000000000000000000000123e50",
            1.23,
        ),
        ("100e-777777777777777777777777777", 0.0),
        (
            "1010101010101010101010101010101010101010",
            10101010101010101010e20,
        ),
        (
            "0.1010101010101010101010101010101010101010",
            0.1010101010101010101,
        ),
        ("0e1000000000000000000000000000000000000000000000", 0.0),
        (
            "1000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             00000000",
            1e308,
        ),
        (
            "1000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             .0e8",
            1e308,
        ),
        (
            "1000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             e8",
            1e308,
        ),
        (
            "1000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000000000000000000000000000000000000000000000\
             000000000000000000e-10",
            1e308,
        ),
    ]);
}

#[test]
fn test_value_as_f64() {
    let v = serde_json::from_str::<Value>("1e1000");

    #[cfg(not(feature = "arbitrary_precision"))]
    assert!(v.is_err());

    #[cfg(feature = "arbitrary_precision")]
    assert_eq!(v.unwrap().as_f64(), None);
}

// Test roundtrip with some values that were not perfectly roundtripped by the
// old f64 deserializer.
#[cfg(feature = "float_roundtrip")]
#[test]
fn test_roundtrip_f64() {
    for &float in &[
        // Samples from quickcheck-ing roundtrip with `input: f64`. Comments
        // indicate the value returned by the old deserializer.
        51.24817837550540_4,  // 51.2481783755054_1
        -93.3113703768803_3,  // -93.3113703768803_2
        -36.5739948427534_36, // -36.5739948427534_4
        52.31400820410624_4,  // 52.31400820410624_
        97.4536532003468_5,   // 97.4536532003468_4
        // Samples from `rng.next_u64` + `f64::from_bits` + `is_finite` filter.
        2.0030397744267762e-253,
        7.101215824554616e260,
        1.769268377902049e74,
        -1.6727517818542075e58,
        3.9287532173373315e299,
    ] {
        let json = serde_json::to_string(&float).unwrap();
        let output: f64 = serde_json::from_str(&json).unwrap();
        assert_eq!(float, output);
    }
}

#[test]
fn test_roundtrip_f32() {
    // This number has 1 ULP error if parsed via f64 and converted to f32.
    // https://github.com/serde-rs/json/pull/671#issuecomment-628534468
    let float = 7.038531e-26;
    let json = serde_json::to_string(&float).unwrap();
    let output: f32 = serde_json::from_str(&json).unwrap();
    assert_eq!(float, output);
}

#[test]
fn test_serialize_char() {
    let value = json!(
        ({
            let mut map = BTreeMap::new();
            map.insert('c', ());
            map
        })
    );
    assert_eq!(&Value::Null, value.get("c").unwrap());
}

#[cfg(feature = "arbitrary_precision")]
#[test]
fn test_malicious_number() {
    #[derive(Serialize)]
    #[serde(rename = "$serde_json::private::Number")]
    struct S {
        #[serde(rename = "$serde_json::private::Number")]
        f: &'static str,
    }

    let actual = serde_json::to_value(&S { f: "not a number" })
        .unwrap_err()
        .to_string();
    assert_eq!(actual, "invalid number at line 1 column 1");
}

#[test]
fn test_parse_number() {
    test_parse_ok(vec![
        ("0.0", Number::from_f64(0.0f64).unwrap()),
        ("3.0", Number::from_f64(3.0f64).unwrap()),
        ("3.1", Number::from_f64(3.1).unwrap()),
        ("-1.2", Number::from_f64(-1.2).unwrap()),
        ("0.4", Number::from_f64(0.4).unwrap()),
    ]);

    test_fromstr_parse_err::<Number>(&[
        (" 1.0", "invalid number at line 1 column 1"),
        ("1.0 ", "invalid number at line 1 column 4"),
        ("\t1.0", "invalid number at line 1 column 1"),
        ("1.0\t", "invalid number at line 1 column 4"),
    ]);

    #[cfg(feature = "arbitrary_precision")]
    test_parse_ok(vec![
        ("1e999", Number::from_string_unchecked("1e999".to_owned())),
        ("1e+999", Number::from_string_unchecked("1e+999".to_owned())),
        ("-1e999", Number::from_string_unchecked("-1e999".to_owned())),
        ("1e-999", Number::from_string_unchecked("1e-999".to_owned())),
        ("1E999", Number::from_string_unchecked("1E999".to_owned())),
        ("1E+999", Number::from_string_unchecked("1E+999".to_owned())),
        ("-1E999", Number::from_string_unchecked("-1E999".to_owned())),
        ("1E-999", Number::from_string_unchecked("1E-999".to_owned())),
        ("1E+000", Number::from_string_unchecked("1E+000".to_owned())),
        (
            "2.3e999",
            Number::from_string_unchecked("2.3e999".to_owned()),
        ),
        (
            "-2.3e999",
            Number::from_string_unchecked("-2.3e999".to_owned()),
        ),
    ]);
}

#[test]
fn test_parse_string() {
    test_parse_err::<String>(&[
        ("\"", "EOF while parsing a string at line 1 column 1"),
        ("\"lol", "EOF while parsing a string at line 1 column 4"),
        ("\"lol\"a", "trailing characters at line 1 column 6"),
        (
            "\"\\uD83C\\uFFFF\"",
            "lone leading surrogate in hex escape at line 1 column 13",
        ),
        (
            "\"\n\"",
            "control character (\\u0000-\\u001F) found while parsing a string at line 2 column 0",
        ),
        (
            "\"\x1F\"",
            "control character (\\u0000-\\u001F) found while parsing a string at line 1 column 2",
        ),
    ]);

    test_parse_slice_err::<String>(&[
        (
            &[b'"', 159, 146, 150, b'"'],
            "invalid unicode code point at line 1 column 5",
        ),
        (
            &[b'"', b'\\', b'n', 159, 146, 150, b'"'],
            "invalid unicode code point at line 1 column 7",
        ),
        (
            &[b'"', b'\\', b'u', 48, 48, 51],
            "EOF while parsing a string at line 1 column 6",
        ),
        (
            &[b'"', b'\\', b'u', 250, 48, 51, 48, b'"'],
            "invalid escape at line 1 column 7",
        ),
        (
            &[b'"', b'\\', b'u', 48, 250, 51, 48, b'"'],
            "invalid escape at line 1 column 7",
        ),
        (
            &[b'"', b'\\', b'u', 48, 48, 250, 48, b'"'],
            "invalid escape at line 1 column 7",
        ),
        (
            &[b'"', b'\\', b'u', 48, 48, 51, 250, b'"'],
            "invalid escape at line 1 column 7",
        ),
        (
            &[b'"', b'\n', b'"'],
            "control character (\\u0000-\\u001F) found while parsing a string at line 2 column 0",
        ),
        (
            &[b'"', b'\x1F', b'"'],
            "control character (\\u0000-\\u001F) found while parsing a string at line 1 column 2",
        ),
    ]);

    test_parse_ok(vec![
        ("\"\"", String::new()),
        ("\"foo\"", "foo".to_owned()),
        (" \"foo\" ", "foo".to_owned()),
        ("\"\\\"\"", "\"".to_owned()),
        ("\"\\b\"", "\x08".to_owned()),
        ("\"\\n\"", "\n".to_owned()),
        ("\"\\r\"", "\r".to_owned()),
        ("\"\\t\"", "\t".to_owned()),
        ("\"\\u12ab\"", "\u{12ab}".to_owned()),
        ("\"\\uAB12\"", "\u{AB12}".to_owned()),
        ("\"\\uD83C\\uDF95\"", "\u{1F395}".to_owned()),
    ]);
}

#[test]
fn test_parse_list() {
    test_parse_err::<Vec<f64>>(&[
        ("[", "EOF while parsing a list at line 1 column 1"),
        ("[ ", "EOF while parsing a list at line 1 column 2"),
        ("[1", "EOF while parsing a list at line 1 column 2"),
        ("[1,", "EOF while parsing a value at line 1 column 3"),
        ("[1,]", "trailing comma at line 1 column 4"),
        ("[1 2]", "expected `,` or `]` at line 1 column 4"),
        ("[]a", "trailing characters at line 1 column 3"),
    ]);

    test_parse_ok(vec![
        ("[]", vec![]),
        ("[ ]", vec![]),
        ("[null]", vec![()]),
        (" [ null ] ", vec![()]),
    ]);

    test_parse_ok(vec![("[true]", vec![true])]);

    test_parse_ok(vec![("[3,1]", vec![3u64, 1]), (" [ 3 , 1 ] ", vec![3, 1])]);

    test_parse_ok(vec![("[[3], [1, 2]]", vec![vec![3u64], vec![1, 2]])]);

    test_parse_ok(vec![("[1]", (1u64,))]);

    test_parse_ok(vec![("[1, 2]", (1u64, 2u64))]);

    test_parse_ok(vec![("[1, 2, 3]", (1u64, 2u64, 3u64))]);

    test_parse_ok(vec![("[1, [2, 3]]", (1u64, (2u64, 3u64)))]);
}

#[test]
fn test_parse_object() {
    test_parse_err::<BTreeMap<String, u32>>(&[
        ("{", "EOF while parsing an object at line 1 column 1"),
        ("{ ", "EOF while parsing an object at line 1 column 2"),
        ("{1", "key must be a string at line 1 column 2"),
        ("{ \"a\"", "EOF while parsing an object at line 1 column 5"),
        ("{\"a\"", "EOF while parsing an object at line 1 column 4"),
        ("{\"a\" ", "EOF while parsing an object at line 1 column 5"),
        ("{\"a\" 1", "expected `:` at line 1 column 6"),
        ("{\"a\":", "EOF while parsing a value at line 1 column 5"),
        ("{\"a\":1", "EOF while parsing an object at line 1 column 6"),
        ("{\"a\":1 1", "expected `,` or `}` at line 1 column 8"),
        ("{\"a\":1,", "EOF while parsing a value at line 1 column 7"),
        ("{}a", "trailing characters at line 1 column 3"),
    ]);

    test_parse_ok(vec![
        ("{}", treemap!()),
        ("{ }", treemap!()),
        ("{\"a\":3}", treemap!("a".to_owned() => 3u64)),
        ("{ \"a\" : 3 }", treemap!("a".to_owned() => 3)),
        (
            "{\"a\":3,\"b\":4}",
            treemap!("a".to_owned() => 3, "b".to_owned() => 4),
        ),
        (
            " { \"a\" : 3 , \"b\" : 4 } ",
            treemap!("a".to_owned() => 3, "b".to_owned() => 4),
        ),
    ]);

    test_parse_ok(vec![(
        "{\"a\": {\"b\": 3, \"c\": 4}}",
        treemap!(
            "a".to_owned() => treemap!(
                "b".to_owned() => 3u64,
                "c".to_owned() => 4,
            ),
        ),
    )]);

    test_parse_ok(vec![("{\"c\":null}", treemap!('c' => ()))]);
}

#[test]
fn test_parse_struct() {
    test_parse_err::<Outer>(&[
        (
            "5",
            "invalid type: integer `5`, expected struct Outer at line 1 column 1",
        ),
        (
            "\"hello\"",
            "invalid type: string \"hello\", expected struct Outer at line 1 column 7",
        ),
        (
            "{\"inner\": true}",
            "invalid type: boolean `true`, expected a sequence at line 1 column 14",
        ),
        ("{}", "missing field `inner` at line 1 column 2"),
        (
            r#"{"inner": [{"b": 42, "c": []}]}"#,
            "missing field `a` at line 1 column 29",
        ),
    ]);

    test_parse_ok(vec![
        (
            "{
                \"inner\": []
            }",
            Outer { inner: vec![] },
        ),
        (
            "{
                \"inner\": [
                    { \"a\": null, \"b\": 2, \"c\": [\"abc\", \"xyz\"] }
                ]
            }",
            Outer {
                inner: vec![Inner {
                    a: (),
                    b: 2,
                    c: vec!["abc".to_owned(), "xyz".to_owned()],
                }],
            },
        ),
    ]);

    let v: Outer = from_str(
        "[
            [
                [ null, 2, [\"abc\", \"xyz\"] ]
            ]
        ]",
    )
    .unwrap();

    assert_eq!(
        v,
        Outer {
            inner: vec![Inner {
                a: (),
                b: 2,
                c: vec!["abc".to_owned(), "xyz".to_owned()],
            }],
        }
    );

    let j = json!([null, 2, []]);
    Inner::deserialize(&j).unwrap();
    Inner::deserialize(j).unwrap();
}

#[test]
fn test_parse_option() {
    test_parse_ok(vec![
        ("null", None::<String>),
        ("\"jodhpurs\"", Some("jodhpurs".to_owned())),
    ]);

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Foo {
        x: Option<isize>,
    }

    let value: Foo = from_str("{}").unwrap();
    assert_eq!(value, Foo { x: None });

    test_parse_ok(vec![
        ("{\"x\": null}", Foo { x: None }),
        ("{\"x\": 5}", Foo { x: Some(5) }),
    ]);
}

#[test]
fn test_parse_enum_errors() {
    test_parse_err::<Animal>(
        &[
            ("{}", "expected value at line 1 column 2"),
            ("[]", "expected value at line 1 column 1"),
            ("\"unknown\"",
             "unknown variant `unknown`, expected one of `Dog`, `Frog`, `Cat`, `AntHive` at line 1 column 9"),
            ("{\"unknown\":null}",
             "unknown variant `unknown`, expected one of `Dog`, `Frog`, `Cat`, `AntHive` at line 1 column 10"),
            ("{\"Dog\":", "EOF while parsing a value at line 1 column 7"),
            ("{\"Dog\":}", "expected value at line 1 column 8"),
            ("{\"Dog\":{}}", "invalid type: map, expected unit at line 1 column 7"),
            ("\"Frog\"", "invalid type: unit variant, expected tuple variant"),
            ("\"Frog\" 0 ", "invalid type: unit variant, expected tuple variant"),
            ("{\"Frog\":{}}",
             "invalid type: map, expected tuple variant Animal::Frog at line 1 column 8"),
            ("{\"Cat\":[]}", "invalid length 0, expected struct variant Animal::Cat with 2 elements at line 1 column 9"),
            ("{\"Cat\":[0]}", "invalid length 1, expected struct variant Animal::Cat with 2 elements at line 1 column 10"),
            ("{\"Cat\":[0, \"\", 2]}", "trailing characters at line 1 column 16"),
            ("{\"Cat\":{\"age\": 5, \"name\": \"Kate\", \"foo\":\"bar\"}",
             "unknown field `foo`, expected `age` or `name` at line 1 column 39"),

            // JSON does not allow trailing commas in data structures
            ("{\"Cat\":[0, \"Kate\",]}", "trailing comma at line 1 column 19"),
            ("{\"Cat\":{\"age\": 2, \"name\": \"Kate\",}}",
             "trailing comma at line 1 column 34"),
        ],
    );
}

#[test]
fn test_parse_enum() {
    test_parse_ok(vec![
        ("\"Dog\"", Animal::Dog),
        (" \"Dog\" ", Animal::Dog),
        (
            "{\"Frog\":[\"Henry\",[]]}",
            Animal::Frog("Henry".to_owned(), vec![]),
        ),
        (
            " { \"Frog\": [ \"Henry\" , [ 349, 102 ] ] } ",
            Animal::Frog("Henry".to_owned(), vec![349, 102]),
        ),
        (
            "{\"Cat\": {\"age\": 5, \"name\": \"Kate\"}}",
            Animal::Cat {
                age: 5,
                name: "Kate".to_owned(),
            },
        ),
        (
            " { \"Cat\" : { \"age\" : 5 , \"name\" : \"Kate\" } } ",
            Animal::Cat {
                age: 5,
                name: "Kate".to_owned(),
            },
        ),
        (
            " { \"AntHive\" : [\"Bob\", \"Stuart\"] } ",
            Animal::AntHive(vec!["Bob".to_owned(), "Stuart".to_owned()]),
        ),
    ]);

    test_parse_unusual_ok(vec![
        ("{\"Dog\":null}", Animal::Dog),
        (" { \"Dog\" : null } ", Animal::Dog),
    ]);

    test_parse_ok(vec![(
        concat!(
            "{",
            "  \"a\": \"Dog\",",
            "  \"b\": {\"Frog\":[\"Henry\", []]}",
            "}"
        ),
        treemap!(
            "a".to_owned() => Animal::Dog,
            "b".to_owned() => Animal::Frog("Henry".to_owned(), vec![]),
        ),
    )]);
}

#[test]
fn test_parse_trailing_whitespace() {
    test_parse_ok(vec![
        ("[1, 2] ", vec![1u64, 2]),
        ("[1, 2]\n", vec![1, 2]),
        ("[1, 2]\t", vec![1, 2]),
        ("[1, 2]\t \n", vec![1, 2]),
    ]);
}

#[test]
fn test_multiline_errors() {
    test_parse_err::<BTreeMap<String, String>>(&[(
        "{\n  \"foo\":\n \"bar\"",
        "EOF while parsing an object at line 3 column 6",
    )]);
}

#[test]
fn test_missing_option_field() {
    #[derive(Debug, PartialEq, Deserialize)]
    struct Foo {
        x: Option<u32>,
    }

    let value: Foo = from_str("{}").unwrap();
    assert_eq!(value, Foo { x: None });

    let value: Foo = from_str("{\"x\": 5}").unwrap();
    assert_eq!(value, Foo { x: Some(5) });

    let value: Foo = from_value(json!({})).unwrap();
    assert_eq!(value, Foo { x: None });

    let value: Foo = from_value(json!({"x": 5})).unwrap();
    assert_eq!(value, Foo { x: Some(5) });
}

#[test]
fn test_missing_nonoption_field() {
    #[derive(Debug, PartialEq, Deserialize)]
    struct Foo {
        x: u32,
    }

    test_parse_err::<Foo>(&[("{}", "missing field `x` at line 1 column 2")]);
}

#[test]
fn test_missing_renamed_field() {
    #[derive(Debug, PartialEq, Deserialize)]
    struct Foo {
        #[serde(rename = "y")]
        x: Option<u32>,
    }

    let value: Foo = from_str("{}").unwrap();
    assert_eq!(value, Foo { x: None });

    let value: Foo = from_str("{\"y\": 5}").unwrap();
    assert_eq!(value, Foo { x: Some(5) });

    let value: Foo = from_value(json!({})).unwrap();
    assert_eq!(value, Foo { x: None });

    let value: Foo = from_value(json!({"y": 5})).unwrap();
    assert_eq!(value, Foo { x: Some(5) });
}

#[test]
fn test_serialize_seq_with_no_len() {
    #[derive(Clone, Debug, PartialEq)]
    struct MyVec<T>(Vec<T>);

    impl<T> ser::Serialize for MyVec<T>
    where
        T: ser::Serialize,
    {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: ser::Serializer,
        {
            let mut seq = serializer.serialize_seq(None)?;
            for elem in &self.0 {
                seq.serialize_element(elem)?;
            }
            seq.end()
        }
    }

    struct Visitor<T> {
        marker: PhantomData<MyVec<T>>,
    }

    impl<'de, T> de::Visitor<'de> for Visitor<T>
    where
        T: de::Deserialize<'de>,
    {
        type Value = MyVec<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("array")
        }

        fn visit_unit<E>(self) -> Result<MyVec<T>, E>
        where
            E: de::Error,
        {
            Ok(MyVec(Vec::new()))
        }

        fn visit_seq<V>(self, mut visitor: V) -> Result<MyVec<T>, V::Error>
        where
            V: de::SeqAccess<'de>,
        {
            let mut values = Vec::new();

            while let Some(value) = visitor.next_element()? {
                values.push(value);
            }

            Ok(MyVec(values))
        }
    }

    impl<'de, T> de::Deserialize<'de> for MyVec<T>
    where
        T: de::Deserialize<'de>,
    {
        fn deserialize<D>(deserializer: D) -> Result<MyVec<T>, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            deserializer.deserialize_map(Visitor {
                marker: PhantomData,
            })
        }
    }

    let mut vec = Vec::new();
    vec.push(MyVec(Vec::new()));
    vec.push(MyVec(Vec::new()));
    let vec: MyVec<MyVec<u32>> = MyVec(vec);

    test_encode_ok(&[(vec.clone(), "[[],[]]")]);

    let s = to_string_pretty(&vec).unwrap();
    let expected = pretty_str!([[], []]);
    assert_eq!(s, expected);
}

#[test]
fn test_serialize_map_with_no_len() {
    #[derive(Clone, Debug, PartialEq)]
    struct MyMap<K, V>(BTreeMap<K, V>);

    impl<K, V> ser::Serialize for MyMap<K, V>
    where
        K: ser::Serialize + Ord,
        V: ser::Serialize,
    {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: ser::Serializer,
        {
            let mut map = serializer.serialize_map(None)?;
            for (k, v) in &self.0 {
                map.serialize_entry(k, v)?;
            }
            map.end()
        }
    }

    struct Visitor<K, V> {
        marker: PhantomData<MyMap<K, V>>,
    }

    impl<'de, K, V> de::Visitor<'de> for Visitor<K, V>
    where
        K: de::Deserialize<'de> + Eq + Ord,
        V: de::Deserialize<'de>,
    {
        type Value = MyMap<K, V>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("map")
        }

        fn visit_unit<E>(self) -> Result<MyMap<K, V>, E>
        where
            E: de::Error,
        {
            Ok(MyMap(BTreeMap::new()))
        }

        fn visit_map<Visitor>(self, mut visitor: Visitor) -> Result<MyMap<K, V>, Visitor::Error>
        where
            Visitor: de::MapAccess<'de>,
        {
            let mut values = BTreeMap::new();

            while let Some((key, value)) = visitor.next_entry()? {
                values.insert(key, value);
            }

            Ok(MyMap(values))
        }
    }

    impl<'de, K, V> de::Deserialize<'de> for MyMap<K, V>
    where
        K: de::Deserialize<'de> + Eq + Ord,
        V: de::Deserialize<'de>,
    {
        fn deserialize<D>(deserializer: D) -> Result<MyMap<K, V>, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            deserializer.deserialize_map(Visitor {
                marker: PhantomData,
            })
        }
    }

    let mut map = BTreeMap::new();
    map.insert("a", MyMap(BTreeMap::new()));
    map.insert("b", MyMap(BTreeMap::new()));
    let map: MyMap<_, MyMap<u32, u32>> = MyMap(map);

    test_encode_ok(&[(map.clone(), "{\"a\":{},\"b\":{}}")]);

    let s = to_string_pretty(&map).unwrap();
    let expected = pretty_str!({
        "a": {},
        "b": {}
    });
    assert_eq!(s, expected);
}

#[cfg(not(miri))]
#[test]
fn test_deserialize_from_stream() {
    use serde_json::to_writer;
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Message {
        message: String,
    }

    let l = TcpListener::bind("localhost:20000").unwrap();

    thread::spawn(|| {
        let l = l;
        for stream in l.incoming() {
            let mut stream = stream.unwrap();
            let read_stream = stream.try_clone().unwrap();

            let mut de = Deserializer::from_reader(read_stream);
            let request = Message::deserialize(&mut de).unwrap();
            let response = Message {
                message: request.message,
            };
            to_writer(&mut stream, &response).unwrap();
        }
    });

    let mut stream = TcpStream::connect("localhost:20000").unwrap();
    let request = Message {
        message: "hi there".to_owned(),
    };
    to_writer(&mut stream, &request).unwrap();

    let mut de = Deserializer::from_reader(stream);
    let response = Message::deserialize(&mut de).unwrap();

    assert_eq!(request, response);
}

#[test]
fn test_serialize_rejects_adt_keys() {
    let map = treemap!(
        Some("a") => 2,
        Some("b") => 4,
        None => 6,
    );

    let err = to_vec(&map).unwrap_err();
    assert_eq!(err.to_string(), "key must be a string");
}

#[test]
fn test_bytes_ser() {
    let buf = vec![];
    let bytes = Bytes::new(&buf);
    assert_eq!(to_string(&bytes).unwrap(), "[]".to_owned());

    let buf = vec![1, 2, 3];
    let bytes = Bytes::new(&buf);
    assert_eq!(to_string(&bytes).unwrap(), "[1,2,3]".to_owned());
}

#[test]
fn test_byte_buf_ser() {
    let bytes = ByteBuf::new();
    assert_eq!(to_string(&bytes).unwrap(), "[]".to_owned());

    let bytes = ByteBuf::from(vec![1, 2, 3]);
    assert_eq!(to_string(&bytes).unwrap(), "[1,2,3]".to_owned());
}

#[test]
fn test_byte_buf_de() {
    let bytes = ByteBuf::new();
    let v: ByteBuf = from_str("[]").unwrap();
    assert_eq!(v, bytes);

    let bytes = ByteBuf::from(vec![1, 2, 3]);
    let v: ByteBuf = from_str("[1, 2, 3]").unwrap();
    assert_eq!(v, bytes);
}

#[test]
fn test_byte_buf_de_invalid_surrogates() {
    let bytes = ByteBuf::from(vec![237, 160, 188]);
    let v: ByteBuf = from_str(r#""\ud83c""#).unwrap();
    assert_eq!(v, bytes);

    let bytes = ByteBuf::from(vec![237, 160, 188, 10]);
    let v: ByteBuf = from_str(r#""\ud83c\n""#).unwrap();
    assert_eq!(v, bytes);

    let bytes = ByteBuf::from(vec![237, 160, 188, 32]);
    let v: ByteBuf = from_str(r#""\ud83c ""#).unwrap();
    assert_eq!(v, bytes);

    let res = from_str::<ByteBuf>(r#""\ud83c\!""#);
    assert!(res.is_err());

    let res = from_str::<ByteBuf>(r#""\ud83c\u""#);
    assert!(res.is_err());

    // lone trailing surrogate
    let bytes = ByteBuf::from(vec![237, 176, 129]);
    let v: ByteBuf = from_str(r#""\udc01""#).unwrap();
    assert_eq!(v, bytes);

    // leading surrogate followed by other leading surrogate
    let bytes = ByteBuf::from(vec![237, 160, 188, 237, 160, 188]);
    let v: ByteBuf = from_str(r#""\ud83c\ud83c""#).unwrap();
    assert_eq!(v, bytes);

    // leading surrogate followed by "a" (U+0061) in \u encoding
    let bytes = ByteBuf::from(vec![237, 160, 188, 97]);
    let v: ByteBuf = from_str(r#""\ud83c\u0061""#).unwrap();
    assert_eq!(v, bytes);

    // leading surrogate followed by U+0080
    let bytes = ByteBuf::from(vec![237, 160, 188, 194, 128]);
    let v: ByteBuf = from_str(r#""\ud83c\u0080""#).unwrap();
    assert_eq!(v, bytes);

    // leading surrogate followed by U+FFFF
    let bytes = ByteBuf::from(vec![237, 160, 188, 239, 191, 191]);
    let v: ByteBuf = from_str(r#""\ud83c\uffff""#).unwrap();
    assert_eq!(v, bytes);
}

#[test]
fn test_byte_buf_de_surrogate_pair() {
    // leading surrogate followed by trailing surrogate
    let bytes = ByteBuf::from(vec![240, 159, 128, 128]);
    let v: ByteBuf = from_str(r#""\ud83c\udc00""#).unwrap();
    assert_eq!(v, bytes);

    // leading surrogate followed by a surrogate pair
    let bytes = ByteBuf::from(vec![237, 160, 188, 240, 159, 128, 128]);
    let v: ByteBuf = from_str(r#""\ud83c\ud83c\udc00""#).unwrap();
    assert_eq!(v, bytes);
}

#[cfg(feature = "raw_value")]
#[test]
fn test_raw_de_invalid_surrogates() {
    use serde_json::value::RawValue;

    assert!(from_str::<Box<RawValue>>(r#""\ud83c""#).is_ok());
    assert!(from_str::<Box<RawValue>>(r#""\ud83c\n""#).is_ok());
    assert!(from_str::<Box<RawValue>>(r#""\ud83c ""#).is_ok());
    assert!(from_str::<Box<RawValue>>(r#""\udc01 ""#).is_ok());
    assert!(from_str::<Box<RawValue>>(r#""\udc01\!""#).is_err());
    assert!(from_str::<Box<RawValue>>(r#""\udc01\u""#).is_err());
    assert!(from_str::<Box<RawValue>>(r#""\ud83c\ud83c""#).is_ok());
    assert!(from_str::<Box<RawValue>>(r#""\ud83c\u0061""#).is_ok());
    assert!(from_str::<Box<RawValue>>(r#""\ud83c\u0080""#).is_ok());
    assert!(from_str::<Box<RawValue>>(r#""\ud83c\uffff""#).is_ok());
}

#[cfg(feature = "raw_value")]
#[test]
fn test_raw_de_surrogate_pair() {
    use serde_json::value::RawValue;

    assert!(from_str::<Box<RawValue>>(r#""\ud83c\udc00""#).is_ok());
}

#[test]
fn test_byte_buf_de_multiple() {
    let s: Vec<ByteBuf> = from_str(r#"["ab\nc", "cd\ne"]"#).unwrap();
    let a = ByteBuf::from(b"ab\nc".to_vec());
    let b = ByteBuf::from(b"cd\ne".to_vec());
    assert_eq!(vec![a, b], s);
}

#[test]
fn test_json_pointer() {
    // Test case taken from https://tools.ietf.org/html/rfc6901#page-5
    let data: Value = from_str(
        r#"{
        "foo": ["bar", "baz"],
        "": 0,
        "a/b": 1,
        "c%d": 2,
        "e^f": 3,
        "g|h": 4,
        "i\\j": 5,
        "k\"l": 6,
        " ": 7,
        "m~n": 8
    }"#,
    )
    .unwrap();
    assert_eq!(data.pointer("").unwrap(), &data);
    assert_eq!(data.pointer("/foo").unwrap(), &json!(["bar", "baz"]));
    assert_eq!(data.pointer("/foo/0").unwrap(), &json!("bar"));
    assert_eq!(data.pointer("/").unwrap(), &json!(0));
    assert_eq!(data.pointer("/a~1b").unwrap(), &json!(1));
    assert_eq!(data.pointer("/c%d").unwrap(), &json!(2));
    assert_eq!(data.pointer("/e^f").unwrap(), &json!(3));
    assert_eq!(data.pointer("/g|h").unwrap(), &json!(4));
    assert_eq!(data.pointer("/i\\j").unwrap(), &json!(5));
    assert_eq!(data.pointer("/k\"l").unwrap(), &json!(6));
    assert_eq!(data.pointer("/ ").unwrap(), &json!(7));
    assert_eq!(data.pointer("/m~0n").unwrap(), &json!(8));
    // Invalid pointers
    assert!(data.pointer("/unknown").is_none());
    assert!(data.pointer("/e^f/ertz").is_none());
    assert!(data.pointer("/foo/00").is_none());
    assert!(data.pointer("/foo/01").is_none());
}

#[test]
fn test_json_pointer_mut() {
    // Test case taken from https://tools.ietf.org/html/rfc6901#page-5
    let mut data: Value = from_str(
        r#"{
        "foo": ["bar", "baz"],
        "": 0,
        "a/b": 1,
        "c%d": 2,
        "e^f": 3,
        "g|h": 4,
        "i\\j": 5,
        "k\"l": 6,
        " ": 7,
        "m~n": 8
    }"#,
    )
    .unwrap();

    // Basic pointer checks
    assert_eq!(data.pointer_mut("/foo").unwrap(), &json!(["bar", "baz"]));
    assert_eq!(data.pointer_mut("/foo/0").unwrap(), &json!("bar"));
    assert_eq!(data.pointer_mut("/").unwrap(), 0);
    assert_eq!(data.pointer_mut("/a~1b").unwrap(), 1);
    assert_eq!(data.pointer_mut("/c%d").unwrap(), 2);
    assert_eq!(data.pointer_mut("/e^f").unwrap(), 3);
    assert_eq!(data.pointer_mut("/g|h").unwrap(), 4);
    assert_eq!(data.pointer_mut("/i\\j").unwrap(), 5);
    assert_eq!(data.pointer_mut("/k\"l").unwrap(), 6);
    assert_eq!(data.pointer_mut("/ ").unwrap(), 7);
    assert_eq!(data.pointer_mut("/m~0n").unwrap(), 8);

    // Invalid pointers
    assert!(data.pointer_mut("/unknown").is_none());
    assert!(data.pointer_mut("/e^f/ertz").is_none());
    assert!(data.pointer_mut("/foo/00").is_none());
    assert!(data.pointer_mut("/foo/01").is_none());

    // Mutable pointer checks
    *data.pointer_mut("/").unwrap() = 100.into();
    assert_eq!(data.pointer("/").unwrap(), 100);
    *data.pointer_mut("/foo/0").unwrap() = json!("buzz");
    assert_eq!(data.pointer("/foo/0").unwrap(), &json!("buzz"));

    // Example of ownership stealing
    assert_eq!(
        data.pointer_mut("/a~1b")
            .map(|m| mem::replace(m, json!(null)))
            .unwrap(),
        1
    );
    assert_eq!(data.pointer("/a~1b").unwrap(), &json!(null));

    // Need to compare against a clone so we don't anger the borrow checker
    // by taking out two references to a mutable value
    let mut d2 = data.clone();
    assert_eq!(data.pointer_mut("").unwrap(), &mut d2);
}

#[test]
fn test_stack_overflow() {
    let brackets: String = iter::repeat('[')
        .take(127)
        .chain(iter::repeat(']').take(127))
        .collect();
    let _: Value = from_str(&brackets).unwrap();

    let brackets = "[".repeat(129);
    test_parse_err::<Value>(&[(&brackets, "recursion limit exceeded at line 1 column 128")]);
}

#[test]
#[cfg(feature = "unbounded_depth")]
fn test_disable_recursion_limit() {
    let brackets: String = iter::repeat('[')
        .take(140)
        .chain(iter::repeat(']').take(140))
        .collect();

    let mut deserializer = Deserializer::from_str(&brackets);
    deserializer.disable_recursion_limit();
    Value::deserialize(&mut deserializer).unwrap();
}

#[test]
fn test_integer_key() {
    // map with integer keys
    let map = treemap!(
        1 => 2,
        -1 => 6,
    );
    let j = r#"{"-1":6,"1":2}"#;
    test_encode_ok(&[(&map, j)]);
    test_parse_ok(vec![(j, map)]);

    test_parse_err::<BTreeMap<i32, ()>>(&[
        (
            r#"{"x":null}"#,
            "invalid value: expected key to be a number in quotes at line 1 column 2",
        ),
        (
            r#"{" 123":null}"#,
            "invalid value: expected key to be a number in quotes at line 1 column 2",
        ),
        (r#"{"123 ":null}"#, "expected `\"` at line 1 column 6"),
    ]);

    let err = from_value::<BTreeMap<i32, ()>>(json!({" 123":null})).unwrap_err();
    assert_eq!(
        err.to_string(),
        "invalid value: expected key to be a number in quotes",
    );

    let err = from_value::<BTreeMap<i32, ()>>(json!({"123 ":null})).unwrap_err();
    assert_eq!(
        err.to_string(),
        "invalid value: expected key to be a number in quotes",
    );
}

#[test]
fn test_integer128_key() {
    let map = treemap! {
        100000000000000000000000000000000000000u128 => (),
    };
    let j = r#"{"100000000000000000000000000000000000000":null}"#;
    assert_eq!(to_string(&map).unwrap(), j);
    assert_eq!(from_str::<BTreeMap<u128, ()>>(j).unwrap(), map);
}

#[test]
fn test_float_key() {
    #[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone)]
    struct Float;
    impl Serialize for Float {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_f32(1.23)
        }
    }
    impl<'de> Deserialize<'de> for Float {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            f32::deserialize(deserializer).map(|_| Float)
        }
    }

    // map with float key
    let map = treemap!(Float => "x".to_owned());
    let j = r#"{"1.23":"x"}"#;

    test_encode_ok(&[(&map, j)]);
    test_parse_ok(vec![(j, map)]);

    let j = r#"{"x": null}"#;
    test_parse_err::<BTreeMap<Float, ()>>(&[(
        j,
        "invalid value: expected key to be a number in quotes at line 1 column 2",
    )]);
}

#[test]
fn test_deny_non_finite_f32_key() {
    // We store float bits so that we can derive Ord, and other traits. In a
    // real context the code might involve a crate like ordered-float.

    #[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone)]
    struct F32Bits(u32);
    impl Serialize for F32Bits {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_f32(f32::from_bits(self.0))
        }
    }

    let map = treemap!(F32Bits(f32::INFINITY.to_bits()) => "x".to_owned());
    assert!(serde_json::to_string(&map).is_err());
    assert!(serde_json::to_value(map).is_err());

    let map = treemap!(F32Bits(f32::NEG_INFINITY.to_bits()) => "x".to_owned());
    assert!(serde_json::to_string(&map).is_err());
    assert!(serde_json::to_value(map).is_err());

    let map = treemap!(F32Bits(f32::NAN.to_bits()) => "x".to_owned());
    assert!(serde_json::to_string(&map).is_err());
    assert!(serde_json::to_value(map).is_err());
}

#[test]
fn test_deny_non_finite_f64_key() {
    // We store float bits so that we can derive Ord, and other traits. In a
    // real context the code might involve a crate like ordered-float.

    #[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone)]
    struct F64Bits(u64);
    impl Serialize for F64Bits {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_f64(f64::from_bits(self.0))
        }
    }

    let map = treemap!(F64Bits(f64::INFINITY.to_bits()) => "x".to_owned());
    assert!(serde_json::to_string(&map).is_err());
    assert!(serde_json::to_value(map).is_err());

    let map = treemap!(F64Bits(f64::NEG_INFINITY.to_bits()) => "x".to_owned());
    assert!(serde_json::to_string(&map).is_err());
    assert!(serde_json::to_value(map).is_err());

    let map = treemap!(F64Bits(f64::NAN.to_bits()) => "x".to_owned());
    assert!(serde_json::to_string(&map).is_err());
    assert!(serde_json::to_value(map).is_err());
}

#[test]
fn test_boolean_key() {
    let map = treemap!(false => 0, true => 1);
    let j = r#"{"false":0,"true":1}"#;
    test_encode_ok(&[(&map, j)]);
    test_parse_ok(vec![(j, map)]);
}

#[test]
fn test_borrowed_key() {
    let map: BTreeMap<&str, ()> = from_str("{\"borrowed\":null}").unwrap();
    let expected = treemap! { "borrowed" => () };
    assert_eq!(map, expected);

    #[derive(Deserialize, Debug, Ord, PartialOrd, Eq, PartialEq)]
    struct NewtypeStr<'a>(&'a str);

    let map: BTreeMap<NewtypeStr, ()> = from_str("{\"borrowed\":null}").unwrap();
    let expected = treemap! { NewtypeStr("borrowed") => () };
    assert_eq!(map, expected);
}

#[test]
fn test_effectively_string_keys() {
    #[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Serialize, Deserialize)]
    enum Enum {
        One,
        Two,
    }
    let map = treemap! {
        Enum::One => 1,
        Enum::Two => 2,
    };
    let expected = r#"{"One":1,"Two":2}"#;
    test_encode_ok(&[(&map, expected)]);
    test_parse_ok(vec![(expected, map)]);

    #[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Serialize, Deserialize)]
    struct Wrapper(String);
    let map = treemap! {
        Wrapper("zero".to_owned()) => 0,
        Wrapper("one".to_owned()) => 1,
    };
    let expected = r#"{"one":1,"zero":0}"#;
    test_encode_ok(&[(&map, expected)]);
    test_parse_ok(vec![(expected, map)]);
}

#[test]
fn test_json_macro() {
    // This is tricky because the <...> is not a single TT and the comma inside
    // looks like an array element separator.
    let _ = json!([
        <Result<(), ()> as Clone>::clone(&Ok(())),
        <Result<(), ()> as Clone>::clone(&Err(()))
    ]);

    // Same thing but in the map values.
    let _ = json!({
        "ok": <Result<(), ()> as Clone>::clone(&Ok(())),
        "err": <Result<(), ()> as Clone>::clone(&Err(()))
    });

    // It works in map keys but only if they are parenthesized.
    let _ = json!({
        (<Result<&str, ()> as Clone>::clone(&Ok("")).unwrap()): "ok",
        (<Result<(), &str> as Clone>::clone(&Err("")).unwrap_err()): "err"
    });

    #[deny(unused_results)]
    let _ = json!({ "architecture": [true, null] });
}

#[test]
fn issue_220() {
    #[derive(Debug, PartialEq, Eq, Deserialize)]
    enum E {
        V(u8),
    }

    assert!(from_str::<E>(r#" "V"0 "#).is_err());

    assert_eq!(from_str::<E>(r#"{"V": 0}"#).unwrap(), E::V(0));
}

#[test]
fn test_partialeq_number() {
    macro_rules! number_partialeq_ok {
        ($($n:expr)*) => {
            $(
                let value = to_value($n).unwrap();
                let s = $n.to_string();
                assert_eq!(value, $n);
                assert_eq!($n, value);
                assert_ne!(value, s);
            )*
        };
    }

    number_partialeq_ok!(0 1 100
        i8::MIN i8::MAX i16::MIN i16::MAX i32::MIN i32::MAX i64::MIN i64::MAX
        u8::MIN u8::MAX u16::MIN u16::MAX u32::MIN u32::MAX u64::MIN u64::MAX
        f32::MIN f32::MAX f32::MIN_EXP f32::MAX_EXP f32::MIN_POSITIVE
        f64::MIN f64::MAX f64::MIN_EXP f64::MAX_EXP f64::MIN_POSITIVE
        f32::consts::E f32::consts::PI f32::consts::LN_2 f32::consts::LOG2_E
        f64::consts::E f64::consts::PI f64::consts::LN_2 f64::consts::LOG2_E
    );
}

#[test]
fn test_partialeq_string() {
    let v = to_value("42").unwrap();
    assert_eq!(v, "42");
    assert_eq!("42", v);
    assert_ne!(v, 42);
    assert_eq!(v, String::from("42"));
    assert_eq!(String::from("42"), v);
}

#[test]
fn test_partialeq_bool() {
    let v = to_value(true).unwrap();
    assert_eq!(v, true);
    assert_eq!(true, v);
    assert_ne!(v, false);
    assert_ne!(v, "true");
    assert_ne!(v, 1);
    assert_ne!(v, 0);
}

struct FailReader(io::ErrorKind);

impl io::Read for FailReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(self.0, "oh no!"))
    }
}

#[test]
fn test_category() {
    assert!(from_str::<String>("123").unwrap_err().is_data());

    assert!(from_str::<String>("]").unwrap_err().is_syntax());

    assert!(from_str::<String>("").unwrap_err().is_eof());
    assert!(from_str::<String>("\"").unwrap_err().is_eof());
    assert!(from_str::<String>("\"\\").unwrap_err().is_eof());
    assert!(from_str::<String>("\"\\u").unwrap_err().is_eof());
    assert!(from_str::<String>("\"\\u0").unwrap_err().is_eof());
    assert!(from_str::<String>("\"\\u00").unwrap_err().is_eof());
    assert!(from_str::<String>("\"\\u000").unwrap_err().is_eof());

    assert!(from_str::<Vec<usize>>("[").unwrap_err().is_eof());
    assert!(from_str::<Vec<usize>>("[0").unwrap_err().is_eof());
    assert!(from_str::<Vec<usize>>("[0,").unwrap_err().is_eof());

    assert!(from_str::<BTreeMap<String, usize>>("{")
        .unwrap_err()
        .is_eof());
    assert!(from_str::<BTreeMap<String, usize>>("{\"k\"")
        .unwrap_err()
        .is_eof());
    assert!(from_str::<BTreeMap<String, usize>>("{\"k\":")
        .unwrap_err()
        .is_eof());
    assert!(from_str::<BTreeMap<String, usize>>("{\"k\":0")
        .unwrap_err()
        .is_eof());
    assert!(from_str::<BTreeMap<String, usize>>("{\"k\":0,")
        .unwrap_err()
        .is_eof());

    let fail = FailReader(io::ErrorKind::NotConnected);
    assert!(from_reader::<_, String>(fail).unwrap_err().is_io());
}

#[test]
// Clippy false positive: https://github.com/Manishearth/rust-clippy/issues/292
#[allow(clippy::needless_lifetimes)]
fn test_into_io_error() {
    fn io_error<'de, T: Deserialize<'de> + Debug>(j: &'static str) -> io::Error {
        from_str::<T>(j).unwrap_err().into()
    }

    assert_eq!(
        io_error::<String>("\"\\u").kind(),
        io::ErrorKind::UnexpectedEof
    );
    assert_eq!(io_error::<String>("0").kind(), io::ErrorKind::InvalidData);
    assert_eq!(io_error::<String>("]").kind(), io::ErrorKind::InvalidData);

    let fail = FailReader(io::ErrorKind::NotConnected);
    let io_err: io::Error = from_reader::<_, u8>(fail).unwrap_err().into();
    assert_eq!(io_err.kind(), io::ErrorKind::NotConnected);
}

#[test]
fn test_borrow() {
    let s: &str = from_str("\"borrowed\"").unwrap();
    assert_eq!("borrowed", s);

    let s: &str = from_slice(b"\"borrowed\"").unwrap();
    assert_eq!("borrowed", s);
}

#[test]
fn null_invalid_type() {
    let err = serde_json::from_str::<String>("null").unwrap_err();
    assert_eq!(
        format!("{}", err),
        String::from("invalid type: null, expected a string at line 1 column 4")
    );
}

#[test]
fn test_integer128() {
    let signed = &[i128::MIN, -1, 0, 1, i128::MAX];
    let unsigned = &[0, 1, u128::MAX];

    for integer128 in signed {
        let expected = integer128.to_string();
        assert_eq!(to_string(integer128).unwrap(), expected);
        assert_eq!(from_str::<i128>(&expected).unwrap(), *integer128);
    }

    for integer128 in unsigned {
        let expected = integer128.to_string();
        assert_eq!(to_string(integer128).unwrap(), expected);
        assert_eq!(from_str::<u128>(&expected).unwrap(), *integer128);
    }

    test_parse_err::<i128>(&[
        (
            "-170141183460469231731687303715884105729",
            "number out of range at line 1 column 40",
        ),
        (
            "170141183460469231731687303715884105728",
            "number out of range at line 1 column 39",
        ),
    ]);

    test_parse_err::<u128>(&[
        ("-1", "number out of range at line 1 column 1"),
        (
            "340282366920938463463374607431768211456",
            "number out of range at line 1 column 39",
        ),
    ]);
}

#[test]
fn test_integer128_to_value() {
    let signed = &[i128::from(i64::MIN), i128::from(u64::MAX)];
    let unsigned = &[0, u128::from(u64::MAX)];

    for integer128 in signed {
        let expected = integer128.to_string();
        assert_eq!(to_value(integer128).unwrap().to_string(), expected);
    }

    for integer128 in unsigned {
        let expected = integer128.to_string();
        assert_eq!(to_value(integer128).unwrap().to_string(), expected);
    }

    if !cfg!(feature = "arbitrary_precision") {
        let err = to_value(u128::from(u64::MAX) + 1).unwrap_err();
        assert_eq!(err.to_string(), "number out of range");
    }
}

#[cfg(feature = "raw_value")]
#[test]
fn test_borrowed_raw_value() {
    #[derive(Serialize, Deserialize)]
    struct Wrapper<'a> {
        a: i8,
        #[serde(borrow)]
        b: &'a RawValue,
        c: i8,
    }

    let wrapper_from_str: Wrapper =
        serde_json::from_str(r#"{"a": 1, "b": {"foo": 2}, "c": 3}"#).unwrap();
    assert_eq!(r#"{"foo": 2}"#, wrapper_from_str.b.get());

    let wrapper_to_string = serde_json::to_string(&wrapper_from_str).unwrap();
    assert_eq!(r#"{"a":1,"b":{"foo": 2},"c":3}"#, wrapper_to_string);

    let wrapper_to_value = serde_json::to_value(&wrapper_from_str).unwrap();
    assert_eq!(json!({"a": 1, "b": {"foo": 2}, "c": 3}), wrapper_to_value);

    let array_from_str: Vec<&RawValue> =
        serde_json::from_str(r#"["a", 42, {"foo": "bar"}, null]"#).unwrap();
    assert_eq!(r#""a""#, array_from_str[0].get());
    assert_eq!("42", array_from_str[1].get());
    assert_eq!(r#"{"foo": "bar"}"#, array_from_str[2].get());
    assert_eq!("null", array_from_str[3].get());

    let array_to_string = serde_json::to_string(&array_from_str).unwrap();
    assert_eq!(r#"["a",42,{"foo": "bar"},null]"#, array_to_string);
}

#[cfg(feature = "raw_value")]
#[test]
fn test_raw_value_in_map_key() {
    #[derive(RefCast)]
    #[repr(transparent)]
    struct RawMapKey(RawValue);

    #[allow(unknown_lints)]
    #[allow(non_local_definitions)] // false positive: https://github.com/rust-lang/rust/issues/121621
    impl<'de> Deserialize<'de> for &'de RawMapKey {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let raw_value = <&RawValue>::deserialize(deserializer)?;
            Ok(RawMapKey::ref_cast(raw_value))
        }
    }

    impl PartialEq for RawMapKey {
        fn eq(&self, other: &Self) -> bool {
            self.0.get() == other.0.get()
        }
    }

    impl Eq for RawMapKey {}

    impl Hash for RawMapKey {
        fn hash<H: Hasher>(&self, hasher: &mut H) {
            self.0.get().hash(hasher);
        }
    }

    let map_from_str: HashMap<&RawMapKey, &RawValue> =
        serde_json::from_str(r#" {"\\k":"\\v"} "#).unwrap();
    let (map_k, map_v) = map_from_str.into_iter().next().unwrap();
    assert_eq!("\"\\\\k\"", map_k.0.get());
    assert_eq!("\"\\\\v\"", map_v.get());
}

#[cfg(feature = "raw_value")]
#[test]
fn test_boxed_raw_value() {
    #[derive(Serialize, Deserialize)]
    struct Wrapper {
        a: i8,
        b: Box<RawValue>,
        c: i8,
    }

    let wrapper_from_str: Wrapper =
        serde_json::from_str(r#"{"a": 1, "b": {"foo": 2}, "c": 3}"#).unwrap();
    assert_eq!(r#"{"foo": 2}"#, wrapper_from_str.b.get());

    let wrapper_from_reader: Wrapper =
        serde_json::from_reader(br#"{"a": 1, "b": {"foo": 2}, "c": 3}"#.as_ref()).unwrap();
    assert_eq!(r#"{"foo": 2}"#, wrapper_from_reader.b.get());

    let wrapper_from_value: Wrapper =
        serde_json::from_value(json!({"a": 1, "b": {"foo": 2}, "c": 3})).unwrap();
    assert_eq!(r#"{"foo":2}"#, wrapper_from_value.b.get());

    let wrapper_to_string = serde_json::to_string(&wrapper_from_str).unwrap();
    assert_eq!(r#"{"a":1,"b":{"foo": 2},"c":3}"#, wrapper_to_string);

    let wrapper_to_value = serde_json::to_value(&wrapper_from_str).unwrap();
    assert_eq!(json!({"a": 1, "b": {"foo": 2}, "c": 3}), wrapper_to_value);

    let array_from_str: Vec<Box<RawValue>> =
        serde_json::from_str(r#"["a", 42, {"foo": "bar"}, null]"#).unwrap();
    assert_eq!(r#""a""#, array_from_str[0].get());
    assert_eq!("42", array_from_str[1].get());
    assert_eq!(r#"{"foo": "bar"}"#, array_from_str[2].get());
    assert_eq!("null", array_from_str[3].get());

    let array_from_reader: Vec<Box<RawValue>> =
        serde_json::from_reader(br#"["a", 42, {"foo": "bar"}, null]"#.as_ref()).unwrap();
    assert_eq!(r#""a""#, array_from_reader[0].get());
    assert_eq!("42", array_from_reader[1].get());
    assert_eq!(r#"{"foo": "bar"}"#, array_from_reader[2].get());
    assert_eq!("null", array_from_reader[3].get());

    let array_to_string = serde_json::to_string(&array_from_str).unwrap();
    assert_eq!(r#"["a",42,{"foo": "bar"},null]"#, array_to_string);
}

#[cfg(feature = "raw_value")]
#[test]
fn test_raw_invalid_utf8() {
    let j = &[b'"', b'\xCE', b'\xF8', b'"'];
    let value_err = serde_json::from_slice::<Value>(j).unwrap_err();
    let raw_value_err = serde_json::from_slice::<Box<RawValue>>(j).unwrap_err();

    assert_eq!(
        value_err.to_string(),
        "invalid unicode code point at line 1 column 4",
    );
    assert_eq!(
        raw_value_err.to_string(),
        "invalid unicode code point at line 1 column 4",
    );
}

#[cfg(feature = "raw_value")]
#[test]
fn test_serialize_unsized_value_to_raw_value() {
    assert_eq!(
        serde_json::value::to_raw_value("foobar").unwrap().get(),
        r#""foobar""#,
    );
}

#[test]
fn test_borrow_in_map_key() {
    #[derive(Deserialize, Debug)]
    struct Outer {
        #[allow(dead_code)]
        map: BTreeMap<MyMapKey, ()>,
    }

    #[derive(Ord, PartialOrd, Eq, PartialEq, Debug)]
    struct MyMapKey(usize);

    impl<'de> Deserialize<'de> for MyMapKey {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            let s = <&str>::deserialize(deserializer)?;
            let n = s.parse().map_err(de::Error::custom)?;
            Ok(MyMapKey(n))
        }
    }

    let value = json!({ "map": { "1": null } });
    Outer::deserialize(&value).unwrap();
}

#[test]
fn test_value_into_deserializer() {
    #[derive(Deserialize)]
    struct Outer {
        inner: Inner,
    }

    #[derive(Deserialize)]
    struct Inner {
        string: String,
    }

    let mut map = BTreeMap::new();
    map.insert("inner", json!({ "string": "Hello World" }));

    let outer = Outer::deserialize(serde::de::value::MapDeserializer::new(
        map.iter().map(|(k, v)| (*k, v)),
    ))
    .unwrap();
    assert_eq!(outer.inner.string, "Hello World");

    let outer = Outer::deserialize(map.into_deserializer()).unwrap();
    assert_eq!(outer.inner.string, "Hello World");
}

#[test]
fn hash_positive_and_negative_zero() {
    let rand = std::hash::RandomState::new();

    let k1 = serde_json::from_str::<Number>("0.0").unwrap();
    let k2 = serde_json::from_str::<Number>("-0.0").unwrap();
    if cfg!(feature = "arbitrary_precision") {
        assert_ne!(k1, k2);
        assert_ne!(rand.hash_one(k1), rand.hash_one(k2));
    } else {
        assert_eq!(k1, k2);
        assert_eq!(rand.hash_one(k1), rand.hash_one(k2));
    }
}

#[test]
fn test_control_character_search() {
    // Different space circumstances
    for n in 0..16 {
        for m in 0..16 {
            test_parse_err::<String>(&[(
                &format!("\"{}\n{}\"", " ".repeat(n), " ".repeat(m)),
                "control character (\\u0000-\\u001F) found while parsing a string at line 2 column 0",
            )]);
        }
    }

    // Multiple occurrences
    test_parse_err::<String>(&[(
        "\"\t\n\r\"",
        "control character (\\u0000-\\u001F) found while parsing a string at line 1 column 2",
    )]);
}

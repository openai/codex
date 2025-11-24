#![allow(clippy::float_cmp, clippy::non_ascii_literal)]

#[macro_use]
mod macros;

use proc_macro2::{Delimiter, Group, Literal, Span, TokenStream, TokenTree};
use quote::ToTokens;
use std::iter::FromIterator;
use std::str::FromStr;
use syn::{Lit, LitFloat, LitInt, LitStr};

fn lit(s: &str) -> Lit {
    match TokenStream::from_str(s)
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
    {
        TokenTree::Literal(lit) => Lit::new(lit),
        _ => panic!(),
    }
}

#[test]
fn strings() {
    fn test_string(s: &str, value: &str) {
        match lit(s) {
            Lit::Str(lit) => {
                assert_eq!(lit.value(), value);
                let again = lit.into_token_stream().to_string();
                if again != s {
                    test_string(&again, value);
                }
            }
            wrong => panic!("{:?}", wrong),
        }
    }

    test_string("\"a\"", "a");
    test_string("\"\\n\"", "\n");
    test_string("\"\\r\"", "\r");
    test_string("\"\\t\"", "\t");
    test_string("\"🐕\"", "🐕"); // NOTE: This is an emoji
    test_string("\"\\\"\"", "\"");
    test_string("\"'\"", "'");
    test_string("\"\"", "");
    test_string("\"\\u{1F415}\"", "\u{1F415}");
    test_string("\"\\u{1_2__3_}\"", "\u{123}");
    test_string(
        "\"contains\nnewlines\\\nescaped newlines\"",
        "contains\nnewlinesescaped newlines",
    );
    test_string(
        "\"escaped newline\\\n \x0C unsupported whitespace\"",
        "escaped newline\x0C unsupported whitespace",
    );
    test_string("r\"raw\nstring\\\nhere\"", "raw\nstring\\\nhere");
    test_string("\"...\"q", "...");
    test_string("r\"...\"q", "...");
    test_string("r##\"...\"##q", "...");
}

#[test]
fn byte_strings() {
    fn test_byte_string(s: &str, value: &[u8]) {
        match lit(s) {
            Lit::ByteStr(lit) => {
                assert_eq!(lit.value(), value);
                let again = lit.into_token_stream().to_string();
                if again != s {
                    test_byte_string(&again, value);
                }
            }
            wrong => panic!("{:?}", wrong),
        }
    }

    test_byte_string("b\"a\"", b"a");
    test_byte_string("b\"\\n\"", b"\n");
    test_byte_string("b\"\\r\"", b"\r");
    test_byte_string("b\"\\t\"", b"\t");
    test_byte_string("b\"\\\"\"", b"\"");
    test_byte_string("b\"'\"", b"'");
    test_byte_string("b\"\"", b"");
    test_byte_string(
        "b\"contains\nnewlines\\\nescaped newlines\"",
        b"contains\nnewlinesescaped newlines",
    );
    test_byte_string("br\"raw\nstring\\\nhere\"", b"raw\nstring\\\nhere");
    test_byte_string("b\"...\"q", b"...");
    test_byte_string("br\"...\"q", b"...");
    test_byte_string("br##\"...\"##q", b"...");
}

#[test]
fn bytes() {
    fn test_byte(s: &str, value: u8) {
        match lit(s) {
            Lit::Byte(lit) => {
                assert_eq!(lit.value(), value);
                let again = lit.into_token_stream().to_string();
                assert_eq!(again, s);
            }
            wrong => panic!("{:?}", wrong),
        }
    }

    test_byte("b'a'", b'a');
    test_byte("b'\\n'", b'\n');
    test_byte("b'\\r'", b'\r');
    test_byte("b'\\t'", b'\t');
    test_byte("b'\\''", b'\'');
    test_byte("b'\"'", b'"');
    test_byte("b'a'q", b'a');
}

#[test]
fn chars() {
    fn test_char(s: &str, value: char) {
        match lit(s) {
            Lit::Char(lit) => {
                assert_eq!(lit.value(), value);
                let again = lit.into_token_stream().to_string();
                if again != s {
                    test_char(&again, value);
                }
            }
            wrong => panic!("{:?}", wrong),
        }
    }

    test_char("'a'", 'a');
    test_char("'\\n'", '\n');
    test_char("'\\r'", '\r');
    test_char("'\\t'", '\t');
    test_char("'🐕'", '🐕'); // NOTE: This is an emoji
    test_char("'\\''", '\'');
    test_char("'\"'", '"');
    test_char("'\\u{1F415}'", '\u{1F415}');
    test_char("'a'q", 'a');
}

#[test]
fn ints() {
    fn test_int(s: &str, value: u64, suffix: &str) {
        match lit(s) {
            Lit::Int(lit) => {
                assert_eq!(lit.base10_digits().parse::<u64>().unwrap(), value);
                assert_eq!(lit.suffix(), suffix);
                let again = lit.into_token_stream().to_string();
                if again != s {
                    test_int(&again, value, suffix);
                }
            }
            wrong => panic!("{:?}", wrong),
        }
    }

    test_int("5", 5, "");
    test_int("5u32", 5, "u32");
    test_int("0E", 0, "E");
    test_int("0ECMA", 0, "ECMA");
    test_int("0o0A", 0, "A");
    test_int("5_0", 50, "");
    test_int("5_____0_____", 50, "");
    test_int("0x7f", 127, "");
    test_int("0x7F", 127, "");
    test_int("0b1001", 9, "");
    test_int("0o73", 59, "");
    test_int("0x7Fu8", 127, "u8");
    test_int("0b1001i8", 9, "i8");
    test_int("0o73u32", 59, "u32");
    test_int("0x__7___f_", 127, "");
    test_int("0x__7___F_", 127, "");
    test_int("0b_1_0__01", 9, "");
    test_int("0o_7__3", 59, "");
    test_int("0x_7F__u8", 127, "u8");
    test_int("0b__10__0_1i8", 9, "i8");
    test_int("0o__7__________________3u32", 59, "u32");
    test_int("0e1\u{5c5}", 0, "e1\u{5c5}");
}

#[test]
fn floats() {
    fn test_float(s: &str, value: f64, suffix: &str) {
        match lit(s) {
            Lit::Float(lit) => {
                assert_eq!(lit.base10_digits().parse::<f64>().unwrap(), value);
                assert_eq!(lit.suffix(), suffix);
                let again = lit.into_token_stream().to_string();
                if again != s {
                    test_float(&again, value, suffix);
                }
            }
            wrong => panic!("{:?}", wrong),
        }
    }

    test_float("5.5", 5.5, "");
    test_float("5.5E12", 5.5e12, "");
    test_float("5.5e12", 5.5e12, "");
    test_float("1.0__3e-12", 1.03e-12, "");
    test_float("1.03e+12", 1.03e12, "");
    test_float("9e99e99", 9e99, "e99");
    test_float("1e_0", 1.0, "");
    test_float("0.0ECMA", 0.0, "ECMA");
}

#[test]
fn negative() {
    let span = Span::call_site();
    assert_eq!("-1", LitInt::new("-1", span).to_string());
    assert_eq!("-1i8", LitInt::new("-1i8", span).to_string());
    assert_eq!("-1i16", LitInt::new("-1i16", span).to_string());
    assert_eq!("-1i32", LitInt::new("-1i32", span).to_string());
    assert_eq!("-1i64", LitInt::new("-1i64", span).to_string());
    assert_eq!("-1.5", LitFloat::new("-1.5", span).to_string());
    assert_eq!("-1.5f32", LitFloat::new("-1.5f32", span).to_string());
    assert_eq!("-1.5f64", LitFloat::new("-1.5f64", span).to_string());
}

#[test]
fn suffix() {
    fn get_suffix(token: &str) -> String {
        let lit = syn::parse_str::<Lit>(token).unwrap();
        match lit {
            Lit::Str(lit) => lit.suffix().to_owned(),
            Lit::ByteStr(lit) => lit.suffix().to_owned(),
            Lit::Byte(lit) => lit.suffix().to_owned(),
            Lit::Char(lit) => lit.suffix().to_owned(),
            Lit::Int(lit) => lit.suffix().to_owned(),
            Lit::Float(lit) => lit.suffix().to_owned(),
            _ => unimplemented!(),
        }
    }

    assert_eq!(get_suffix("\"\"s"), "s");
    assert_eq!(get_suffix("r\"\"r"), "r");
    assert_eq!(get_suffix("b\"\"b"), "b");
    assert_eq!(get_suffix("br\"\"br"), "br");
    assert_eq!(get_suffix("r#\"\"#r"), "r");
    assert_eq!(get_suffix("'c'c"), "c");
    assert_eq!(get_suffix("b'b'b"), "b");
    assert_eq!(get_suffix("1i32"), "i32");
    assert_eq!(get_suffix("1_i32"), "i32");
    assert_eq!(get_suffix("1.0f32"), "f32");
    assert_eq!(get_suffix("1.0_f32"), "f32");
}

#[test]
fn test_deep_group_empty() {
    let tokens = TokenStream::from_iter(vec![TokenTree::Group(Group::new(
        Delimiter::None,
        TokenStream::from_iter(vec![TokenTree::Group(Group::new(
            Delimiter::None,
            TokenStream::from_iter(vec![TokenTree::Literal(Literal::string("hi"))]),
        ))]),
    ))]);

    snapshot!(tokens as Lit, @r#""hi""# );
}

#[test]
fn test_error() {
    let err = syn::parse_str::<LitStr>("...").unwrap_err();
    assert_eq!("expected string literal", err.to_string());

    let err = syn::parse_str::<LitStr>("5").unwrap_err();
    assert_eq!("expected string literal", err.to_string());
}

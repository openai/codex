use super::*;

use snapbox::prelude::*;
use snapbox::str;

use crate::ascii::digit1 as digit;
use crate::binary::u16;
use crate::binary::u8;
use crate::binary::Endianness;
use crate::error::ErrMode;
use crate::error::ParserError;
#[cfg(feature = "alloc")]
use crate::lib::std::borrow::ToOwned;
use crate::prelude::*;
use crate::stream::Stream;
use crate::token::take;
use crate::ModalResult;
use crate::Partial;

#[cfg(feature = "alloc")]
use crate::lib::std::vec::Vec;

#[test]
fn eof_on_slices() {
    let not_over: &[u8] = &b"Hello, world!"[..];
    let is_over: &[u8] = &b""[..];

    let res_not_over = eof.parse_peek(not_over);
    assert_parse!(
        res_not_over,
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                72,
                101,
                108,
                108,
                111,
                44,
                32,
                119,
                111,
                114,
                108,
                100,
                33,
            ],
        },
    ),
)

"#]]
        .raw()
    );

    let res_over = eof.parse_peek(is_over);
    assert_parse!(
        res_over,
        str![[r#"
Ok(
    (
        [],
        [],
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn eof_on_strs() {
    let not_over: &str = "Hello, world!";
    let is_over: &str = "";

    let res_not_over = eof.parse_peek(not_over);
    assert_parse!(
        res_not_over,
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: "Hello, world!",
        },
    ),
)

"#]]
        .raw()
    );

    let res_over = eof.parse_peek(is_over);
    assert_parse!(
        res_over,
        str![[r#"
Ok(
    (
        "",
        "",
    ),
)

"#]]
        .raw()
    );
}

use crate::lib::std::convert::From;
impl From<u32> for CustomError {
    fn from(_: u32) -> Self {
        CustomError
    }
}

impl<I: Stream> ParserError<I> for CustomError {
    type Inner = Self;

    fn from_input(_: &I) -> Self {
        CustomError
    }

    fn into_inner(self) -> Result<Self::Inner, Self> {
        Ok(self)
    }
}

struct CustomError;
#[allow(dead_code)]
fn custom_error<'i>(input: &mut &'i [u8]) -> ModalResult<&'i [u8], CustomError> {
    //fix_error!(input, CustomError<_>, alphanumeric)
    crate::ascii::alphanumeric1.parse_next(input)
}

#[test]
fn test_parser_flat_map() {
    let input: &[u8] = &[3, 100, 101, 102, 103, 104][..];
    assert_parse!(
        u8.flat_map(take).parse_peek(input),
        str![[r#"
Ok(
    (
        [
            103,
            104,
        ],
        [
            100,
            101,
            102,
        ],
    ),
)

"#]]
        .raw()
    );
}

#[allow(dead_code)]
fn test_closure_compiles_195(input: &mut &[u8]) -> ModalResult<()> {
    u8.flat_map(|num| repeat(num as usize, u16(Endianness::Big)))
        .parse_next(input)
}

#[test]
fn test_parser_verify_map() {
    let input: &[u8] = &[50][..];
    assert_parse!(
        u8.verify_map(|u| if u < 20 { Some(u) } else { None })
            .parse_peek(input),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                50,
            ],
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        u8.verify_map(|u| if u > 20 { Some(u) } else { None })
            .parse_peek(input),
        str![[r#"
Ok(
    (
        [],
        50,
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn test_parser_map_parser() {
    let input: &[u8] = &[100, 101, 102, 103, 104][..];
    assert_parse!(
        take(4usize).and_then(take(2usize)).parse_peek(input),
        str![[r#"
Ok(
    (
        [
            104,
        ],
        [
            100,
            101,
        ],
    ),
)

"#]]
        .raw()
    );
}

#[test]
#[cfg(feature = "std")]
fn test_parser_into() {
    use crate::token::take;

    assert_parse!(
        take(3u8)
            .output_into::<Vec<u8>>()
            .parse_peek(&b"abcdefg"[..]),
        str![[r#"
Ok(
    (
        [
            100,
            101,
            102,
            103,
        ],
        [
            97,
            98,
            99,
        ],
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn opt_test() {
    fn opt_abcd<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, Option<&'i [u8]>> {
        opt("abcd").parse_next(i)
    }

    let a = &b"abcdef"[..];
    let b = &b"bcdefg"[..];
    let c = &b"ab"[..];
    assert_parse!(
        opt_abcd.parse_peek(Partial::new(a)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                101,
                102,
            ],
            partial: true,
        },
        Some(
            [
                97,
                98,
                99,
                100,
            ],
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        opt_abcd.parse_peek(Partial::new(b)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                98,
                99,
                100,
                101,
                102,
                103,
            ],
            partial: true,
        },
        None,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        opt_abcd.parse_peek(Partial::new(c)),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn peek_test() {
    fn peek_literal<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        peek("abcd").parse_next(i)
    }

    assert_parse!(
        peek_literal.parse_peek(Partial::new(&b"abcdef"[..])),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                97,
                98,
                99,
                100,
                101,
                102,
            ],
            partial: true,
        },
        [
            97,
            98,
            99,
            100,
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        peek_literal.parse_peek(Partial::new(&b"ab"[..])),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        peek_literal.parse_peek(Partial::new(&b"xxx"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn not_test() {
    fn not_aaa<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, ()> {
        not("aaa").parse_next(i)
    }

    assert_parse!(
        not_aaa.parse_peek(Partial::new(&b"aaa"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    97,
                    97,
                    97,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        not_aaa.parse_peek(Partial::new(&b"aa"[..])),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        not_aaa.parse_peek(Partial::new(&b"abcd"[..])),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                97,
                98,
                99,
                100,
            ],
            partial: true,
        },
        (),
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn test_parser_verify() {
    use crate::token::take;

    fn test<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        take(5u8)
            .verify(|slice: &[u8]| slice[0] == b'a')
            .parse_next(i)
    }
    assert_parse!(
        test.parse_peek(Partial::new(&b"bcd"[..])),
        str![[r#"
Err(
    Incomplete(
        Size(
            2,
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        test.parse_peek(Partial::new(&b"bcdefg"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    98,
                    99,
                    100,
                    101,
                    102,
                    103,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        test.parse_peek(Partial::new(&b"abcdefg"[..])),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                102,
                103,
            ],
            partial: true,
        },
        [
            97,
            98,
            99,
            100,
            101,
        ],
    ),
)

"#]]
        .raw()
    );
}

#[test]
#[allow(unused)]
fn test_parser_verify_ref() {
    use crate::token::take;

    let mut parser1 = take(3u8).verify(|s: &[u8]| s == &b"abc"[..]);

    assert_parse!(
        parser1.parse_peek(&b"abcd"[..]),
        str![[r#"
Ok(
    (
        [
            100,
        ],
        [
            97,
            98,
            99,
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        parser1.parse_peek(&b"defg"[..]),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                100,
                101,
                102,
                103,
            ],
        },
    ),
)

"#]]
        .raw()
    );

    fn parser2<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], u32> {
        crate::binary::be_u32
            .verify(|val: &u32| *val < 3)
            .parse_next(i)
    }
}

#[test]
#[cfg(feature = "alloc")]
fn test_parser_verify_alloc() {
    use crate::token::take;
    let mut parser1 = take(3u8)
        .map(|s: &[u8]| s.to_vec())
        .verify(|s: &[u8]| s == &b"abc"[..]);

    assert_parse!(
        parser1.parse_peek(&b"abcd"[..]),
        str![[r#"
Ok(
    (
        [
            100,
        ],
        [
            97,
            98,
            99,
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        parser1.parse_peek(&b"defg"[..]),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                100,
                101,
                102,
                103,
            ],
        },
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn fail_test() {
    let a = "string";
    let b = "another string";

    assert_parse!(
        fail::<_, &str, _>.parse_peek(a),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: "string",
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        fail::<_, &str, _>.parse_peek(b),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: "another string",
        },
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn complete() {
    fn err_test<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], &'i [u8]> {
        let _ = "ijkl".parse_next(i)?;
        "mnop".parse_next(i)
    }
    let a = &b"ijklmn"[..];

    let res_a = err_test.parse_peek(a);
    assert_parse!(
        res_a,
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                109,
                110,
            ],
        },
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn separated_pair_test() {
    #[allow(clippy::type_complexity)]
    fn sep_pair_abc_def<'i>(
        i: &mut Partial<&'i [u8]>,
    ) -> TestResult<Partial<&'i [u8]>, (&'i [u8], &'i [u8])> {
        separated_pair("abc", ",", "def").parse_next(i)
    }

    assert_parse!(
        sep_pair_abc_def.parse_peek(Partial::new(&b"abc,defghijkl"[..])),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                103,
                104,
                105,
                106,
                107,
                108,
            ],
            partial: true,
        },
        (
            [
                97,
                98,
                99,
            ],
            [
                100,
                101,
                102,
            ],
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        sep_pair_abc_def.parse_peek(Partial::new(&b"ab"[..])),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        sep_pair_abc_def.parse_peek(Partial::new(&b"abc,d"[..])),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        sep_pair_abc_def.parse_peek(Partial::new(&b"xxx"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        sep_pair_abc_def.parse_peek(Partial::new(&b"xxx,def"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                    44,
                    100,
                    101,
                    102,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        sep_pair_abc_def.parse_peek(Partial::new(&b"abc,xxx"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn preceded_test() {
    fn preceded_abcd_efgh<'i>(
        i: &mut Partial<&'i [u8]>,
    ) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        preceded("abcd", "efgh").parse_next(i)
    }

    assert_parse!(
        preceded_abcd_efgh.parse_peek(Partial::new(&b"abcdefghijkl"[..])),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                105,
                106,
                107,
                108,
            ],
            partial: true,
        },
        [
            101,
            102,
            103,
            104,
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        preceded_abcd_efgh.parse_peek(Partial::new(&b"ab"[..])),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        preceded_abcd_efgh.parse_peek(Partial::new(&b"abcde"[..])),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        preceded_abcd_efgh.parse_peek(Partial::new(&b"xxx"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        preceded_abcd_efgh.parse_peek(Partial::new(&b"xxxxdef"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                    120,
                    100,
                    101,
                    102,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        preceded_abcd_efgh.parse_peek(Partial::new(&b"abcdxxx"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn terminated_test() {
    fn terminated_abcd_efgh<'i>(
        i: &mut Partial<&'i [u8]>,
    ) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        terminated("abcd", "efgh").parse_next(i)
    }

    assert_parse!(
        terminated_abcd_efgh.parse_peek(Partial::new(&b"abcdefghijkl"[..])),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                105,
                106,
                107,
                108,
            ],
            partial: true,
        },
        [
            97,
            98,
            99,
            100,
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        terminated_abcd_efgh.parse_peek(Partial::new(&b"ab"[..])),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        terminated_abcd_efgh.parse_peek(Partial::new(&b"abcde"[..])),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        terminated_abcd_efgh.parse_peek(Partial::new(&b"xxx"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        terminated_abcd_efgh.parse_peek(Partial::new(&b"xxxxdef"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                    120,
                    100,
                    101,
                    102,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        terminated_abcd_efgh.parse_peek(Partial::new(&b"abcdxxxx"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                    120,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn delimited_test() {
    fn delimited_abc_def_ghi<'i>(
        i: &mut Partial<&'i [u8]>,
    ) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        delimited("abc", "def", "ghi").parse_next(i)
    }

    assert_parse!(
        delimited_abc_def_ghi.parse_peek(Partial::new(&b"abcdefghijkl"[..])),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                106,
                107,
                108,
            ],
            partial: true,
        },
        [
            100,
            101,
            102,
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        delimited_abc_def_ghi.parse_peek(Partial::new(&b"ab"[..])),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        delimited_abc_def_ghi.parse_peek(Partial::new(&b"abcde"[..])),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        delimited_abc_def_ghi.parse_peek(Partial::new(&b"abcdefgh"[..])),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        delimited_abc_def_ghi.parse_peek(Partial::new(&b"xxx"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        delimited_abc_def_ghi.parse_peek(Partial::new(&b"xxxdefghi"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                    100,
                    101,
                    102,
                    103,
                    104,
                    105,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        delimited_abc_def_ghi.parse_peek(Partial::new(&b"abcxxxghi"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                    103,
                    104,
                    105,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        delimited_abc_def_ghi.parse_peek(Partial::new(&b"abcdefxxx"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
}

#[cfg(feature = "alloc")]
#[test]
fn alt_test() {
    #[cfg(feature = "alloc")]
    use crate::{
        error::ParserError,
        lib::std::{fmt::Debug, string::String},
    };

    #[cfg(feature = "alloc")]
    #[derive(Debug, Clone, Eq, PartialEq)]
    struct ErrorStr(String);

    #[cfg(feature = "alloc")]
    impl From<u32> for ErrorStr {
        fn from(i: u32) -> Self {
            ErrorStr(format!("custom error code: {i}"))
        }
    }

    #[cfg(feature = "alloc")]
    impl<'a> From<&'a str> for ErrorStr {
        fn from(i: &'a str) -> Self {
            ErrorStr(format!("custom error message: {i}"))
        }
    }

    #[cfg(feature = "alloc")]
    impl<I: Stream + Debug> ParserError<I> for ErrorStr {
        type Inner = Self;

        fn from_input(input: &I) -> Self {
            ErrorStr(format!("custom error message: ({input:?})"))
        }

        fn append(self, input: &I, _: &<I as Stream>::Checkpoint) -> Self {
            ErrorStr(format!("custom error message: ({input:?}) - {self:?}"))
        }

        fn into_inner(self) -> Result<Self::Inner, Self> {
            Ok(self)
        }
    }

    fn work<'i>(input: &mut &'i [u8]) -> ModalResult<&'i [u8], ErrorStr> {
        Ok(input.finish())
    }

    #[allow(unused_variables)]
    fn dont_work<'i>(input: &mut &'i [u8]) -> ModalResult<&'i [u8], ErrorStr> {
        Err(ErrMode::Backtrack(ErrorStr("abcd".to_owned())))
    }

    fn work2<'i>(_input: &mut &'i [u8]) -> ModalResult<&'i [u8], ErrorStr> {
        Ok(&b""[..])
    }

    fn alt1<'i>(i: &mut &'i [u8]) -> ModalResult<&'i [u8], ErrorStr> {
        alt((dont_work, dont_work)).parse_next(i)
    }
    fn alt2<'i>(i: &mut &'i [u8]) -> ModalResult<&'i [u8], ErrorStr> {
        alt((dont_work, work)).parse_next(i)
    }
    fn alt3<'i>(i: &mut &'i [u8]) -> ModalResult<&'i [u8], ErrorStr> {
        alt((dont_work, dont_work, work2, dont_work)).parse_next(i)
    }
    //named!(alt1, alt!(dont_work | dont_work));
    //named!(alt2, alt!(dont_work | work));
    //named!(alt3, alt!(dont_work | dont_work | work2 | dont_work));

    let a = &b"abcd"[..];
    assert_eq!(
        alt1.parse_peek(a),
        Err(ErrMode::Backtrack(ErrorStr(
            "custom error message: ([97, 98, 99, 100]) - ErrorStr(\"abcd\")".to_owned()
        )))
    );
    assert_eq!(alt2.parse_peek(a), Ok((&b""[..], a)));
    assert_eq!(alt3.parse_peek(a), Ok((a, &b""[..])));

    fn alt4<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], &'i [u8]> {
        alt(("abcd", "efgh")).parse_next(i)
    }
    let b = &b"efgh"[..];
    assert_parse!(
        alt4.parse_peek(a),
        str![[r#"
Ok(
    (
        [],
        [
            97,
            98,
            99,
            100,
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        alt4.parse_peek(b),
        str![[r#"
Ok(
    (
        [],
        [
            101,
            102,
            103,
            104,
        ],
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn alt_incomplete() {
    fn alt1<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        alt(("a", "bc", "def")).parse_next(i)
    }

    let a = &b""[..];
    assert_parse!(
        alt1.parse_peek(Partial::new(a)),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    let a = &b"b"[..];
    assert_parse!(
        alt1.parse_peek(Partial::new(a)),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    let a = &b"bcd"[..];
    assert_parse!(
        alt1.parse_peek(Partial::new(a)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                100,
            ],
            partial: true,
        },
        [
            98,
            99,
        ],
    ),
)

"#]]
        .raw()
    );
    let a = &b"cde"[..];
    assert_parse!(
        alt1.parse_peek(Partial::new(a)),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    99,
                    100,
                    101,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    let a = &b"de"[..];
    assert_parse!(
        alt1.parse_peek(Partial::new(a)),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    let a = &b"defg"[..];
    assert_parse!(
        alt1.parse_peek(Partial::new(a)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                103,
            ],
            partial: true,
        },
        [
            100,
            101,
            102,
        ],
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn alt_array() {
    fn alt1<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], &'i [u8]> {
        alt(["a", "bc", "def"]).parse_next(i)
    }

    let i = &b"a"[..];
    assert_parse!(
        alt1.parse_peek(i),
        str![[r#"
Ok(
    (
        [],
        [
            97,
        ],
    ),
)

"#]]
        .raw()
    );

    let i = &b"bc"[..];
    assert_parse!(
        alt1.parse_peek(i),
        str![[r#"
Ok(
    (
        [],
        [
            98,
            99,
        ],
    ),
)

"#]]
        .raw()
    );

    let i = &b"defg"[..];
    assert_parse!(
        alt1.parse_peek(i),
        str![[r#"
Ok(
    (
        [
            103,
        ],
        [
            100,
            101,
            102,
        ],
    ),
)

"#]]
        .raw()
    );

    let i = &b"z"[..];
    assert_parse!(
        alt1.parse_peek(i),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                122,
            ],
        },
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn alt_dynamic_array() {
    fn alt1<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], &'i [u8]> {
        alt(&mut ["a", "bc", "def"][..]).parse_next(i)
    }

    let a = &b"a"[..];
    assert_parse!(
        alt1.parse_peek(a),
        str![[r#"
Ok(
    (
        [],
        [
            97,
        ],
    ),
)

"#]]
        .raw()
    );

    let bc = &b"bc"[..];
    assert_parse!(
        alt1.parse_peek(bc),
        str![[r#"
Ok(
    (
        [],
        [
            98,
            99,
        ],
    ),
)

"#]]
        .raw()
    );

    let defg = &b"defg"[..];
    assert_parse!(
        alt1.parse_peek(defg),
        str![[r#"
Ok(
    (
        [
            103,
        ],
        [
            100,
            101,
            102,
        ],
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn permutation_test() {
    #[allow(clippy::type_complexity)]
    fn perm<'i>(
        i: &mut Partial<&'i [u8]>,
    ) -> TestResult<Partial<&'i [u8]>, (&'i [u8], &'i [u8], &'i [u8])> {
        permutation(("abcd", "efg", "hi")).parse_next(i)
    }

    let a = &b"abcdefghijk"[..];
    assert_parse!(
        perm.parse_peek(Partial::new(a)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                106,
                107,
            ],
            partial: true,
        },
        (
            [
                97,
                98,
                99,
                100,
            ],
            [
                101,
                102,
                103,
            ],
            [
                104,
                105,
            ],
        ),
    ),
)

"#]]
        .raw()
    );
    let b = &b"efgabcdhijk"[..];
    assert_parse!(
        perm.parse_peek(Partial::new(b)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                106,
                107,
            ],
            partial: true,
        },
        (
            [
                97,
                98,
                99,
                100,
            ],
            [
                101,
                102,
                103,
            ],
            [
                104,
                105,
            ],
        ),
    ),
)

"#]]
        .raw()
    );
    let c = &b"hiefgabcdjk"[..];
    assert_parse!(
        perm.parse_peek(Partial::new(c)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                106,
                107,
            ],
            partial: true,
        },
        (
            [
                97,
                98,
                99,
                100,
            ],
            [
                101,
                102,
                103,
            ],
            [
                104,
                105,
            ],
        ),
    ),
)

"#]]
        .raw()
    );

    let d = &b"efgxyzabcdefghi"[..];
    assert_parse!(
        perm.parse_peek(Partial::new(d)),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    121,
                    122,
                    97,
                    98,
                    99,
                    100,
                    101,
                    102,
                    103,
                    104,
                    105,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );

    let e = &b"efgabc"[..];
    assert_parse!(
        perm.parse_peek(Partial::new(e)),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
}

#[test]
#[cfg(feature = "alloc")]
fn separated0_test() {
    fn multi<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, Vec<&'i [u8]>> {
        separated(0.., "abcd", ",").parse_next(i)
    }
    fn multi_empty<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, Vec<&'i [u8]>> {
        separated(0.., "", ",").parse_next(i)
    }
    fn multi_longsep<'i>(
        i: &mut Partial<&'i [u8]>,
    ) -> TestResult<Partial<&'i [u8]>, Vec<&'i [u8]>> {
        separated(0.., "abcd", "..").parse_next(i)
    }

    let a = &b"abcdef"[..];
    let b = &b"abcd,abcdef"[..];
    let c = &b"azerty"[..];
    let d = &b",,abc"[..];
    let e = &b"abcd,abcd,ef"[..];
    let f = &b"abc"[..];
    let g = &b"abcd."[..];
    let h = &b"abcd,abc"[..];

    assert_parse!(
        multi.parse_peek(Partial::new(a)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                101,
                102,
            ],
            partial: true,
        },
        [
            [
                97,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(b)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                101,
                102,
            ],
            partial: true,
        },
        [
            [
                97,
                98,
                99,
                100,
            ],
            [
                97,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(c)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                97,
                122,
                101,
                114,
                116,
                121,
            ],
            partial: true,
        },
        [],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi_empty.parse_peek(Partial::new(d)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                97,
                98,
                99,
            ],
            partial: true,
        },
        [
            [],
            [],
            [],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(e)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                44,
                101,
                102,
            ],
            partial: true,
        },
        [
            [
                97,
                98,
                99,
                100,
            ],
            [
                97,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );

    assert_parse!(
        multi.parse_peek(Partial::new(f)),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi_longsep.parse_peek(Partial::new(g)),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(h)),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
}

#[test]
#[cfg(feature = "alloc")]
#[cfg_attr(debug_assertions, should_panic)]
fn separated0_empty_sep_test() {
    fn empty_sep<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, Vec<&'i [u8]>> {
        separated(0.., "abc", "").parse_next(i)
    }

    let i = &b"abcabc"[..];

    assert_parse!(
        empty_sep.parse_peek(Partial::new(i)),
        str![[r#"
Err(
    Cut(
        InputError {
            input: Partial {
                input: [
                    97,
                    98,
                    99,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
    );
}

#[test]
#[cfg(feature = "alloc")]
fn separated1_test() {
    fn multi<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, Vec<&'i [u8]>> {
        separated(1.., "abcd", ",").parse_next(i)
    }
    fn multi_longsep<'i>(
        i: &mut Partial<&'i [u8]>,
    ) -> TestResult<Partial<&'i [u8]>, Vec<&'i [u8]>> {
        separated(1.., "abcd", "..").parse_next(i)
    }

    let a = &b"abcdef"[..];
    let b = &b"abcd,abcdef"[..];
    let c = &b"azerty"[..];
    let d = &b"abcd,abcd,ef"[..];

    let f = &b"abc"[..];
    let g = &b"abcd."[..];
    let h = &b"abcd,abc"[..];

    assert_parse!(
        multi.parse_peek(Partial::new(a)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                101,
                102,
            ],
            partial: true,
        },
        [
            [
                97,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(b)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                101,
                102,
            ],
            partial: true,
        },
        [
            [
                97,
                98,
                99,
                100,
            ],
            [
                97,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(c)),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    97,
                    122,
                    101,
                    114,
                    116,
                    121,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(d)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                44,
                101,
                102,
            ],
            partial: true,
        },
        [
            [
                97,
                98,
                99,
                100,
            ],
            [
                97,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );

    assert_parse!(
        multi.parse_peek(Partial::new(f)),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi_longsep.parse_peek(Partial::new(g)),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(h)),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
}

#[test]
#[cfg(feature = "alloc")]
fn separated_test() {
    fn multi<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, Vec<&'i [u8]>> {
        separated(2..=4, "abcd", ",").parse_next(i)
    }

    let a = &b"abcd,ef"[..];
    let b = &b"abcd,abcd,efgh"[..];
    let c = &b"abcd,abcd,abcd,abcd,efgh"[..];
    let d = &b"abcd,abcd,abcd,abcd,abcd,efgh"[..];
    let e = &b"abcd,ab"[..];

    assert_parse!(
        multi.parse_peek(Partial::new(a)),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    101,
                    102,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(b)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                44,
                101,
                102,
                103,
                104,
            ],
            partial: true,
        },
        [
            [
                97,
                98,
                99,
                100,
            ],
            [
                97,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(c)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                44,
                101,
                102,
                103,
                104,
            ],
            partial: true,
        },
        [
            [
                97,
                98,
                99,
                100,
            ],
            [
                97,
                98,
                99,
                100,
            ],
            [
                97,
                98,
                99,
                100,
            ],
            [
                97,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(d)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                44,
                97,
                98,
                99,
                100,
                44,
                101,
                102,
                103,
                104,
            ],
            partial: true,
        },
        [
            [
                97,
                98,
                99,
                100,
            ],
            [
                97,
                98,
                99,
                100,
            ],
            [
                97,
                98,
                99,
                100,
            ],
            [
                97,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(e)),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
}

#[test]
#[cfg(feature = "alloc")]
fn repeat0_test() {
    fn multi<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, Vec<&'i [u8]>> {
        repeat(0.., "abcd").parse_next(i)
    }

    assert_parse!(
        multi.parse_peek(Partial::new(&b"abcdef"[..])),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                101,
                102,
            ],
            partial: true,
        },
        [
            [
                97,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(&b"abcdabcdefgh"[..])),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                101,
                102,
                103,
                104,
            ],
            partial: true,
        },
        [
            [
                97,
                98,
                99,
                100,
            ],
            [
                97,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(&b"azerty"[..])),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                97,
                122,
                101,
                114,
                116,
                121,
            ],
            partial: true,
        },
        [],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(&b"abcdab"[..])),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(&b"abcd"[..])),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(&b""[..])),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
}

#[test]
#[cfg(feature = "alloc")]
#[cfg_attr(debug_assertions, should_panic)]
fn repeat0_empty_test() {
    fn multi_empty<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, Vec<&'i [u8]>> {
        repeat(0.., "").parse_next(i)
    }

    assert_parse!(
        multi_empty.parse_peek(Partial::new(&b"abcdef"[..])),
        str![[r#"
Err(
    Cut(
        InputError {
            input: Partial {
                input: [
                    97,
                    98,
                    99,
                    100,
                    101,
                    102,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
    );
}

#[test]
#[cfg(feature = "alloc")]
fn repeat1_test() {
    fn multi<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, Vec<&'i [u8]>> {
        repeat(1.., "abcd").parse_next(i)
    }

    let a = &b"abcdef"[..];
    let b = &b"abcdabcdefgh"[..];
    let c = &b"azerty"[..];
    let d = &b"abcdab"[..];

    assert_parse!(
        multi.parse_peek(Partial::new(a)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                101,
                102,
            ],
            partial: true,
        },
        [
            [
                97,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(b)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                101,
                102,
                103,
                104,
            ],
            partial: true,
        },
        [
            [
                97,
                98,
                99,
                100,
            ],
            [
                97,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(c)),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    97,
                    122,
                    101,
                    114,
                    116,
                    121,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(d)),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
}

#[test]
#[cfg(feature = "alloc")]
fn repeat_till_test() {
    #[allow(clippy::type_complexity)]
    fn multi<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], (Vec<&'i [u8]>, &'i [u8])> {
        repeat_till(0.., "abcd", "efgh").parse_next(i)
    }

    let a = b"abcdabcdefghabcd";
    let b = b"efghabcd";
    let c = b"azerty";

    assert_parse!(
        multi.parse_peek(&a[..]),
        str![[r#"
Ok(
    (
        [
            97,
            98,
            99,
            100,
        ],
        (
            [
                [
                    97,
                    98,
                    99,
                    100,
                ],
                [
                    97,
                    98,
                    99,
                    100,
                ],
            ],
            [
                101,
                102,
                103,
                104,
            ],
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(&b[..]),
        str![[r#"
Ok(
    (
        [
            97,
            98,
            99,
            100,
        ],
        (
            [],
            [
                101,
                102,
                103,
                104,
            ],
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(&c[..]),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                97,
                122,
                101,
                114,
                116,
                121,
            ],
        },
    ),
)

"#]]
        .raw()
    );
}

#[test]
#[cfg(feature = "alloc")]
fn repeat_till_range_test() {
    #[allow(clippy::type_complexity)]
    fn multi<'i>(i: &mut &'i str) -> TestResult<&'i str, (Vec<&'i str>, &'i str)> {
        repeat_till(2..=4, "ab", "cd").parse_next(i)
    }

    assert_parse!(
        multi.parse_peek("cd"),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: "cd",
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek("abcd"),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: "cd",
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek("ababcd"),
        str![[r#"
Ok(
    (
        "",
        (
            [
                "ab",
                "ab",
            ],
            "cd",
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek("abababcd"),
        str![[r#"
Ok(
    (
        "",
        (
            [
                "ab",
                "ab",
                "ab",
            ],
            "cd",
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek("ababababcd"),
        str![[r#"
Ok(
    (
        "",
        (
            [
                "ab",
                "ab",
                "ab",
                "ab",
            ],
            "cd",
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek("abababababcd"),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: "abcd",
        },
    ),
)

"#]]
        .raw()
    );
}

#[test]
#[cfg(feature = "std")]
fn infinite_many() {
    fn tst<'i>(input: &mut &'i [u8]) -> TestResult<&'i [u8], &'i [u8]> {
        println!("input: {input:?}");
        Err(ParserError::from_input(input))
    }

    // should not go into an infinite loop
    fn multi0<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], Vec<&'i [u8]>> {
        repeat(0.., tst).parse_next(i)
    }
    let a = &b"abcdef"[..];
    assert_parse!(
        multi0.parse_peek(a),
        str![[r#"
Ok(
    (
        [
            97,
            98,
            99,
            100,
            101,
            102,
        ],
        [],
    ),
)

"#]]
        .raw()
    );

    fn multi1<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], Vec<&'i [u8]>> {
        repeat(1.., tst).parse_next(i)
    }
    let a = &b"abcdef"[..];
    assert_parse!(
        multi1.parse_peek(a),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                97,
                98,
                99,
                100,
                101,
                102,
            ],
        },
    ),
)

"#]]
        .raw()
    );
}

#[test]
#[cfg(feature = "alloc")]
fn repeat_test() {
    fn multi<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, Vec<&'i [u8]>> {
        repeat(2..=4, "Abcd").parse_next(i)
    }

    let a = &b"Abcdef"[..];
    let b = &b"AbcdAbcdefgh"[..];
    let c = &b"AbcdAbcdAbcdAbcdefgh"[..];
    let d = &b"AbcdAbcdAbcdAbcdAbcdefgh"[..];
    let e = &b"AbcdAb"[..];

    assert_parse!(
        multi.parse_peek(Partial::new(a)),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    101,
                    102,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(b)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                101,
                102,
                103,
                104,
            ],
            partial: true,
        },
        [
            [
                65,
                98,
                99,
                100,
            ],
            [
                65,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(c)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                101,
                102,
                103,
                104,
            ],
            partial: true,
        },
        [
            [
                65,
                98,
                99,
                100,
            ],
            [
                65,
                98,
                99,
                100,
            ],
            [
                65,
                98,
                99,
                100,
            ],
            [
                65,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(d)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                65,
                98,
                99,
                100,
                101,
                102,
                103,
                104,
            ],
            partial: true,
        },
        [
            [
                65,
                98,
                99,
                100,
            ],
            [
                65,
                98,
                99,
                100,
            ],
            [
                65,
                98,
                99,
                100,
            ],
            [
                65,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(e)),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
}

#[test]
#[cfg(feature = "alloc")]
fn count_test() {
    const TIMES: usize = 2;
    fn cnt_2<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, Vec<&'i [u8]>> {
        repeat(TIMES, "abc").parse_next(i)
    }

    assert_parse!(
        cnt_2.parse_peek(Partial::new(&b"abcabcabcdef"[..])),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                97,
                98,
                99,
                100,
                101,
                102,
            ],
            partial: true,
        },
        [
            [
                97,
                98,
                99,
            ],
            [
                97,
                98,
                99,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        cnt_2.parse_peek(Partial::new(&b"ab"[..])),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        cnt_2.parse_peek(Partial::new(&b"abcab"[..])),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        cnt_2.parse_peek(Partial::new(&b"xxx"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        cnt_2.parse_peek(Partial::new(&b"xxxabcabcdef"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                    97,
                    98,
                    99,
                    97,
                    98,
                    99,
                    100,
                    101,
                    102,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        cnt_2.parse_peek(Partial::new(&b"abcxxxabcdef"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    120,
                    120,
                    120,
                    97,
                    98,
                    99,
                    100,
                    101,
                    102,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
}

#[test]
#[cfg(feature = "alloc")]
fn count_zero() {
    const TIMES: usize = 0;
    fn counter_2<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], Vec<&'i [u8]>> {
        repeat(TIMES, "abc").parse_next(i)
    }

    let done = &b"abcabcabcdef"[..];
    let incomplete_1 = &b"ab"[..];
    let incomplete_2 = &b"abcab"[..];
    let error = &b"xxx"[..];
    let error_1 = &b"xxxabcabcdef"[..];
    let error_2 = &b"abcxxxabcdef"[..];

    assert_parse!(
        counter_2.parse_peek(done),
        str![[r#"
Ok(
    (
        [
            97,
            98,
            99,
            97,
            98,
            99,
            97,
            98,
            99,
            100,
            101,
            102,
        ],
        [],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        counter_2.parse_peek(incomplete_1),
        str![[r#"
Ok(
    (
        [
            97,
            98,
        ],
        [],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        counter_2.parse_peek(incomplete_2),
        str![[r#"
Ok(
    (
        [
            97,
            98,
            99,
            97,
            98,
        ],
        [],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        counter_2.parse_peek(error),
        str![[r#"
Ok(
    (
        [
            120,
            120,
            120,
        ],
        [],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        counter_2.parse_peek(error_1),
        str![[r#"
Ok(
    (
        [
            120,
            120,
            120,
            97,
            98,
            99,
            97,
            98,
            99,
            100,
            101,
            102,
        ],
        [],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        counter_2.parse_peek(error_2),
        str![[r#"
Ok(
    (
        [
            97,
            98,
            99,
            120,
            120,
            120,
            97,
            98,
            99,
            100,
            101,
            102,
        ],
        [],
    ),
)

"#]]
        .raw()
    );
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct NilError;

impl<I: Stream> ParserError<I> for NilError {
    type Inner = Self;

    fn from_input(_: &I) -> NilError {
        NilError
    }

    fn into_inner(self) -> Result<Self::Inner, Self> {
        Ok(self)
    }
}

#[test]
#[cfg(feature = "alloc")]
fn fold_repeat0_test() {
    fn fold_into_vec<T>(mut acc: Vec<T>, item: T) -> Vec<T> {
        acc.push(item);
        acc
    }
    fn multi<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, Vec<&'i [u8]>> {
        repeat(0.., "abcd")
            .fold(Vec::new, fold_into_vec)
            .parse_next(i)
    }

    assert_parse!(
        multi.parse_peek(Partial::new(&b"abcdef"[..])),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                101,
                102,
            ],
            partial: true,
        },
        [
            [
                97,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(&b"abcdabcdefgh"[..])),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                101,
                102,
                103,
                104,
            ],
            partial: true,
        },
        [
            [
                97,
                98,
                99,
                100,
            ],
            [
                97,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(&b"azerty"[..])),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                97,
                122,
                101,
                114,
                116,
                121,
            ],
            partial: true,
        },
        [],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(&b"abcdab"[..])),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(&b"abcd"[..])),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(&b""[..])),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
}

#[test]
#[cfg(feature = "alloc")]
#[cfg_attr(debug_assertions, should_panic)]
fn fold_repeat0_empty_test() {
    fn fold_into_vec<T>(mut acc: Vec<T>, item: T) -> Vec<T> {
        acc.push(item);
        acc
    }
    fn multi_empty<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, Vec<&'i [u8]>> {
        repeat(0.., "").fold(Vec::new, fold_into_vec).parse_next(i)
    }

    assert_parse!(
        multi_empty.parse_peek(Partial::new(&b"abcdef"[..])),
        str![[r#"
Err(
    Cut(
        InputError {
            input: Partial {
                input: [
                    97,
                    98,
                    99,
                    100,
                    101,
                    102,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
    );
}

#[test]
#[cfg(feature = "alloc")]
fn fold_repeat1_test() {
    fn fold_into_vec<T>(mut acc: Vec<T>, item: T) -> Vec<T> {
        acc.push(item);
        acc
    }
    fn multi<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, Vec<&'i [u8]>> {
        repeat(1.., "abcd")
            .fold(Vec::new, fold_into_vec)
            .parse_next(i)
    }

    let a = &b"abcdef"[..];
    let b = &b"abcdabcdefgh"[..];
    let c = &b"azerty"[..];
    let d = &b"abcdab"[..];

    assert_parse!(
        multi.parse_peek(Partial::new(a)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                101,
                102,
            ],
            partial: true,
        },
        [
            [
                97,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(b)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                101,
                102,
                103,
                104,
            ],
            partial: true,
        },
        [
            [
                97,
                98,
                99,
                100,
            ],
            [
                97,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(c)),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    97,
                    122,
                    101,
                    114,
                    116,
                    121,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(d)),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
}

#[test]
#[cfg(feature = "alloc")]
fn fold_repeat_test() {
    fn fold_into_vec<T>(mut acc: Vec<T>, item: T) -> Vec<T> {
        acc.push(item);
        acc
    }
    fn multi<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, Vec<&'i [u8]>> {
        repeat(2..=4, "Abcd")
            .fold(Vec::new, fold_into_vec)
            .parse_next(i)
    }

    let a = &b"Abcdef"[..];
    let b = &b"AbcdAbcdefgh"[..];
    let c = &b"AbcdAbcdAbcdAbcdefgh"[..];
    let d = &b"AbcdAbcdAbcdAbcdAbcdefgh"[..];
    let e = &b"AbcdAb"[..];

    assert_parse!(
        multi.parse_peek(Partial::new(a)),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    101,
                    102,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(b)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                101,
                102,
                103,
                104,
            ],
            partial: true,
        },
        [
            [
                65,
                98,
                99,
                100,
            ],
            [
                65,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(c)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                101,
                102,
                103,
                104,
            ],
            partial: true,
        },
        [
            [
                65,
                98,
                99,
                100,
            ],
            [
                65,
                98,
                99,
                100,
            ],
            [
                65,
                98,
                99,
                100,
            ],
            [
                65,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(d)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                65,
                98,
                99,
                100,
                101,
                102,
                103,
                104,
            ],
            partial: true,
        },
        [
            [
                65,
                98,
                99,
                100,
            ],
            [
                65,
                98,
                99,
                100,
            ],
            [
                65,
                98,
                99,
                100,
            ],
            [
                65,
                98,
                99,
                100,
            ],
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        multi.parse_peek(Partial::new(e)),
        str![[r#"
Err(
    Incomplete(
        Unknown,
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn repeat0_count_test() {
    fn count0_nums<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], usize> {
        repeat(0.., (digit, ",")).parse_next(i)
    }

    assert_parse!(
        count0_nums.parse_peek(&b"123,junk"[..]),
        str![[r#"
Ok(
    (
        [
            106,
            117,
            110,
            107,
        ],
        1,
    ),
)

"#]]
        .raw()
    );

    assert_parse!(
        count0_nums.parse_peek(&b"123,45,junk"[..]),
        str![[r#"
Ok(
    (
        [
            106,
            117,
            110,
            107,
        ],
        2,
    ),
)

"#]]
        .raw()
    );

    assert_parse!(
        count0_nums.parse_peek(&b"1,2,3,4,5,6,7,8,9,0,junk"[..]),
        str![[r#"
Ok(
    (
        [
            106,
            117,
            110,
            107,
        ],
        10,
    ),
)

"#]]
        .raw()
    );

    assert_parse!(
        count0_nums.parse_peek(&b"hello"[..]),
        str![[r#"
Ok(
    (
        [
            104,
            101,
            108,
            108,
            111,
        ],
        0,
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn repeat1_count_test() {
    fn count1_nums<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], usize> {
        repeat(1.., (digit, ",")).parse_next(i)
    }

    assert_parse!(
        count1_nums.parse_peek(&b"123,45,junk"[..]),
        str![[r#"
Ok(
    (
        [
            106,
            117,
            110,
            107,
        ],
        2,
    ),
)

"#]]
        .raw()
    );

    assert_parse!(
        count1_nums.parse_peek(&b"1,2,3,4,5,6,7,8,9,0,junk"[..]),
        str![[r#"
Ok(
    (
        [
            106,
            117,
            110,
            107,
        ],
        10,
    ),
)

"#]]
        .raw()
    );

    assert_parse!(
        count1_nums.parse_peek(&b"hello"[..]),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                104,
                101,
                108,
                108,
                111,
            ],
        },
    ),
)

"#]]
        .raw()
    );
}

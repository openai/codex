use super::*;

#[cfg(feature = "std")]
use proptest::prelude::*;
use snapbox::prelude::*;
use snapbox::str;

use crate::ascii::Caseless;
use crate::combinator::delimited;
use crate::error::ErrMode;
use crate::error::InputError;
use crate::prelude::*;
use crate::stream::AsChar;
use crate::token::literal;
use crate::Partial;

#[test]
fn complete_take_while_m_n_utf8_all_matching() {
    assert_parse!(
        take_while(1..=4, |c: char| c.is_alphabetic()).parse_peek("øn"),
        str![[r#"
Ok(
    (
        "",
        "øn",
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn complete_take_while_m_n_utf8_all_matching_substring() {
    assert_parse!(
        take_while(1, |c: char| c.is_alphabetic()).parse_peek("øn"),
        str![[r#"
Ok(
    (
        "n",
        "ø",
    ),
)

"#]]
        .raw()
    );
}

#[cfg(feature = "std")]
proptest! {
  #[test]
  #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
  fn complete_take_while_m_n_bounds(m in 0..20usize, n in 0..20usize, valid in 0..20usize, invalid in 0..20usize) {
      let input = format!("{:a<valid$}{:b<invalid$}", "", "", valid=valid, invalid=invalid);
      let mut model_input = input.as_str();
      let expected = model_complete_take_while_m_n(m, n, valid, &mut model_input);
      if m <= n {
          let actual = take_while(m..=n, |c: char| c == 'a').parse_peek(input.as_str());
          assert_eq!(expected.map(|o| (model_input, o)), actual);
      }
  }
}

#[cfg(feature = "std")]
fn model_complete_take_while_m_n<'i>(
    m: usize,
    n: usize,
    valid: usize,
    input: &mut &'i str,
) -> ModalResult<&'i str> {
    if n < m {
        Err(crate::error::ParserError::from_input(input))
    } else if m <= valid {
        let offset = n.min(valid);
        Ok(input.next_slice(offset))
    } else {
        Err(crate::error::ParserError::from_input(input))
    }
}

#[test]
fn complete_take_until() {
    fn take_until_5_10<'i>(i: &mut &'i str) -> TestResult<&'i str, &'i str> {
        take_until(5..=8, "end").parse_next(i)
    }
    assert_parse!(
        take_until_5_10.parse_peek("end"),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: "end",
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        take_until_5_10.parse_peek("1234end"),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: "1234end",
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        take_until_5_10.parse_peek("12345end"),
        str![[r#"
Ok(
    (
        "end",
        "12345",
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        take_until_5_10.parse_peek("123456end"),
        str![[r#"
Ok(
    (
        "end",
        "123456",
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        take_until_5_10.parse_peek("12345678end"),
        str![[r#"
Ok(
    (
        "end",
        "12345678",
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        take_until_5_10.parse_peek("123456789end"),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: "123456789end",
        },
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn complete_take_until_empty() {
    fn take_until_empty<'i>(i: &mut &'i str) -> TestResult<&'i str, &'i str> {
        take_until(0, "").parse_next(i)
    }
    assert_parse!(
        take_until_empty.parse_peek(""),
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
    assert_parse!(
        take_until_empty.parse_peek("end"),
        str![[r#"
Ok(
    (
        "end",
        "",
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn complete_literal_case_insensitive() {
    fn caseless_bytes<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], &'i [u8]> {
        literal(Caseless("ABcd")).parse_next(i)
    }
    assert_parse!(
        caseless_bytes.parse_peek(&b"aBCdefgh"[..]),
        str![[r#"
Ok(
    (
        [
            101,
            102,
            103,
            104,
        ],
        [
            97,
            66,
            67,
            100,
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        caseless_bytes.parse_peek(&b"abcdefgh"[..]),
        str![[r#"
Ok(
    (
        [
            101,
            102,
            103,
            104,
        ],
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
        caseless_bytes.parse_peek(&b"ABCDefgh"[..]),
        str![[r#"
Ok(
    (
        [
            101,
            102,
            103,
            104,
        ],
        [
            65,
            66,
            67,
            68,
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        caseless_bytes.parse_peek(&b"ab"[..]),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                97,
                98,
            ],
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        caseless_bytes.parse_peek(&b"Hello"[..]),
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
            ],
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        caseless_bytes.parse_peek(&b"Hel"[..]),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                72,
                101,
                108,
            ],
        },
    ),
)

"#]]
        .raw()
    );

    fn caseless_str<'i>(i: &mut &'i str) -> TestResult<&'i str, &'i str> {
        literal(Caseless("ABcd")).parse_next(i)
    }
    assert_parse!(
        caseless_str.parse_peek("aBCdefgh"),
        str![[r#"
Ok(
    (
        "efgh",
        "aBCd",
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        caseless_str.parse_peek("abcdefgh"),
        str![[r#"
Ok(
    (
        "efgh",
        "abcd",
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        caseless_str.parse_peek("ABCDefgh"),
        str![[r#"
Ok(
    (
        "efgh",
        "ABCD",
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        caseless_str.parse_peek("ab"),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: "ab",
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        caseless_str.parse_peek("Hello"),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: "Hello",
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        caseless_str.parse_peek("Hel"),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: "Hel",
        },
    ),
)

"#]]
        .raw()
    );

    fn matches_kelvin<'i>(i: &mut &'i str) -> TestResult<&'i str, &'i str> {
        literal(Caseless("k")).parse_next(i)
    }
    assert_parse!(
        matches_kelvin.parse_peek("K"),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: "K",
        },
    ),
)

"#]]
        .raw()
    );

    fn is_kelvin<'i>(i: &mut &'i str) -> TestResult<&'i str, &'i str> {
        literal(Caseless("K")).parse_next(i)
    }
    assert_parse!(
        is_kelvin.parse_peek("k"),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: "k",
        },
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn complete_literal_fixed_size_array() {
    fn test<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], &'i [u8]> {
        literal([0x42]).parse_next(i)
    }
    fn test2<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], &'i [u8]> {
        literal(&[0x42]).parse_next(i)
    }

    let input = &[0x42, 0x00][..];
    assert_parse!(
        test.parse_peek(input),
        str![[r#"
Ok(
    (
        [
            0,
        ],
        [
            66,
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        test2.parse_peek(input),
        str![[r#"
Ok(
    (
        [
            0,
        ],
        [
            66,
        ],
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn complete_literal_char() {
    fn test<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], &'i [u8]> {
        literal('B').parse_next(i)
    }
    assert_parse!(
        test.parse_peek(&[0x42, 0x00][..]),
        str![[r#"
Ok(
    (
        [
            0,
        ],
        [
            66,
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        test.parse_peek(&[b'A', b'\0'][..]),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                65,
                0,
            ],
        },
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn complete_literal_byte() {
    fn test<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], &'i [u8]> {
        literal(b'B').parse_next(i)
    }
    assert_parse!(
        test.parse_peek(&[0x42, 0x00][..]),
        str![[r#"
Ok(
    (
        [
            0,
        ],
        [
            66,
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        test.parse_peek(&[b'A', b'\0'][..]),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                65,
                0,
            ],
        },
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn partial_any_str() {
    use super::any;
    assert_parse!(
        any.parse_peek(Partial::new("Ә")),
        str![[r#"
Ok(
    (
        Partial {
            input: "",
            partial: true,
        },
        'Ә',
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn partial_one_of_test() {
    fn f<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, u8> {
        one_of(['a', 'b']).parse_next(i)
    }

    let a = &b"abcd"[..];
    assert_parse!(
        f.parse_peek(Partial::new(a)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                98,
                99,
                100,
            ],
            partial: true,
        },
        97,
    ),
)

"#]]
        .raw()
    );

    let b = &b"cde"[..];
    assert_parse!(
        f.parse_peek(Partial::new(b)),
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

    fn utf8<'i>(i: &mut Partial<&'i str>) -> TestResult<Partial<&'i str>, char> {
        one_of(['+', '\u{FF0B}']).parse_next(i)
    }

    assert!(utf8.parse_peek(Partial::new("+")).is_ok());
    assert!(utf8.parse_peek(Partial::new("\u{FF0B}")).is_ok());
}

#[test]
fn char_byteslice() {
    fn f<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, char> {
        'c'.parse_next(i)
    }

    let a = &b"abcd"[..];
    assert_parse!(
        f.parse_peek(Partial::new(a)),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    97,
                    98,
                    99,
                    100,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );

    let b = &b"cde"[..];
    assert_parse!(
        f.parse_peek(Partial::new(b)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                100,
                101,
            ],
            partial: true,
        },
        'c',
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn char_str() {
    fn f<'i>(i: &mut Partial<&'i str>) -> TestResult<Partial<&'i str>, char> {
        'c'.parse_next(i)
    }

    let a = "abcd";
    assert_parse!(
        f.parse_peek(Partial::new(a)),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: "abcd",
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );

    let b = "cde";
    assert_parse!(
        f.parse_peek(Partial::new(b)),
        str![[r#"
Ok(
    (
        Partial {
            input: "de",
            partial: true,
        },
        'c',
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn partial_none_of_test() {
    fn f<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, u8> {
        none_of(['a', 'b']).parse_next(i)
    }

    let a = &b"abcd"[..];
    assert_parse!(
        f.parse_peek(Partial::new(a)),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    97,
                    98,
                    99,
                    100,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );

    let b = &b"cde"[..];
    assert_parse!(
        f.parse_peek(Partial::new(b)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                100,
                101,
            ],
            partial: true,
        },
        99,
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn partial_is_a() {
    fn a_or_b<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        take_while(1.., ['a', 'b']).parse_next(i)
    }

    let a = Partial::new(&b"abcd"[..]);
    assert_parse!(
        a_or_b.parse_peek(a),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                99,
                100,
            ],
            partial: true,
        },
        [
            97,
            98,
        ],
    ),
)

"#]]
        .raw()
    );

    let b = Partial::new(&b"bcde"[..]);
    assert_parse!(
        a_or_b.parse_peek(b),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                99,
                100,
                101,
            ],
            partial: true,
        },
        [
            98,
        ],
    ),
)

"#]]
        .raw()
    );

    let c = Partial::new(&b"cdef"[..]);
    assert_parse!(
        a_or_b.parse_peek(c),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
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

    let d = Partial::new(&b"bacdef"[..]);
    assert_parse!(
        a_or_b.parse_peek(d),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                99,
                100,
                101,
                102,
            ],
            partial: true,
        },
        [
            98,
            97,
        ],
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn partial_is_not() {
    fn a_or_b<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        take_till(1.., ['a', 'b']).parse_next(i)
    }

    let a = Partial::new(&b"cdab"[..]);
    assert_parse!(
        a_or_b.parse_peek(a),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                97,
                98,
            ],
            partial: true,
        },
        [
            99,
            100,
        ],
    ),
)

"#]]
        .raw()
    );

    let b = Partial::new(&b"cbde"[..]);
    assert_parse!(
        a_or_b.parse_peek(b),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                98,
                100,
                101,
            ],
            partial: true,
        },
        [
            99,
        ],
    ),
)

"#]]
        .raw()
    );

    let c = Partial::new(&b"abab"[..]);
    assert_parse!(
        a_or_b.parse_peek(c),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    97,
                    98,
                    97,
                    98,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );

    let d = Partial::new(&b"cdefba"[..]);
    assert_parse!(
        a_or_b.parse_peek(d),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                98,
                97,
            ],
            partial: true,
        },
        [
            99,
            100,
            101,
            102,
        ],
    ),
)

"#]]
        .raw()
    );

    let e = Partial::new(&b"e"[..]);
    assert_parse!(
        a_or_b.parse_peek(e),
        str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn partial_take_until_incomplete() {
    fn y<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        take_until(0.., "end").parse_next(i)
    }
    assert_parse!(
        y.parse_peek(Partial::new(&b"nd"[..])),
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
        y.parse_peek(Partial::new(&b"123"[..])),
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
        y.parse_peek(Partial::new(&b"123en"[..])),
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
fn partial_take_until_incomplete_s() {
    fn ys<'i>(i: &mut Partial<&'i str>) -> TestResult<Partial<&'i str>, &'i str> {
        take_until(0.., "end").parse_next(i)
    }
    assert_parse!(
        ys.parse_peek(Partial::new("123en")),
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
fn partial_take() {
    use crate::ascii::{
        alpha1 as alpha, alphanumeric1 as alphanumeric, digit1 as digit, hex_digit1 as hex_digit,
        multispace1 as multispace, oct_digit1 as oct_digit, space1 as space,
    };

    fn x<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        delimited("<!--", take(5_usize), "-->").take().parse_next(i)
    }
    let r = x.parse_peek(Partial::new(&b"<!-- abc --> aaa"[..]));
    assert_parse!(
        r,
        str![[r#"
Ok(
    (
        Partial {
            input: [
                32,
                97,
                97,
                97,
            ],
            partial: true,
        },
        [
            60,
            33,
            45,
            45,
            32,
            97,
            98,
            99,
            32,
            45,
            45,
            62,
        ],
    ),
)

"#]]
        .raw()
    );

    fn ya<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        alpha.take().parse_next(i)
    }
    let ra = ya.parse_peek(Partial::new(&b"abc;"[..]));
    assert_parse!(
        ra,
        str![[r#"
Ok(
    (
        Partial {
            input: [
                59,
            ],
            partial: true,
        },
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

    fn yd<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        digit.take().parse_next(i)
    }
    let rd = yd.parse_peek(Partial::new(&b"123;"[..]));
    assert_parse!(
        rd,
        str![[r#"
Ok(
    (
        Partial {
            input: [
                59,
            ],
            partial: true,
        },
        [
            49,
            50,
            51,
        ],
    ),
)

"#]]
        .raw()
    );

    fn yhd<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        hex_digit.take().parse_next(i)
    }
    let rhd = yhd.parse_peek(Partial::new(&b"123abcDEF;"[..]));
    assert_parse!(
        rhd,
        str![[r#"
Ok(
    (
        Partial {
            input: [
                59,
            ],
            partial: true,
        },
        [
            49,
            50,
            51,
            97,
            98,
            99,
            68,
            69,
            70,
        ],
    ),
)

"#]]
        .raw()
    );

    fn yod<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        oct_digit.take().parse_next(i)
    }
    let rod = yod.parse_peek(Partial::new(&b"1234567;"[..]));
    assert_parse!(
        rod,
        str![[r#"
Ok(
    (
        Partial {
            input: [
                59,
            ],
            partial: true,
        },
        [
            49,
            50,
            51,
            52,
            53,
            54,
            55,
        ],
    ),
)

"#]]
        .raw()
    );

    fn yan<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        alphanumeric.take().parse_next(i)
    }
    let ran = yan.parse_peek(Partial::new(&b"123abc;"[..]));
    assert_parse!(
        ran,
        str![[r#"
Ok(
    (
        Partial {
            input: [
                59,
            ],
            partial: true,
        },
        [
            49,
            50,
            51,
            97,
            98,
            99,
        ],
    ),
)

"#]]
        .raw()
    );

    fn ys<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        space.take().parse_next(i)
    }
    let rs = ys.parse_peek(Partial::new(&b" \t;"[..]));
    assert_parse!(
        rs,
        str![[r#"
Ok(
    (
        Partial {
            input: [
                59,
            ],
            partial: true,
        },
        [
            32,
            9,
        ],
    ),
)

"#]]
        .raw()
    );

    fn yms<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        multispace.take().parse_next(i)
    }
    let rms = yms.parse_peek(Partial::new(&b" \t\r\n;"[..]));
    assert_parse!(
        rms,
        str![[r#"
Ok(
    (
        Partial {
            input: [
                59,
            ],
            partial: true,
        },
        [
            32,
            9,
            13,
            10,
        ],
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn partial_take_while0() {
    fn f<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        take_while(0.., AsChar::is_alpha).parse_next(i)
    }
    let a = &b""[..];
    let b = &b"abcd"[..];
    let c = &b"abcd123"[..];
    let d = &b"123"[..];

    assert_parse!(
        f.parse_peek(Partial::new(a)),
        str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        f.parse_peek(Partial::new(b)),
        str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        f.parse_peek(Partial::new(c)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                49,
                50,
                51,
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
        f.parse_peek(Partial::new(d)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                49,
                50,
                51,
            ],
            partial: true,
        },
        [],
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn partial_take_while1() {
    fn f<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        take_while(1.., AsChar::is_alpha).parse_next(i)
    }
    let a = &b""[..];
    let b = &b"abcd"[..];
    let c = &b"abcd123"[..];
    let d = &b"123"[..];

    assert_parse!(
        f.parse_peek(Partial::new(a)),
        str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        f.parse_peek(Partial::new(b)),
        str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        f.parse_peek(Partial::new(c)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                49,
                50,
                51,
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
        f.parse_peek(Partial::new(d)),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    49,
                    50,
                    51,
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
fn partial_take_while_m_n() {
    fn x<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        take_while(2..=4, AsChar::is_alpha).parse_next(i)
    }
    let a = &b""[..];
    let b = &b"a"[..];
    let c = &b"abc"[..];
    let d = &b"abc123"[..];
    let e = &b"abcde"[..];
    let f = &b"123"[..];

    assert_parse!(
        x.parse_peek(Partial::new(a)),
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
        x.parse_peek(Partial::new(b)),
        str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        x.parse_peek(Partial::new(c)),
        str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        x.parse_peek(Partial::new(d)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                49,
                50,
                51,
            ],
            partial: true,
        },
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
        x.parse_peek(Partial::new(e)),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                101,
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
        x.parse_peek(Partial::new(f)),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    49,
                    50,
                    51,
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
fn partial_take_till0() {
    fn f<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        take_till(0.., AsChar::is_alpha).parse_next(i)
    }
    let a = &b""[..];
    let b = &b"abcd"[..];
    let c = &b"123abcd"[..];
    let d = &b"123"[..];

    assert_parse!(
        f.parse_peek(Partial::new(a)),
        str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        f.parse_peek(Partial::new(b)),
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
        [],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        f.parse_peek(Partial::new(c)),
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
        [
            49,
            50,
            51,
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        f.parse_peek(Partial::new(d)),
        str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn partial_take_till1() {
    fn f<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        take_till(1.., AsChar::is_alpha).parse_next(i)
    }
    let a = &b""[..];
    let b = &b"abcd"[..];
    let c = &b"123abcd"[..];
    let d = &b"123"[..];

    assert_parse!(
        f.parse_peek(Partial::new(a)),
        str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        f.parse_peek(Partial::new(b)),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    97,
                    98,
                    99,
                    100,
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
        f.parse_peek(Partial::new(c)),
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
        [
            49,
            50,
            51,
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        f.parse_peek(Partial::new(d)),
        str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn partial_take_while_utf8() {
    fn f<'i>(i: &mut Partial<&'i str>) -> TestResult<Partial<&'i str>, &'i str> {
        take_while(0.., |c| c != '點').parse_next(i)
    }

    assert_parse!(
        f.parse_peek(Partial::new("")),
        str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        f.parse_peek(Partial::new("abcd")),
        str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        f.parse_peek(Partial::new("abcd點")),
        str![[r#"
Ok(
    (
        Partial {
            input: "點",
            partial: true,
        },
        "abcd",
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        f.parse_peek(Partial::new("abcd點a")),
        str![[r#"
Ok(
    (
        Partial {
            input: "點a",
            partial: true,
        },
        "abcd",
    ),
)

"#]]
        .raw()
    );

    fn g<'i>(i: &mut Partial<&'i str>) -> TestResult<Partial<&'i str>, &'i str> {
        take_while(0.., |c| c == '點').parse_next(i)
    }

    assert_parse!(
        g.parse_peek(Partial::new("")),
        str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        g.parse_peek(Partial::new("點abcd")),
        str![[r#"
Ok(
    (
        Partial {
            input: "abcd",
            partial: true,
        },
        "點",
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        g.parse_peek(Partial::new("點點點a")),
        str![[r#"
Ok(
    (
        Partial {
            input: "a",
            partial: true,
        },
        "點點點",
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn partial_take_till0_utf8() {
    fn f<'i>(i: &mut Partial<&'i str>) -> TestResult<Partial<&'i str>, &'i str> {
        take_till(0.., |c| c == '點').parse_next(i)
    }

    assert_parse!(
        f.parse_peek(Partial::new("")),
        str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        f.parse_peek(Partial::new("abcd")),
        str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        f.parse_peek(Partial::new("abcd點")),
        str![[r#"
Ok(
    (
        Partial {
            input: "點",
            partial: true,
        },
        "abcd",
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        f.parse_peek(Partial::new("abcd點a")),
        str![[r#"
Ok(
    (
        Partial {
            input: "點a",
            partial: true,
        },
        "abcd",
    ),
)

"#]]
        .raw()
    );

    fn g<'i>(i: &mut Partial<&'i str>) -> TestResult<Partial<&'i str>, &'i str> {
        take_till(0.., |c| c != '點').parse_next(i)
    }

    assert_parse!(
        g.parse_peek(Partial::new("")),
        str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        g.parse_peek(Partial::new("點abcd")),
        str![[r#"
Ok(
    (
        Partial {
            input: "abcd",
            partial: true,
        },
        "點",
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        g.parse_peek(Partial::new("點點點a")),
        str![[r#"
Ok(
    (
        Partial {
            input: "a",
            partial: true,
        },
        "點點點",
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn partial_take_utf8() {
    fn f<'i>(i: &mut Partial<&'i str>) -> TestResult<Partial<&'i str>, &'i str> {
        take(3_usize).parse_next(i)
    }

    assert_parse!(
        f.parse_peek(Partial::new("")),
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
        f.parse_peek(Partial::new("ab")),
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
        f.parse_peek(Partial::new("點")),
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
        f.parse_peek(Partial::new("ab點cd")),
        str![[r#"
Ok(
    (
        Partial {
            input: "cd",
            partial: true,
        },
        "ab點",
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        f.parse_peek(Partial::new("a點bcd")),
        str![[r#"
Ok(
    (
        Partial {
            input: "cd",
            partial: true,
        },
        "a點b",
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        f.parse_peek(Partial::new("a點b")),
        str![[r#"
Ok(
    (
        Partial {
            input: "",
            partial: true,
        },
        "a點b",
    ),
)

"#]]
        .raw()
    );

    fn g<'i>(i: &mut Partial<&'i str>) -> TestResult<Partial<&'i str>, &'i str> {
        take_while(0.., |c| c == '點').parse_next(i)
    }

    assert_parse!(
        g.parse_peek(Partial::new("")),
        str![[r#"
Err(
    Incomplete(
        Size(
            1,
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        g.parse_peek(Partial::new("點abcd")),
        str![[r#"
Ok(
    (
        Partial {
            input: "abcd",
            partial: true,
        },
        "點",
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        g.parse_peek(Partial::new("點點點a")),
        str![[r#"
Ok(
    (
        Partial {
            input: "a",
            partial: true,
        },
        "點點點",
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn partial_take_while_m_n_utf8_fixed() {
    fn parser<'i>(i: &mut Partial<&'i str>) -> TestResult<Partial<&'i str>, &'i str> {
        take_while(1, |c| c == 'A' || c == '😃').parse_next(i)
    }
    assert_parse!(
        parser.parse_peek(Partial::new("A!")),
        str![[r#"
Ok(
    (
        Partial {
            input: "!",
            partial: true,
        },
        "A",
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        parser.parse_peek(Partial::new("😃!")),
        str![[r#"
Ok(
    (
        Partial {
            input: "!",
            partial: true,
        },
        "😃",
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn partial_take_while_m_n_utf8_range() {
    fn parser<'i>(i: &mut Partial<&'i str>) -> TestResult<Partial<&'i str>, &'i str> {
        take_while(1..=2, |c| c == 'A' || c == '😃').parse_next(i)
    }
    assert_parse!(
        parser.parse_peek(Partial::new("A!")),
        str![[r#"
Ok(
    (
        Partial {
            input: "!",
            partial: true,
        },
        "A",
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        parser.parse_peek(Partial::new("😃!")),
        str![[r#"
Ok(
    (
        Partial {
            input: "!",
            partial: true,
        },
        "😃",
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn partial_take_while_m_n_utf8_full_match_fixed() {
    fn parser<'i>(i: &mut Partial<&'i str>) -> TestResult<Partial<&'i str>, &'i str> {
        take_while(1, |c: char| c.is_alphabetic()).parse_next(i)
    }
    assert_parse!(
        parser.parse_peek(Partial::new("øn")),
        str![[r#"
Ok(
    (
        Partial {
            input: "n",
            partial: true,
        },
        "ø",
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn partial_take_while_m_n_utf8_full_match_range() {
    fn parser<'i>(i: &mut Partial<&'i str>) -> TestResult<Partial<&'i str>, &'i str> {
        take_while(1..=2, |c: char| c.is_alphabetic()).parse_next(i)
    }
    assert_parse!(
        parser.parse_peek(Partial::new("øn")),
        str![[r#"
Ok(
    (
        Partial {
            input: "",
            partial: true,
        },
        "øn",
    ),
)

"#]]
        .raw()
    );
}

#[test]
#[cfg(feature = "std")]
fn partial_take_take_while0() {
    fn x<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        take_while(0.., AsChar::is_alphanum).parse_next(i)
    }
    fn y<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        x.take().parse_next(i)
    }
    assert_parse!(
        x.parse_peek(Partial::new(&b"ab."[..])),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                46,
            ],
            partial: true,
        },
        [
            97,
            98,
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        y.parse_peek(Partial::new(&b"ab."[..])),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                46,
            ],
            partial: true,
        },
        [
            97,
            98,
        ],
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn partial_literal_case_insensitive() {
    fn caseless_bytes<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        literal(Caseless("ABcd")).parse_next(i)
    }
    assert_parse!(
        caseless_bytes.parse_peek(Partial::new(&b"aBCdefgh"[..])),
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
            97,
            66,
            67,
            100,
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        caseless_bytes.parse_peek(Partial::new(&b"abcdefgh"[..])),
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
        caseless_bytes.parse_peek(Partial::new(&b"ABCDefgh"[..])),
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
            65,
            66,
            67,
            68,
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        caseless_bytes.parse_peek(Partial::new(&b"ab"[..])),
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
        caseless_bytes.parse_peek(Partial::new(&b"Hello"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    72,
                    101,
                    108,
                    108,
                    111,
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
        caseless_bytes.parse_peek(Partial::new(&b"Hel"[..])),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    72,
                    101,
                    108,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );

    fn caseless_str<'i>(i: &mut Partial<&'i str>) -> TestResult<Partial<&'i str>, &'i str> {
        literal(Caseless("ABcd")).parse_next(i)
    }
    assert_parse!(
        caseless_str.parse_peek(Partial::new("aBCdefgh")),
        str![[r#"
Ok(
    (
        Partial {
            input: "efgh",
            partial: true,
        },
        "aBCd",
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        caseless_str.parse_peek(Partial::new("abcdefgh")),
        str![[r#"
Ok(
    (
        Partial {
            input: "efgh",
            partial: true,
        },
        "abcd",
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        caseless_str.parse_peek(Partial::new("ABCDefgh")),
        str![[r#"
Ok(
    (
        Partial {
            input: "efgh",
            partial: true,
        },
        "ABCD",
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        caseless_str.parse_peek(Partial::new("ab")),
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
        caseless_str.parse_peek(Partial::new("Hello")),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: "Hello",
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        caseless_str.parse_peek(Partial::new("Hel")),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: "Hel",
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );

    fn matches_kelvin<'i>(i: &mut Partial<&'i str>) -> TestResult<Partial<&'i str>, &'i str> {
        literal(Caseless("k")).parse_next(i)
    }
    assert_parse!(
        matches_kelvin.parse_peek(Partial::new("K")),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: "K",
                partial: true,
            },
        },
    ),
)

"#]]
        .raw()
    );

    fn is_kelvin<'i>(i: &mut Partial<&'i str>) -> TestResult<Partial<&'i str>, &'i str> {
        literal(Caseless("K")).parse_next(i)
    }
    assert_parse!(
        is_kelvin.parse_peek(Partial::new("k")),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: "k",
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
fn partial_literal_fixed_size_array() {
    fn test<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        literal([0x42]).parse_next(i)
    }
    fn test2<'i>(i: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, &'i [u8]> {
        literal(&[0x42]).parse_next(i)
    }
    let input = Partial::new(&[0x42, 0x00][..]);
    assert_parse!(
        test.parse_peek(input),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                0,
            ],
            partial: true,
        },
        [
            66,
        ],
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        test2.parse_peek(input),
        str![[r#"
Ok(
    (
        Partial {
            input: [
                0,
            ],
            partial: true,
        },
        [
            66,
        ],
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn rest_on_slices() {
    let input: &[u8] = &b"Hello, world!"[..];
    assert_parse!(
        rest.parse_peek(input),
        str![[r#"
Ok(
    (
        [],
        [
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
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn rest_on_strs() {
    let input: &str = "Hello, world!";
    assert_parse!(
        rest.parse_peek(input),
        str![[r#"
Ok(
    (
        "",
        "Hello, world!",
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn rest_len_on_slices() {
    let input: &[u8] = &b"Hello, world!"[..];
    assert_parse!(
        rest_len.parse_peek(input),
        str![[r#"
Ok(
    (
        [
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
        13,
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn tokenslice_literals() {
    type TokenSlice<'i, 't> = crate::stream::TokenSlice<'i, Token<'t>>;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Token<'i> {
        kind: TokenKind,
        raw: &'i str,
    }

    impl PartialEq<&str> for Token<'_> {
        fn eq(&self, other: &&str) -> bool {
            self.raw == *other
        }
    }

    impl PartialEq<TokenKind> for Token<'_> {
        fn eq(&self, other: &TokenKind) -> bool {
            self.kind == *other
        }
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    enum TokenKind {
        If,
        LeftParen,
        RightParen,
        LeftCurly,
        RightCurly,
        Value,
    }

    impl<'i, 't> Parser<TokenSlice<'i, 't>, &'i Token<'t>, ErrMode<InputError<TokenSlice<'i, 't>>>>
        for TokenKind
    {
        fn parse_next(
            &mut self,
            input: &mut TokenSlice<'i, 't>,
        ) -> TestResult<TokenSlice<'i, 't>, &'i Token<'t>> {
            literal(*self).parse_next(input).map(|t| &t[0])
        }
    }

    let input = [
        Token {
            kind: TokenKind::If,
            raw: "if",
        },
        Token {
            kind: TokenKind::LeftParen,
            raw: "(",
        },
        Token {
            kind: TokenKind::Value,
            raw: "hello",
        },
        Token {
            kind: TokenKind::RightParen,
            raw: ")",
        },
        Token {
            kind: TokenKind::LeftCurly,
            raw: "{",
        },
        Token {
            kind: TokenKind::RightCurly,
            raw: "}",
        },
    ];
    let mut input = TokenSlice::new(&input);

    assert_parse!(
        (
            TokenKind::If,
            TokenKind::LeftParen,
            "hello",
            TokenKind::RightParen
        )
            .parse_next(&mut input),
        str![[r#"
Ok(
    (
        Token {
            kind: If,
            raw: "if",
        },
        Token {
            kind: LeftParen,
            raw: "(",
        },
        [
            Token {
                kind: Value,
                raw: "hello",
            },
        ],
        Token {
            kind: RightParen,
            raw: ")",
        },
    ),
)

"#]]
        .raw()
    );
}

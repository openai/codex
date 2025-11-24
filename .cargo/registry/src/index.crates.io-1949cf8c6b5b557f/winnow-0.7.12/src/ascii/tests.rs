use super::*;
use snapbox::prelude::*;
use snapbox::str;

mod complete {
    use super::*;

    use proptest::prelude::*;

    use crate::combinator::alt;
    use crate::error::ErrMode;
    use crate::error::InputError;
    use crate::prelude::*;
    use crate::stream::ParseSlice;
    use crate::token::none_of;
    use crate::token::one_of;
    #[cfg(feature = "alloc")]
    use crate::{lib::std::string::String, lib::std::vec::Vec};

    #[test]
    fn character() {
        let a: &[u8] = b"abcd";
        let b: &[u8] = b"1234";
        let c: &[u8] = b"a123";
        let d: &[u8] = "azé12".as_bytes();
        let e: &[u8] = b" ";
        let f: &[u8] = b" ;";
        //assert_parse!(alpha1::<_, InputError>(a), str![]);
        assert_parse!(
            alpha1.parse_peek(a),
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
            alpha1.parse_peek(b),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                49,
                50,
                51,
                52,
            ],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            alpha1.parse_peek(c),
            str![[r#"
Ok(
    (
        [
            49,
            50,
            51,
        ],
        [
            97,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            alpha1.parse_peek(d),
            str![[r#"
Ok(
    (
        [
            195,
            169,
            49,
            50,
        ],
        [
            97,
            122,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            digit1.parse_peek(a),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                97,
                98,
                99,
                100,
            ],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            digit1.parse_peek(b),
            str![[r#"
Ok(
    (
        [],
        [
            49,
            50,
            51,
            52,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            digit1.parse_peek(c),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                97,
                49,
                50,
                51,
            ],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            digit1.parse_peek(d),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                97,
                122,
                195,
                169,
                49,
                50,
            ],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_digit1.parse_peek(a),
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
            hex_digit1.parse_peek(b),
            str![[r#"
Ok(
    (
        [],
        [
            49,
            50,
            51,
            52,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_digit1.parse_peek(c),
            str![[r#"
Ok(
    (
        [],
        [
            97,
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
            hex_digit1.parse_peek(d),
            str![[r#"
Ok(
    (
        [
            122,
            195,
            169,
            49,
            50,
        ],
        [
            97,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_digit1.parse_peek(e),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                32,
            ],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            oct_digit1.parse_peek(a),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                97,
                98,
                99,
                100,
            ],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            oct_digit1.parse_peek(b),
            str![[r#"
Ok(
    (
        [],
        [
            49,
            50,
            51,
            52,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            oct_digit1.parse_peek(c),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                97,
                49,
                50,
                51,
            ],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            oct_digit1.parse_peek(d),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                97,
                122,
                195,
                169,
                49,
                50,
            ],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            alphanumeric1.parse_peek(a),
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
        //assert_parse!(fix_error!(b,(), alphanumeric), str![]);
        assert_parse!(
            alphanumeric1.parse_peek(c),
            str![[r#"
Ok(
    (
        [],
        [
            97,
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
            alphanumeric1.parse_peek(d),
            str![[r#"
Ok(
    (
        [
            195,
            169,
            49,
            50,
        ],
        [
            97,
            122,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            space1.parse_peek(e),
            str![[r#"
Ok(
    (
        [],
        [
            32,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            space1.parse_peek(f),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        [
            32,
        ],
    ),
)

"#]]
            .raw()
        );
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn character_s() {
        let a = "abcd";
        let b = "1234";
        let c = "a123";
        let d = "azé12";
        let e = " ";
        assert_parse!(
            alpha1.parse_peek(a),
            str![[r#"
Ok(
    (
        "",
        "abcd",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            alpha1.parse_peek(b),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: "1234",
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            alpha1.parse_peek(c),
            str![[r#"
Ok(
    (
        "123",
        "a",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            alpha1.parse_peek(d),
            str![[r#"
Ok(
    (
        "é12",
        "az",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            digit1.parse_peek(a),
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
        assert_parse!(
            digit1.parse_peek(b),
            str![[r#"
Ok(
    (
        "",
        "1234",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            digit1.parse_peek(c),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: "a123",
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            digit1.parse_peek(d),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: "azé12",
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_digit1.parse_peek(a),
            str![[r#"
Ok(
    (
        "",
        "abcd",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_digit1.parse_peek(b),
            str![[r#"
Ok(
    (
        "",
        "1234",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_digit1.parse_peek(c),
            str![[r#"
Ok(
    (
        "",
        "a123",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_digit1.parse_peek(d),
            str![[r#"
Ok(
    (
        "zé12",
        "a",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_digit1.parse_peek(e),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: " ",
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            oct_digit1.parse_peek(a),
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
        assert_parse!(
            oct_digit1.parse_peek(b),
            str![[r#"
Ok(
    (
        "",
        "1234",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            oct_digit1.parse_peek(c),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: "a123",
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            oct_digit1.parse_peek(d),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: "azé12",
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            alphanumeric1.parse_peek(a),
            str![[r#"
Ok(
    (
        "",
        "abcd",
    ),
)

"#]]
            .raw()
        );
        //assert_parse!(fix_error!(b,(), alphanumeric), str![]);
        assert_parse!(
            alphanumeric1.parse_peek(c),
            str![[r#"
Ok(
    (
        "",
        "a123",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            alphanumeric1.parse_peek(d),
            str![[r#"
Ok(
    (
        "é12",
        "az",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            space1.parse_peek(e),
            str![[r#"
Ok(
    (
        "",
        " ",
    ),
)

"#]]
            .raw()
        );
    }

    use crate::stream::Offset;
    #[test]
    fn offset() {
        let a = &b"abcd;"[..];
        let b = &b"1234;"[..];
        let c = &b"a123;"[..];
        let d = &b" \t;"[..];
        let e = &b" \t\r\n;"[..];
        let f = &b"123abcDEF;"[..];

        match alpha1::<_, InputError<_>>.parse_peek(a) {
            Ok((i, _)) => {
                assert_eq!(i.offset_from(&a) + i.len(), a.len());
            }
            _ => panic!("wrong return type in offset test for alpha"),
        }
        match digit1::<_, InputError<_>>.parse_peek(b) {
            Ok((i, _)) => {
                assert_eq!(i.offset_from(&b) + i.len(), b.len());
            }
            _ => panic!("wrong return type in offset test for digit"),
        }
        match alphanumeric1::<_, InputError<_>>.parse_peek(c) {
            Ok((i, _)) => {
                assert_eq!(i.offset_from(&c) + i.len(), c.len());
            }
            _ => panic!("wrong return type in offset test for alphanumeric"),
        }
        match space1::<_, InputError<_>>.parse_peek(d) {
            Ok((i, _)) => {
                assert_eq!(i.offset_from(&d) + i.len(), d.len());
            }
            _ => panic!("wrong return type in offset test for space"),
        }
        match multispace1::<_, InputError<_>>.parse_peek(e) {
            Ok((i, _)) => {
                assert_eq!(i.offset_from(&e) + i.len(), e.len());
            }
            _ => panic!("wrong return type in offset test for multispace"),
        }
        match hex_digit1::<_, InputError<_>>.parse_peek(f) {
            Ok((i, _)) => {
                assert_eq!(i.offset_from(&f) + i.len(), f.len());
            }
            _ => panic!("wrong return type in offset test for hex_digit"),
        }
        match oct_digit1::<_, InputError<_>>.parse_peek(f) {
            Ok((i, _)) => {
                assert_eq!(i.offset_from(&f) + i.len(), f.len());
            }
            _ => panic!("wrong return type in offset test for oct_digit"),
        }
    }

    #[test]
    fn is_till_line_ending_bytes() {
        let a: &[u8] = b"ab12cd\nefgh";
        assert_parse!(
            till_line_ending.parse_peek(a),
            str![[r#"
Ok(
    (
        [
            10,
            101,
            102,
            103,
            104,
        ],
        [
            97,
            98,
            49,
            50,
            99,
            100,
        ],
    ),
)

"#]]
            .raw()
        );

        let b: &[u8] = b"ab12cd\nefgh\nijkl";
        assert_parse!(
            till_line_ending.parse_peek(b),
            str![[r#"
Ok(
    (
        [
            10,
            101,
            102,
            103,
            104,
            10,
            105,
            106,
            107,
            108,
        ],
        [
            97,
            98,
            49,
            50,
            99,
            100,
        ],
    ),
)

"#]]
            .raw()
        );

        let c: &[u8] = b"ab12cd\r\nefgh\nijkl";
        assert_parse!(
            till_line_ending.parse_peek(c),
            str![[r#"
Ok(
    (
        [
            13,
            10,
            101,
            102,
            103,
            104,
            10,
            105,
            106,
            107,
            108,
        ],
        [
            97,
            98,
            49,
            50,
            99,
            100,
        ],
    ),
)

"#]]
            .raw()
        );

        let d: &[u8] = b"ab12cd";
        assert_parse!(
            till_line_ending.parse_peek(d),
            str![[r#"
Ok(
    (
        [],
        [
            97,
            98,
            49,
            50,
            99,
            100,
        ],
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn is_till_line_ending_str() {
        let f = "βèƒôřè\rÂßÇáƒƭèř";
        assert_parse!(
            till_line_ending.parse_peek(f),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: "\rÂßÇáƒƭèř",
        },
    ),
)

"#]]
            .raw()
        );

        let g2: &str = "ab12cd";
        assert_parse!(
            till_line_ending.parse_peek(g2),
            str![[r#"
Ok(
    (
        "",
        "ab12cd",
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn hex_digit_test() {
        let i = &b"0123456789abcdefABCDEF;"[..];
        assert_parse!(
            hex_digit1.parse_peek(i),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        [
            48,
            49,
            50,
            51,
            52,
            53,
            54,
            55,
            56,
            57,
            97,
            98,
            99,
            100,
            101,
            102,
            65,
            66,
            67,
            68,
            69,
            70,
        ],
    ),
)

"#]]
            .raw()
        );

        let i = &b"g"[..];
        assert_parse!(
            hex_digit1.parse_peek(i),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                103,
            ],
        },
    ),
)

"#]]
            .raw()
        );

        let i = &b"G"[..];
        assert_parse!(
            hex_digit1.parse_peek(i),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                71,
            ],
        },
    ),
)

"#]]
            .raw()
        );

        assert!(AsChar::is_hex_digit(b'0'));
        assert!(AsChar::is_hex_digit(b'9'));
        assert!(AsChar::is_hex_digit(b'a'));
        assert!(AsChar::is_hex_digit(b'f'));
        assert!(AsChar::is_hex_digit(b'A'));
        assert!(AsChar::is_hex_digit(b'F'));
        assert!(!AsChar::is_hex_digit(b'g'));
        assert!(!AsChar::is_hex_digit(b'G'));
        assert!(!AsChar::is_hex_digit(b'/'));
        assert!(!AsChar::is_hex_digit(b':'));
        assert!(!AsChar::is_hex_digit(b'@'));
        assert!(!AsChar::is_hex_digit(b'\x60'));
    }

    #[test]
    fn oct_digit_test() {
        let i = &b"01234567;"[..];
        assert_parse!(
            oct_digit1.parse_peek(i),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        [
            48,
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

        let i = &b"8"[..];
        assert_parse!(
            oct_digit1.parse_peek(i),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                56,
            ],
        },
    ),
)

"#]]
            .raw()
        );

        assert!(AsChar::is_oct_digit(b'0'));
        assert!(AsChar::is_oct_digit(b'7'));
        assert!(!AsChar::is_oct_digit(b'8'));
        assert!(!AsChar::is_oct_digit(b'9'));
        assert!(!AsChar::is_oct_digit(b'a'));
        assert!(!AsChar::is_oct_digit(b'A'));
        assert!(!AsChar::is_oct_digit(b'/'));
        assert!(!AsChar::is_oct_digit(b':'));
        assert!(!AsChar::is_oct_digit(b'@'));
        assert!(!AsChar::is_oct_digit(b'\x60'));
    }

    #[test]
    fn full_line_windows() {
        #[allow(clippy::type_complexity)]
        fn take_full_line<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], (&'i [u8], &'i [u8])> {
            (till_line_ending, line_ending).parse_next(i)
        }
        let input = b"abc\r\n";
        let output = take_full_line.parse_peek(input);
        assert_parse!(
            output,
            str![[r#"
Ok(
    (
        [],
        (
            [
                97,
                98,
                99,
            ],
            [
                13,
                10,
            ],
        ),
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn full_line_unix() {
        #[allow(clippy::type_complexity)]
        fn take_full_line<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], (&'i [u8], &'i [u8])> {
            (till_line_ending, line_ending).parse_next(i)
        }
        let input = b"abc\n";
        let output = take_full_line.parse_peek(input);
        assert_parse!(
            output,
            str![[r#"
Ok(
    (
        [],
        (
            [
                97,
                98,
                99,
            ],
            [
                10,
            ],
        ),
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn check_windows_lineending() {
        let input = b"\r\n";
        let output = line_ending.parse_peek(&input[..]);
        assert_parse!(
            output,
            str![[r#"
Ok(
    (
        [],
        [
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
    fn check_unix_lineending() {
        let input = b"\n";
        let output = line_ending.parse_peek(&input[..]);
        assert_parse!(
            output,
            str![[r#"
Ok(
    (
        [],
        [
            10,
        ],
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn cr_lf() {
        assert_parse!(
            crlf.parse_peek(&b"\r\na"[..]),
            str![[r#"
Ok(
    (
        [
            97,
        ],
        [
            13,
            10,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            crlf.parse_peek(&b"\r"[..]),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                13,
            ],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            crlf.parse_peek(&b"\ra"[..]),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                13,
                97,
            ],
        },
    ),
)

"#]]
            .raw()
        );

        assert_parse!(
            crlf.parse_peek("\r\na"),
            str![[r#"
Ok(
    (
        "a",
        "\r\n",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            crlf.parse_peek("\r"),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: "\r",
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            crlf.parse_peek("\ra"),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: "\ra",
        },
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn end_of_line() {
        assert_parse!(
            line_ending.parse_peek(&b"\na"[..]),
            str![[r#"
Ok(
    (
        [
            97,
        ],
        [
            10,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            line_ending.parse_peek(&b"\r\na"[..]),
            str![[r#"
Ok(
    (
        [
            97,
        ],
        [
            13,
            10,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            line_ending.parse_peek(&b"\r"[..]),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                13,
            ],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            line_ending.parse_peek(&b"\ra"[..]),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                13,
                97,
            ],
        },
    ),
)

"#]]
            .raw()
        );

        assert_parse!(
            line_ending.parse_peek("\na"),
            str![[r#"
Ok(
    (
        "a",
        "\n",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            line_ending.parse_peek("\r\na"),
            str![[r#"
Ok(
    (
        "a",
        "\r\n",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            line_ending.parse_peek("\r"),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: "\r",
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            line_ending.parse_peek("\ra"),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: "\ra",
        },
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn dec_uint_tests() {
        fn dec_u32<'i>(input: &mut &'i [u8]) -> TestResult<&'i [u8], u32> {
            dec_uint.parse_next(input)
        }

        assert_parse!(
            dec_u32.parse_peek(&b";"[..]),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                59,
            ],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            dec_u32.parse_peek(&b"0;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            dec_u32.parse_peek(&b"1;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            dec_u32.parse_peek(&b"32;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        32,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            dec_u32.parse_peek(&b"1000000000000000000000;"[..]), // overflow
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                49,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                59,
            ],
        },
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn dec_int_tests() {
        fn dec_i32<'i>(input: &mut &'i [u8]) -> TestResult<&'i [u8], i32> {
            dec_int.parse_next(input)
        }

        assert_parse!(
            dec_i32.parse_peek(&b";"[..]),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                59,
            ],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            dec_i32.parse_peek(&b"0;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            dec_i32.parse_peek(&b"1;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            dec_i32.parse_peek(&b"32;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        32,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            dec_i32.parse_peek(&b"-0;"[..]),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                45,
                48,
                59,
            ],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            dec_i32.parse_peek(&b"-1;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        -1,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            dec_i32.parse_peek(&b"-32;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        -32,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            dec_i32.parse_peek(&b"1000000000000000000000;"[..]), // overflow
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                49,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                48,
                59,
            ],
        },
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn hex_uint_tests() {
        fn hex_u32<'i>(input: &mut &'i [u8]) -> TestResult<&'i [u8], u32> {
            hex_uint.parse_next(input)
        }

        assert_parse!(
            hex_u32.parse_peek(&b";"[..]),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                59,
            ],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_u32.parse_peek(&b"ff;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        255,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_u32.parse_peek(&b"1be2;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        7138,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_u32.parse_peek(&b"c5a31be2;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        3315801058,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_u32.parse_peek(&b"C5A31be2;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        3315801058,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_u32.parse_peek(&b"00c5a31be2;"[..]), // overflow
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                48,
                48,
                99,
                53,
                97,
                51,
                49,
                98,
                101,
                50,
                59,
            ],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_u32.parse_peek(&b"c5a31be201;"[..]), // overflow
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                99,
                53,
                97,
                51,
                49,
                98,
                101,
                50,
                48,
                49,
                59,
            ],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_u32.parse_peek(&b"ffffffff;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        4294967295,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_u32.parse_peek(&b"ffffffffffffffff;"[..]), // overflow
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                102,
                102,
                102,
                102,
                102,
                102,
                102,
                102,
                102,
                102,
                102,
                102,
                102,
                102,
                102,
                102,
                59,
            ],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_u32.parse_peek(&b"ffffffffffffffff"[..]), // overflow
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                102,
                102,
                102,
                102,
                102,
                102,
                102,
                102,
                102,
                102,
                102,
                102,
                102,
                102,
                102,
                102,
            ],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_u32.parse_peek(&b"0x1be2;"[..]),
            str![[r#"
Ok(
    (
        [
            120,
            49,
            98,
            101,
            50,
            59,
        ],
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_u32.parse_peek(&b"12af"[..]),
            str![[r#"
Ok(
    (
        [],
        4783,
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    #[cfg(feature = "std")]
    fn float_test() {
        let test_cases = [
            "+3.14",
            "3.14",
            "-3.14",
            "0",
            "0.0",
            "1.",
            ".789",
            "-.5",
            "1e7",
            "-1E-7",
            ".3e-2",
            "1.e4",
            "1.2e4",
            "12.34",
            "-1.234E-12",
            "-1.234e-12",
            "0.00000000000000000087",
            "inf",
            "Inf",
            "infinity",
            "Infinity",
            "-inf",
            "-Inf",
            "-infinity",
            "-Infinity",
            "+inf",
            "+Inf",
            "+infinity",
            "+Infinity",
        ];

        for test in test_cases {
            let expected32 = str::parse::<f32>(test).unwrap();
            let expected64 = str::parse::<f64>(test).unwrap();

            println!("now parsing: {test} -> {expected32}");

            assert_eq!(
                float::<_, _, InputError<_>>.parse_peek(test.as_bytes()),
                Ok((&b""[..], expected32))
            );
            assert_eq!(
                float::<_, _, InputError<_>>.parse_peek(test),
                Ok(("", expected32))
            );

            assert_eq!(
                float::<_, _, InputError<_>>.parse_peek(test.as_bytes()),
                Ok((&b""[..], expected64))
            );
            assert_eq!(
                float::<_, _, InputError<_>>.parse_peek(test),
                Ok(("", expected64))
            );
        }

        let remaining_exponent = "-1.234E-";
        assert_parse!(
            float::<_, f64, _>.parse_peek(remaining_exponent),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: "",
        },
    ),
)

"#]]
            .raw()
        );

        let nan_test_cases = ["nan", "NaN", "NAN"];

        for test in nan_test_cases {
            println!("now parsing: {test}");

            let (remaining, parsed) = float::<_, f32, ()>.parse_peek(test.as_bytes()).unwrap();
            assert!(parsed.is_nan());
            assert!(remaining.is_empty());

            let (remaining, parsed) = float::<_, f32, ()>.parse_peek(test).unwrap();
            assert!(parsed.is_nan());
            assert!(remaining.is_empty());

            let (remaining, parsed) = float::<_, f64, ()>.parse_peek(test.as_bytes()).unwrap();
            assert!(parsed.is_nan());
            assert!(remaining.is_empty());

            let (remaining, parsed) = float::<_, f64, ()>.parse_peek(test).unwrap();
            assert!(parsed.is_nan());
            assert!(remaining.is_empty());
        }
    }

    proptest! {
      #[test]
      #[cfg(feature = "std")]
      #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
      fn floats(s in "\\PC*") {
          println!("testing {s}");
          let res1 = parse_f64.parse_peek(&s);
          let res2 = float::<_, f64, ErrMode<()>>.parse_peek(s.as_str());
          assert_eq!(res1, res2);
      }
    }

    #[cfg(feature = "std")]
    fn parse_f64(i: &mut &str) -> ModalResult<f64, ()> {
        match take_float_or_exceptions.parse_next(i) {
            Err(e) => Err(e),
            Ok(s) => {
                if s.is_empty() {
                    return Err(ErrMode::Backtrack(()));
                }
                match s.parse_slice() {
                    Some(n) => Ok(n),
                    None => Err(ErrMode::Backtrack(())),
                }
            }
        }
    }

    // issue #1336 "take_escaped hangs if normal parser accepts empty"
    #[test]
    #[cfg_attr(debug_assertions, should_panic)]
    fn complete_take_escaped_hang_1() {
        // issue #1336 "take_escaped hangs if normal parser accepts empty"
        fn escaped_string<'i>(input: &mut &'i str) -> TestResult<&'i str, &'i str> {
            use crate::ascii::alpha0;
            use crate::token::one_of;
            take_escaped(alpha0, '\\', one_of(['n'])).parse_next(input)
        }

        let input = "7";
        assert_parse!(
            escaped_string.parse_peek(input),
            str![[r#"
Err(
    Cut(
        InputError {
            input: "7",
        },
    ),
)

"#]]
        );
    }

    // issue #1336 "take_escaped hangs if normal parser accepts empty"
    #[test]
    #[cfg_attr(debug_assertions, should_panic)]
    fn complete_take_escaped_hang_2() {
        // issue #1336 "take_escaped hangs if normal parser accepts empty"
        fn escaped_string<'i>(input: &mut &'i str) -> TestResult<&'i str, &'i str> {
            use crate::ascii::alpha0;
            use crate::token::one_of;
            take_escaped(alpha0, '\\', one_of(['n'])).parse_next(input)
        }

        let input = "a7";
        assert_parse!(
            escaped_string.parse_peek(input),
            str![[r#"
Err(
    Cut(
        InputError {
            input: "7",
        },
    ),
)

"#]]
        );
    }

    #[test]
    #[cfg_attr(debug_assertions, should_panic)]
    fn complete_take_escaped_hang_1118() {
        // issue ##1118 take_escaped does not work with empty string
        fn unquote<'i>(input: &mut &'i str) -> TestResult<&'i str, &'i str> {
            use crate::combinator::delimited;
            use crate::combinator::opt;
            use crate::token::one_of;

            delimited(
                '"',
                take_escaped(
                    opt(none_of(['\\', '"'])),
                    '\\',
                    one_of(['\\', '"', 'r', 'n', 't']),
                ),
                '"',
            )
            .parse_next(input)
        }

        let input = r#""""#;
        assert_parse!(
            unquote.parse_peek(input),
            str![[r#"
Err(
    Cut(
        InputError {
            input: "/"",
        },
    ),
)

"#]]
        );
    }

    #[cfg(feature = "alloc")]
    #[allow(unused_variables)]
    #[test]
    fn complete_escaping() {
        use crate::ascii::{alpha1 as alpha, digit1 as digit};
        use crate::token::one_of;

        fn esc<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], &'i [u8]> {
            take_escaped(alpha, '\\', one_of(['\"', 'n', '\\'])).parse_next(i)
        }
        assert_parse!(
            esc.parse_peek(&b"abcd;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
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
            esc.parse_peek(&b"ab\\\"cd;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        [
            97,
            98,
            92,
            34,
            99,
            100,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek(&b"\\\"abcd;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        [
            92,
            34,
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
            esc.parse_peek(&b"\\n;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        [
            92,
            110,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek(&b"ab\\\"12"[..]),
            str![[r#"
Ok(
    (
        [
            49,
            50,
        ],
        [
            97,
            98,
            92,
            34,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek(&b"AB\\"[..]),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek(&b"AB\\A"[..]),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                65,
            ],
        },
    ),
)

"#]]
            .raw()
        );

        fn esc2<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], &'i [u8]> {
            take_escaped(digit, '\\', one_of(['\"', 'n', '\\'])).parse_next(i)
        }
        assert_parse!(
            esc2.parse_peek(&b"12\\nnn34"[..]),
            str![[r#"
Ok(
    (
        [
            110,
            110,
            51,
            52,
        ],
        [
            49,
            50,
            92,
            110,
        ],
    ),
)

"#]]
            .raw()
        );
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn complete_escaping_str() {
        use crate::ascii::{alpha1 as alpha, digit1 as digit};
        use crate::token::one_of;

        fn esc<'i>(i: &mut &'i str) -> TestResult<&'i str, &'i str> {
            take_escaped(alpha, '\\', one_of(['\"', 'n', '\\'])).parse_next(i)
        }
        assert_parse!(
            esc.parse_peek("abcd;"),
            str![[r#"
Ok(
    (
        ";",
        "abcd",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek("ab\\\"cd;"),
            str![[r#"
Ok(
    (
        ";",
        "ab\\\"cd",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek("\\\"abcd;"),
            str![[r#"
Ok(
    (
        ";",
        "\\\"abcd",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek("\\n;"),
            str![[r#"
Ok(
    (
        ";",
        "\\n",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek("ab\\\"12"),
            str![[r#"
Ok(
    (
        "12",
        "ab\\\"",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek("AB\\"),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: "",
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek("AB\\A"),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: "A",
        },
    ),
)

"#]]
            .raw()
        );

        fn esc2<'i>(i: &mut &'i str) -> TestResult<&'i str, &'i str> {
            take_escaped(digit, '\\', one_of(['\"', 'n', '\\'])).parse_next(i)
        }
        assert_parse!(
            esc2.parse_peek("12\\nnn34"),
            str![[r#"
Ok(
    (
        "nn34",
        "12\\n",
    ),
)

"#]]
            .raw()
        );

        fn esc3<'i>(i: &mut &'i str) -> TestResult<&'i str, &'i str> {
            take_escaped(alpha, '\u{241b}', one_of(['\"', 'n'])).parse_next(i)
        }
        assert_parse!(
            esc3.parse_peek("ab␛ncd;"),
            str![[r#"
Ok(
    (
        ";",
        "ab␛ncd",
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn test_take_escaped_error() {
        fn esc<'i>(i: &mut &'i str) -> TestResult<&'i str, &'i str> {
            use crate::ascii::digit1;
            take_escaped(digit1, '\\', one_of(['\"', 'n', '\\'])).parse_next(i)
        }

        assert_parse!(
            esc.parse_peek("abcd"),
            str![[r#"
Ok(
    (
        "abcd",
        "",
    ),
)

"#]]
            .raw()
        );
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn complete_escape_transform() {
        use crate::ascii::alpha1 as alpha;

        #[cfg(feature = "alloc")]
        fn to_s(i: Vec<u8>) -> String {
            String::from_utf8_lossy(&i).into_owned()
        }

        fn esc<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], String> {
            escaped(alpha, '\\', alt((b'\\', b'"', "n".value(b'\n'))))
                .map(to_s)
                .parse_next(i)
        }

        assert_parse!(
            esc.parse_peek(&b"abcd;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        "abcd",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek(&b"ab\\\"cd;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        "ab\"cd",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek(&b"\\\"abcd;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        "\"abcd",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek(&b"\\n;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        "\n",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek(&b"ab\\\"12"[..]),
            str![[r#"
Ok(
    (
        [
            49,
            50,
        ],
        "ab\"",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek(&b"AB\\"[..]),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [],
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek(&b"AB\\A"[..]),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: [
                65,
            ],
        },
    ),
)

"#]]
            .raw()
        );

        fn esc2<'i>(i: &mut &'i [u8]) -> TestResult<&'i [u8], String> {
            escaped(
                alpha,
                '&',
                alt((
                    "egrave;".value("è".as_bytes()),
                    "agrave;".value("à".as_bytes()),
                )),
            )
            .map(to_s)
            .parse_next(i)
        }
        assert_parse!(
            esc2.parse_peek(&b"ab&egrave;DEF;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        "abèDEF",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc2.parse_peek(&b"ab&egrave;D&agrave;EF;"[..]),
            str![[r#"
Ok(
    (
        [
            59,
        ],
        "abèDàEF",
    ),
)

"#]]
            .raw()
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn complete_escape_transform_str() {
        use crate::ascii::alpha1 as alpha;

        fn esc<'i>(i: &mut &'i str) -> TestResult<&'i str, String> {
            escaped(
                alpha,
                '\\',
                alt((
                    '\\',
                    '"',
                    "n".value('\n'),
                    ("x", hex_uint).map(|(_, hex)| char::from_u32(hex).unwrap()),
                )),
            )
            .parse_next(i)
        }

        assert_parse!(
            esc.parse_peek("abcd;"),
            str![[r#"
Ok(
    (
        ";",
        "abcd",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek("ab\\\"cd;"),
            str![[r#"
Ok(
    (
        ";",
        "ab\"cd",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek("\\\"abcd;"),
            str![[r#"
Ok(
    (
        ";",
        "\"abcd",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek("\\n;"),
            str![[r#"
Ok(
    (
        ";",
        "\n",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek("ab\\\"12"),
            str![[r#"
Ok(
    (
        "12",
        "ab\"",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek("ab\\x20"),
            str![[r#"
Ok(
    (
        "",
        "ab ",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek("AB\\"),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: "",
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc.parse_peek("AB\\A"),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: "A",
        },
    ),
)

"#]]
            .raw()
        );

        fn esc2<'i>(i: &mut &'i str) -> TestResult<&'i str, String> {
            escaped(
                alpha,
                '&',
                alt(("egrave;".value("è"), "agrave;".value("à"))),
            )
            .parse_next(i)
        }
        assert_parse!(
            esc2.parse_peek("ab&egrave;DEF;"),
            str![[r#"
Ok(
    (
        ";",
        "abèDEF",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            esc2.parse_peek("ab&egrave;D&agrave;EF;"),
            str![[r#"
Ok(
    (
        ";",
        "abèDàEF",
    ),
)

"#]]
            .raw()
        );

        fn esc3<'i>(i: &mut &'i str) -> TestResult<&'i str, String> {
            escaped(alpha, '␛', alt(("0".value("\0"), "n".value("\n")))).parse_next(i)
        }
        assert_parse!(
            esc3.parse_peek("a␛0bc␛n"),
            str![[r#"
Ok(
    (
        "",
        "a\0bc\n",
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_escaped_error() {
        fn esc_trans<'i>(i: &mut &'i str) -> TestResult<&'i str, String> {
            use crate::ascii::digit1;
            escaped(digit1, '\\', "n").parse_next(i)
        }

        assert_parse!(
            esc_trans.parse_peek("abcd"),
            str![[r#"
Ok(
    (
        "abcd",
        "",
    ),
)

"#]]
            .raw()
        );
    }
}

mod partial {
    use super::*;

    use crate::error::InputError;
    use crate::prelude::*;
    use crate::Partial;

    #[test]
    fn character() {
        let a: &[u8] = b"abcd";
        let b: &[u8] = b"1234";
        let c: &[u8] = b"a123";
        let d: &[u8] = "azé12".as_bytes();
        let e: &[u8] = b" ";
        let f: &[u8] = b" ;";
        //assert_parse!(alpha1::<_, Error<_>>(a), Err(ErrMode::Incomplete(Needed::new(1))));
        assert_parse!(
            alpha1.parse_peek(Partial::new(a)),
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
            alpha1.parse_peek(Partial::new(b)),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    49,
                    50,
                    51,
                    52,
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
            alpha1.parse_peek(Partial::new(c)),
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
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            alpha1.parse_peek(Partial::new(d)),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                195,
                169,
                49,
                50,
            ],
            partial: true,
        },
        [
            97,
            122,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            digit1.parse_peek(Partial::new(a)),
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
            digit1.parse_peek(Partial::new(b)),
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
            digit1.parse_peek(Partial::new(c)),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    97,
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
        assert_parse!(
            digit1.parse_peek(Partial::new(d)),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    97,
                    122,
                    195,
                    169,
                    49,
                    50,
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
            hex_digit1.parse_peek(Partial::new(a)),
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
            hex_digit1.parse_peek(Partial::new(b)),
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
            hex_digit1.parse_peek(Partial::new(c)),
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
            hex_digit1.parse_peek(Partial::new(d)),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                122,
                195,
                169,
                49,
                50,
            ],
            partial: true,
        },
        [
            97,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_digit1.parse_peek(Partial::new(e)),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    32,
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
            oct_digit1.parse_peek(Partial::new(a)),
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
            oct_digit1.parse_peek(Partial::new(b)),
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
            oct_digit1.parse_peek(Partial::new(c)),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    97,
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
        assert_parse!(
            oct_digit1.parse_peek(Partial::new(d)),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    97,
                    122,
                    195,
                    169,
                    49,
                    50,
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
            alphanumeric1.parse_peek(Partial::new(a)),
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
        //assert_parse!(fix_error!(b,(), alphanumeric1), str![]);
        assert_parse!(
            alphanumeric1.parse_peek(Partial::new(c)),
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
            alphanumeric1.parse_peek(Partial::new(d)),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                195,
                169,
                49,
                50,
            ],
            partial: true,
        },
        [
            97,
            122,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            space1.parse_peek(Partial::new(e)),
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
            space1.parse_peek(Partial::new(f)),
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
        ],
    ),
)

"#]]
            .raw()
        );
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn character_s() {
        let a = "abcd";
        let b = "1234";
        let c = "a123";
        let d = "azé12";
        let e = " ";
        assert_parse!(
            alpha1.parse_peek(Partial::new(a)),
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
            alpha1.parse_peek(Partial::new(b)),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: "1234",
                partial: true,
            },
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            alpha1.parse_peek(Partial::new(c)),
            str![[r#"
Ok(
    (
        Partial {
            input: "123",
            partial: true,
        },
        "a",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            alpha1.parse_peek(Partial::new(d)),
            str![[r#"
Ok(
    (
        Partial {
            input: "é12",
            partial: true,
        },
        "az",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            digit1.parse_peek(Partial::new(a)),
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
        assert_parse!(
            digit1.parse_peek(Partial::new(b)),
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
            digit1.parse_peek(Partial::new(c)),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: "a123",
                partial: true,
            },
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            digit1.parse_peek(Partial::new(d)),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: "azé12",
                partial: true,
            },
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_digit1.parse_peek(Partial::new(a)),
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
            hex_digit1.parse_peek(Partial::new(b)),
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
            hex_digit1.parse_peek(Partial::new(c)),
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
            hex_digit1.parse_peek(Partial::new(d)),
            str![[r#"
Ok(
    (
        Partial {
            input: "zé12",
            partial: true,
        },
        "a",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_digit1.parse_peek(Partial::new(e)),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: " ",
                partial: true,
            },
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            oct_digit1.parse_peek(Partial::new(a)),
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
        assert_parse!(
            oct_digit1.parse_peek(Partial::new(b)),
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
            oct_digit1.parse_peek(Partial::new(c)),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: "a123",
                partial: true,
            },
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            oct_digit1.parse_peek(Partial::new(d)),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: "azé12",
                partial: true,
            },
        },
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            alphanumeric1.parse_peek(Partial::new(a)),
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
        //assert_parse!(fix_error!(b,(), alphanumeric1), str![]);
        assert_parse!(
            alphanumeric1.parse_peek(Partial::new(c)),
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
            alphanumeric1.parse_peek(Partial::new(d)),
            str![[r#"
Ok(
    (
        Partial {
            input: "é12",
            partial: true,
        },
        "az",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            space1.parse_peek(Partial::new(e)),
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

    use crate::stream::Offset;
    #[test]
    fn offset() {
        let a = &b"abcd;"[..];
        let b = &b"1234;"[..];
        let c = &b"a123;"[..];
        let d = &b" \t;"[..];
        let e = &b" \t\r\n;"[..];
        let f = &b"123abcDEF;"[..];

        match alpha1::<_, InputError<_>>.parse_peek(Partial::new(a)) {
            Ok((i, _)) => {
                let i = i.into_inner();
                assert_eq!(i.offset_from(&a) + i.len(), a.len());
            }
            _ => panic!("wrong return type in offset test for alpha"),
        }
        match digit1::<_, InputError<_>>.parse_peek(Partial::new(b)) {
            Ok((i, _)) => {
                let i = i.into_inner();
                assert_eq!(i.offset_from(&b) + i.len(), b.len());
            }
            _ => panic!("wrong return type in offset test for digit"),
        }
        match alphanumeric1::<_, InputError<_>>.parse_peek(Partial::new(c)) {
            Ok((i, _)) => {
                let i = i.into_inner();
                assert_eq!(i.offset_from(&c) + i.len(), c.len());
            }
            _ => panic!("wrong return type in offset test for alphanumeric"),
        }
        match space1::<_, InputError<_>>.parse_peek(Partial::new(d)) {
            Ok((i, _)) => {
                let i = i.into_inner();
                assert_eq!(i.offset_from(&d) + i.len(), d.len());
            }
            _ => panic!("wrong return type in offset test for space"),
        }
        match multispace1::<_, InputError<_>>.parse_peek(Partial::new(e)) {
            Ok((i, _)) => {
                let i = i.into_inner();
                assert_eq!(i.offset_from(&e) + i.len(), e.len());
            }
            _ => panic!("wrong return type in offset test for multispace"),
        }
        match hex_digit1::<_, InputError<_>>.parse_peek(Partial::new(f)) {
            Ok((i, _)) => {
                let i = i.into_inner();
                assert_eq!(i.offset_from(&f) + i.len(), f.len());
            }
            _ => panic!("wrong return type in offset test for hex_digit"),
        }
        match oct_digit1::<_, InputError<_>>.parse_peek(Partial::new(f)) {
            Ok((i, _)) => {
                let i = i.into_inner();
                assert_eq!(i.offset_from(&f) + i.len(), f.len());
            }
            _ => panic!("wrong return type in offset test for oct_digit"),
        }
    }

    #[test]
    fn is_till_line_ending_bytes() {
        let a: &[u8] = b"ab12cd\nefgh";
        assert_parse!(
            till_line_ending.parse_peek(Partial::new(a)),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                10,
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
            49,
            50,
            99,
            100,
        ],
    ),
)

"#]]
            .raw()
        );

        let b: &[u8] = b"ab12cd\nefgh\nijkl";
        assert_parse!(
            till_line_ending.parse_peek(Partial::new(b)),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                10,
                101,
                102,
                103,
                104,
                10,
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
            49,
            50,
            99,
            100,
        ],
    ),
)

"#]]
            .raw()
        );

        let c: &[u8] = b"ab12cd\r\nefgh\nijkl";
        assert_parse!(
            till_line_ending.parse_peek(Partial::new(c)),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                13,
                10,
                101,
                102,
                103,
                104,
                10,
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
            49,
            50,
            99,
            100,
        ],
    ),
)

"#]]
            .raw()
        );

        let d: &[u8] = b"ab12cd";
        assert_parse!(
            till_line_ending.parse_peek(Partial::new(d)),
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
    fn is_till_line_ending_str() {
        let f = "βèƒôřè\rÂßÇáƒƭèř";
        assert_parse!(
            till_line_ending.parse_peek(Partial::new(f)),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: "\rÂßÇáƒƭèř",
                partial: true,
            },
        },
    ),
)

"#]]
            .raw()
        );

        let g2: &str = "ab12cd";
        assert_parse!(
            till_line_ending.parse_peek(Partial::new(g2)),
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
    fn hex_digit_test() {
        let i = &b"0123456789abcdefABCDEF;"[..];
        assert_parse!(
            hex_digit1.parse_peek(Partial::new(i)),
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
            48,
            49,
            50,
            51,
            52,
            53,
            54,
            55,
            56,
            57,
            97,
            98,
            99,
            100,
            101,
            102,
            65,
            66,
            67,
            68,
            69,
            70,
        ],
    ),
)

"#]]
            .raw()
        );

        let i = &b"g"[..];
        assert_parse!(
            hex_digit1.parse_peek(Partial::new(i)),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
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

        let i = &b"G"[..];
        assert_parse!(
            hex_digit1.parse_peek(Partial::new(i)),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    71,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
            .raw()
        );

        assert!(AsChar::is_hex_digit(b'0'));
        assert!(AsChar::is_hex_digit(b'9'));
        assert!(AsChar::is_hex_digit(b'a'));
        assert!(AsChar::is_hex_digit(b'f'));
        assert!(AsChar::is_hex_digit(b'A'));
        assert!(AsChar::is_hex_digit(b'F'));
        assert!(!AsChar::is_hex_digit(b'g'));
        assert!(!AsChar::is_hex_digit(b'G'));
        assert!(!AsChar::is_hex_digit(b'/'));
        assert!(!AsChar::is_hex_digit(b':'));
        assert!(!AsChar::is_hex_digit(b'@'));
        assert!(!AsChar::is_hex_digit(b'\x60'));
    }

    #[test]
    fn oct_digit_test() {
        let i = &b"01234567;"[..];
        assert_parse!(
            oct_digit1.parse_peek(Partial::new(i)),
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
            48,
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

        let i = &b"8"[..];
        assert_parse!(
            oct_digit1.parse_peek(Partial::new(i)),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    56,
                ],
                partial: true,
            },
        },
    ),
)

"#]]
            .raw()
        );

        assert!(AsChar::is_oct_digit(b'0'));
        assert!(AsChar::is_oct_digit(b'7'));
        assert!(!AsChar::is_oct_digit(b'8'));
        assert!(!AsChar::is_oct_digit(b'9'));
        assert!(!AsChar::is_oct_digit(b'a'));
        assert!(!AsChar::is_oct_digit(b'A'));
        assert!(!AsChar::is_oct_digit(b'/'));
        assert!(!AsChar::is_oct_digit(b':'));
        assert!(!AsChar::is_oct_digit(b'@'));
        assert!(!AsChar::is_oct_digit(b'\x60'));
    }

    #[test]
    fn full_line_windows() {
        #[allow(clippy::type_complexity)]
        fn take_full_line<'i>(
            i: &mut Partial<&'i [u8]>,
        ) -> TestResult<Partial<&'i [u8]>, (&'i [u8], &'i [u8])> {
            (till_line_ending, line_ending).parse_next(i)
        }
        let input = b"abc\r\n";
        let output = take_full_line.parse_peek(Partial::new(input));
        assert_parse!(
            output,
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        (
            [
                97,
                98,
                99,
            ],
            [
                13,
                10,
            ],
        ),
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn full_line_unix() {
        #[allow(clippy::type_complexity)]
        fn take_full_line<'i>(
            i: &mut Partial<&'i [u8]>,
        ) -> TestResult<Partial<&'i [u8]>, (&'i [u8], &'i [u8])> {
            (till_line_ending, line_ending).parse_next(i)
        }
        let input = b"abc\n";
        let output = take_full_line.parse_peek(Partial::new(input));
        assert_parse!(
            output,
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        (
            [
                97,
                98,
                99,
            ],
            [
                10,
            ],
        ),
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn check_windows_lineending() {
        let input = b"\r\n";
        let output = line_ending.parse_peek(Partial::new(&input[..]));
        assert_parse!(
            output,
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        [
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
    fn check_unix_lineending() {
        let input = b"\n";
        let output = line_ending.parse_peek(Partial::new(&input[..]));
        assert_parse!(
            output,
            str![[r#"
Ok(
    (
        Partial {
            input: [],
            partial: true,
        },
        [
            10,
        ],
    ),
)

"#]]
            .raw()
        );
    }

    #[test]
    fn cr_lf() {
        assert_parse!(
            crlf.parse_peek(Partial::new(&b"\r\na"[..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                97,
            ],
            partial: true,
        },
        [
            13,
            10,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            crlf.parse_peek(Partial::new(&b"\r"[..])),
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
            crlf.parse_peek(Partial::new(&b"\ra"[..])),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    13,
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
            crlf.parse_peek(Partial::new("\r\na")),
            str![[r#"
Ok(
    (
        Partial {
            input: "a",
            partial: true,
        },
        "\r\n",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            crlf.parse_peek(Partial::new("\r")),
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
            crlf.parse_peek(Partial::new("\ra")),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: "\ra",
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
    fn end_of_line() {
        assert_parse!(
            line_ending.parse_peek(Partial::new(&b"\na"[..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                97,
            ],
            partial: true,
        },
        [
            10,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            line_ending.parse_peek(Partial::new(&b"\r\na"[..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                97,
            ],
            partial: true,
        },
        [
            13,
            10,
        ],
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            line_ending.parse_peek(Partial::new(&b"\r"[..])),
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
            line_ending.parse_peek(Partial::new(&b"\ra"[..])),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    13,
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
            line_ending.parse_peek(Partial::new("\na")),
            str![[r#"
Ok(
    (
        Partial {
            input: "a",
            partial: true,
        },
        "\n",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            line_ending.parse_peek(Partial::new("\r\na")),
            str![[r#"
Ok(
    (
        Partial {
            input: "a",
            partial: true,
        },
        "\r\n",
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            line_ending.parse_peek(Partial::new("\r")),
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
            line_ending.parse_peek(Partial::new("\ra")),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: "\ra",
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
    fn hex_uint_tests() {
        fn hex_u32<'i>(input: &mut Partial<&'i [u8]>) -> TestResult<Partial<&'i [u8]>, u32> {
            hex_uint.parse_next(input)
        }

        assert_parse!(
            hex_u32.parse_peek(Partial::new(&b";"[..])),
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    59,
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
            hex_u32.parse_peek(Partial::new(&b"ff;"[..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                59,
            ],
            partial: true,
        },
        255,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_u32.parse_peek(Partial::new(&b"1be2;"[..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                59,
            ],
            partial: true,
        },
        7138,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_u32.parse_peek(Partial::new(&b"c5a31be2;"[..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                59,
            ],
            partial: true,
        },
        3315801058,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_u32.parse_peek(Partial::new(&b"C5A31be2;"[..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                59,
            ],
            partial: true,
        },
        3315801058,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_u32.parse_peek(Partial::new(&b"00c5a31be2;"[..])), // overflow
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    48,
                    48,
                    99,
                    53,
                    97,
                    51,
                    49,
                    98,
                    101,
                    50,
                    59,
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
            hex_u32.parse_peek(Partial::new(&b"c5a31be201;"[..])), // overflow
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    99,
                    53,
                    97,
                    51,
                    49,
                    98,
                    101,
                    50,
                    48,
                    49,
                    59,
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
            hex_u32.parse_peek(Partial::new(&b"ffffffff;"[..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                59,
            ],
            partial: true,
        },
        4294967295,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_u32.parse_peek(Partial::new(&b"ffffffffffffffff;"[..])), // overflow
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    59,
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
            hex_u32.parse_peek(Partial::new(&b"ffffffffffffffff"[..])), // overflow
            str![[r#"
Err(
    Backtrack(
        InputError {
            input: Partial {
                input: [
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
                    102,
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
            hex_u32.parse_peek(Partial::new(&b"0x1be2;"[..])),
            str![[r#"
Ok(
    (
        Partial {
            input: [
                120,
                49,
                98,
                101,
                50,
                59,
            ],
            partial: true,
        },
        0,
    ),
)

"#]]
            .raw()
        );
        assert_parse!(
            hex_u32.parse_peek(Partial::new(&b"12af"[..])),
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
}

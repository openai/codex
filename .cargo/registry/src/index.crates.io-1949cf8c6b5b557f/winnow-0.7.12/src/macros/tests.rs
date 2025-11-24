use snapbox::prelude::*;
use snapbox::str;

use crate::ascii::dec_uint;
use crate::ascii::digit0;
use crate::combinator::dispatch;
use crate::combinator::empty;
use crate::combinator::fail;
use crate::combinator::seq;
use crate::prelude::*;
use crate::token::any;

#[test]
fn dispatch_basics() {
    fn escape_seq_char<'i>(input: &mut &'i str) -> TestResult<&'i str, char> {
        dispatch! {any;
            'b' => empty.value('\u{8}'),
            'f' => empty.value('\u{c}'),
            'n' => empty.value('\n'),
            'r' => empty.value('\r'),
            't' => empty.value('\t'),
            '\\' => empty.value('\\'),
            '"' => empty.value('"'),
            _ => fail::<_, char, _>,
        }
        .parse_next(input)
    }
    assert_parse!(
        escape_seq_char.parse_peek("b123"),
        str![[r#"
Ok(
    (
        "123",
        '\u{8}',
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        escape_seq_char.parse_peek("error"),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: "rror",
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        escape_seq_char.parse_peek(""),
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
}

#[test]
fn seq_struct_basics() {
    #[derive(Debug, PartialEq)]
    struct Point {
        x: u32,
        y: u32,
    }

    fn parser<'i>(input: &mut &'i str) -> TestResult<&'i str, Point> {
        seq! {
            Point {
                x: dec_uint,
                _: ',',
                y: dec_uint,
            }
        }
        .parse_next(input)
    }
    assert_parse!(
        parser.parse_peek("123,4 remaining"),
        str![[r#"
Ok(
    (
        " remaining",
        Point {
            x: 123,
            y: 4,
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        parser.parse_peek("123, remaining"),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: " remaining",
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        parser.parse_peek(""),
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
}

#[test]
fn seq_struct_default_init() {
    #[derive(Debug, PartialEq, Default)]
    struct Point {
        x: u32,
        y: u32,
        z: u32,
    }

    fn parser<'i>(input: &mut &'i str) -> TestResult<&'i str, Point> {
        seq! {
            Point {
                x: dec_uint,
                _: ',',
                y: dec_uint,
                ..Default::default()
            }
        }
        .parse_next(input)
    }
    assert_parse!(
        parser.parse_peek("123,4 remaining"),
        str![[r#"
Ok(
    (
        " remaining",
        Point {
            x: 123,
            y: 4,
            z: 0,
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        parser.parse_peek("123, remaining"),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: " remaining",
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        parser.parse_peek(""),
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
}

#[test]
fn seq_struct_trailing_comma_elided() {
    #![allow(dead_code)]

    #[derive(Debug, PartialEq)]
    struct Point {
        x: u32,
        y: u32,
    }

    fn parser<'i>(input: &mut &'i str) -> TestResult<&'i str, Point> {
        seq! {
            Point {
                x: dec_uint,
                _: ',',
                y: dec_uint,
                _: empty,
            }
        }
        .parse_next(input)
    }
}

#[test]
fn seq_struct_no_trailing_comma() {
    #![allow(dead_code)]

    #[derive(Debug, PartialEq)]
    struct Point {
        x: u32,
        y: u32,
    }

    fn parser<'i>(input: &mut &'i str) -> TestResult<&'i str, Point> {
        seq! {
            Point {
                x: dec_uint,
                _: ',',
                y: dec_uint
            }
        }
        .parse_next(input)
    }
}

#[test]
fn seq_struct_no_trailing_comma_elided() {
    #![allow(dead_code)]

    #[derive(Debug, PartialEq)]
    struct Point {
        x: u32,
        y: u32,
    }

    fn parser<'i>(input: &mut &'i str) -> TestResult<&'i str, Point> {
        seq! {
            Point {
                x: dec_uint,
                _: ',',
                y: dec_uint,
                _: empty
            }
        }
        .parse_next(input)
    }
}

#[test]
fn seq_enum_struct_variant() {
    #[derive(Debug, PartialEq, Eq)]
    enum Expr {
        Add { lhs: u32, rhs: u32 },
        Mul(u32, u32),
    }

    fn add<'i>(input: &mut &'i [u8]) -> TestResult<&'i [u8], Expr> {
        seq! {Expr::Add {
            lhs: dec_uint::<_, u32, _>,
            _: b" + ",
            rhs: dec_uint::<_, u32, _>,
        }}
        .parse_next(input)
    }

    fn mul<'i>(input: &mut &'i [u8]) -> TestResult<&'i [u8], Expr> {
        seq!(Expr::Mul(
             dec_uint::<_, u32, _>,
             _: b" * ",
             dec_uint::<_, u32, _>,
        ))
        .parse_next(input)
    }

    assert_parse!(
        add.parse_peek(&b"1 + 2"[..]),
        str![[r#"
Ok(
    (
        [],
        Add {
            lhs: 1,
            rhs: 2,
        },
    ),
)

"#]]
        .raw()
    );

    assert_parse!(
        mul.parse_peek(&b"3 * 4"[..]),
        str![[r#"
Ok(
    (
        [],
        Mul(
            3,
            4,
        ),
    ),
)

"#]]
        .raw()
    );
}

#[test]
fn seq_struct_borrow() {
    #![allow(dead_code)]

    #[derive(Debug, PartialEq)]
    struct Point {
        x: u32,
        y: u32,
    }

    fn parser<'i>(input: &mut &'i str) -> TestResult<&'i str, Point> {
        let mut dec_uint = digit0.parse_to();
        seq! {
            Point {
                x: dec_uint,
                _: ',',
                y: dec_uint,
                _: empty
            }
        }
        .parse_next(input)
    }
}

#[test]
fn seq_tuple_struct_basics() {
    #[derive(Debug, PartialEq)]
    struct Point(u32, u32);

    fn parser<'i>(input: &mut &'i str) -> TestResult<&'i str, Point> {
        seq! {
            Point(
                dec_uint,
                _: ',',
                dec_uint,
            )
        }
        .parse_next(input)
    }
    assert_parse!(
        parser.parse_peek("123,4 remaining"),
        str![[r#"
Ok(
    (
        " remaining",
        Point(
            123,
            4,
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        parser.parse_peek("123, remaining"),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: " remaining",
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        parser.parse_peek(""),
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
}

#[test]
fn seq_tuple_struct_trailing_comma_elided() {
    #![allow(dead_code)]

    #[derive(Debug, PartialEq)]
    struct Point(u32, u32);

    fn parser<'i>(input: &mut &'i str) -> TestResult<&'i str, Point> {
        seq! {
            Point(
                dec_uint,
                _: ',',
                dec_uint,
                _: empty,
            )
        }
        .parse_next(input)
    }
}

#[test]
fn seq_tuple_struct_no_trailing_comma() {
    #![allow(dead_code)]

    #[derive(Debug, PartialEq)]
    struct Point(u32, u32);

    fn parser<'i>(input: &mut &'i str) -> TestResult<&'i str, Point> {
        seq! {
            Point(
                dec_uint,
                _: ',',
                dec_uint
            )
        }
        .parse_next(input)
    }
}

#[test]
fn seq_tuple_struct_no_trailing_comma_elided() {
    #![allow(dead_code)]

    #[derive(Debug, PartialEq)]
    struct Point(u32, u32);

    fn parser<'i>(input: &mut &'i str) -> TestResult<&'i str, Point> {
        seq! {
            Point(
                dec_uint,
                _: ',',
                dec_uint,
                _: empty
            )
        }
        .parse_next(input)
    }
}

#[test]
fn seq_tuple_basics() {
    fn parser<'i>(input: &mut &'i str) -> TestResult<&'i str, (u32, u32)> {
        seq! {
            (
                dec_uint,
                _: ',',
                dec_uint,
            )
        }
        .parse_next(input)
    }
    assert_parse!(
        parser.parse_peek("123,4 remaining"),
        str![[r#"
Ok(
    (
        " remaining",
        (
            123,
            4,
        ),
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        parser.parse_peek("123, remaining"),
        str![[r#"
Err(
    Backtrack(
        InputError {
            input: " remaining",
        },
    ),
)

"#]]
        .raw()
    );
    assert_parse!(
        parser.parse_peek(""),
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
}

#[test]
fn seq_tuple_trailing_comma_elided() {
    #![allow(dead_code)]

    fn parser<'i>(input: &mut &'i str) -> TestResult<&'i str, (u32, u32)> {
        seq! {
            (
                dec_uint,
                _: ',',
                dec_uint,
                _: empty,
            )
        }
        .parse_next(input)
    }
}

#[test]
fn seq_tuple_no_trailing_comma() {
    #![allow(dead_code)]

    fn parser<'i>(input: &mut &'i str) -> TestResult<&'i str, (u32, u32)> {
        seq! {
            (
                dec_uint,
                _: ',',
                dec_uint
            )
        }
        .parse_next(input)
    }
}

#[test]
fn seq_tuple_no_trailing_comma_elided() {
    #![allow(dead_code)]

    fn parser<'i>(input: &mut &'i str) -> TestResult<&'i str, (u32, u32)> {
        seq! {
            (
                dec_uint,
                _: ',',
                dec_uint,
                _: empty
            )
        }
        .parse_next(input)
    }
}

#[test]
fn seq_tuple_no_parens() {
    #![allow(dead_code)]

    fn parser<'i>(input: &mut &'i str) -> TestResult<&'i str, (u32, u32)> {
        seq! (
            dec_uint,
            _: ',',
            dec_uint,
        )
        .parse_next(input)
    }
}

#[test]
fn seq_tuple_borrow() {
    #![allow(dead_code)]

    #[derive(Debug, PartialEq)]
    struct Point(u32, u32);

    fn parser<'i>(input: &mut &'i str) -> TestResult<&'i str, Point> {
        let mut dec_uint = digit0.parse_to();
        seq! {
            Point(
                dec_uint,
                _: ',',
                dec_uint,
                _: empty
            )
        }
        .parse_next(input)
    }
}

use super::*;

use snapbox::assert_data_eq;
use snapbox::prelude::*;
use snapbox::str;

#[test]
fn test_lex_ascii_char() {
    let cases = [(
        ".trailing",
        str![[r#"
Token {
    kind: Dot,
    span: 0..1,
}

"#]]
        .raw(),
        str!["trailing"].raw(),
    )];
    for (stream, expected_tokens, expected_stream) in cases {
        dbg!(stream);
        let mut stream = Stream::new(stream);
        let actual_tokens = lex_ascii_char(&mut stream, TokenKind::Dot);
        assert_data_eq!(actual_tokens.to_debug(), expected_tokens.raw());
        let stream = *stream;
        assert_data_eq!(stream, expected_stream.raw());
    }
}

#[test]
fn test_lex_whitespace() {
    let cases = [
        (
            " ",
            str![[r#"
Token {
    kind: Whitespace,
    span: 0..1,
}

"#]]
            .raw(),
            str![].raw(),
        ),
        (
            " \t  \t  \t ",
            str![[r#"
Token {
    kind: Whitespace,
    span: 0..9,
}

"#]]
            .raw(),
            str![].raw(),
        ),
        (
            " \n",
            str![[r#"
Token {
    kind: Whitespace,
    span: 0..1,
}

"#]]
            .raw(),
            str![[r#"


"#]]
            .raw(),
        ),
        (
            " #",
            str![[r#"
Token {
    kind: Whitespace,
    span: 0..1,
}

"#]]
            .raw(),
            str!["#"].raw(),
        ),
        (
            " a",
            str![[r#"
Token {
    kind: Whitespace,
    span: 0..1,
}

"#]]
            .raw(),
            str!["a"].raw(),
        ),
    ];
    for (stream, expected_tokens, expected_stream) in cases {
        dbg!(stream);
        let mut stream = Stream::new(stream);
        let actual_tokens = lex_whitespace(&mut stream);
        assert_data_eq!(actual_tokens.to_debug(), expected_tokens.raw());
        let stream = *stream;
        assert_data_eq!(stream, expected_stream.raw());
    }
}

#[test]
fn test_lex_comment() {
    let cases = [
        (
            "#",
            str![[r#"
Token {
    kind: Comment,
    span: 0..1,
}

"#]]
            .raw(),
            str![""].raw(),
        ),
        (
            "# content",
            str![[r#"
Token {
    kind: Comment,
    span: 0..9,
}

"#]]
            .raw(),
            str![""].raw(),
        ),
        (
            "# content \ntrailing",
            str![[r#"
Token {
    kind: Comment,
    span: 0..10,
}

"#]]
            .raw(),
            str![[r#"

trailing
"#]]
            .raw(),
        ),
        (
            "# content \r\ntrailing",
            str![[r#"
Token {
    kind: Comment,
    span: 0..10,
}

"#]]
            .raw(),
            str![[r#"

trailing
"#]]
            .raw(),
        ),
        (
            "# content \0continue",
            str![[r#"
Token {
    kind: Comment,
    span: 0..19,
}

"#]]
            .raw(),
            str![""].raw(),
        ),
    ];
    for (stream, expected_tokens, expected_stream) in cases {
        dbg!(stream);
        let mut stream = Stream::new(stream);
        let actual_tokens = lex_comment(&mut stream);
        assert_data_eq!(actual_tokens.to_debug(), expected_tokens.raw());
        let stream = *stream;
        assert_data_eq!(stream, expected_stream.raw());
    }
}

#[test]
fn test_lex_crlf() {
    let cases = [
        (
            "\r\ntrailing",
            str![[r#"
Token {
    kind: Newline,
    span: 0..2,
}

"#]]
            .raw(),
            str!["trailing"].raw(),
        ),
        (
            "\rtrailing",
            str![[r#"
Token {
    kind: Newline,
    span: 0..1,
}

"#]]
            .raw(),
            str!["trailing"].raw(),
        ),
    ];
    for (stream, expected_tokens, expected_stream) in cases {
        dbg!(stream);
        let mut stream = Stream::new(stream);
        let actual_tokens = lex_crlf(&mut stream);
        assert_data_eq!(actual_tokens.to_debug(), expected_tokens.raw());
        let stream = *stream;
        assert_data_eq!(stream, expected_stream.raw());
    }
}

#[test]
fn test_lex_literal_string() {
    let cases = [
        (
            "''",
            str![[r#"
Token {
    kind: LiteralString,
    span: 0..2,
}

"#]]
            .raw(),
            str![""].raw(),
        ),
        (
            "''trailing",
            str![[r#"
Token {
    kind: LiteralString,
    span: 0..2,
}

"#]]
            .raw(),
            str!["trailing"].raw(),
        ),
        (
            "'content'trailing",
            str![[r#"
Token {
    kind: LiteralString,
    span: 0..9,
}

"#]]
            .raw(),
            str!["trailing"].raw(),
        ),
        (
            "'content",
            str![[r#"
Token {
    kind: LiteralString,
    span: 0..8,
}

"#]]
            .raw(),
            str![""].raw(),
        ),
        (
            "'content\ntrailing",
            str![[r#"
Token {
    kind: LiteralString,
    span: 0..8,
}

"#]]
            .raw(),
            str![[r#"

trailing
"#]]
            .raw(),
        ),
    ];
    for (stream, expected_tokens, expected_stream) in cases {
        dbg!(stream);
        let mut stream = Stream::new(stream);
        let actual_tokens = lex_literal_string(&mut stream);
        assert_data_eq!(actual_tokens.to_debug(), expected_tokens.raw());
        let stream = *stream;
        assert_data_eq!(stream, expected_stream.raw());
    }
}

#[test]
fn test_lex_ml_literal_string() {
    let cases = [
        (
            "''''''",
            str![[r#"
Token {
    kind: MlLiteralString,
    span: 0..6,
}

"#]]
            .raw(),
            str![""].raw(),
        ),
        (
            "''''''trailing",
            str![[r#"
Token {
    kind: MlLiteralString,
    span: 0..6,
}

"#]]
            .raw(),
            str!["trailing"].raw(),
        ),
        (
            "'''content'''trailing",
            str![[r#"
Token {
    kind: MlLiteralString,
    span: 0..13,
}

"#]]
            .raw(),
            str!["trailing"].raw(),
        ),
        (
            "'''content",
            str![[r#"
Token {
    kind: MlLiteralString,
    span: 0..10,
}

"#]]
            .raw(),
            str![""].raw(),
        ),
        (
            "'''content'",
            str![[r#"
Token {
    kind: MlLiteralString,
    span: 0..11,
}

"#]]
            .raw(),
            str![""].raw(),
        ),
        (
            "'''content''",
            str![[r#"
Token {
    kind: MlLiteralString,
    span: 0..12,
}

"#]]
            .raw(),
            str![""].raw(),
        ),
        (
            "'''content\ntrailing",
            str![[r#"
Token {
    kind: MlLiteralString,
    span: 0..19,
}

"#]]
            .raw(),
            str![""].raw(),
        ),
        (
            "'''''''trailing",
            str![[r#"
Token {
    kind: MlLiteralString,
    span: 0..7,
}

"#]]
            .raw(),
            str!["trailing"].raw(),
        ),
        (
            "''''''''trailing",
            str![[r#"
Token {
    kind: MlLiteralString,
    span: 0..8,
}

"#]]
            .raw(),
            str!["trailing"].raw(),
        ),
        (
            "'''''''''trailing",
            str![[r#"
Token {
    kind: MlLiteralString,
    span: 0..8,
}

"#]]
            .raw(),
            str!["'trailing"].raw(),
        ),
        (
            "'''''content''''trailing",
            str![[r#"
Token {
    kind: MlLiteralString,
    span: 0..16,
}

"#]]
            .raw(),
            str!["trailing"].raw(),
        ),
        (
            "'''''content'''''trailing",
            str![[r#"
Token {
    kind: MlLiteralString,
    span: 0..17,
}

"#]]
            .raw(),
            str!["trailing"].raw(),
        ),
        (
            "'''''content''''''trailing",
            str![[r#"
Token {
    kind: MlLiteralString,
    span: 0..17,
}

"#]]
            .raw(),
            str!["'trailing"].raw(),
        ),
    ];
    for (stream, expected_tokens, expected_stream) in cases {
        dbg!(stream);
        let mut stream = Stream::new(stream);
        let actual_tokens = lex_ml_literal_string(&mut stream);
        assert_data_eq!(actual_tokens.to_debug(), expected_tokens.raw());
        let stream = *stream;
        assert_data_eq!(stream, expected_stream.raw());
    }
}

#[test]
fn test_lex_basic_string() {
    let cases = [
        (
            r#""""#,
            str![[r#"
Token {
    kind: BasicString,
    span: 0..2,
}

"#]]
            .raw(),
            str![].raw(),
        ),
        (
            r#"""trailing"#,
            str![[r#"
Token {
    kind: BasicString,
    span: 0..2,
}

"#]]
            .raw(),
            str!["trailing"].raw(),
        ),
        (
            r#""content"trailing"#,
            str![[r#"
Token {
    kind: BasicString,
    span: 0..9,
}

"#]]
            .raw(),
            str!["trailing"].raw(),
        ),
        (
            r#""content"#,
            str![[r#"
Token {
    kind: BasicString,
    span: 0..8,
}

"#]]
            .raw(),
            str![].raw(),
        ),
        (
            r#""content\ntrailing"#,
            str![[r#"
Token {
    kind: BasicString,
    span: 0..18,
}

"#]]
            .raw(),
            str![].raw(),
        ),
    ];
    for (stream, expected_tokens, expected_stream) in cases {
        dbg!(stream);
        let mut stream = Stream::new(stream);
        let actual_tokens = lex_basic_string(&mut stream);
        assert_data_eq!(actual_tokens.to_debug(), expected_tokens.raw());
        let stream = *stream;
        assert_data_eq!(stream, expected_stream.raw());
    }
}

#[test]
fn test_lex_atom() {
    let cases = [
        (
            "hello",
            str![[r#"
Token {
    kind: Atom,
    span: 0..5,
}

"#]]
            .raw(),
            str![""].raw(),
        ),
        (
            "hello = world",
            str![[r#"
Token {
    kind: Atom,
    span: 0..5,
}

"#]]
            .raw(),
            str![" = world"].raw(),
        ),
        (
            "1.100e100 ]",
            str![[r#"
Token {
    kind: Atom,
    span: 0..1,
}

"#]]
            .raw(),
            str![".100e100 ]"].raw(),
        ),
        (
            "a.b.c = 5",
            str![[r#"
Token {
    kind: Atom,
    span: 0..1,
}

"#]]
            .raw(),
            str![".b.c = 5"].raw(),
        ),
        (
            "true ]",
            str![[r#"
Token {
    kind: Atom,
    span: 0..4,
}

"#]]
            .raw(),
            str![" ]"].raw(),
        ),
    ];
    for (stream, expected_tokens, expected_stream) in cases {
        dbg!(stream);
        let mut stream = Stream::new(stream);
        let actual_tokens = lex_atom(&mut stream);
        assert_data_eq!(actual_tokens.to_debug(), expected_tokens.raw());
        let stream = *stream;
        assert_data_eq!(stream, expected_stream.raw());
    }
}

#[track_caller]
fn t(input: &str, expected: impl IntoData) {
    let source = crate::Source::new(input);
    let actual = source.lex().into_vec();
    assert_data_eq!(actual.to_debug(), expected);

    if !actual.is_empty() {
        let spans = actual.iter().map(|t| t.span()).collect::<Vec<_>>();
        assert_eq!(spans.first().unwrap().start(), 0);
        assert_eq!(spans.last().unwrap().end(), input.len());
        for i in 0..(spans.len() - 1) {
            let current = &spans[i];
            let next = &spans[i + 1];
            assert_eq!(current.end(), next.start());
        }
    }
}

#[test]
fn literal_strings() {
    t(
        "''",
        str![[r#"
[
    Token {
        kind: LiteralString,
        span: 0..2,
    },
    Token {
        kind: Eof,
        span: 2..2,
    },
]

"#]]
        .raw(),
    );
    t(
        "''''''",
        str![[r#"
[
    Token {
        kind: MlLiteralString,
        span: 0..6,
    },
    Token {
        kind: Eof,
        span: 6..6,
    },
]

"#]]
        .raw(),
    );
    t(
        "'''\n'''",
        str![[r#"
[
    Token {
        kind: MlLiteralString,
        span: 0..7,
    },
    Token {
        kind: Eof,
        span: 7..7,
    },
]

"#]]
        .raw(),
    );
    t(
        "'a'",
        str![[r#"
[
    Token {
        kind: LiteralString,
        span: 0..3,
    },
    Token {
        kind: Eof,
        span: 3..3,
    },
]

"#]]
        .raw(),
    );
    t(
        "'\"a'",
        str![[r#"
[
    Token {
        kind: LiteralString,
        span: 0..4,
    },
    Token {
        kind: Eof,
        span: 4..4,
    },
]

"#]]
        .raw(),
    );
    t(
        "''''a'''",
        str![[r#"
[
    Token {
        kind: MlLiteralString,
        span: 0..8,
    },
    Token {
        kind: Eof,
        span: 8..8,
    },
]

"#]]
        .raw(),
    );
    t(
        "'''\n'a\n'''",
        str![[r#"
[
    Token {
        kind: MlLiteralString,
        span: 0..10,
    },
    Token {
        kind: Eof,
        span: 10..10,
    },
]

"#]]
        .raw(),
    );
    t(
        "'''a\n'a\r\n'''",
        str![[r#"
[
    Token {
        kind: MlLiteralString,
        span: 0..12,
    },
    Token {
        kind: Eof,
        span: 12..12,
    },
]

"#]]
        .raw(),
    );
}

#[test]
fn basic_strings() {
    t(
        r#""""#,
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..2,
    },
    Token {
        kind: Eof,
        span: 2..2,
    },
]

"#]]
        .raw(),
    );
    t(
        r#""""""""#,
        str![[r#"
[
    Token {
        kind: MlBasicString,
        span: 0..6,
    },
    Token {
        kind: Eof,
        span: 6..6,
    },
]

"#]]
        .raw(),
    );
    t(
        r#""a""#,
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..3,
    },
    Token {
        kind: Eof,
        span: 3..3,
    },
]

"#]]
        .raw(),
    );
    t(
        r#""""a""""#,
        str![[r#"
[
    Token {
        kind: MlBasicString,
        span: 0..7,
    },
    Token {
        kind: Eof,
        span: 7..7,
    },
]

"#]]
        .raw(),
    );
    t(
        r#""\t""#,
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..4,
    },
    Token {
        kind: Eof,
        span: 4..4,
    },
]

"#]]
        .raw(),
    );
    t(
        r#""\u0000""#,
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..8,
    },
    Token {
        kind: Eof,
        span: 8..8,
    },
]

"#]]
        .raw(),
    );
    t(
        r#""\U00000000""#,
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..12,
    },
    Token {
        kind: Eof,
        span: 12..12,
    },
]

"#]]
        .raw(),
    );
    t(
        r#""\U000A0000""#,
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..12,
    },
    Token {
        kind: Eof,
        span: 12..12,
    },
]

"#]]
        .raw(),
    );
    t(
        r#""\\t""#,
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..5,
    },
    Token {
        kind: Eof,
        span: 5..5,
    },
]

"#]]
        .raw(),
    );
    t(
        "\"\t\"",
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..3,
    },
    Token {
        kind: Eof,
        span: 3..3,
    },
]

"#]]
        .raw(),
    );
    t(
        "\"\"\"\n\t\"\"\"",
        str![[r#"
[
    Token {
        kind: MlBasicString,
        span: 0..8,
    },
    Token {
        kind: Eof,
        span: 8..8,
    },
]

"#]]
        .raw(),
    );
    t(
        "\"\"\"\\\n\"\"\"",
        str![[r#"
[
    Token {
        kind: MlBasicString,
        span: 0..8,
    },
    Token {
        kind: Eof,
        span: 8..8,
    },
]

"#]]
        .raw(),
    );
    t(
        "\"\"\"\\\n     \t   \t  \\\r\n  \t \n  \t \r\n\"\"\"",
        str![[r#"
[
    Token {
        kind: MlBasicString,
        span: 0..34,
    },
    Token {
        kind: Eof,
        span: 34..34,
    },
]

"#]]
        .raw(),
    );
    t(
        r#""\r""#,
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..4,
    },
    Token {
        kind: Eof,
        span: 4..4,
    },
]

"#]]
        .raw(),
    );
    t(
        r#""\n""#,
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..4,
    },
    Token {
        kind: Eof,
        span: 4..4,
    },
]

"#]]
        .raw(),
    );
    t(
        r#""\b""#,
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..4,
    },
    Token {
        kind: Eof,
        span: 4..4,
    },
]

"#]]
        .raw(),
    );
    t(
        r#""a\fa""#,
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..6,
    },
    Token {
        kind: Eof,
        span: 6..6,
    },
]

"#]]
        .raw(),
    );
    t(
        r#""\"a""#,
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..5,
    },
    Token {
        kind: Eof,
        span: 5..5,
    },
]

"#]]
        .raw(),
    );
    t(
        "\"\"\"\na\"\"\"",
        str![[r#"
[
    Token {
        kind: MlBasicString,
        span: 0..8,
    },
    Token {
        kind: Eof,
        span: 8..8,
    },
]

"#]]
        .raw(),
    );
    t(
        "\"\"\"\n\"\"\"",
        str![[r#"
[
    Token {
        kind: MlBasicString,
        span: 0..7,
    },
    Token {
        kind: Eof,
        span: 7..7,
    },
]

"#]]
        .raw(),
    );
    t(
        r#""""a\"""b""""#,
        str![[r#"
[
    Token {
        kind: MlBasicString,
        span: 0..12,
    },
    Token {
        kind: Eof,
        span: 12..12,
    },
]

"#]]
        .raw(),
    );
    t(
        r#""\a"#,
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..3,
    },
    Token {
        kind: Eof,
        span: 3..3,
    },
]

"#]]
        .raw(),
    );
    t(
        "\"\\\n",
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..2,
    },
    Token {
        kind: Newline,
        span: 2..3,
    },
    Token {
        kind: Eof,
        span: 3..3,
    },
]

"#]]
        .raw(),
    );
    t(
        "\"\\\r\n",
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..3,
    },
    Token {
        kind: Newline,
        span: 3..4,
    },
    Token {
        kind: Eof,
        span: 4..4,
    },
]

"#]]
        .raw(),
    );
    t(
        "\"\\",
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..2,
    },
    Token {
        kind: Eof,
        span: 2..2,
    },
]

"#]]
        .raw(),
    );
    t(
        "\"\u{0}",
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..2,
    },
    Token {
        kind: Eof,
        span: 2..2,
    },
]

"#]]
        .raw(),
    );
    t(
        r#""\U00""#,
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..6,
    },
    Token {
        kind: Eof,
        span: 6..6,
    },
]

"#]]
        .raw(),
    );
    t(
        r#""\U00"#,
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..5,
    },
    Token {
        kind: Eof,
        span: 5..5,
    },
]

"#]]
        .raw(),
    );
    t(
        r#""\uD800"#,
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..7,
    },
    Token {
        kind: Eof,
        span: 7..7,
    },
]

"#]]
        .raw(),
    );
    t(
        r#""\UFFFFFFFF"#,
        str![[r#"
[
    Token {
        kind: BasicString,
        span: 0..11,
    },
    Token {
        kind: Eof,
        span: 11..11,
    },
]

"#]]
        .raw(),
    );
}

#[test]
fn keylike() {
    t(
        "foo",
        str![[r#"
[
    Token {
        kind: Atom,
        span: 0..3,
    },
    Token {
        kind: Eof,
        span: 3..3,
    },
]

"#]]
        .raw(),
    );
    t(
        "0bar",
        str![[r#"
[
    Token {
        kind: Atom,
        span: 0..4,
    },
    Token {
        kind: Eof,
        span: 4..4,
    },
]

"#]]
        .raw(),
    );
    t(
        "bar0",
        str![[r#"
[
    Token {
        kind: Atom,
        span: 0..4,
    },
    Token {
        kind: Eof,
        span: 4..4,
    },
]

"#]]
        .raw(),
    );
    t(
        "1234",
        str![[r#"
[
    Token {
        kind: Atom,
        span: 0..4,
    },
    Token {
        kind: Eof,
        span: 4..4,
    },
]

"#]]
        .raw(),
    );
    t(
        "a-b",
        str![[r#"
[
    Token {
        kind: Atom,
        span: 0..3,
    },
    Token {
        kind: Eof,
        span: 3..3,
    },
]

"#]]
        .raw(),
    );
    t(
        "a_B",
        str![[r#"
[
    Token {
        kind: Atom,
        span: 0..3,
    },
    Token {
        kind: Eof,
        span: 3..3,
    },
]

"#]]
        .raw(),
    );
    t(
        "-_-",
        str![[r#"
[
    Token {
        kind: Atom,
        span: 0..3,
    },
    Token {
        kind: Eof,
        span: 3..3,
    },
]

"#]]
        .raw(),
    );
    t(
        "___",
        str![[r#"
[
    Token {
        kind: Atom,
        span: 0..3,
    },
    Token {
        kind: Eof,
        span: 3..3,
    },
]

"#]]
        .raw(),
    );
}

#[test]
fn all() {
    t(
        " a ",
        str![[r#"
[
    Token {
        kind: Whitespace,
        span: 0..1,
    },
    Token {
        kind: Atom,
        span: 1..2,
    },
    Token {
        kind: Whitespace,
        span: 2..3,
    },
    Token {
        kind: Eof,
        span: 3..3,
    },
]

"#]]
        .raw(),
    );

    t(
        " a\t [[]] \t [] {} , . =\n# foo \r\n#foo \n ",
        str![[r#"
[
    Token {
        kind: Whitespace,
        span: 0..1,
    },
    Token {
        kind: Atom,
        span: 1..2,
    },
    Token {
        kind: Whitespace,
        span: 2..4,
    },
    Token {
        kind: LeftSquareBracket,
        span: 4..5,
    },
    Token {
        kind: LeftSquareBracket,
        span: 5..6,
    },
    Token {
        kind: RightSquareBracket,
        span: 6..7,
    },
    Token {
        kind: RightSquareBracket,
        span: 7..8,
    },
    Token {
        kind: Whitespace,
        span: 8..11,
    },
    Token {
        kind: LeftSquareBracket,
        span: 11..12,
    },
    Token {
        kind: RightSquareBracket,
        span: 12..13,
    },
    Token {
        kind: Whitespace,
        span: 13..14,
    },
    Token {
        kind: LeftCurlyBracket,
        span: 14..15,
    },
    Token {
        kind: RightCurlyBracket,
        span: 15..16,
    },
    Token {
        kind: Whitespace,
        span: 16..17,
    },
    Token {
        kind: Comma,
        span: 17..18,
    },
    Token {
        kind: Whitespace,
        span: 18..19,
    },
    Token {
        kind: Dot,
        span: 19..20,
    },
    Token {
        kind: Whitespace,
        span: 20..21,
    },
    Token {
        kind: Equals,
        span: 21..22,
    },
    Token {
        kind: Newline,
        span: 22..23,
    },
    Token {
        kind: Comment,
        span: 23..29,
    },
    Token {
        kind: Newline,
        span: 29..31,
    },
    Token {
        kind: Comment,
        span: 31..36,
    },
    Token {
        kind: Newline,
        span: 36..37,
    },
    Token {
        kind: Whitespace,
        span: 37..38,
    },
    Token {
        kind: Eof,
        span: 38..38,
    },
]

"#]]
        .raw(),
    );
}

#[test]
fn bare_cr_bad() {
    t(
        "\r",
        str![[r#"
[
    Token {
        kind: Newline,
        span: 0..1,
    },
    Token {
        kind: Eof,
        span: 1..1,
    },
]

"#]]
        .raw(),
    );
    t(
        "'\n",
        str![[r#"
[
    Token {
        kind: LiteralString,
        span: 0..1,
    },
    Token {
        kind: Newline,
        span: 1..2,
    },
    Token {
        kind: Eof,
        span: 2..2,
    },
]

"#]]
        .raw(),
    );
    t(
        "'\u{0}",
        str![[r#"
[
    Token {
        kind: LiteralString,
        span: 0..2,
    },
    Token {
        kind: Eof,
        span: 2..2,
    },
]

"#]]
        .raw(),
    );
    t(
        "'",
        str![[r#"
[
    Token {
        kind: LiteralString,
        span: 0..1,
    },
    Token {
        kind: Eof,
        span: 1..1,
    },
]

"#]]
        .raw(),
    );
    t(
        "\u{0}",
        str![[r#"
[
    Token {
        kind: Atom,
        span: 0..1,
    },
    Token {
        kind: Eof,
        span: 1..1,
    },
]

"#]]
        .raw(),
    );
}

#[test]
fn bad_comment() {
    t(
        "#\u{0}",
        str![[r#"
[
    Token {
        kind: Comment,
        span: 0..2,
    },
    Token {
        kind: Eof,
        span: 2..2,
    },
]

"#]]
        .raw(),
    );
}

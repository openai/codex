use super::Tok::*;
use super::{Error, ErrorCode, Tok, Tokenizer};

enum Expectation<'a> {
    ExpectTok(Tok<'a>),
    ExpectErr(ErrorCode),
}

use self::Expectation::*;

fn gen_test(input: &str, expected: Vec<(&str, Expectation)>) {
    // use $ to signal EOL because it can be replaced with a single space
    // for spans, and because it applies also to r#XXX# style strings:
    let input = input.replace('$', "\n");

    let tokenizer = Tokenizer::new(&input, 0);
    let len = expected.len();
    for (token, (expected_span, expectation)) in tokenizer.zip(expected.into_iter()) {
        let expected_start = expected_span.find('~').unwrap();
        let expected_end = expected_span.rfind('~').unwrap() + 1;
        println!("token: {:?}", token);
        match expectation {
            ExpectTok(expected_tok) => {
                assert_eq!(Ok((expected_start, expected_tok, expected_end)), token);
            }
            ExpectErr(expected_ec) => assert_eq!(
                Err(Error {
                    location: expected_start,
                    code: expected_ec,
                }),
                token
            ),
        }
    }

    let tokenizer = Tokenizer::new(&input, 0);
    assert_eq!(None, tokenizer.skip(len).next());
}

fn test(input: &str, expected: Vec<(&str, Tok)>) {
    let generic_expected = expected
        .into_iter()
        .map(|(span, tok)| (span, ExpectTok(tok)))
        .collect();
    gen_test(input, generic_expected);
}

fn test_err(input: &str, expected: (&str, ErrorCode)) {
    let (span, ec) = expected;
    gen_test(input, vec![(span, ExpectErr(ec))])
}

#[test]
fn basic() {
    test(
        "extern foo",
        vec![("~~~~~~    ", Extern), ("       ~~~", Id("foo"))],
    );
}

#[test]
fn eol_comment() {
    test(
        "extern // This is a comment$ foo",
        vec![
            ("~~~~~~                          ", Extern),
            ("                             ~~~", Id("foo")),
        ],
    );
}

#[test]
fn code1() {
    test(
        "=> a(b, c),",
        vec![
            ("~~~~~~~~~~ ", EqualsGreaterThanCode(" a(b, c)")),
            ("          ~", Comma),
        ],
    );
}

#[test]
fn rule_id_then_equalsgreaterthancode_functioncall() {
    test(
        "id => a(b, c),",
        vec![
            ("~~            ", Id("id")),
            ("   ~~~~~~~~~~ ", EqualsGreaterThanCode(" a(b, c)")),
            ("             ~", Comma),
        ],
    );
}

#[test]
fn rule_stringliteral_slash_dot_then_equalsgreaterthancode_functioncall() {
    test(
        r#" "\." => a(b, c),"#,
        vec![
            (r#" ~~~~            "#, StringLiteral(r#"\."#)),
            (r#"      ~~~~~~~~~~ "#, EqualsGreaterThanCode(" a(b, c)")),
            (r#"                ~"#, Comma),
        ],
    );
}

#[test]
fn rule_stringliteral_slash_dot_then_equalsgreaterthancode_many_characters_in_stringliteral() {
    test(
        r#" "\." => "Planet Earth" ,"#,
        vec![
            (r#" ~~~~                    "#, StringLiteral(r#"\."#)),
            (
                r#"      ~~~~~~~~~~~~~~~~~~ "#,
                EqualsGreaterThanCode(r#" "Planet Earth" "#),
            ),
            (r#"                        ~"#, Comma),
        ],
    );
}

#[test]
fn rule_stringliteral_slash_dot_then_equalsgreaterthancode_one_character_dot_in_stringliteral() {
    test(
        r#" "\." => "." ,"#,
        vec![
            (r#" ~~~~         "#, StringLiteral(r#"\."#)),
            (r#"      ~~~~~~~ "#, EqualsGreaterThanCode(r#" "." "#)),
            (r#"             ~"#, Comma),
        ],
    );
}

#[test]
fn rule_stringliteral_slash_openningbracket_then_equalsgreaterthancode_one_character_openningbracket_in_stringliteral(
) {
    test(
        r#" "\(" => "(" ,"#,
        vec![
            (r#" ~~~~         "#, StringLiteral(r#"\("#)),
            (r#"      ~~~~~~~ "#, EqualsGreaterThanCode(r#" "(" "#)),
            (r#"             ~"#, Comma),
        ],
    );
}

#[test]
fn rule_stringliteral_slash_openningbracket_then_equalsgreaterthancode_empty_stringliteral() {
    test(
        r#" "\(" => "" ,"#,
        vec![
            (r#" ~~~~        "#, StringLiteral(r#"\("#)),
            (r#"      ~~~~~~ "#, EqualsGreaterThanCode(r#" "" "#)),
            (r#"            ~"#, Comma),
        ],
    );
}

#[test]
fn rule_stringliteral_slash_dot_then_equalsgreaterthancode_one_character_dot() {
    test(
        r#" "\." => '.' ,"#,
        vec![
            (r#" ~~~~         "#, StringLiteral(r#"\."#)),
            (r#"      ~~~~~~~ "#, EqualsGreaterThanCode(r#" '.' "#)),
            (r#"             ~"#, Comma),
        ],
    );
}

#[test]
fn rule_stringliteral_slash_openningbracket_then_equalsgreaterthancode_one_character_openningbracket(
) {
    test(
        r#" "\(" => '(' ,"#,
        vec![
            (r#" ~~~~         "#, StringLiteral(r#"\("#)),
            (r#"      ~~~~~~~ "#, EqualsGreaterThanCode(r#" '(' "#)),
            (r#"             ~"#, Comma),
        ],
    );
}

#[test]
fn equalsgreaterthancode_one_character_openningbracket() {
    test(
        r#"=> '(' ,"#,
        vec![
            (r#"~~~~~~~ "#, EqualsGreaterThanCode(r#" '(' "#)),
            (r#"       ~"#, Comma),
        ],
    );
}

#[test]
fn equalsgreaterthancode_one_character_escaped_n() {
    test(
        r#"=> '\n' ,"#,
        vec![
            (r#"~~~~~~~~ "#, EqualsGreaterThanCode(r#" '\n' "#)),
            (r#"        ~"#, Comma),
        ],
    );
}

#[test]
fn equalsgreaterthancode_one_character_escaped_w() {
    test(
        r#"=> '\w' ,"#,
        vec![
            (r#"~~~~~~~~ "#, EqualsGreaterThanCode(r#" '\w' "#)),
            (r#"        ~"#, Comma),
        ],
    );
}

#[test]
fn equalsgreaterthancode_one_character_escaped_planet123() {
    test(
        r#"=> '\planet123' ,"#,
        vec![
            (
                r#"~~~~~~~~~~~~~~~~ "#,
                EqualsGreaterThanCode(r#" '\planet123' "#),
            ),
            (r#"                ~"#, Comma),
        ],
    );
}

#[test]
fn equalsgreaterthancode_one_character_openningcurlybracket() {
    test(
        r#"=> '{' ,"#,
        vec![
            (r#"~~~~~~~ "#, EqualsGreaterThanCode(r#" '{' "#)),
            (r#"       ~"#, Comma),
        ],
    );
}

#[test]
fn equalsgreaterthancode_one_character_openningsquarebracket() {
    test(
        r#"=> '[' ,"#,
        vec![
            (r#"~~~~~~~ "#, EqualsGreaterThanCode(r#" '[' "#)),
            (r#"       ~"#, Comma),
        ],
    );
}

#[test]
fn equalsgreaterthancode_one_character_openningbracket_wrapped_by_brackets() {
    test(
        r#"=> ('(') ,"#,
        vec![
            (r#"~~~~~~~~~ "#, EqualsGreaterThanCode(r#" ('(') "#)),
            (r#"         ~"#, Comma),
        ],
    );
}

#[test]
fn equalsgreaterthancode_one_character_closingbracket_wrapped_by_brackets() {
    test(
        r#"=> (')') ,"#,
        vec![
            (r#"~~~~~~~~~ "#, EqualsGreaterThanCode(r#" (')') "#)),
            (r#"         ~"#, Comma),
        ],
    );
}

#[test]
fn equalsgreaterthancode_tuple() {
    test(
        r#"=> (1,2,3) ,"#,
        vec![
            (r#"~~~~~~~~~~~ "#, EqualsGreaterThanCode(r#" (1,2,3) "#)),
            (r#"           ~"#, Comma),
        ],
    );
}

#[test]
fn equalsgreaterthancode_statement_with_lifetime() {
    test(
        r#"=> HuffmanTable::<Code<'a>>::new() ,"#,
        vec![
            (
                r#"~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ "#,
                EqualsGreaterThanCode(r#" HuffmanTable::<Code<'a>>::new() "#),
            ),
            (r#"                                   ~"#, Comma),
        ],
    );
}

#[test]
fn equalsgreaterthancode_statement_with_many_lifetimes() {
    test(
        r#"=> (HuffmanTable::<Code<'a, 'b>>::new()),"#,
        vec![
            (
                r#"~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ "#,
                EqualsGreaterThanCode(r#" (HuffmanTable::<Code<'a, 'b>>::new())"#),
            ),
            (r#"                                        ~"#, Comma),
        ],
    );
}

#[test]
fn equalsgreaterthancode_nested_function_with_lifetimes() {
    test(
        r#"=> fn foo<'a>(x: &'a i32, y: &'a i32) -> &'a i32 {} ,"#,
        vec![
            (
                r#"~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ "#,
                EqualsGreaterThanCode(r#" fn foo<'a>(x: &'a i32, y: &'a i32) -> &'a i32 {} "#),
            ),
            (
                r#"                                                    ~"#,
                Comma,
            ),
        ],
    );
}

#[test]
fn where_with_lifetimes() {
    test(
        r#"where <'a,bar<'b,'c>>,baz;"#,
        vec![
            (r#"~~~~~                     "#, Where),
            (r#"      ~                   "#, LessThan),
            (r#"       ~~                 "#, Lifetime("'a")),
            (r#"         ~                "#, Comma),
            (r#"          ~~~             "#, MacroId("bar")),
            (r#"             ~            "#, LessThan),
            (r#"              ~~          "#, Lifetime("'b")),
            (r#"                ~         "#, Comma),
            (r#"                 ~~       "#, Lifetime("'c")),
            (r#"                   ~      "#, GreaterThan),
            (r#"                    ~     "#, GreaterThan),
            (r#"                     ~    "#, Comma),
            (r#"                      ~~~ "#, Id("baz")),
            (r#"                         ~"#, Semi),
        ],
    );
}

#[test]
fn forall() {
    test(
        r#"for<'a, 'b, 'c> FnMut"#,
        vec![
            (r#"~~~                  "#, For),
            (r#"   ~                 "#, LessThan),
            (r#"    ~~               "#, Lifetime("'a")),
            (r#"      ~              "#, Comma),
            (r#"        ~~           "#, Lifetime("'b")),
            (r#"          ~          "#, Comma),
            (r#"            ~~       "#, Lifetime("'c")),
            (r#"              ~      "#, GreaterThan),
            (r#"                ~~~~~"#, Id("FnMut")),
        ],
    );
}

#[test]
fn where_forall_fnmut_with_return_type() {
    test(
        r#"where F: for<'a> FnMut(&'a T) -> U;"#,
        vec![
            (r#"~~~~~                              "#, Where),
            (r#"      ~                            "#, Id("F")),
            (r#"       ~                           "#, Colon),
            (r#"         ~~~                       "#, For),
            (r#"            ~                      "#, LessThan),
            (r#"             ~~                    "#, Lifetime("'a")),
            (r#"               ~                   "#, GreaterThan),
            (r#"                 ~~~~~             "#, Id("FnMut")),
            (r#"                      ~            "#, LeftParen),
            (r#"                       ~           "#, Ampersand),
            (r#"                        ~~         "#, Lifetime("'a")),
            (r#"                           ~       "#, Id("T")),
            (r#"                            ~      "#, RightParen),
            (r#"                              ~~   "#, MinusGreaterThan),
            (r#"                                 ~ "#, Id("U")),
            (r#"                                  ~"#, Semi),
        ],
    );
}

#[test]
fn equalsgreaterthancode_error_unbalanced() {
    test_err(r#"=> (,"#, (r#"~    "#, ErrorCode::UnterminatedCode))
}

#[test]
fn equalsgreaterthancode_error_unbalanced_closingbracket_character() {
    test_err(
        r#"=> (,')',"#,
        (r#"~        "#, ErrorCode::UnterminatedCode),
    )
}

#[test]
fn equalsgreaterthancode_error_unterminated_string_literal() {
    test_err(
        r#"=>  "Jan III Sobieski"#,
        (
            r#"    ~                "#,
            ErrorCode::UnterminatedStringLiteral,
        ),
    )
}

#[test]
fn equalsgreaterthancode_error_unterminated_character_literal() {
    test_err(
        r#"=>  '\x233  "#,
        (r#"    ~       "#, ErrorCode::UnterminatedCharacterLiteral),
    )
}

#[test]
fn equalsgreaterthancode_error_end_of_input_instead_of_closing_normal_character_literal() {
    test_err(
        r#"=>  'x"#,
        (r#"    ~ "#, ErrorCode::UnterminatedCharacterLiteral),
    )
}

#[test]
fn equalsgreaterthancode_single_quote_literal() {
    test(
        r#"=> { println!('\''); },"#,
        vec![
            (
                r#"~~~~~~~~~~~~~~~~~~~~~~ "#,
                EqualsGreaterThanCode(r#" { println!('\''); }"#),
            ),
            (r#"                      ~"#, Comma),
        ],
    );
}

#[test]
fn code_paren() {
    // Issue #25
    test(
        r#"=> a("(", c),"#,
        vec![
            (r#"~~~~~~~~~~~~ "#, EqualsGreaterThanCode(r#" a("(", c)"#)),
            (r#"            ~"#, Comma),
        ],
    );
}

#[test]
fn code_regex_paren() {
    // Issue #25
    test(
        r###"=> a(r##"("#""##, c),"###,
        vec![
            (
                r###"~~~~~~~~~~~~~~~~~~~~ "###,
                EqualsGreaterThanCode(r###" a(r##"("#""##, c)"###),
            ),
            (r###"                    ~"###, Comma),
        ],
    );
}

#[test]
fn code_comment_eol() {
    test(
        "=> a(// (
),",
        vec![
            (
                "~~~~~~~~~
~,",
                EqualsGreaterThanCode(" a(// (\n)"),
            ),
            (
                "=> a(// (
)~",
                Comma,
            ),
        ],
    );
}

#[test]
fn code2() {
    test(
        "=>? a(b, c),",
        vec![
            ("~~~~~~~~~~~ ", EqualsGreaterThanQuestionCode(" a(b, c)")),
            ("           ~", Comma),
        ],
    );
}

#[test]
#[should_panic]
fn code_forgot_comma() {
    test(
        "=> a(b, c),",
        vec![
            ("~~~~~~~~~~ ", EqualsGreaterThanCode(" a(b, c)")),
            // intentionally forget the comma token; this is more of a test of `test`
        ],
    );
}

#[test]
fn various_kinds_of_ids() {
    test(
        "foo<T<'a,U,`Z*{}`,r#type,r#use>>",
        vec![
            ("~~~                             ", MacroId("foo")),
            ("   ~                            ", LessThan),
            ("    ~                           ", MacroId("T")),
            ("     ~                          ", LessThan),
            ("      ~~                        ", Lifetime("'a")),
            ("        ~                       ", Comma),
            ("         ~                      ", Id("U")),
            ("          ~                     ", Comma),
            ("           ~~~~~~               ", Escape("Z*{}")),
            ("                 ~              ", Comma),
            ("                  ~~~~~~        ", Id("r#type")),
            ("                        ~       ", Comma),
            ("                         ~~~~~  ", Id("r#use")),
            ("                              ~ ", GreaterThan),
            ("                               ~", GreaterThan),
        ],
    );
}

#[test]
fn string_literals() {
    test(
        r#"foo "bar\"\n" baz"#,
        vec![
            (r#"~~~              "#, Id("foo")),
            (r#"    ~~~~~~~~~    "#, StringLiteral(r#"bar\"\n"#)),
            (r#"              ~~~"#, Id("baz")),
        ],
    );
}

#[test]
fn use1() {
    test(
        r#"use foo::bar; baz"#,
        vec![
            (r#"~~~~~~~~~~~~     "#, Use(" foo::bar")),
            (r#"            ~    "#, Semi),
            (r#"              ~~~"#, Id("baz")),
        ],
    );
}

#[test]
fn use2() {
    test(
        r#"use {foo,bar}; baz"#,
        vec![
            (r#"~~~~~~~~~~~~~     "#, Use(" {foo,bar}")),
            (r#"             ~    "#, Semi),
            (r#"               ~~~"#, Id("baz")),
        ],
    );
}

#[test]
fn where1() {
    test(
        r#"where <foo,bar>,baz;"#,
        vec![
            (r#"~~~~~               "#, Where),
            (r#"      ~             "#, LessThan),
            (r#"       ~~~          "#, Id("foo")),
            (r#"          ~         "#, Comma),
            (r#"           ~~~      "#, Id("bar")),
            (r#"              ~     "#, GreaterThan),
            (r#"               ~    "#, Comma),
            (r#"                ~~~ "#, Id("baz")),
            (r#"                   ~"#, Semi),
        ],
    );
}

#[test]
fn regex1() {
    test(
        r#####"raa r##" #"#"" "#"##rrr"#####,
        vec![
            (r#####"~~~                    "#####, Id("raa")),
            (
                r#####"    ~~~~~~~~~~~~~~~~   "#####,
                RegexLiteral(r##" #"#"" "#"##),
            ),
            (r#####"                    ~~~"#####, Id("rrr")),
        ],
    );
}

#[test]
fn hash_token() {
    test(r#" # "#, vec![(r#" ~ "#, Hash)]);
}

#[test]
fn shebang_attribute_normal_text() {
    test(
        r#" #![Attribute] "#,
        vec![(r#" ~~~~~~~~~~~~~ "#, ShebangAttribute("#![Attribute]"))],
    );
}

#[test]
fn shebang_attribute_special_characters_without_quotes() {
    test(
        r#" #![set width = 80] "#,
        vec![(
            r#" ~~~~~~~~~~~~~~~~~~ "#,
            ShebangAttribute("#![set width = 80]"),
        )],
    );
}

#[test]
fn shebang_attribute_special_characters_with_quotes() {
    test(
        r#" #![set width = "80"] "#,
        vec![(
            r#" ~~~~~~~~~~~~~~~~~~~~ "#,
            ShebangAttribute(r#"#![set width = "80"]"#),
        )],
    );
}

#[test]
fn shebang_attribute_special_characters_closing_sqbracket_in_string_literal() {
    test(
        r#" #![set width = "80]"] "#,
        vec![(
            r#" ~~~~~~~~~~~~~~~~~~~~~ "#,
            ShebangAttribute(r#"#![set width = "80]"]"#),
        )],
    );
}

#[test]
fn shebang_attribute_special_characters_opening_sqbracket_in_string_literal() {
    test(
        r#" #![set width = "[80"] "#,
        vec![(
            r#" ~~~~~~~~~~~~~~~~~~~~~ "#,
            ShebangAttribute(r#"#![set width = "[80"]"#),
        )],
    );
}

#[test]
fn shebang_attribute_special_characters_nested_sqbrackets() {
    test(
        r#" #![set width = [80]] "#,
        vec![(
            r#" ~~~~~~~~~~~~~~~~~~~~ "#,
            ShebangAttribute(r#"#![set width = [80]]"#),
        )],
    );
}

#[test]
fn regex2() {
    test(r#"r"(123""#, vec![(r#"~~~~~~~"#, RegexLiteral(r"(123"))]);
}

#[test]
fn char_literals() {
    test(
        r#"'foo' 'a 'b '!' '!!' '\'' 'c"#,
        vec![
            (r#"~~~~~                       "#, CharLiteral("foo")),
            (r#"      ~~                    "#, Lifetime("'a")),
            (r#"         ~~                 "#, Lifetime("'b")),
            (r#"            ~~~             "#, CharLiteral("!")),
            (r#"                ~~~~        "#, CharLiteral("!!")),
            (r#"                     ~~~~   "#, CharLiteral("\\'")),
            (r#"                          ~~"#, Lifetime("'c")),
        ],
    );
}

#[test]
fn string_escapes() {
    use super::apply_string_escapes;
    use std::borrow::Cow;

    assert_eq!(apply_string_escapes(r#"foo"#, 5), Ok(Cow::Borrowed("foo")));
    assert_eq!(
        apply_string_escapes(r#"\\"#, 10),
        Ok(Cow::Owned::<str>(r#"\"#.into()))
    );
    assert_eq!(
        apply_string_escapes(r#"\""#, 15),
        Ok(Cow::Owned::<str>(r#"""#.into()))
    );
    assert_eq!(
        apply_string_escapes(r#"up\ndown"#, 25),
        Ok(Cow::Owned::<str>("up\ndown".into()))
    );
    assert_eq!(
        apply_string_escapes(r#"forth\rback"#, 25),
        Ok(Cow::Owned::<str>("forth\rback".into()))
    );
    assert_eq!(
        apply_string_escapes(r#"left\tright"#, 40),
        Ok(Cow::Owned::<str>("left\tright".into()))
    );

    // Errors.
    assert_eq!(
        apply_string_escapes("\u{192}\\oo", 65), // "ƒ\oo"
        Err(Error {
            location: 68,
            code: ErrorCode::UnrecognizedEscape
        })
    );
    // LALRPOP doesn't support the other Rust escape sequences.
    assert_eq!(
        apply_string_escapes(r#"star: \u{2a}"#, 105),
        Err(Error {
            location: 112,
            code: ErrorCode::UnrecognizedEscape
        })
    );
}

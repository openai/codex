use crate::grammar::parse_tree::Grammar;
use crate::lexer::dfa::interpret;
use crate::normalize::resolve::resolve;
use crate::normalize::NormResult;
use crate::parser;
use crate::test_util;

fn validate_grammar(grammar: &str) -> NormResult<Grammar> {
    let parsed_grammar = parser::parse_grammar(grammar).expect("parse grammar");
    let parsed_grammar = resolve(parsed_grammar).expect("resolve");
    super::validate(parsed_grammar)
}

fn check_err(expected_err: &str, grammar: &str, span: &str) {
    let err = validate_grammar(grammar).unwrap_err();
    test_util::check_norm_err(expected_err, span, err);
}

fn check_intern_token(grammar: &str, expected_tokens: Vec<(&'static str, &'static str)>) {
    let parsed_grammar = validate_grammar(grammar).expect("validate");
    let intern_token = parsed_grammar.intern_token().expect("intern_token");
    println!("intern_token: {:?}", intern_token);
    for (input, expected_user_name) in expected_tokens {
        let actual_user_name =
            interpret::interpret(&intern_token.dfa, input).map(|(index, text)| {
                let user_name = &intern_token.match_entries[index.index()].user_name;
                (user_name.clone(), text)
            });
        let actual_user_name = format!("{:?}", actual_user_name);
        if expected_user_name != actual_user_name {
            panic!(
                "input `{}` matched `{}` but we expected `{}`",
                input, actual_user_name, expected_user_name
            );
        }
    }
}

#[test]
fn unknown_terminal() {
    check_err(
        r#"terminal `"\+"` does not have a pattern defined for it"#,
        r#"grammar; extern { enum Term { } } X = X "+";"#,
        r#"                                        ~~~ "#,
    );
}

#[test]
fn unknown_id_terminal() {
    check_err(
        r#"terminal `"foo"` does not have a pattern defined for it"#,
        r#"grammar; extern { enum Term { } } X = X "foo";"#,
        r#"                                        ~~~~~ "#,
    );
}

#[test]
fn tick_input_lifetime_already_declared() {
    check_err(
        r#".*the `'input` lifetime is implicit and cannot be declared"#,
        r#"grammar<'input>; X = X "foo";"#,
        r#"~~~~~~~                      "#,
    );
}

#[test]
fn input_parameter_already_declared() {
    check_err(
        r#".*the `input` parameter is implicit and cannot be declared"#,
        r#"grammar(input:u32); X = X "foo";"#,
        r#"~~~~~~~                         "#,
    );
}

#[test]
fn invalid_regular_expression_unterminated_group() {
    check_err(
        r#"unclosed group"#,
        r#"grammar; X = X r"(123";"#,
        r#"               ~~~~~~~ "#,
    );
}

#[test]
fn quoted_literals() {
    check_intern_token(
        r#"grammar; X = X "+" "-" "foo" "(" ")";"#,
        vec![
            ("+", r#"Some(("+", "+"))"#),
            ("-", r#"Some(("-", "-"))"#),
            ("(", r#"Some(("(", "("))"#),
            (")", r#"Some((")", ")"))"#),
            ("foo", r#"Some(("foo", "foo"))"#),
            ("<", r#"None"#),
        ],
    );
}

#[test]
fn regex_literals() {
    check_intern_token(
        r#"grammar; X = X r"[a-z]+" r"[0-9]+";"#,
        vec![
            ("a", r##"Some((r#"[a-z]+"#, "a"))"##),
            ("def", r##"Some((r#"[a-z]+"#, "def"))"##),
            ("1", r##"Some((r#"[0-9]+"#, "1"))"##),
            ("9123456", r##"Some((r#"[0-9]+"#, "9123456"))"##),
        ],
    );
}

/// Basic test for match mappings.
#[test]
fn match_mappings() {
    check_intern_token(
        r#"grammar; match { r"(?i)begin" => "BEGIN" } else { "abc" => ALPHA } X = "BEGIN" ALPHA;"#,
        vec![
            ("BEGIN", r##"Some(("BEGIN", "BEGIN"))"##),
            ("begin", r##"Some(("BEGIN", "begin"))"##),
            ("abc", r#"Some((ALPHA, "abc"))"#),
        ],
    );
}

/// Match mappings, exercising precedence. Here the ID regex *would*
/// be ambiguous with the begin regex.
#[test]
fn match_precedence() {
    check_intern_token(
        r#"grammar; match { r"(?i)begin" => "BEGIN" } else { r"\w+" => ID } X = ();"#,
        vec![
            ("BEGIN", r##"Some(("BEGIN", "BEGIN"))"##),
            ("begin", r##"Some(("BEGIN", "begin"))"##),
            ("abc", r#"Some((ID, "abc"))"#),
        ],
    );
}

/// Test that, without a `catch-all`, using unrecognized literals is an error.
#[test]
fn invalid_match_literal() {
    check_err(
        r#"terminal `"foo"` does not have a match mapping defined for it"#,
        r#"grammar; match { r"(?i)begin" => "BEGIN" } X = "foo";"#,
        r#"                                               ~~~~~ "#,
    );
}

/// Test that, without a `catch-all`, using unrecognized literals is an error.
#[test]
fn invalid_match_regex_literal() {
    check_err(
        r##"terminal `r#"foo"#` does not have a match mapping defined for it"##,
        r#"grammar; match { r"(?i)begin" => "BEGIN" } X = r"foo";"#,
        r#"                                               ~~~~~~ "#,
    );
}

/// Test that, with a catch-all, the previous two examples work.
#[test]
fn match_catch_all() {
    let grammar = r#"grammar; match { r"(?i)begin" => "BEGIN", _ } X = { "foo", r"foo" };"#;
    assert!(validate_grammar(grammar).is_ok())
}

#[test]
fn complex_match() {
    let grammar = r##"
        grammar;
        match {
            "abc"        => "ABC",
            r"(?i)begin" => BEGIN
        }

        pub Query: String = {
            "ABC" BEGIN => String::from("Success")
        };
"##;
    assert!(validate_grammar(grammar).is_ok())
}

/// Test that overlapping regular expressions are still forbidden within one level
/// of a match declaration.
#[test]
fn ambiguity_within_match() {
    check_err(
        r##"ambiguity detected between the terminal `r#"b"#` and the terminal `r#"\(\?i\)b"#`"##,
        r#"grammar; match { r"(?i)b" => "B", r"b" => "b" }"#,
        r#"                                  ~~~~~~~~~~~~ "#,
    );
}

/// Test that using the **exact same regular expression** twice is
/// forbidden, even across multiple levels of the match expression.
/// No good reason to do that.
#[test]
fn same_literal_twice() {
    check_err(
        r##"multiple match entries for `r#"\(\?i\)b"#`"##,
        r#"grammar; match { r"(?i)b" => "B" } else { r"(?i)b" => "b" }"#,
        r#"                                          ~~~~~~~~~~~~~~~~ "#,
    );
}

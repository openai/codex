use crate::grammar::parse_tree::Span;
use crate::parser;
use regex::Regex;

fn check_err(expected_err: &str, grammar: &str) {
    let expected_err = Regex::new(expected_err).unwrap();

    // the string will have a `>>>` and `<<<` in it, which serve to
    // indicate the span where an error is expected.
    let start_index = grammar.find(">>>").unwrap();
    let grammar = grammar.replace(">>>", ""); // remove the `>>>` marker
    let end_index = grammar.rfind("<<<").unwrap();
    let grammar = grammar.replace("<<<", "");

    assert!(start_index <= end_index);

    let parsed_grammar = parser::parse_grammar(&grammar).unwrap();
    match super::resolve(parsed_grammar) {
        Ok(_) => {
            panic!("expected error for grammar");
        }
        Err(err) => {
            assert_eq!(err.span, Span(start_index, end_index));
            assert!(
                expected_err.is_match(&err.message),
                "unexpected error text `{}`, did not match `{}`",
                err.message,
                expected_err
            );
        }
    }
}

#[test]
fn unknown_nonterminal() {
    check_err("no definition found for `Y`", r#"grammar; X = X >>>Y<<<;"#);
}

#[test]
fn unknown_nonterminal_in_macro_arg() {
    check_err(
        "no definition found for `Y`",
        r#"grammar; X = X Id<>>>Y<<<>; Id<T> = T;"#,
    );
}

#[test]
fn unknown_nonterminal_in_repeat_question() {
    check_err("no definition found for `Y`", r#"grammar; X = >>>Y<<<?;"#);
}

#[test]
fn unknown_nonterminal_two() {
    check_err(
        "no definition found for `Expr`",
        r#"grammar; Term = { <n:"Num"> => n.as_num(), "A" <>>>Expr<<<> "B" };"#,
    );
}

#[test]
fn double_nonterminal() {
    check_err(
        "two nonterminals declared with the name `A`",
        r#"grammar; A = "Foo"; >>>A<<< = "Bar";"#,
    );
}

#[test]
fn repeated_macro_arg() {
    check_err(
        "multiple macro arguments declared with the name `Y`",
        r#"grammar; >>>X<Y,Y><<< = "foo";"#,
    );
}

#[test]
fn overlapping_terminal_and_nonterminal() {
    check_err(
        "terminal and nonterminal both declared with the name `A`",
        r#"grammar; A = "Foo"; extern { enum Foo { >>>A => Foo::A(..) <<<} }"#,
    );
}

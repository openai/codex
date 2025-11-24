use crate::parser;
use crate::test_util::compare;

use super::expand_macros;

#[test]
fn test_comma() {
    let grammar = parser::parse_grammar(
        r#"
grammar;
    Comma<E>: Vec<E> =
       <v:(<E> ",")*> <e:E?> =>
           v.into_iter().chain(e.into_iter()).collect();

    Ids = Comma<"Id">;
"#,
    )
    .unwrap();

    let actual = expand_macros(grammar).unwrap();

    let expected = parser::parse_grammar(
        r##"
grammar;
    Ids = `Comma<"Id">`;

    `Comma<"Id">`: Vec<#"Id"#> =
        <v:`(<"Id"> ",")*`> <e:`"Id"?`> => v.into_iter().chain(e.into_iter()).collect();

    #[inline]
    `"Id"?`: core::option::Option<#"Id"#> = {
        "Id" => Some(<>),
        => None
    };

    #[inline]
    `(<"Id"> ",")*`: alloc::vec::Vec<#`(<"Id"> ",")`#> = {
        => alloc::vec![],
        <v:`(<"Id"> ",")+`> => v,
    };

    #[inline]
    `(<"Id"> ",")`: #"Id"# = {
        <"Id"> "," => <>,
    };

    `(<"Id"> ",")+`: alloc::vec::Vec<#`(<"Id"> ",")`#> = {
        `(<"Id"> ",")` => alloc::vec![<>],
        <v:`(<"Id"> ",")+`> <e:`(<"Id"> ",")`> => { let mut v = v; v.push(e); v },
    };
"##,
    )
    .unwrap();

    compare(actual, expected);
}

#[test]
fn test_if_match() {
    let grammar = parser::parse_grammar(
        r#"
grammar;
    Expr<E> = {
       "A" if E == "A*C",
       "B" if E ~~ "^A*C$",
       "C" if E != "A*C",
       "D" if E !~ "^A*C$"
    };

    Expr1 = Expr<"A*C">;
    Expr2 = Expr<"AAC">;
    Expr3 = Expr<"ABC">;
"#,
    )
    .unwrap();

    let actual = expand_macros(grammar).unwrap();

    let expected = parser::parse_grammar(
        r#"
grammar;
    Expr1 = `Expr<"A*C">`;
    Expr2 = `Expr<"AAC">`;
    Expr3 = `Expr<"ABC">`;

    `Expr<"ABC">` = { "C", "D" };
    `Expr<"AAC">` = { "B", "C" };
    `Expr<"A*C">` = { "A", "D" };
"#,
    )
    .unwrap();

    compare(actual, expected);
}

#[test]
fn test_lookahead() {
    let grammar = parser::parse_grammar(
        r#"
        grammar;
        Expr = @L;
"#,
    )
    .unwrap();

    let actual = expand_macros(grammar).unwrap();

    let expected = parser::parse_grammar(
        r#"
        grammar;
        Expr = `@L`;
        #[inline] `@L` = =>@L;
"#,
    )
    .unwrap();

    compare(actual, expected);
}

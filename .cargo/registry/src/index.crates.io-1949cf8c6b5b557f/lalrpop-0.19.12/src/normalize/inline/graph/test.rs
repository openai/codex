use super::inline_order;
use crate::grammar::repr::NonterminalString;
use crate::normalize::lower_helper;
use crate::parser;
use crate::session::Session;
use string_cache::DefaultAtom as Atom;

#[test]
fn test_inline_self_cycle() {
    let grammar = parser::parse_grammar(
        r#"
    grammar;
    extern { }
    #[inline] A: () = A;
"#,
    )
    .unwrap();
    let grammar = lower_helper(&Session::test(), grammar, true).unwrap();
    assert!(inline_order(&grammar).is_err());
}

#[test]
fn test_inline_cycle_3() {
    let grammar = parser::parse_grammar(
        r#"
    grammar;
    extern { }
    #[inline] A: () = B;
    #[inline] B: () = C;
    #[inline] C: () = A;
"#,
    )
    .unwrap();
    let grammar = lower_helper(&Session::test(), grammar, true).unwrap();
    assert!(inline_order(&grammar).is_err());
}

#[test]
fn test_inline_order() {
    // because C references A, we inline A first.
    let grammar = parser::parse_grammar(
        r#"
    grammar;
    extern { }
    #[inline] A: () = B;
    B: () = C;
    #[inline] C: () = A;
"#,
    )
    .unwrap();
    let grammar = lower_helper(&Session::test(), grammar, true).unwrap();
    let a = NonterminalString(Atom::from("A"));
    let c = NonterminalString(Atom::from("C"));
    assert_eq!(inline_order(&grammar).unwrap(), vec![a, c]);
}

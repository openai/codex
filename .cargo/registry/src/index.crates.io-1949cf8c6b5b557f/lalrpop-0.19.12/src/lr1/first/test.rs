use super::FirstSets;
use crate::grammar::repr::*;
use crate::lr1::lookahead::Token::EOF;
use crate::lr1::lookahead::{Token, TokenSet};
use crate::lr1::tls::Lr1Tls;
use string_cache::DefaultAtom as Atom;
use crate::test_util::normalized_grammar;

pub fn nt(t: &str) -> Symbol {
    Symbol::Nonterminal(NonterminalString(Atom::from(t)))
}

pub fn term(t: &str) -> Symbol {
    Symbol::Terminal(TerminalString::quoted(Atom::from(t)))
}

fn la(t: &str) -> Token {
    Token::Terminal(TerminalString::quoted(Atom::from(t)))
}

fn first0(first: &FirstSets, symbols: &[Symbol]) -> Vec<Token> {
    let v = first.first0(symbols);
    v.iter().collect()
}

fn first1(first: &FirstSets, symbols: &[Symbol], lookahead: Token) -> Vec<Token> {
    let v = first.first1(symbols, &TokenSet::from(lookahead));
    v.iter().collect()
}

#[test]
fn basic_first1() {
    let grammar = normalized_grammar(
        r#"
    grammar;
    A = B "C";
    B: Option<u32> = {
        "D" => Some(1),
        => None
    };
    X = "E"; // intentionally unreachable
"#,
    );
    let _lr1_tls = Lr1Tls::install(grammar.terminals.clone());
    let first_sets = FirstSets::new(&grammar);

    assert_eq!(first1(&first_sets, &[nt("A")], EOF), vec![la("C"), la("D")]);

    assert_eq!(first1(&first_sets, &[nt("B")], EOF), vec![la("D"), EOF]);

    assert_eq!(
        first1(&first_sets, &[nt("B"), term("E")], EOF),
        vec![la("D"), la("E")]
    );

    assert_eq!(
        first1(&first_sets, &[nt("B"), nt("X")], EOF),
        vec![la("D"), la("E")]
    );
}

#[test]
fn basic_first0() {
    let grammar = normalized_grammar(
        r#"
    grammar;
    A = B "C";
    B: Option<u32> = {
        "D" => Some(1),
        => None
    };
    X = "E"; // intentionally unreachable
"#,
    );
    let _lr1_tls = Lr1Tls::install(grammar.terminals.clone());
    let first_sets = FirstSets::new(&grammar);

    assert_eq!(first0(&first_sets, &[nt("A")]), vec![la("C"), la("D")]);

    assert_eq!(first0(&first_sets, &[nt("B")]), vec![la("D"), EOF]);

    assert_eq!(
        first0(&first_sets, &[nt("B"), term("E")]),
        vec![la("D"), la("E")]
    );

    assert_eq!(
        first0(&first_sets, &[nt("B"), nt("X")]),
        vec![la("D"), la("E")]
    );

    assert_eq!(first0(&first_sets, &[nt("X")]), vec![la("E")]);
}

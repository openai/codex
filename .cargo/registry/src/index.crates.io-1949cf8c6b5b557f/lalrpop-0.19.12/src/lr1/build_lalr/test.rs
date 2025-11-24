use super::super::interpret::interpret;
use super::build_lalr_states;
use crate::grammar::repr::*;
use crate::lr1::tls::Lr1Tls;
use string_cache::DefaultAtom as Atom;
use crate::test_util::normalized_grammar;
use crate::tls::Tls;

fn nt(t: &str) -> NonterminalString {
    NonterminalString(Atom::from(t))
}

macro_rules! tokens {
    ($($x:expr),*) => {
        vec![$(TerminalString::quoted(Atom::from($x))),*]
    }
}

#[test]
fn figure9_23() {
    let _tls = Tls::test();

    let grammar = normalized_grammar(
        r#"
        grammar;
        extern { enum Tok { "-" => .., "N" => .., "(" => .., ")" => .. } }
        S: () = E       => ();
        E: () = {
            E "-" T     => (),
            T           => ()
        };
        T: () = {
            "N"         => (),
            "(" E ")"   => ()
        };
   "#,
    );

    let _lr1_tls = Lr1Tls::install(grammar.terminals.clone());

    let states = build_lalr_states(&grammar, nt("S")).unwrap();
    println!("{:#?}", states);

    let tree = interpret(&states, tokens!["N", "-", "(", "N", "-", "N", ")"]).unwrap();
    assert_eq!(
        &format!("{:?}", tree)[..],
        r#"[S: [E: [E: [T: "N"]], "-", [T: "(", [E: [E: [T: "N"]], "-", [T: "N"]], ")"]]]"#
    );
}

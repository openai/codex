use crate::grammar::repr::*;
use crate::lr1::build_states;
use crate::lr1::core::*;
use crate::lr1::first::FirstSets;
use crate::lr1::tls::Lr1Tls;
use string_cache::DefaultAtom as Atom;
use crate::test_util::{expect_debug, normalized_grammar};
use crate::tls::Tls;

use super::super::Tracer;

fn nt(t: &str) -> NonterminalString {
    NonterminalString(Atom::from(t))
}

#[test]
fn shift_backtrace_1() {
    // This grammar yields a S/R conflict. Is it `(int -> int) -> int`
    // or `int -> (int -> int)`?

    let _tls = Tls::test();
    let grammar = normalized_grammar(
        r#"
grammar;
pub Ty: () = {
    "int" => (),
    "bool" => (),
    <t1:Ty> "->" <t2:Ty> => (),
};
"#,
    );
    let _lr1_tls = Lr1Tls::install(grammar.terminals.clone());
    let first_sets = FirstSets::new(&grammar);
    let err = build_states(&grammar, nt("Ty")).unwrap_err();
    let conflict = err.conflicts[0].clone();
    println!("conflict={:?}", conflict);

    // Gin up the LR0 item involved in the shift/reduce conflict:
    //
    //     Ty = Ty (*) -> Ty (shift)
    //
    // from the item that we can reduce:
    //
    //     Ty = Ty -> Ty (*) (reduce)

    assert!(conflict.production.symbols.len() == 3);
    let item = Item::lr0(conflict.production, 1);
    println!("item={:?}", item);
    let tracer = Tracer::new(&first_sets, &err.states);
    let graph = tracer.backtrace_shift(conflict.state, item);
    expect_debug(
        &graph,
        r#"
[
    (Nonterminal(Ty) -([], Some(Ty), ["->", Ty])-> Nonterminal(Ty)),
    (Nonterminal(Ty) -([Ty], Some("->"), [Ty])-> Item(Ty = Ty (*) "->" Ty)),
    (Item(Ty = Ty "->" (*) Ty) -([Ty, "->"], Some(Ty), [])-> Nonterminal(Ty))
]
"#
        .trim(),
    );

    let list: Vec<_> = graph
        .lr0_examples(item)
        .map(|example| example.paint_unstyled())
        .collect();
    expect_debug(
        &list,
        r#"
[
    [
        "  Ty "->" Ty "->" Ty",
        "  │       └─Ty─────┤",
        "  └─Ty─────────────┘"
    ]
]
"#
        .trim(),
    );
}

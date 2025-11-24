use crate::grammar::repr::*;
use crate::lr1::build_states;
use crate::lr1::core::Item;
use crate::lr1::first::FirstSets;
use crate::lr1::interpret::interpret_partial;
use crate::lr1::lookahead::{Token, TokenSet};
use crate::lr1::tls::Lr1Tls;
use string_cache::DefaultAtom as Atom;
use crate::test_util::{expect_debug, normalized_grammar};
use crate::tls::Tls;

use super::super::Tracer;

fn nt(t: &str) -> NonterminalString {
    NonterminalString(Atom::from(t))
}

fn term(t: &str) -> TerminalString {
    TerminalString::quoted(Atom::from(t))
}

macro_rules! terms {
    ($($t:expr),*) => {
        vec![$(term($t)),*]
    }
}

fn test_grammar1() -> Grammar {
    normalized_grammar(
        r#"
    grammar;

    pub Start: () = Stmt;

    pub Stmt: () = {
        Exprs ";",
        Exprs
    };

    Exprs: () = {
        Expr,
        Exprs "," Expr
    };

    Expr: () = {
        "Int",
        Expr "+" "Int",
    };
"#,
    )
}

#[test]
fn backtrace1() {
    let _tls = Tls::test();
    let grammar = test_grammar1();
    let _lr1_tls = Lr1Tls::install(grammar.terminals.clone());
    let first_sets = FirstSets::new(&grammar);
    let states = build_states(&grammar, nt("Start")).unwrap();
    let tracer = Tracer::new(&first_sets, &states);
    let state_stack = interpret_partial(&states, terms!["Int"]).unwrap();
    let top_state = *state_stack.last().unwrap();

    // Top state will have items like:
    //
    // Expr = "Int" (*) [EOF],
    // Expr = "Int" (*) ["+"],
    // Expr = "Int" (*) [","],
    // Expr = "Int" (*) [";"]
    //
    // Select the ";" one.
    let semi = Token::Terminal(term(";"));
    let semi_item = states[top_state.0]
        .items
        .vec
        .iter().find(|item| item.lookahead.contains(&semi))
        .unwrap();

    let backtrace = tracer.backtrace_reduce(top_state, semi_item.to_lr0());

    println!("{:#?}", backtrace);

    let pictures: Vec<_> = backtrace
        .lr0_examples(semi_item.to_lr0())
        .map(|e| e.paint_unstyled())
        .collect();
    expect_debug(
        &pictures,
        r#"
[
    [
        "  Exprs "," "Int"  ╷ ";"",
        "  │         └─Expr─┤   │",
        "  ├─Exprs──────────┘   │",
        "  └─Stmt───────────────┘"
    ],
    [
        "  Exprs "," "Int"  ╷ "," Expr",
        "  │         └─Expr─┤        │",
        "  ├─Exprs──────────┘        │",
        "  └─Exprs───────────────────┘"
    ],
    [
        "  "Int"   ╷ ";"",
        "  ├─Expr──┤   │",
        "  ├─Exprs─┘   │",
        "  └─Stmt──────┘"
    ],
    [
        "  "Int"   ╷ "," Expr",
        "  ├─Expr──┤        │",
        "  ├─Exprs─┘        │",
        "  └─Exprs──────────┘"
    ],
    [
        "  "Int"  ╷ "+" "Int"",
        "  ├─Expr─┘         │",
        "  └─Expr───────────┘"
    ]
]
"#
        .trim(),
    );
}

#[test]
fn backtrace2() {
    let _tls = Tls::test();
    // This grammar yields a S/R conflict. Is it (int -> int) -> int
    // or int -> (int -> int)?
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
    let tracer = Tracer::new(&first_sets, &err.states);
    let conflict = err.conflicts[0].clone();
    println!("conflict={:?}", conflict);
    let item = Item {
        production: conflict.production,
        index: conflict.production.symbols.len(),
        lookahead: conflict.lookahead.clone(),
    };
    println!("item={:?}", item);
    let backtrace = tracer.backtrace_reduce(conflict.state, item.to_lr0());
    println!("{:#?}", backtrace);
    expect_debug(
        &backtrace,
        r#"
[
    (Nonterminal(Ty) -([Ty, "->"], Some(Ty), [])-> Item(Ty = Ty "->" (*) Ty)),
    (Nonterminal(Ty) -([Ty, "->"], Some(Ty), [])-> Nonterminal(Ty)),
    (Nonterminal(Ty) -([Ty, "->", Ty], None, [])-> Item(Ty = Ty "->" Ty (*))),
    (Item(Ty = (*) Ty "->" Ty) -([], Some(Ty), ["->", Ty])-> Nonterminal(Ty))
]
"#
        .trim(),
    );

    // Check that we can successfully enumerate and paint the examples
    // here.
    let pictures: Vec<_> = backtrace
        .lr1_examples(&first_sets, &item)
        .map(|e| e.paint_unstyled())
        .collect();
    expect_debug(
        &pictures,
        r#"
[
    [
        "  Ty "->" Ty "->" Ty",
        "  ├─Ty─────┘       │",
        "  └─Ty─────────────┘"
    ]
]
"#
        .trim(),
    );
}

#[test]
fn reduce_backtrace_3_graph() {
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
    let item = Item {
        production: conflict.production,
        index: conflict.production.symbols.len(),
        lookahead: conflict.lookahead.clone(),
    };
    println!("item={:?}", item);
    let tracer = Tracer::new(&first_sets, &err.states);
    let graph = tracer.backtrace_reduce(conflict.state, item.to_lr0());
    expect_debug(
        &graph,
        r#"
[
    (Nonterminal(Ty) -([Ty, "->"], Some(Ty), [])-> Item(Ty = Ty "->" (*) Ty)),
    (Nonterminal(Ty) -([Ty, "->"], Some(Ty), [])-> Nonterminal(Ty)),
    (Nonterminal(Ty) -([Ty, "->", Ty], None, [])-> Item(Ty = Ty "->" Ty (*))),
    (Item(Ty = (*) Ty "->" Ty) -([], Some(Ty), ["->", Ty])-> Nonterminal(Ty))
]
"#
        .trim(),
    );

    let list: Vec<_> = graph
        .lr1_examples(&first_sets, &item)
        .map(|example| example.paint_unstyled())
        .collect();
    expect_debug(
        &list,
        r#"
[
    [
        "  Ty "->" Ty "->" Ty",
        "  ├─Ty─────┘       │",
        "  └─Ty─────────────┘"
    ]
]
"#
        .trim(),
    );
}

#[test]
fn backtrace_filter() {
    let _tls = Tls::test();
    let grammar = normalized_grammar(
        r#"
    grammar;

    pub Start: () = Stmt;

    pub Stmt: () = {
        Exprs ";"
    };

    Exprs: () = {
        Expr,
        Exprs "," Expr
    };

    Expr: () = {
        ExprAtom ExprSuffix
    };

    ExprSuffix: () = {
        (),
        "?",
    };

    ExprAtom: () = {
        "Int",
    };
"#,
    );
    let _lr1_tls = Lr1Tls::install(grammar.terminals.clone());
    let states = build_states(&grammar, nt("Start")).unwrap();
    let first_sets = FirstSets::new(&grammar);
    let tracer = Tracer::new(&first_sets, &states);
    let state_stack = interpret_partial(&states, terms!["Int"]).unwrap();
    let top_state = *state_stack.last().unwrap();

    // Top state will have an item like:
    //
    // Expr = "Int" (*) [",", ";"],
    let semi = Token::Terminal(term(";"));
    let lr1_item = states[top_state.0]
        .items
        .vec
        .iter().find(|item| item.lookahead.contains(&semi))
        .unwrap();

    let backtrace = tracer.backtrace_reduce(top_state, lr1_item.to_lr0());

    println!("{:#?}", backtrace);

    // With no filtering, we get examples with both `;` and `,` as
    // lookahead (though `ExprSuffix` is in the way).
    let pictures: Vec<_> = backtrace
        .lr0_examples(lr1_item.to_lr0())
        .map(|e| e.paint_unstyled())
        .collect();
    expect_debug(
        &pictures,
        r#"
[
    [
        "  Exprs "," "Int"      ╷ ExprSuffix ";"",
        "  │         ├─ExprAtom─┘          │   │",
        "  │         └─Expr────────────────┤   │",
        "  ├─Exprs─────────────────────────┘   │",
        "  └─Stmt──────────────────────────────┘"
    ],
    [
        "  Exprs "," "Int"      ╷ ExprSuffix "," Expr",
        "  │         ├─ExprAtom─┘          │        │",
        "  │         └─Expr────────────────┤        │",
        "  ├─Exprs─────────────────────────┘        │",
        "  └─Exprs──────────────────────────────────┘"
    ],
    [
        "  "Int"      ╷ ExprSuffix ";"",
        "  ├─ExprAtom─┘          │   │",
        "  ├─Expr────────────────┤   │",
        "  ├─Exprs───────────────┘   │",
        "  └─Stmt────────────────────┘"
    ],
    [
        "  "Int"      ╷ ExprSuffix "," Expr",
        "  ├─ExprAtom─┘          │        │",
        "  ├─Expr────────────────┤        │",
        "  ├─Exprs───────────────┘        │",
        "  └─Exprs────────────────────────┘"
    ]
]
"#
        .trim(),
    );

    // Select those with `;` as lookahead
    let semi_item = lr1_item.with_lookahead(TokenSet::from(semi));
    let pictures: Vec<_> = backtrace
        .lr1_examples(&first_sets, &semi_item)
        .map(|e| e.paint_unstyled())
        .collect();
    expect_debug(
        &pictures,
        r#"
[
    [
        "  Exprs "," "Int"      ╷ ExprSuffix ";"",
        "  │         ├─ExprAtom─┘          │   │",
        "  │         └─Expr────────────────┤   │",
        "  ├─Exprs─────────────────────────┘   │",
        "  └─Stmt──────────────────────────────┘"
    ],
    [
        "  "Int"      ╷ ExprSuffix ";"",
        "  ├─ExprAtom─┘          │   │",
        "  ├─Expr────────────────┤   │",
        "  ├─Exprs───────────────┘   │",
        "  └─Stmt────────────────────┘"
    ]
]
"#
        .trim(),
    );
}

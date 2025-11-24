use crate::grammar::repr::*;
use crate::lr1::core::*;
use string_cache::DefaultAtom as Atom;
use crate::test_util::expect_debug;
use crate::tls::Tls;

macro_rules! nt {
    ($x:ident) => {
        NonterminalString(Atom::from(stringify!($x)))
    };
}

macro_rules! syms {
    ($($x:ident),*) => {
        vec![$(Symbol::Nonterminal(nt!($x))),*]
    }
}

macro_rules! production {
    ($x:ident = $($y:ident)*) => {
        Production {
            nonterminal: nt!($x),
            symbols: syms![$($y),*],
            action: ActionFn::new(0),
            span: Span(0, 0)
        }
    }
}

use super::TraceGraph;

#[test]
fn enumerator() {
    let _tls = Tls::test();

    // Build this graph:
    //
    //     X = X0 (*) X1
    //     ^
    //     |
    //   {X0}
    //     |
    // +-> X <-- Z = Z0 (*) X Z1
    // |
    // Y = Y0 (*) X Y1
    //
    // which enumerates out to:
    //
    //    [Y0 X0 (*) X1 Y1]
    //    [Z0 X0 (*) X1 Z1]

    let productions = vec![
        production![X = X0 X1],
        production![Y = Y0 X Y1],
        production![Z = Z0 X Z1],
    ];

    let mut graph = TraceGraph::new();

    let item0 = Item::lr0(&productions[0], 1); // X = X0 (*) X1
    graph.add_edge(nt!(X), item0, item0.symbol_sets());

    let item1 = Item::lr0(&productions[1], 1); // Y = Y0 (*) X Y1
    graph.add_edge(item1, nt!(X), item1.symbol_sets());

    let item2 = Item::lr0(&productions[2], 1); // Z = Z0 (*) X Z1
    graph.add_edge(item2, nt!(X), item2.symbol_sets());

    let enumerator = graph.lr0_examples(Item::lr0(&productions[0], 1));
    let list: Vec<_> = enumerator.map(|example| example.paint_unstyled()).collect();
    expect_debug(
        &list,
        r#"
[
    [
        "  Z0 X0 X1 Z1",
        "  │  └─X─┘  │",
        "  └─Z───────┘"
    ],
    [
        "  Y0 X0 X1 Y1",
        "  │  └─X─┘  │",
        "  └─Y───────┘"
    ]
]
"#
        .trim(),
    );
}

#[test]
fn enumerator1() {
    let _tls = Tls::test();

    // Build this graph:
    //
    //     W = W0 W1 (*)
    //     ^
    //  {W0,W1}
    //     |
    //     W
    //     ^
    //   {X0}
    //     |
    // +-> X <-- Z = Z0 (*) X Z1
    // |
    // Y = Y0 (*) X Y1
    //
    // which enumerates out to:
    //
    //    [Y0 X0 (*) X1 Y1]
    //    [Z0 X0 (*) X1 Z1]

    let productions = vec![
        production![W = W0 W1],
        production![X = X0 W X1], // where X1 may be empty
        production![Y = Y0 X Y1],
        production![Z = Z0 X Z1],
    ];

    let mut graph = TraceGraph::new();

    let item0 = Item::lr0(&productions[0], 2); // W = W0 W1 (*)
    graph.add_edge(nt!(W), item0, item0.symbol_sets());

    graph.add_edge(
        nt!(X),
        nt!(W),
        SymbolSets {
            prefix: &productions[1].symbols[..1],
            cursor: Some(&productions[1].symbols[1]),
            suffix: &productions[1].symbols[2..],
        },
    );

    let item1 = Item::lr0(&productions[2], 1);
    graph.add_edge(item1, nt!(X), item1.symbol_sets());

    let item2 = Item::lr0(&productions[3], 1);
    graph.add_edge(item2, nt!(X), item2.symbol_sets());

    let enumerator = graph.lr0_examples(Item::lr0(&productions[0], 2));
    let list: Vec<_> = enumerator.map(|example| example.paint_unstyled()).collect();
    expect_debug(
        &list,
        r#"
[
    [
        "  Z0 X0 W0 W1 X1 Z1",
        "  │  │  └─W─┘  │  │",
        "  │  └─X───────┘  │",
        "  └─Z─────────────┘"
    ],
    [
        "  Y0 X0 W0 W1 X1 Y1",
        "  │  │  └─W─┘  │  │",
        "  │  └─X───────┘  │",
        "  └─Y─────────────┘"
    ]
]
"#
        .trim(),
    );
}

use crate::grammar::repr::*;
use crate::lr1::build;
use crate::lr1::build_states;
use crate::lr1::core::*;
use crate::lr1::first::FirstSets;
use crate::lr1::interpret;
use crate::lr1::state_graph::StateGraph;
use crate::lr1::tls::Lr1Tls;
use string_cache::DefaultAtom as Atom;
use crate::test_util::{expect_debug, normalized_grammar};
use crate::tls::Tls;

use super::construct::*;
use super::lane::*;
use super::table::*;

macro_rules! tokens {
    ($($x:expr),*) => {
        vec![$(TerminalString::quoted(Atom::from($x))),*]
    }
}

fn sym(t: &str) -> Symbol {
    if t.chars().next().unwrap().is_uppercase() {
        Symbol::Nonterminal(nt(t))
    } else {
        Symbol::Terminal(term(t))
    }
}

fn term(t: &str) -> TerminalString {
    TerminalString::quoted(Atom::from(t))
}

fn nt(t: &str) -> NonterminalString {
    NonterminalString(Atom::from(t))
}

fn traverse(states: &[LR0State], tokens: &[&str]) -> StateIndex {
    interpret::interpret_partial(states, tokens.iter().map(|&s| term(s)))
        .unwrap()
        .pop()
        .unwrap()
}

/// A simplified version of the paper's initial grammar; this version
/// only has one inconsistent state (the same state they talk about in
/// the paper).
pub fn paper_example_g0() -> Grammar {
    normalized_grammar(
        r#"
grammar;

pub G: () = {
    X "c",
    Y "d",
};

X: () = {
    "e" X,
    "e",}
;

Y: () = {
    "e" Y,
    "e"
};
"#,
    )
}

/// A (corrected) version of the sample grammar G1 from the paper. The
/// grammar as written in the text omits some items, but the diagrams
/// seem to contain the full set. I believe this is one of the
/// smallest examples that still requires splitting states from the
/// LR0 states.
pub fn paper_example_g1() -> Grammar {
    normalized_grammar(
        r#"
grammar;

pub G: () = {
    // if "a" is input, then lookahead "d" means "reduce X"
    // and lookahead "c" means "reduce "Y"
    "a" X "d",
    "a" Y "c",

    // if "b" is input, then lookahead "d" means "reduce Y"
    // and lookahead "c" means "reduce X.
    "b" X "c",
    "b" Y "d",
};

X: () = {
    "e" X,
    "e",
};

Y: () = {
    "e" Y,
    "e"
};
"#,
    )
}

fn build_table<'grammar>(
    grammar: &'grammar Grammar,
    goal: &str,
    tokens: &[&str],
) -> LaneTable<'grammar> {
    let lr0_err = build::build_lr0_states(grammar, nt(goal)).unwrap_err();

    // Push the `tokens` to find the index of the inconsistent state
    let inconsistent_state_index = traverse(&lr0_err.states, tokens);
    assert!(lr0_err
        .conflicts
        .iter()
        .any(|c| c.state == inconsistent_state_index));
    let inconsistent_state = &lr0_err.states[inconsistent_state_index.0];
    println!("inconsistent_state={:#?}", inconsistent_state.items);

    // Extract conflicting items and trace the lanes, constructing a table
    let conflicting_items = super::conflicting_actions(inconsistent_state);
    println!("conflicting_items={:#?}", conflicting_items);
    let first_sets = FirstSets::new(grammar);
    let state_graph = StateGraph::new(&lr0_err.states);
    let mut tracer = LaneTracer::new(
        grammar,
        nt("G"),
        &lr0_err.states,
        &first_sets,
        &state_graph,
        conflicting_items.len(),
    );
    for (i, conflicting_item) in conflicting_items.iter().enumerate() {
        tracer.start_trace(
            inconsistent_state.index,
            ConflictIndex::new(i),
            conflicting_item.clone(),
        );
    }

    tracer.into_table()
}

#[test]
fn g0_conflict_1() {
    let _tls = Tls::test();
    let grammar = paper_example_g0();
    let _lr1_tls = Lr1Tls::install(grammar.terminals.clone());
    let table = build_table(&grammar, "G", &["e"]);
    println!("{:#?}", table);
    // conflicting_actions={
    //     Shift("e") // C0
    //     Reduce(X = "e" => ActionFn(4)) // C1
    //     Reduce(Y = "e" => ActionFn(6)) // C2
    // }
    expect_debug(
        &table,
        r#"
| State | C0    | C1    | C2    | Successors |
| S0    |       | ["c"] | ["d"] | {S3}       |
| S3    | ["e"] | []    | []    | {S3}       |
"#
        .trim_start(),
    );
}

#[test]
fn paper_example_g1_conflict_1() {
    let _tls = Tls::test();
    let grammar = paper_example_g1();
    let _lr1_tls = Lr1Tls::install(grammar.terminals.clone());
    let table = build_table(&grammar, "G", &["a", "e"]);
    println!("{:#?}", table);
    // conflicting_actions={
    //     Shift("e") // C0
    //     Reduce(X = "e" => ActionFn(6)) // C1
    //     Reduce(Y = "e" => ActionFn(8)) // C2
    // }
    expect_debug(
        &table,
        r#"
| State | C0    | C1    | C2    | Successors |
| S1    |       | ["d"] | ["c"] | {S5}       |
| S2    |       | ["c"] | ["d"] | {S5}       |
| S5    | ["e"] | []    | []    | {S5}       |
"#
        .trim_start(),
    );
}

#[test]
fn paper_example_g0_build() {
    let _tls = Tls::test();
    let grammar = paper_example_g0();
    let _lr1_tls = Lr1Tls::install(grammar.terminals.clone());
    let lr0_err = build::build_lr0_states(&grammar, nt("G")).unwrap_err();
    let states = LaneTableConstruct::new(&grammar, nt("G"))
        .construct()
        .expect("failed to build lane table states");

    // we do not require more *states* than LR(0), just different lookahead
    assert_eq!(states.len(), lr0_err.states.len());

    let tree = interpret::interpret(&states, tokens!["e", "c"]).unwrap();
    expect_debug(&tree, r#"[G: [X: "e"], "c"]"#);

    let tree = interpret::interpret(&states, tokens!["e", "e", "c"]).unwrap();
    expect_debug(&tree, r#"[G: [X: "e", [X: "e"]], "c"]"#);

    let tree = interpret::interpret(&states, tokens!["e", "e", "d"]).unwrap();
    expect_debug(&tree, r#"[G: [Y: "e", [Y: "e"]], "d"]"#);

    interpret::interpret(&states, tokens!["e", "e", "e"]).unwrap_err();
}

#[test]
fn paper_example_g1_build() {
    let _tls = Tls::test();
    let grammar = paper_example_g1();
    let _lr1_tls = Lr1Tls::install(grammar.terminals.clone());
    let lr0_err = build::build_lr0_states(&grammar, nt("__G")).unwrap_err();
    let states = build_states(&grammar, nt("__G")).expect("failed to build lane table states");

    // we require more *states* than LR(0), not just different lookahead
    assert_eq!(states.len() - lr0_err.states.len(), 1);

    let tree = interpret::interpret(&states, tokens!["a", "e", "e", "d"]).unwrap();
    expect_debug(&tree, r#"[__G: [G: "a", [X: "e", [X: "e"]], "d"]]"#);

    let tree = interpret::interpret(&states, tokens!["b", "e", "e", "d"]).unwrap();
    expect_debug(&tree, r#"[__G: [G: "b", [Y: "e", [Y: "e"]], "d"]]"#);

    interpret::interpret(&states, tokens!["e", "e", "e"]).unwrap_err();
}

pub fn paper_example_large() -> Grammar {
    normalized_grammar(
        r#"
grammar;

pub G: () = {
    "x" W "a",
    "x" V "t",
    "y" W "b",
    "y" V "t",
    "z" W "r",
    "z" V "b",
    "u" U X "a",
    "u" U Y "r",
};

W: () = {
    U X C
};

V: () = {
    U Y "d"
};

X: () = {
    "k" "t" U X P,
    "k" "t"
};

Y: () = {
    "k" "t" U Y "u",
    "k" "t"
};

U: () = {
    U "k" "t",
    "s"
};

E: () = {
    "a",
    "b",
    "c",
    "v",
};

C: () = {
    "c",
    "w"
};

P: () = {
    "z"
};
"#,
    )
}

#[test]
fn large_conflict_1() {
    let _tls = Tls::test();
    let grammar = paper_example_large();
    let _lr1_tls = Lr1Tls::install(grammar.terminals.clone());
    let table = build_table(&grammar, "G", &["x", "s", "k", "t"]);
    println!("{:#?}", table);

    // conflicting_actions={
    //     Shift("s") // C0
    //     Reduce(U = U "k" "t") // C1
    //     Reduce(X = "k" "t") // C2
    //     Reduce(Y = "k" "t") // C3
    // }

    expect_debug(
        &table,
        r#"
| State | C0    | C1    | C2         | C3    | Successors |
| S1    |       | ["k"] |            |       | {S5}       |
| S2    |       | ["k"] |            |       | {S7}       |
| S3    |       | ["k"] |            |       | {S7}       |
| S4    |       | ["k"] |            |       | {S7}       |
| S5    |       |       | ["a"]      | ["r"] | {S16}      |
| S7    |       |       | ["c", "w"] | ["d"] | {S16}      |
| S16   |       |       |            |       | {S27}      |
| S27   | ["s"] | ["k"] |            |       | {S32}      |
| S32   |       |       | ["z"]      | ["u"] | {S16}      |
"#
        .trim_start(),
    );

    // ^^ This differs in some particulars from what appears in the
    // paper, but I believe it to be correct, and the paper to be wrong.
    //
    // Here is the table using the state names from the paper. I've
    // marked the differences with `(*)`. Note that the paper does not
    // include the C0 column (the shift).
    //
    // | State | pi1   | pi2   | pi3        | Successors |
    // | B     | ["k"] |       | *1         | {G}        |
    // | C     | ["k"] |       | *1         | {G}        |
    // | D     | ["k"] |       | *1         | {G}        |
    // | E     | ["k"] |       |            | {F}        |
    // | F     |       | ["r"] | ["a"]      | {H}        |
    // | G     |       | ["d"] | ["c", "w"] | {H}        |
    // | H     |       |       |            | {I}        |
    // | I     | ["k"] |       |            | {J}        |
    // | J     |       | ["u"] | ["z"] *2   | {H}        |
    //
    // *1 - the paper lists "a", "b", and "r" here as lookaheads.  We
    // do not. This is because when we trace back pi3, we never reach
    // those states, as we have already acquired the necessary token
    // of context earlier. I can imagine a distinct lane tracing
    // algorithm that considers *sets* of conflicts and only
    // terminates when all sets have context, but it's much more
    // complex to implement, and seems to add little value.
    //
    // *2 - the paper does not list this context, and yet it seems to
    // present. If you trace back "t" and "k" you reach state J which
    // has the item "X = k t (*)". This "unepsilons" to "X = k t U (*)
    // X P", and the lookahead from the "X" here is FIRST(P) which is
    // "z".
}

#[test]
fn paper_example_large_build() {
    let _tls = Tls::test();
    let grammar = paper_example_large();
    let _lr1_tls = Lr1Tls::install(grammar.terminals.clone());
    let states = LaneTableConstruct::new(&grammar, nt("G"))
        .construct()
        .expect("failed to build lane table states");

    let tree = interpret::interpret(&states, tokens!["y", "s", "k", "t", "c", "b"]).unwrap();
    expect_debug(
        &tree,
        r#"[G: "y", [W: [U: "s"], [X: "k", "t"], [C: "c"]], "b"]"#,
    );
}

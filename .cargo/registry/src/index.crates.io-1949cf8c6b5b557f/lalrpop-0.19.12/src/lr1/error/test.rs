use crate::grammar::repr::*;
use crate::lr1::build_states;
use crate::lr1::tls::Lr1Tls;
use string_cache::DefaultAtom as Atom;
use crate::test_util::normalized_grammar;
use crate::tls::Tls;

use super::{ConflictClassification, ErrorReportingCx};

fn nt(t: &str) -> NonterminalString {
    NonterminalString(Atom::from(t))
}

#[test]
fn priority_conflict() {
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
    let err = build_states(&grammar, nt("Ty")).unwrap_err();
    let mut cx = ErrorReportingCx::new(&grammar, &err.states, &err.conflicts);
    let conflicts = super::token_conflicts(&err.conflicts);
    let conflict = &conflicts[0];

    println!("conflict={:?}", conflict);

    match cx.classify(conflict) {
        ConflictClassification::Precedence {
            shift,
            reduce,
            nonterminal,
        } => {
            println!(
                "shift={:#?}, reduce={:#?}, nonterminal={:?}",
                shift, reduce, nonterminal
            );
            assert_eq!(shift.symbols.len(), 5); // Ty -> Ty -> Ty
            assert_eq!(shift.cursor, 3); // Ty -> Ty -> Ty
            assert_eq!(shift.symbols, reduce.symbols);
            assert_eq!(shift.cursor, reduce.cursor);
            assert_eq!(nonterminal, nt("Ty"));
        }
        r => panic!("wrong classification {:#?}", r),
    }
}

#[test]
fn expr_braced_conflict() {
    let _tls = Tls::test();
    let grammar = normalized_grammar(
        r#"
grammar;
pub Expr: () = {
    "Id" => (),
    "Id" "{" "}" => (),
    "Expr" "+" "Id" => (),
    "if" Expr "{" "}" => (),
};
"#,
    );
    let _lr1_tls = Lr1Tls::install(grammar.terminals.clone());
    let err = build_states(&grammar, nt("Expr")).unwrap_err();
    let mut cx = ErrorReportingCx::new(&grammar, &err.states, &err.conflicts);
    let conflicts = super::token_conflicts(&err.conflicts);
    let conflict = &conflicts[0];

    println!("conflict={:?}", conflict);

    match cx.classify(conflict) {
        ConflictClassification::InsufficientLookahead { .. } => {}
        r => panic!("wrong classification {:#?}", r),
    }
}

#[test]
fn suggest_question_conflict() {
    let _tls = Tls::test();
    let grammar = normalized_grammar(
        r#"
        grammar;

        pub E: () = {
            "L",
            "&" OPT_L E
        };

        OPT_L: () = {
            (),
            "L"
        };
"#,
    );
    let _lr1_tls = Lr1Tls::install(grammar.terminals.clone());
    let err = build_states(&grammar, nt("E")).unwrap_err();
    let mut cx = ErrorReportingCx::new(&grammar, &err.states, &err.conflicts);
    let conflicts = super::token_conflicts(&err.conflicts);
    let conflict = &conflicts[0];

    println!("conflict={:?}", conflict);

    match cx.classify(conflict) {
        ConflictClassification::SuggestQuestion {
            shift: _,
            reduce: _,
            nonterminal,
            symbol,
        } => {
            assert_eq!(nonterminal, nt("OPT_L"));
            assert_eq!(
                symbol,
                Symbol::Terminal(TerminalString::quoted(Atom::from("L")))
            );
        }
        r => panic!("wrong classification {:#?}", r),
    }
}

#[test]
fn suggest_inline_conflict() {
    let _tls = Tls::test();
    let grammar = normalized_grammar(
        r##"
grammar;

pub ImportDecl: () = {
    "import" <Path> ";" => (),
    "import" <Path> "." "*" ";" => (),
};

Path: () = {
    <head: Ident> <tail: ("." <Ident>)*> => ()
};

Ident = r#"[a-zA-Z][a-zA-Z0-9]*"#;
"##,
    );
    let _lr1_tls = Lr1Tls::install(grammar.terminals.clone());
    let err = build_states(&grammar, nt("ImportDecl")).unwrap_err();
    let mut cx = ErrorReportingCx::new(&grammar, &err.states, &err.conflicts);
    let conflicts = super::token_conflicts(&err.conflicts);
    let conflict = &conflicts[0];

    println!("conflict={:?}", conflict);

    match cx.classify(conflict) {
        ConflictClassification::SuggestInline {
            shift: _,
            reduce: _,
            nonterminal,
        } => {
            assert_eq!(nonterminal, nt("Path"));
        }
        r => panic!("wrong classification {:#?}", r),
    }
}

/// This example used to cause an out-of-bounds error.
#[test]
fn issue_249() {
    let _tls = Tls::test();
    let grammar = normalized_grammar(
        r##"
grammar;

pub Func = StructDecl* VarDecl*;
StructDecl = "<" StructParameter* ">";
StructParameter = "may_dangle"?;
VarDecl = "let";
"##,
    );
    let _lr1_tls = Lr1Tls::install(grammar.terminals.clone());
    let err = build_states(&grammar, nt("Func")).unwrap_err();
    let mut cx = ErrorReportingCx::new(&grammar, &err.states, &err.conflicts);
    let conflicts = super::token_conflicts(&err.conflicts);
    for conflict in &conflicts {
        println!("conflict={:?}", conflict);
        cx.classify(conflict);
    }
}

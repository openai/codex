use crate::grammar::parse_tree::NonterminalString;
use crate::grammar::repr::Grammar;
use crate::normalize::{self, NormResult};
use crate::parser;
use crate::session::Session;
use string_cache::DefaultAtom as Atom;

use super::inline;

fn inlined_grammar(text: &str) -> NormResult<Grammar> {
    let g = parser::parse_grammar(text).unwrap();
    let g = normalize::lower_helper(&Session::test(), g, true).unwrap();
    inline(g)
}

#[test]
fn sri() {
    // This grammar gets a shift-reduce conflict because if the input
    // is "&" (*) "L", then we see two possibilities, and we must decide
    // between them:
    //
    // "&" (*) "L" E
    //  |       |  |
    //  +-------+--|
    //          |
    //          E
    //
    // or
    //
    // "&"      (*) "L"
    //  |            |
    //  |  OPT_L     E
    //  |   |        |
    //  +---+---+----+
    //          |
    //          E
    //
    // to some extent this may be a false conflict, in that inlined
    // rules would address it, but it's an interesting one for
    // producing a useful error message.

    let grammar = inlined_grammar(
        r#"
        grammar;

        E: () = {
            "L",
            "&" OPT_L E
        };

        #[inline] OPT_L: () = {
            (),
            "L"
        };
    "#,
    )
    .unwrap();

    let nt = NonterminalString(Atom::from("E"));

    // After inlining, we expect:
    //
    // E = "L"
    // E = "&" E
    // E = "&" "L" E
    //
    // Note that the `()` also gets inlined.
    let e_productions = grammar.productions_for(&nt);
    assert_eq!(e_productions.len(), 3);
    assert_eq!(format!("{:?}", e_productions[0].symbols), r#"["L"]"#);
    assert_eq!(format!("{:?}", e_productions[1].symbols), r#"["&", E]"#);
    assert_eq!(
        format!("{:?}", e_productions[2].symbols),
        r#"["&", "L", E]"#
    );
}

#[test]
fn issue_55() {
    let grammar = inlined_grammar(
        r#"
grammar;

pub E: () = {
    "X" "{" <a:AT*> <e:ET> <b:AT*> "}" => ()
};

AT: () = {
    "type" ";" => ()
};

ET: () = {
    "enum" "{" "}" => ()
};
    "#,
    )
    .unwrap();
    let nt = NonterminalString(Atom::from("E"));

    // The problem in issue #55 was that we would inline both `AT*`
    // the same way, so we ended up with `E = X { ET }` and `E = X {
    // AT+ ET AT+ }` but not `E = X { AT+ ET }` or `E = X { ET AT+ }`.
    assert!(grammar.productions_for(&nt).len() == 4);
}

use crate::grammar::parse_tree::NonterminalString;
use crate::grammar::repr::TypeRepr;
use crate::normalize::macro_expand::expand_macros;
use crate::normalize::token_check;
use crate::normalize::tyinfer::infer_types;
use crate::parser;
use string_cache::DefaultAtom as Atom;

fn type_repr(s: &str) -> TypeRepr {
    let type_ref = parser::parse_type_ref(s).unwrap();
    type_ref.type_repr()
}

fn compare(g1: &str, expected: Vec<(&'static str, &'static str)>) {
    let grammar = parser::parse_grammar(g1).unwrap();
    let grammar = expand_macros(grammar).unwrap();
    let grammar = token_check::validate(grammar).unwrap();
    let types = infer_types(&grammar).unwrap();

    println!("types table: {:?}", types);

    for (nt_id, nt_type) in expected {
        let id = NonterminalString(Atom::from(nt_id));
        let ty = type_repr(nt_type);
        println!("expected type of {:?} is {:?}", id, ty);
        assert_eq!(types.nonterminal_type(&id), &ty);
    }
}

#[test]
fn test_pairs_and_tokens() {
    compare(
        r#"
grammar;
    extern { enum Tok { "Hi" => Hi(..), "Ho" => Ho(..) } }
    X = Y Z;
    Y: Foo = "Hi";
    Z = "Ho";
"#,
        vec![("X", "(Foo, Tok)"), ("Y", "Foo"), ("Z", "Tok")],
    )
}

#[test]
fn test_cycle_direct() {
    let grammar = parser::parse_grammar(
        r#"
grammar;
    extern { enum Tok { "Hi" => Hi(..), "Ho" => Ho(..) } }
    X = {
        X Y,
        <Y> => vec![<>]
    };
    Y = "Hi";
"#,
    )
    .unwrap();

    let actual = expand_macros(grammar).unwrap();
    assert!(infer_types(&actual).is_err());
}

#[test]
fn test_cycle_indirect() {
    let grammar = parser::parse_grammar(
        r#"
grammar;
    extern { enum Tok { } }
    A = B;
    B = C;
    C = D;
    D = A;
"#,
    )
    .unwrap();

    let actual = expand_macros(grammar).unwrap();
    assert!(infer_types(&actual).is_err());
}

#[test]
fn test_macro_expansion() {
    compare(
        r#"
grammar;
    extern { enum Tok { "Id" => Id(..) } }
    Two<X>: (X, X) = X X;
    Ids = Two<"Id">;
"#,
        vec![("Ids", "(Tok, Tok)"), (r#"Two<"Id">"#, "(Tok, Tok)")],
    )
}

#[test]
fn test_macro_expansion_infer() {
    compare(
        r#"
grammar;
    extern { enum Tok { "Id" => Id(..) } }
    Two<X> = X X;
    Ids = Two<"Id">;
"#,
        vec![("Ids", "(Tok, Tok)"), (r#"Two<"Id">"#, "(Tok, Tok)")],
    )
}

#[test]
fn test_type_question() {
    compare(
        r#"
grammar;
    extern { enum Tok { "Hi" => Hi(..) } }
    X = Y?;
    Y = "Hi";
"#,
        vec![("X", "core::option::Option<Tok>"), ("Y", "Tok")],
    )
}

#[test]
fn test_star_plus_question() {
    compare(
        r#"
grammar;
    extern { enum Tok { "Hi" => Hi(..) } }
    A = Z*;
    X = "Hi"*;
    Y = "Hi"+;
    Z = "Hi"?;
"#,
        vec![
            ("A", "alloc::vec::Vec<core::option::Option<Tok>>"),
            ("X", "alloc::vec::Vec<Tok>"),
            ("Y", "alloc::vec::Vec<Tok>"),
            ("Z", "core::option::Option<Tok>"),
        ],
    )
}

#[test]
fn test_lookahead() {
    compare(
        r#"
grammar;
    extern { type Location = usize; enum Tok { } }
    A = @L;
"#,
        vec![("A", "usize")],
    )
}

#[test]
fn test_spanned_macro() {
    compare(
        r#"
        grammar;
        extern { type Location = usize; enum Tok { "Foo" => Foo(..) } }
        A = Spanned<"Foo">;
        Spanned<T> = {
            @L T @R
        };
"#,
        vec![("A", "(usize, Tok, usize)")],
    )
}

#[test]
fn test_action() {
    compare(
        r#"
grammar;
    extern { enum Tok { "+" => .., "foo" => .. } }

    X = {
        Y,
        <l:X> "+" <r:Y> => l + r
    };

    Y: i32 = "foo" => 22;
"#,
        vec![("X", "i32"), ("Y", "i32")],
    )
}

#[test]
fn test_inconsistent_action() {
    let grammar = parser::parse_grammar(
        r#"
grammar;
    extern { enum Tok { "+" => .., "foo" => .., "bar" => .. } }

    X = {
        Y,
        Z,
        <l:X> "+" <r:Y> => l + r
    };

    Y: i32 = "foo" => 22;

    Z: u32 = "bar" => 22;
"#,
    )
    .unwrap();

    let actual = expand_macros(grammar).unwrap();
    assert!(infer_types(&actual).is_err());
}

#[test]
fn custom_token() {
    compare(
        r#"
grammar;
extern { enum Tok { N => N(<u32>) } }
A = N;
"#,
        vec![("A", "u32")],
    )
}

#[test]
fn intern_token() {
    compare(
        r#"
grammar;
    Z = @L "Ho" @R;
"#,
        vec![("Z", "(usize, &'input str, usize)")],
    )
}

#[test]
fn error() {
    compare(
        r#"
grammar;
    Z = !;
"#,
        vec![(
            "Z",
            "__lalrpop_util::ErrorRecovery<usize, Token<'input>, &'static str>",
        )],
    )
}

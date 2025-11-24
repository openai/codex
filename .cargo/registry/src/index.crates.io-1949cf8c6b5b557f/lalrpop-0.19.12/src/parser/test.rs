use crate::grammar::parse_tree::{GrammarItem, MatchItem};
use crate::parser;

#[test]
fn match_block() {
    let blocks = vec![
        r#"grammar; match { _ }"#,                     // Minimal
        r#"grammar; match { _ } else { _ }"#, // Doesn't really make sense, but should be allowed
        r#"grammar; match { "abc" }"#,        // Single token
        r#"grammar; match { "abc" => "QUOTED" }"#, // Single token with quoted alias
        r#"grammar; match { "abc" => UNQUOTED }"#, // Single token with unquoted alias
        r#"grammar; match { r"(?i)begin" => BEGIN }"#, // Regex
        r#"grammar; match { "abc", "def" => "DEF", _ } else { "foo" => BAR, r"(?i)begin" => BEGIN, _ }"#, // Complex
        r#"grammar; match { "abc" } else { "def" } else { _ }"#, // Multi-chain
    ];

    for block in blocks {
        let parsed = parser::parse_grammar(block)
            .unwrap_or_else(|_| panic!("Invalid grammar; grammar={}", block));
        let first_item = parsed.items.first().expect("has item");
        match *first_item {
            GrammarItem::MatchToken(_) => (), // OK
            _ => panic!("expected MatchToken, but was {:?}", first_item),
        }
    }
}

#[test]
fn match_complex() {
    let parsed = parser::parse_grammar(
        r#"
        grammar;
        match {
            r"(?i)begin" => "BEGIN",
            r"(?i)end" => "END",
        } else {
            r"[a-zA-Z_][a-zA-Z0-9_]*" => IDENTIFIER,
        } else {
            "other",
            _
        }
"#,
    )
    .unwrap();

    // We could probably make some nice system for testing this
    let first_item = parsed.items.first().expect("has item");
    match *first_item {
        GrammarItem::MatchToken(ref data) => {
            // match { ... }
            let contents0 = data.contents.get(0).unwrap();
            // r"(?i)begin" => "BEGIN"
            let item00 = contents0.items.get(0).unwrap();
            match *item00 {
                MatchItem::Mapped(ref sym, ref mapping, _) => {
                    assert_eq!(format!("{:?}", sym), "r#\"(?i)begin\"#");
                    assert_eq!(format!("{}", mapping), "\"BEGIN\"");
                }
                _ => panic!("expected MatchItem::Mapped, but was: {:?}", item00),
            };
            // r"(?i)end" => "END",
            let item01 = contents0.items.get(1).unwrap();
            match *item01 {
                MatchItem::Mapped(ref sym, ref mapping, _) => {
                    assert_eq!(format!("{:?}", sym), "r#\"(?i)end\"#");
                    assert_eq!(format!("{}", mapping), "\"END\"");
                }
                _ => panic!("expected MatchItem::Mapped, but was: {:?}", item00),
            };
            // else { ... }
            let contents1 = data.contents.get(1).unwrap();
            // r"[a-zA-Z_][a-zA-Z0-9_]*" => IDENTIFIER,
            let item10 = contents1.items.get(0).unwrap();
            match *item10 {
                MatchItem::Mapped(ref sym, ref mapping, _) => {
                    assert_eq!(format!("{:?}", sym), "r#\"[a-zA-Z_][a-zA-Z0-9_]*\"#");
                    assert_eq!(format!("{}", mapping), "IDENTIFIER");
                }
                _ => panic!("expected MatchItem::Mapped, but was: {:?}", item10),
            };
            // else { ... }
            let contents2 = data.contents.get(2).unwrap();
            // "other",
            let item20 = contents2.items.get(0).unwrap();
            match *item20 {
                MatchItem::Unmapped(ref sym, _) => {
                    assert_eq!(format!("{:?}", sym), "\"other\"");
                }
                _ => panic!("expected MatchItem::Unmapped, but was: {:?}", item20),
            };
            // _
            let item21 = contents2.items.get(1).unwrap();
            match *item21 {
                MatchItem::CatchAll(_) => (),
                _ => panic!("expected MatchItem::CatchAll, but was: {:?}", item20),
            };
        }
        _ => panic!("expected MatchToken, but was: {:?}", first_item),
    }
}

#[test]
fn where_clauses() {
    let clauses = vec![
        "where T: Debug",
        "where T: Debug + Display",
        "where T: std::ops::Add<usize>",
        "where T: IntoIterator<Item = usize>",
        "where T: 'a",
        "where 'a: 'b",
        "where for<'a> &'a T: Debug",
        "where T: for<'a> Flobbles<'a>",
        "where T: FnMut(usize)",
        "where T: FnMut(usize, bool)",
        "where T: FnMut() -> bool",
        "where T: for<'a> FnMut(&'a usize)",
        "where T: Debug, U: Display",
    ];

    for santa in clauses {
        assert!(
            parser::parse_where_clauses(santa).is_ok(),
            "should parse where clauses: {}",
            santa
        );
    }
}

#[test]
fn grammars_with_where_clauses() {
    let grammars = vec![
        r###"
grammar<T> where T: StaticMethods;
"###,
        r###"
grammar<T>(methods: &mut T) where T: MutMethods;
"###,
        r###"
grammar<'input, T>(methods: &mut T) where T: 'input + Debug + MutMethods;
"###,
        r###"
grammar<F>(methods: &mut F) where F: for<'a> FnMut(&'a usize) -> bool;
"###,
        r###"
grammar<F>(logger: &mut F) where F: for<'a> FnMut(&'a str);
"###,
    ];

    for g in grammars {
        assert!(parser::parse_grammar(g).is_ok());
    }
}

#[test]
fn optional_semicolon() {
    // Semi after block is optional
    let g = r#"
grammar;
pub Foo: () = { Bar }
Bar: () = "bar";
"#;
    assert!(parser::parse_grammar(g).is_ok());

    // Semi after "expression" is mandatory
    let g = r#"
grammar;
pub Foo: () = { Bar };
Bar: () = "bar"
"#;
    assert!(parser::parse_grammar(g).is_err());
}

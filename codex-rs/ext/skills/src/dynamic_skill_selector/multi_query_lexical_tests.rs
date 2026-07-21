use super::*;
use pretty_assertions::assert_eq;

#[test]
fn multi_query_promotes_each_clause_leader() {
    let documents = [
        document(/*id*/ 1, "rust-format", "Format Rust source code."),
        document(/*id*/ 2, "rust-lint", "Fix Rust source lint errors."),
        document(/*id*/ 3, "rust-review", "Review Rust source code."),
        document(
            /*id*/ 4,
            "ci-fix",
            "Diagnose failing GitHub Actions checks.",
        ),
    ];

    let selection = MultiQueryLexicalSkillSelector.select(
        "format and review Rust source code, and then diagnose failing GitHub Actions checks",
        &documents,
        /*limit*/ 4,
    );

    assert!(selection.candidate_ids[..3].contains(&1));
    assert!(selection.candidate_ids[..3].contains(&4));
}

#[test]
fn single_query_matches_the_underlying_selector() {
    let documents = [
        document(/*id*/ 1, "presentations", "Create visual decks."),
        document(/*id*/ 2, "spreadsheets", "Analyze tabular data."),
    ];

    let expected =
        WeightedLexicalSkillSelector.select("create presentations", &documents, /*limit*/ 20);
    let actual = MultiQueryLexicalSkillSelector.select(
        "create presentations",
        &documents,
        /*limit*/ 20,
    );

    assert_eq!(expected, actual);
}

#[test]
fn query_views_split_sentences_and_connectors() {
    assert_eq!(
        vec![
            "format code and then fix tests; write a summary",
            "format code",
            "fix tests",
            "write a summary",
        ],
        query_views("format code and then fix tests; write a summary"),
    );
}

#[test]
fn multi_query_preserves_bounded_input_signals() {
    let long_query = format!("{}\nand inspect logs", "match ".repeat(4 * 1024));
    let names = (0..=1_000)
        .map(|index| format!("candidate-{index}"))
        .collect::<Vec<_>>();
    let documents = names
        .iter()
        .enumerate()
        .map(|(id, name)| SkillSelectionDocument {
            id,
            name,
            short_description: None,
            description: "match logs",
            dependencies: None,
        })
        .collect::<Vec<_>>();

    let selection =
        MultiQueryLexicalSkillSelector.select(&long_query, &documents, /*limit*/ 20);

    assert!(selection.query_truncated);
    assert!(selection.candidate_set_truncated);
    assert_eq!(20, selection.candidate_ids.len());
}

fn document<'a>(id: usize, name: &'a str, description: &'a str) -> SkillSelectionDocument<'a> {
    SkillSelectionDocument {
        id,
        name,
        short_description: None,
        description,
        dependencies: None,
    }
}

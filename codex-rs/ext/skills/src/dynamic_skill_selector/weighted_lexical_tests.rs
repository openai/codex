use super::*;
use pretty_assertions::assert_eq;

#[test]
fn lexical_selector_prioritizes_an_exact_skill_name() {
    let documents = [
        SkillSelectionDocument {
            id: 10,
            name: "slides-helper",
            short_description: None,
            description: "Create presentations and visual decks.",
        },
        SkillSelectionDocument {
            id: 20,
            name: "presentations",
            short_description: None,
            description: "Create or edit PowerPoint presentations.",
        },
        SkillSelectionDocument {
            id: 30,
            name: "spreadsheets",
            short_description: None,
            description: "Analyze tabular data.",
        },
    ];

    let selection = WeightedLexicalSkillSelector.select(
        "Use presentations to create a deck",
        &documents,
        /*limit*/ 20,
    );

    assert_eq!(vec![20, 10], selection.candidate_ids);
    assert!(!selection.query_truncated);
    assert!(!selection.candidate_set_truncated);
}

#[test]
fn lexical_selector_uses_descriptions_and_drops_zero_score_candidates() {
    let documents = [
        SkillSelectionDocument {
            id: 1,
            name: "ci-helper",
            short_description: Some("Diagnose continuous integration failures."),
            description: "Inspect failing GitHub Actions checks and logs.",
        },
        SkillSelectionDocument {
            id: 2,
            name: "document-editor",
            short_description: None,
            description: "Edit Word documents.",
        },
    ];

    let selection = WeightedLexicalSkillSelector.select(
        "Please diagnose the failing GitHub Actions check",
        &documents,
        /*limit*/ 20,
    );

    assert_eq!(vec![1], selection.candidate_ids);
}

#[test]
fn lexical_selector_respects_requested_limit() {
    let names = (0..10)
        .map(|index| format!("lint-{index}"))
        .collect::<Vec<_>>();
    let documents = names
        .iter()
        .enumerate()
        .map(|(id, name)| SkillSelectionDocument {
            id,
            name,
            short_description: None,
            description: "Fix lint errors.",
        })
        .collect::<Vec<_>>();

    let selection =
        WeightedLexicalSkillSelector.select("fix lint errors", &documents, /*limit*/ 3);

    assert_eq!(3, selection.candidate_ids.len());
}

#[test]
fn lexical_selector_reports_bounded_inputs() {
    let long_query = "match ".repeat(MAX_QUERY_BYTES);
    let names = (0..=MAX_CANDIDATES)
        .map(|index| format!("candidate-{index}"))
        .collect::<Vec<_>>();
    let documents = names
        .iter()
        .enumerate()
        .map(|(id, name)| SkillSelectionDocument {
            id,
            name,
            short_description: None,
            description: "match",
        })
        .collect::<Vec<_>>();

    let selection = WeightedLexicalSkillSelector.select(&long_query, &documents, /*limit*/ 20);

    assert!(selection.query_truncated);
    assert!(selection.candidate_set_truncated);
    assert_eq!(20, selection.candidate_ids.len());
    assert!(
        selection
            .candidate_ids
            .iter()
            .all(|id| *id < MAX_CANDIDATES)
    );
}

#[test]
fn lexical_selector_caps_query_terms() {
    let query = (0..=MAX_QUERY_TERMS)
        .map(|index| format!("term{index}"))
        .collect::<Vec<_>>()
        .join(" ");
    let documents = [SkillSelectionDocument {
        id: 1,
        name: "term0",
        short_description: None,
        description: "term0",
    }];

    let selection = WeightedLexicalSkillSelector.select(&query, &documents, /*limit*/ 20);

    assert_eq!(MAX_QUERY_TERMS, selection.query_term_count);
    assert!(selection.query_truncated);
}

#[test]
fn lexical_selector_returns_nothing_for_stop_words_only() {
    let documents = [SkillSelectionDocument {
        id: 1,
        name: "anything",
        short_description: None,
        description: "Do anything.",
    }];

    let selection =
        WeightedLexicalSkillSelector.select("please use the", &documents, /*limit*/ 20);

    assert_eq!(CheapSkillSelection::default(), selection);
}

#[test]
fn selector_can_be_used_behind_the_shared_trait() {
    fn run(selector: &dyn CheapSkillSelector) -> CheapSkillSelection {
        selector.select(
            "review code",
            &[SkillSelectionDocument {
                id: 7,
                name: "code-review",
                short_description: None,
                description: "Review code.",
            }],
            /*limit*/ 20,
        )
    }

    assert_eq!(vec![7], run(&WeightedLexicalSkillSelector).candidate_ids);
}

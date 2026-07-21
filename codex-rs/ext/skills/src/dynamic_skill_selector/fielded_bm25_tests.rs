use super::*;
use pretty_assertions::assert_eq;

#[test]
fn bm25_prioritizes_rare_terms() {
    let documents = [
        document(/*id*/ 1, "review-helper", "Review code and prose."),
        document(
            /*id*/ 2,
            "terraform-review",
            "Review Terraform infrastructure.",
        ),
        document(/*id*/ 3, "document-review", "Review Word documents."),
    ];

    let selection =
        FieldedBm25SkillSelector.select("review terraform", &documents, /*limit*/ 20);

    assert_eq!(vec![2, 3, 1], selection.candidate_ids);
}

#[test]
fn bm25_weights_names_above_descriptions() {
    let documents = [
        document(/*id*/ 1, "slides", "Create presentations."),
        document(/*id*/ 2, "presentations", "Create and edit slides."),
    ];

    let selection = FieldedBm25SkillSelector.select("slides", &documents, /*limit*/ 20);

    assert_eq!(vec![1, 2], selection.candidate_ids);
}

#[test]
fn bm25_drops_candidates_without_matching_terms() {
    let documents = [document(
        /*id*/ 1,
        "spreadsheets",
        "Analyze tabular data.",
    )];

    let selection =
        FieldedBm25SkillSelector.select("render a video", &documents, /*limit*/ 20);

    assert!(selection.candidate_ids.is_empty());
}

#[test]
fn bm25_reports_bounded_inputs() {
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
            dependencies: None,
        })
        .collect::<Vec<_>>();

    let selection = FieldedBm25SkillSelector.select(&long_query, &documents, /*limit*/ 20);

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

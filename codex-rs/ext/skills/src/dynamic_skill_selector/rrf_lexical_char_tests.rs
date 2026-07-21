use pretty_assertions::assert_eq;

use super::*;

#[test]
fn fusion_prefers_candidates_supported_by_both_rankings() {
    let fused = fuse_rankings([&[1, 2, 3], &[2, 3, 4]], /*limit*/ 4);

    assert_eq!(vec![2, 3, 1, 4], fused);
}

#[test]
fn fusion_uses_rank_and_id_to_break_ties() {
    let fused = fuse_rankings([&[20, 10], &[30]], /*limit*/ 3);

    assert_eq!(vec![20, 30, 10], fused);
}

#[test]
fn selector_reports_the_combined_input_bounds() {
    let documents = [SkillSelectionDocument {
        id: 7,
        name: "presentations",
        short_description: None,
        description: "Create visual decks.",
        dependencies: None,
    }];
    let query = "presentation ".repeat(4 * 1024);

    let selection = RrfLexicalCharSkillSelector.select(&query, &documents, /*limit*/ 20);

    assert_eq!(vec![7], selection.candidate_ids);
    assert!(selection.query_truncated);
    assert!(!selection.candidate_set_truncated);
}

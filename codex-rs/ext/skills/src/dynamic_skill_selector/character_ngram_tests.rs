use super::*;
use pretty_assertions::assert_eq;

#[test]
fn ngrams_match_related_word_forms() {
    let documents = [
        document(/*id*/ 1, "presentations", "Create visual decks."),
        document(/*id*/ 2, "spreadsheets", "Analyze tabular data."),
    ];

    let selection =
        CharacterNgramSkillSelector.select("create a presentation", &documents, /*limit*/ 20);

    assert_eq!(vec![1], selection.candidate_ids);
}

#[test]
fn ngrams_tolerate_a_typo() {
    let documents = [
        document(/*id*/ 1, "postgresql", "Manage a relational database."),
        document(/*id*/ 2, "postscript", "Render printable documents."),
    ];

    let selection = CharacterNgramSkillSelector.select(
        "repair my postgrez database",
        &documents,
        /*limit*/ 20,
    );

    assert_eq!(vec![1, 2], selection.candidate_ids);
}

#[test]
fn ngrams_match_cjk_without_word_boundaries() {
    let documents = [
        document(/*id*/ 1, "演示文稿", "创建幻灯片。"),
        document(/*id*/ 2, "电子表格", "分析表格数据。"),
    ];

    let selection =
        CharacterNgramSkillSelector.select("帮我制作演示文稿", &documents, /*limit*/ 20);

    assert_eq!(vec![1], selection.candidate_ids);
}

#[test]
fn ngrams_report_bounded_inputs() {
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

    let selection = CharacterNgramSkillSelector.select(&long_query, &documents, /*limit*/ 20);

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
    }
}

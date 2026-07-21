use codex_skills::SkillDependencies;
use codex_skills::SkillToolDependency;
use pretty_assertions::assert_eq;

use super::*;

#[test]
fn dependency_names_outrank_description_only_matches() {
    let dependencies = SkillDependencies {
        tools: vec![SkillToolDependency {
            r#type: "app".to_string(),
            value: "slack".to_string(),
            description: Some("Send messages to Slack channels.".to_string()),
            transport: None,
            command: None,
            url: None,
        }],
    };
    let documents = [
        SkillSelectionDocument {
            id: 1,
            name: "team-messaging",
            short_description: None,
            description: "Work with team communication.",
            dependencies: Some(&dependencies),
        },
        document(
            /*id*/ 2,
            "documents",
            "Send messages to Slack channels.",
        ),
    ];

    let selection = RoutingCardLexicalSkillSelector.select(
        "send this update to Slack",
        &documents,
        /*limit*/ 20,
    );

    assert_eq!(vec![1, 2], selection.candidate_ids);
}

#[test]
fn stop_word_name_requires_an_exact_query() {
    let documents = [document(/*id*/ 1, "do", "Run a task.")];

    let selection = RoutingCardLexicalSkillSelector.select("do", &documents, /*limit*/ 20);
    assert_eq!(vec![1], selection.candidate_ids);
    assert_eq!(0, selection.query_term_count);

    let selection = RoutingCardLexicalSkillSelector.select(
        "how do I format Rust",
        &documents,
        /*limit*/ 20,
    );
    assert_eq!(Vec::<usize>::new(), selection.candidate_ids);
}

#[test]
fn prefixes_do_not_match_field_terms() {
    let documents = [document(
        /*id*/ 1,
        "presentations",
        "Create presentations.",
    )];

    let selection =
        RoutingCardLexicalSkillSelector.select("present", &documents, /*limit*/ 20);

    assert_eq!(Vec::<usize>::new(), selection.candidate_ids);
}

#[test]
fn dependency_records_are_bounded() {
    let mut tools = (0..MAX_DEPENDENCY_RECORDS)
        .map(|_| SkillToolDependency {
            r#type: "app".to_string(),
            value: "shared".to_string(),
            description: None,
            transport: None,
            command: None,
            url: None,
        })
        .collect::<Vec<_>>();
    tools.push(SkillToolDependency {
        r#type: "app".to_string(),
        value: "unbounded".to_string(),
        description: None,
        transport: None,
        command: None,
        url: None,
    });
    let dependencies = SkillDependencies { tools };
    let document = SkillSelectionDocument {
        id: 1,
        name: "dependency-test",
        short_description: None,
        description: "Test dependencies.",
        dependencies: Some(&dependencies),
    };

    assert_eq!(
        HashSet::from(["shared".to_string()]),
        dependency_terms(&document)
    );
}

#[test]
fn selector_reports_bounded_inputs() {
    let documents = [document(
        /*id*/ 1,
        "presentations",
        "Create visual decks.",
    )];
    let query = "presentations ".repeat(MAX_QUERY_BYTES);

    let selection = RoutingCardLexicalSkillSelector.select(&query, &documents, /*limit*/ 20);

    assert_eq!(vec![1], selection.candidate_ids);
    assert!(selection.query_truncated);
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

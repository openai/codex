use codex_skills::SkillDependencies;
use codex_skills::SkillToolDependency;
use pretty_assertions::assert_eq;

use super::*;

#[test]
fn cold_start_uses_character_routing_metadata() {
    let dependencies = SkillDependencies {
        tools: vec![SkillToolDependency {
            r#type: "app".to_string(),
            value: "slack".to_string(),
            description: None,
            transport: None,
            command: None,
            url: None,
            oauth_callback_port: None,
        }],
    };
    let documents = [
        document(/*id*/ 10, "compilation", "Compile Rust crates."),
        SkillSelectionDocument {
            dependencies: Some(&dependencies),
            ..document(/*id*/ 20, "team-communication", "Share team updates.")
        },
    ];
    let expected = CheapSkillSelection {
        candidate_ids: vec![20],
        query_term_count: 1,
        ..Default::default()
    };

    for selector in selectors(Vec::new(), CharacterRoutingCardSkillSelector::default()) {
        let method = selector.method();
        assert_eq!(
            expected,
            selector.select("slack", &documents, /*limit*/ 50),
            "{method}"
        );
    }
}

#[test]
fn recent_and_text_matches_are_merged_without_duplicates() {
    let documents = [
        document(/*id*/ 10, "rust-build", "Inspect compiler errors."),
        document(
            /*id*/ 20,
            "python-tools",
            "Manage Python environments.",
        ),
        document(/*id*/ 30, "monorepo", "Repository conventions."),
    ];
    let expected = CheapSkillSelection {
        candidate_ids: vec![20, 30, 10],
        query_term_count: 1,
        ..Default::default()
    };

    for selector in selectors(
        vec![30, 10, 20, 20, 999],
        CharacterRoutingCardSkillSelector::default(),
    ) {
        let method = selector.method();
        assert_eq!(
            expected,
            selector.select("python", &documents, /*limit*/ 50),
            "{method}"
        );
    }
}

#[test]
fn three_way_fusion_retains_short_exact_matches() {
    let documents = [
        document(/*id*/ 10, "ci", ""),
        document(/*id*/ 20, "python", ""),
        document(/*id*/ 30, "rust", ""),
    ];
    let expected_rankings = [vec![20, 30], vec![20, 10, 30]];

    for (selector, candidate_ids) in
        selectors(vec![30], CharacterRoutingCardSkillSelector::default())
            .into_iter()
            .zip(expected_rankings)
    {
        let method = selector.method();
        assert_eq!(
            CheapSkillSelection {
                candidate_ids,
                query_term_count: 2,
                ..Default::default()
            },
            selector.select("ci python", &documents, /*limit*/ 50),
            "{method}"
        );
    }
}

#[test]
fn fusion_is_deterministic_capped_and_reports_input_bounds() {
    let documents = (0..1_001)
        .map(|id| document(id, "python", ""))
        .collect::<Vec<_>>();
    let query = "python ".repeat(4 * 1024);
    let expected_rankings = [
        [0, 999].into_iter().chain(1..49).collect::<Vec<_>>(),
        (0..11)
            .chain(std::iter::once(999))
            .chain(11..49)
            .collect::<Vec<_>>(),
    ];

    for (selector, candidate_ids) in selectors(
        vec![1_000, 999, 999, 1_002],
        CharacterRoutingCardSkillSelector::default(),
    )
    .into_iter()
    .zip(expected_rankings)
    {
        let method = selector.method();
        let expected = CheapSkillSelection {
            candidate_ids,
            query_term_count: 64,
            query_truncated: true,
            candidate_set_truncated: true,
        };
        assert_eq!(
            expected,
            selector.select(&query, &documents, /*limit*/ 50),
            "{method}"
        );
        assert_eq!(
            expected,
            selector.select(&query, &documents, usize::MAX),
            "{method}"
        );
        assert_eq!(
            CheapSkillSelection {
                candidate_ids: expected.candidate_ids[..7].to_vec(),
                ..expected
            },
            selector.select(&query, &documents, /*limit*/ 7),
            "{method}"
        );
        assert_eq!(
            CheapSkillSelection::default(),
            selector.select(&query, &documents, /*limit*/ 0),
            "{method}"
        );
    }
}

fn selectors(
    recent_skill_ids: Vec<usize>,
    character_selector: CharacterRoutingCardSkillSelector,
) -> [Box<dyn CheapSkillSelector>; 2] {
    let lru_selector = LruSkillSelector::new(recent_skill_ids);
    [
        Box::new(LruPlusCharacterRoutingSkillSelector::new(
            lru_selector.clone(),
            character_selector.clone(),
        )),
        Box::new(LruPlusLexicalCharacterRoutingSkillSelector::new(
            lru_selector,
            character_selector,
        )),
    ]
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

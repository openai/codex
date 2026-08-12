use pretty_assertions::assert_eq;

use super::*;

#[test]
fn lru_selector_preserves_recent_invocation_order() {
    let documents = [
        SkillSelectionDocument {
            id: 10,
            name: "ci",
            short_description: None,
            description: "Investigate failing checks.",
            dependencies: None,
        },
        SkillSelectionDocument {
            id: 20,
            name: "python-tools",
            short_description: None,
            description: "Manage Python environments.",
            dependencies: None,
        },
        SkillSelectionDocument {
            id: 30,
            name: "monorepo",
            short_description: None,
            description: "Follow repository conventions.",
            dependencies: None,
        },
    ];

    let selection =
        LruSkillSelector::new(vec![30, 10]).select("continue", &documents, /*limit*/ 50);

    assert_eq!(vec![30, 10], selection.candidate_ids);
}

#[test]
fn lru_selector_filters_stale_or_duplicate_skills_and_respects_the_limit() {
    let documents = [
        SkillSelectionDocument {
            id: 10,
            name: "ci",
            short_description: None,
            description: "Investigate failing checks.",
            dependencies: None,
        },
        SkillSelectionDocument {
            id: 20,
            name: "python-tools",
            short_description: None,
            description: "Manage Python environments.",
            dependencies: None,
        },
    ];

    let selection =
        LruSkillSelector::new(vec![99, 10, 10, 20]).select("yes", &documents, /*limit*/ 1);

    assert_eq!(vec![10], selection.candidate_ids);
}

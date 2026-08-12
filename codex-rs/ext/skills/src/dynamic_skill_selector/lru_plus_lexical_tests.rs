use pretty_assertions::assert_eq;

use super::*;

#[test]
fn combined_selector_uses_lexical_results_before_invocation_history_exists() {
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

    let selection = LruPlusLexicalSkillSelector::new(LruSkillSelector::default()).select(
        "manage python environments",
        &documents,
        /*limit*/ 50,
    );

    assert_eq!(vec![20], selection.candidate_ids);
}

#[test]
fn combined_selector_merges_recent_and_newly_matching_skills() {
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

    let selection = LruPlusLexicalSkillSelector::new(LruSkillSelector::new(vec![30, 10])).select(
        "manage python environments",
        &documents,
        /*limit*/ 50,
    );

    assert_eq!(vec![20, 30, 10], selection.candidate_ids);
}

#[test]
fn combined_selector_promotes_skills_supported_by_both_signals() {
    let documents = [
        SkillSelectionDocument {
            id: 10,
            name: "ci",
            short_description: None,
            description: "Investigate failing checks.",
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

    let selection = LruPlusLexicalSkillSelector::new(LruSkillSelector::new(vec![30, 10])).select(
        "investigate failing ci checks",
        &documents,
        /*limit*/ 50,
    );

    assert_eq!(vec![10, 30], selection.candidate_ids);
}

#[test]
fn combined_fusion_keeps_a_top_ranked_skill_ahead_of_a_weak_overlap() {
    let recent = std::iter::once(1)
        .chain(100..148)
        .chain(std::iter::once(2))
        .collect::<Vec<_>>();
    let lexical = (200..249).chain(std::iter::once(2)).collect::<Vec<_>>();

    let fused = fuse_rankings_with_constant(
        [&recent, &lexical],
        /*limit*/ MAX_RESULTS,
        /*rank_constant*/ RRF_K,
    );
    let top_ranked = fused
        .iter()
        .position(|candidate| *candidate == 1)
        .expect("top-ranked skill should be selected");
    let weak_overlap = fused
        .iter()
        .position(|candidate| *candidate == 2)
        .expect("weak overlapping skill should be selected");

    assert!(top_ranked < weak_overlap);
}

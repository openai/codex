#![cfg(test)]

use super::*;
use crate::ContextPresentation;
use crate::ContextProfile;
use crate::ElasticSectionLimits;
use crate::PreviousReview;
use crate::PreviousReviews;
use crate::RequestBudget;
use crate::TrustedSkills;
use crate::TrustedTool;
use crate::TrustedToolSource;
use codex_protocol::ResponseItemId;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;

#[test]
fn classifier_entry_caps_preserve_whole_evidence_and_only_sent_review_ids() {
    const ENTRY_CAP: usize = 100;
    const SECTION_MINIMUM: usize = 4_000;
    const OVERSIZED_BYTES: usize = 500;
    const NOTICE: &str = "Evidence omitted for this request.";
    let kept_review = PreviousReview {
        id: ResponseItemId::from_server("review_kept".to_owned()),
        fragment: "Approved reading README.md.".to_owned(),
    };
    let omitted_review = PreviousReview {
        id: ResponseItemId::from_server("review_omitted".to_owned()),
        fragment: format!("Reviewed: {}", "x".repeat(OVERSIZED_BYTES)),
    };
    let reviews =
        PreviousReviews::try_from_fragments(vec![omitted_review, kept_review.clone()]).unwrap();
    let tool = TrustedTool {
        server: "files".to_owned(),
        connector_id: None,
        source: TrustedToolSource::PluginServiceOrchestrator,
    };
    let skill = "/skills/read-only/SKILL.md";
    let context = CollectedContext {
        sections: vec![
            ContextSection::TrustedTool(tool),
            ContextSection::TrustedSkills(TrustedSkills {
                paths: vec![
                    format!("/skills/{}", "x".repeat(OVERSIZED_BYTES)),
                    skill.to_owned(),
                ],
            }),
            ContextSection::PreviousReviews(reviews),
        ],
    };
    let profile = ContextProfile::asynchronous();
    let fixed = crate::prepare_async_context(
        &context,
        &profile,
        /*entry_number_offset*/ 0,
        ContextPresentation::Async,
        &[],
        /*elastic*/ None,
    )
    .unwrap();
    assert!(fixed.truncations.is_empty());
    let fixed = fixed.into_annotated_messages();
    assert_eq!(
        fixed
            .iter()
            .filter(|item| item
                .metadata
                .as_ref()
                .is_some_and(|metadata| metadata.guardian_review_ids.len() > 1,))
            .count(),
        1
    );

    let limits = ElasticSectionLimits::new(SECTION_MINIMUM, ENTRY_CAP).unwrap();
    let config = crate::budget::tests::complete_minimum_budget(ElasticBudgetConfig {
        sections: [
            (
                ElasticSection::TrustedTools,
                ElasticSectionLimits::new(SECTION_MINIMUM, /*max_section_entry_tokens*/ 1).unwrap(),
            ),
            (ElasticSection::TrustedSkills, limits),
            (ElasticSection::PreviousReviews, limits),
        ]
        .into(),
        ..Default::default()
    });
    let capped = crate::prepare_async_context(
        &context,
        &profile,
        /*entry_number_offset*/ 0,
        ContextPresentation::Async,
        &[],
        Some(&config),
    )
    .unwrap();
    assert_eq!(
        capped
            .truncations
            .iter()
            .map(|loss| loss.component)
            .collect::<Vec<_>>(),
        vec!["trusted_tool", "trusted_skills", "previous_reviews"],
    );
    let delivered = capped
        .prepare_elastic_entries(&config)
        .unwrap()
        .prepare_delivery(&[])
        .unwrap()
        .enforce_budget(
            RequestBudget {
                max_input_tokens: crate::DEFAULT_MAX_INPUT_TOKENS,
                existing_context_tokens: 0,
            },
            NOTICE.to_owned(),
        )
        .unwrap()
        .into_annotated_messages();
    let expected_review = PreviousReviews::try_from_fragments(vec![kept_review])
        .unwrap()
        .into_annotated_message();
    assert!(delivered.contains(&expected_review));
    let developers = delivered
        .iter()
        .filter_map(|envelope| match &envelope.item {
            ResponseItem::Message { role, .. } if role == "developer" => Some(&envelope.item),
            _ => None,
        })
        .collect::<Vec<_>>();
    let expected_skill = codex_context_fragments::ContextualUserFragment::into(TrustedSkills {
        paths: vec![skill.to_owned()],
    });
    assert_eq!(developers, vec![&expected_review.item, &expected_skill]);
    assert!(
        serde_json::to_string(
            &delivered
                .iter()
                .map(|envelope| &envelope.item)
                .collect::<Vec<_>>()
        )
        .unwrap()
        .contains(NOTICE)
    );
}

//! Constructor boundaries preserve valid evidence and reject oversize sections.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn previous_reviews_checks_count_and_size_without_rewriting_fragments() {
    let max_bytes = TruncationPolicy::Tokens(MAX_REVIEW_FRAGMENT_TOKENS).byte_budget();
    let fragment = "é".repeat(max_bytes / 2);
    let fragments = vec![
        PreviousReview {
            id: ResponseItemId::new("review"),
            fragment: fragment.clone(),
        };
        MAX_PREVIOUS_REVIEWS
    ];
    let reviews = PreviousReviews::try_from_fragments(fragments.clone()).unwrap();
    let ResponseItem::Message { content, .. } = reviews.into_annotated_message().item else {
        panic!("expected a review message");
    };
    assert_eq!(
        &content[1..],
        fragments
            .into_iter()
            .map(|review| ContentItem::InputText {
                text: review.fragment
            })
            .collect::<Vec<_>>()
            .as_slice()
    );
    for invalid in [
        vec!["review".to_owned(); MAX_PREVIOUS_REVIEWS + 1],
        vec![format!("{fragment}a")],
    ] {
        assert_eq!(
            PreviousReviews::try_from_fragments(
                invalid
                    .into_iter()
                    .map(|fragment| PreviousReview {
                        id: ResponseItemId::new("review"),
                        fragment,
                    })
                    .collect()
            ),
            Err(SectionError::EvidenceLimitExceeded {
                section: "previous_reviews",
            })
        );
    }
}

#[test]
fn review_delivery_requires_complete_host_metadata_and_preserves_distinct_completions() {
    let first = PreviousReview {
        id: ResponseItemId::new("review"),
        fragment: "Approved reading README.md.".to_owned(),
    };
    // A different completion remains new evidence even when its text is identical.
    let second = PreviousReview {
        id: ResponseItemId::new("review"),
        fragment: first.fragment.clone(),
    };
    let first_message = PreviousReviews::try_from_fragments(vec![first.clone()])
        .unwrap()
        .into_annotated_message();
    let second_message = PreviousReviews::try_from_fragments(vec![second.clone()])
        .unwrap()
        .into_annotated_message();
    let mut shortened = first_message.clone();
    shortened
        .metadata
        .as_mut()
        .unwrap()
        .mark_retained_sources_incomplete();
    let both = vec![first, second.clone()];
    for (history, expected) in [
        (Vec::new(), both.clone()),
        (
            vec![ResponseItemEnvelope::new(first_message.item.clone())],
            both.clone(),
        ),
        (vec![shortened], both.clone()),
        (vec![first_message.clone()], vec![second]),
        (vec![first_message, second_message], Vec::new()),
    ] {
        let mut context = CollectedContext {
            sections: vec![ContextSection::PreviousReviews(
                PreviousReviews::try_from_fragments(both.clone()).unwrap(),
            )],
        };
        context.retain_new_reviews(&history);
        let delivered = context
            .compose(
                crate::ContextPresentation::Async,
                crate::RenderedTranscript {
                    items: Vec::new(),
                    omission_note: None,
                    truncations: Vec::new(),
                },
            )
            .unwrap()
            .into_annotated_messages();
        let expected = if expected.is_empty() {
            Vec::new()
        } else {
            vec![
                PreviousReviews::try_from_fragments(expected)
                    .unwrap()
                    .into_annotated_message(),
            ]
        };
        assert_eq!(delivered, expected);
    }
}

use super::*;
use crate::async_scorer::sampler::INPUT_TOKEN_MARGIN;
use crate::async_scorer::transcript::CollectedTranscript;
use crate::async_scorer::transcript::ContextInput;
use crate::async_scorer::transcript::TranscriptConfig;
use codex_extension_api::ConversationHistorySnapshot;
use codex_guardian_context::ContextBudgeting;
use codex_guardian_context::ContextTarget;
use codex_guardian_context::PlannedAction;
use codex_guardian_context::PlannedActionKind;
use codex_guardian_context::PreviousReviews;
use codex_guardian_reviewer::ConversationState;
use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ImageReference;
use pretty_assertions::assert_eq;

const INPUT_BUDGET: usize = codex_guardian_context::DEFAULT_MAX_INPUT_TOKENS;

struct History {
    items: Vec<ResponseItem>,
    version: u64,
}

impl ConversationHistorySnapshot for History {
    fn history_version(&self) -> u64 {
        self.version
    }
    fn user_message_revision(&self) -> u64 {
        0
    }
    fn items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_> {
        Box::new(self.items.iter())
    }
}

fn collect(
    history: &History,
    images: &[ContentItem],
    mode: ContextBudgeting,
) -> CollectedTranscript {
    let action = PlannedAction {
        tool_descriptions: None,
        kind: PlannedActionKind::Command {
            json: r#"{"tool":"read_file","path":"README.md"}"#.to_owned(),
        },
        reason: None,
    };
    let reviews =
        PreviousReviews::try_from_fragments(vec![codex_guardian_context::PreviousReview {
            id: ResponseItemId::from_server("review-readme".to_owned()),
            fragment: "Approved reading README.md only.".to_owned(),
        }])
        .unwrap();
    let config = TranscriptConfig {
        elastic: (mode == ContextBudgeting::Elastic)
            .then(core_test_support::guardian::elastic_budget_config),
        ..TranscriptConfig::default()
    };
    config
        .collect_context(ContextInput {
            target: ContextTarget::Async,
            history,
            root_conversation: &[],
            root_conversation_candidates: None,
            trusted_user_answers: &["Do not write files.".into()],
            planned_action: Some(&action),
            permissions: None,
            previous_reviews: Some(&reviews),
            trusted_tool: None,
            trusted_skill_paths: &[],
            node_repl_images: Some(images),
        })
        .unwrap()
}

#[test_case::test_case(ContextBudgeting::Fixed; "fixed")]
#[test_case::test_case(ContextBudgeting::Elastic; "elastic")]
fn preparation_preserves_committed_prefix_and_selects_only_new_entries_and_images(
    mode: ContextBudgeting,
) {
    let native = ResponseItem::AgentMessage {
        id: Some(ResponseItemId::from_server("amsg_parent".to_owned())),
        author: "/root".to_owned(),
        recipient: "/root/worker".to_owned(),
        content: vec![
            codex_protocol::models::AgentMessageInputContent::EncryptedContent {
                encrypted_content: "opaque-parent-restriction".to_owned(),
            },
        ],
        internal_chat_message_metadata_passthrough: None,
    };
    let images = [
        ImageReference::Inline {
            image_url: "data:image/png;base64,Zmlyc3Q=".to_owned(),
        },
        ImageReference::File {
            file_id: "file-screenshot".to_owned(),
        },
        ImageReference::Inline {
            image_url: "data:image/png;base64,c2Vjb25k".to_owned(),
        },
    ]
    .map(|image| ContentItem::InputImage {
        image,
        detail: None,
    });
    let input_images = |input: &[ResponseItemEnvelope]| {
        input
            .iter()
            .flat_map(|item| match &item.item {
                ResponseItem::Message { content, .. } => content.as_slice(),
                _ => &[],
            })
            .filter(|item| matches!(item, ContentItem::InputImage { .. }))
            .cloned()
            .collect::<Vec<_>>()
    };
    let mut history = History {
        items: vec![
            responses::user_message_item("Inspect README.md."),
            native.clone(),
            native.clone(),
        ],
        version: 1,
    };
    if let ResponseItem::Message { content, .. } = &mut history.items[0] {
        content.push(images[1].clone());
    }
    let mut state = ConversationState::default();
    let config = sample_request("parent");
    let evidence = collect(&history, &images[..1], mode);
    let first = config
        .prepare_retained(&evidence, &mut state, INPUT_BUDGET)
        .unwrap();
    assert_eq!(
        input_images(&first.input),
        vec![images[1].clone(), images[0].clone()]
    );
    assert!(first.input.iter().all(|item| item.id().is_some()));
    assert_eq!(
        first
            .input
            .iter()
            .filter(|item| matches!(item.item, ResponseItem::AgentMessage { .. }))
            .map(|item| item.item.clone())
            .collect::<Vec<_>>(),
        vec![native.clone()]
    );
    let mut committed = first.input;
    // Keep the actual returned item, including the server ID and metadata.
    let output: ResponseItem =
        serde_json::from_value(ev_assistant_message("msg_classifier_a", "low")["item"].clone())
            .unwrap();
    committed.push(output.into());
    state.complete_review(first.cursor);
    state.commit_snapshot(committed.clone());
    history
        .items
        .push(responses::user_message_item("Inspect LICENSE next."));
    history.items.push(native.clone());
    let mut next_native = native.clone();
    next_native.set_id(Some(ResponseItemId::from_server("amsg_next".to_owned())));
    history.items.push(next_native.clone());
    // Normalize new images before admission; duplicate source detail can also differ.
    const NEW_IMAGE_INDEX: usize = 2;
    let mut new_images = [images[0].clone(), images[NEW_IMAGE_INDEX].clone()];
    for image in &mut new_images {
        if let ContentItem::InputImage { detail, .. } = image {
            *detail = Some(ImageDetail::High);
        }
    }
    let evidence = collect(&history, &new_images, mode);
    // Fits three distinct images, but not another copy of the two retained images.
    let second = config
        .prepare_retained(&evidence, &mut state, /*max_input_tokens*/ 40_000)
        .unwrap();
    assert_eq!(&second.input[..committed.len()], committed);
    assert!(second.truncations.is_empty());
    assert!(state.snapshot().is_none());
    assert_eq!(
        second.input_tokens,
        second
            .input
            .iter()
            .map(|item| codex_guardian_context::estimate_input_tokens(&item.item))
            .fold(0usize, usize::saturating_add)
    );
    assert_eq!(
        input_images(&second.input[committed.len()..]),
        vec![images[NEW_IMAGE_INDEX].clone()]
    );
    assert_eq!(
        second
            .input
            .iter()
            .filter(|item| matches!(item.item, ResponseItem::AgentMessage { .. }))
            .map(|item| item.item.clone())
            .collect::<Vec<_>>(),
        vec![native.clone(), next_native.clone()]
    );
    assert_eq!(second.cursor.transcript_entry_count, 6);
    let delta = serde_json::to_string(
        &second.input[committed.len()..]
            .iter()
            .map(|item| &item.item)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    assert!(delta.contains("TRANSCRIPT DELTA START"));
    assert!(!delta.contains("<guardian_context_omission>"));
    assert!(delta.contains("[4] user: Inspect LICENSE next."));
    assert!(!delta.contains("[1] user: Inspect README.md."));
    assert!(delta.contains("Do not write files."));
    assert!(delta.contains("read_file"));
    assert!(
        matches!(&second.input[committed.len()].item, ResponseItem::Message { role, .. } if role == "user")
    );
    assert!(!delta.contains("Approved reading README.md only."));
    // A rebuilt conversation must receive the current images again.
    let rebuilt = config
        .prepare_retained(&evidence, &mut ConversationState::default(), INPUT_BUDGET)
        .unwrap();
    assert_eq!(input_images(&rebuilt.input), input_images(&second.input));
    assert_eq!(
        rebuilt
            .input
            .iter()
            .filter(|item| matches!(item.item, ResponseItem::AgentMessage { .. }))
            .map(|item| item.item.clone())
            .collect::<Vec<_>>(),
        vec![native, next_native]
    );
}

#[test_case::test_case(ContextBudgeting::Fixed; "fixed")]
#[test_case::test_case(ContextBudgeting::Elastic; "elastic")]
fn rejected_preparation_preserves_committed_history_and_cursor(mode: ContextBudgeting) {
    const FRESH_REQUEST_HEADROOM: usize = 128;
    let mut history = History {
        items: vec![responses::user_message_item("Inspect README.md.")],
        version: 1,
    };
    let mut state = ConversationState::default();
    let config = sample_request("parent");
    let first = config
        .prepare_retained(&collect(&history, &[], mode), &mut state, INPUT_BUDGET)
        .unwrap();
    // Leave room for the fresh request's allocation estimate and ID reservation.
    let limit = first.input_tokens + INPUT_TOKEN_MARGIN + FRESH_REQUEST_HEADROOM;
    let mut committed = first.input;
    committed.push(responses::user_message_item(&"output ".repeat(/*n*/ 400)).into());
    state.complete_review(first.cursor);
    state.commit_snapshot(committed.clone());
    assert!(
        config
            .prepare_retained(&collect(&history, &[], mode), &mut state, limit)
            .is_none()
    );
    assert_eq!(state.cursor(), Some(first.cursor));
    assert_eq!(state.snapshot().unwrap().history(), &committed);
    config
        .prepare_retained(
            &collect(&history, &[], mode),
            &mut ConversationState::default(),
            limit,
        )
        .unwrap();
    history.version += 1;
    assert!(
        config
            .prepare_retained(&collect(&history, &[], mode), &mut state, INPUT_BUDGET)
            .is_none()
    );
    state.reset_transcript();
    assert!(
        config
            .prepare_retained(&collect(&history, &[], mode), &mut state, INPUT_BUDGET)
            .is_none()
    );
    assert_eq!(state.snapshot().unwrap().history(), &committed);
}

#[test]
fn elastic_fits_ids_without_resetting_committed_history() {
    const ID_PADDING_BYTES: usize = 1_024;
    let mode = ContextBudgeting::Elastic;
    let mut config = sample_request("parent");
    let original_generator = std::sync::Arc::clone(&config.item_id_generator);
    let padding = "x".repeat(ID_PADDING_BYTES);
    config.item_id_generator = std::sync::Arc::new(move |prefix| {
        ResponseItemId::from_server(format!("{}{padding}", original_generator(prefix)))
    });
    let mut history = History {
        items: vec![responses::user_message_item("Inspect README.md.")],
        version: 1,
    };
    let mut state = ConversationState::default();
    let first = config
        .prepare_retained(&collect(&history, &[], mode), &mut state, INPUT_BUDGET)
        .unwrap();
    state.complete_review(first.cursor);
    state.commit_snapshot(first.input.clone());
    history
        .items
        .push(responses::user_message_item("Inspect LICENSE next."));
    let image = ContentItem::InputImage {
        image: ImageReference::Inline {
            image_url: "data:image/png;base64,b3B0aW9uYWw=".to_owned(),
        },
        detail: None,
    };
    let evidence = collect(&history, std::slice::from_ref(&image), mode);
    let mut generous = ConversationState::from(state.snapshot().unwrap().clone());
    let full = config
        .prepare_retained(&evidence, &mut generous, INPUT_BUDGET)
        .unwrap();
    assert!(full.input.iter().any(|item| matches!(
        &item.item, ResponseItem::Message { content, .. } if content.contains(&image)
    )));
    let maximum = full.input_tokens - 1;
    let bounded = config
        .prepare_retained(&evidence, &mut state, maximum + INPUT_TOKEN_MARGIN)
        .expect("allocate evidence with room for IDs");
    assert_eq!(&bounded.input[..first.input.len()], first.input);
    assert!(bounded.input_tokens <= maximum);
    assert!(bounded.input.iter().all(|item| item.id().is_some()));
    let suffix = serde_json::to_string(
        &bounded.input[first.input.len()..]
            .iter()
            .map(|item| &item.item)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    assert!(suffix.contains("Inspect LICENSE next."));
    assert!(!suffix.contains("data:image/png;base64,b3B0aW9uYWw="));
    assert!(suffix.contains("<guardian_context_omission>"));
}

#[test_case::test_case(ContextBudgeting::Fixed; "fixed")]
#[test_case::test_case(ContextBudgeting::Elastic; "elastic")]
fn complete_budget_counts_setup_checkpoint_images_and_trusted_evidence(mode: ContextBudgeting) {
    let history = History {
        items: vec![responses::user_message_item("Inspect README.md.")],
        version: 1,
    };
    let mut config = sample_request("parent");
    config.parent_compaction = Some(ResponseItem::ContextCompaction {
        id: Some(ResponseItemId::from_server("cmp_parent".to_owned())),
        encrypted_content: Some("opaque checkpoint".repeat(/*n*/ 100)),
        internal_chat_message_metadata_passthrough: None,
    });
    let images = [ContentItem::InputImage {
        image: ImageReference::Inline {
            image_url: "data:image/png;base64,aGVsbG8=".to_owned(),
        },
        detail: None,
    }];
    let mut state = ConversationState::default();
    let evidence = collect(&history, &images, mode);
    let request = config
        .prepare_retained(&evidence, &mut state, INPUT_BUDGET)
        .unwrap();
    assert_eq!(
        Some(&request.input[2].item),
        config.parent_compaction.as_ref()
    );
    assert!(request.input_tokens > 10_000);
    let maximum = request.input_tokens - 1;
    let smaller = config.prepare_retained(&evidence, &mut state, maximum + INPUT_TOKEN_MARGIN);
    match mode {
        ContextBudgeting::Fixed => {
            assert!(smaller.is_none());
            let exact = config
                .prepare_retained(
                    &evidence,
                    &mut state,
                    request.input_tokens + INPUT_TOKEN_MARGIN,
                )
                .unwrap();
            assert_eq!(exact.input_tokens, request.input_tokens);
            assert!(exact.input.iter().any(|item| matches!(
                &item.item, ResponseItem::Message { content, .. } if content.contains(&images[0])
            )));
        }
        ContextBudgeting::Elastic => {
            let smaller = smaller.expect("Elastic can shed optional evidence");
            assert!(smaller.input_tokens <= maximum);
            assert!(
                smaller
                    .input
                    .iter()
                    .any(|item| Some(&item.item) == config.parent_compaction.as_ref())
            );
        }
    }
}

use super::*;
use crate::async_scorer::transcript::CollectedTranscript;
use crate::async_scorer::transcript::ContextInput;
use crate::async_scorer::transcript::TranscriptConfig;
use codex_extension_api::ConversationHistorySnapshot;
use codex_guardian_context::ContextTarget;
use codex_guardian_context::PlannedAction;
use codex_guardian_context::PlannedActionKind;
use codex_guardian_context::PreviousReviews;
use codex_guardian_reviewer::ConversationState;
use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ImageReference;
use pretty_assertions::assert_eq;

const INPUT_BUDGET: usize = 128_000;

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

fn collect(history: &History, images: &[ContentItem]) -> CollectedTranscript {
    let action = PlannedAction {
        json: r#"{"tool":"read_file","path":"README.md"}"#.to_owned(),
        tool_descriptions: None,
        kind: PlannedActionKind::Command,
        reason: None,
    };
    let reviews =
        PreviousReviews::try_from_fragments(vec![codex_guardian_context::PreviousReview {
            id: ResponseItemId::from_server("review-readme".to_owned()),
            fragment: "Approved reading README.md only.".to_owned(),
        }])
        .unwrap();
    TranscriptConfig::default()
        .collect_context(ContextInput {
            target: ContextTarget::Async,
            history,
            root_conversation: &[],
            trusted_user_answers: &["Do not write files.".to_owned()],
            planned_action: Some(&action),
            permissions: None,
            previous_reviews: Some(&reviews),
            trusted_tool: None,
            trusted_skill_paths: &[],
            node_repl_images: Some(images),
        })
        .unwrap()
}

#[test]
fn preparation_preserves_committed_prefix_and_selects_only_new_entries_and_images() {
    let mut images = [
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
        items: vec![responses::user_message_item("Inspect README.md.")],
        version: 1,
    };
    if let ResponseItem::Message { content, .. } = &mut history.items[0] {
        content.push(images[1].clone());
    }
    let mut state = ConversationState::default();
    let config = sample_request("parent");
    let first = config
        .prepare_retained(&collect(&history, &images[..1]), &mut state, INPUT_BUDGET)
        .unwrap();
    assert_eq!(
        input_images(&first.input),
        vec![images[1].clone(), images[0].clone()]
    );
    assert!(first.input.iter().all(|item| item.id().is_some()));
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
    // Source detail can differ from the normalized image already in classifier history.
    if let ContentItem::InputImage { detail, .. } = &mut images[0] {
        *detail = Some(ImageDetail::High);
    }
    let evidence = collect(&history, &[images[0].clone(), images[2].clone()]);
    // Fits three distinct images, but not another copy of the two retained images.
    let second = config
        .prepare_retained(&evidence, &mut state, /*max_input_tokens*/ 40_000)
        .unwrap();
    assert_eq!(&second.input[..committed.len()], committed);
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
        vec![images[2].clone()]
    );
    assert_eq!(second.cursor.transcript_entry_count, 2);
    let delta = serde_json::to_string(
        &second.input[committed.len()..]
            .iter()
            .map(|item| &item.item)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    assert!(delta.contains("TRANSCRIPT DELTA START"));
    assert!(delta.contains("[2] user: Inspect LICENSE next."));
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
}

#[test]
fn rejected_preparation_preserves_committed_history_and_cursor() {
    let mut history = History {
        items: vec![responses::user_message_item("Inspect README.md.")],
        version: 1,
    };
    let mut state = ConversationState::default();
    let config = sample_request("parent");
    let first = config
        .prepare_retained(&collect(&history, &[]), &mut state, INPUT_BUDGET)
        .unwrap();
    let limit = first.input_tokens + 256;
    let mut committed = first.input;
    committed.push(responses::user_message_item(&"output ".repeat(/*n*/ 400)).into());
    state.complete_review(first.cursor);
    state.commit_snapshot(committed.clone());
    assert!(
        config
            .prepare_retained(&collect(&history, &[]), &mut state, limit)
            .is_none()
    );
    assert_eq!(state.cursor(), Some(first.cursor));
    assert_eq!(state.snapshot().unwrap().history(), &committed);
    config
        .prepare_retained(
            &collect(&history, &[]),
            &mut ConversationState::default(),
            limit,
        )
        .unwrap();
    history.version += 1;
    assert!(
        config
            .prepare_retained(&collect(&history, &[]), &mut state, INPUT_BUDGET)
            .is_none()
    );
    state.reset_transcript();
    assert!(
        config
            .prepare_retained(&collect(&history, &[]), &mut state, INPUT_BUDGET)
            .is_none()
    );
    assert_eq!(state.snapshot().unwrap().history(), &committed);
}

#[test]
fn complete_budget_counts_setup_checkpoint_images_and_trusted_evidence() {
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
    let evidence = collect(&history, &images);
    let request = config
        .prepare_retained(&evidence, &mut state, INPUT_BUDGET)
        .unwrap();
    assert_eq!(
        Some(&request.input[2].item),
        config.parent_compaction.as_ref()
    );
    assert!(request.input_tokens > 10_000);
    assert!(
        config
            .prepare_retained(&evidence, &mut state, request.input_tokens + 255)
            .is_none()
    );
    // The complete input fits exactly at the reserved-output boundary.
    config
        .prepare_retained(&evidence, &mut state, request.input_tokens + 256)
        .unwrap();
}

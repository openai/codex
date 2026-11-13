//! Checks history estimates with and without trusted file-audio durations.
#![cfg(test)]

use super::*;
use crate::context_manager::ContextManager;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::user_input::UserInput;
use codex_protocol_test_support::uuid_v7_item_id_generator_for_test;
use codex_utils_output_truncation::TruncationPolicy;
use pretty_assertions::assert_eq;

#[test]
fn history_sums_measured_and_unknown_file_audio() {
    let mut history = ContextManager::new(uuid_v7_item_id_generator_for_test());
    let item = ResponseInputItem::from(
        ["short", "unknown"]
            .map(|file_id| UserInput::Audio {
                audio: AudioReference::File {
                    file_id: file_id.into(),
                },
            })
            .to_vec(),
    );
    history.record_items(
        [&ResponseItem::from(item)],
        TruncationPolicy::Tokens(10_000),
    );
    let base = BaseInstructions {
        text: String::new(),
        provenance: None,
    };
    assert_eq!(
        history.estimate_token_count_with_base_instructions(&base),
        Some(24_000)
    );
    history.file_audio_durations = Arc::new(FileAudioDurations(BTreeMap::from([(
        "short".into(),
        Duration::from_secs(30),
    )])));
    assert_eq!(
        history.estimate_token_count_with_base_instructions(&base),
        Some(12_300)
    );
    assert_eq!(history.estimated_tokens_since_last_usage(), 12_300);
}

#[test]
fn transient_upload_failure_retains_inline_audio_but_invalid_attachments_are_omitted() {
    use codex_attachment_store::AttachmentStoreError;
    use codex_attachment_store::AttachmentStoreErrorKind;
    use serde_json::json;
    for (kind, expected) in [
        (
            AttachmentStoreErrorKind::Backend,
            json!({"type":"input_audio", "audio_url":"data:audio/wav;base64,QQ=="}),
        ),
        (
            AttachmentStoreErrorKind::InvalidAttachment,
            json!({"type":"input_text", "text":"audio content omitted because the attachment store rejected the clip"}),
        ),
    ] {
        let result = Err(AttachmentStoreError::new(kind, "upload failed"));
        let part =
            tool_audio_after_upload(result, "audio/wav", "data:audio/x-wav;base64,QQ==".into());
        assert_eq!(serde_json::to_value(part).unwrap(), expected);
    }
}

async fn prepared_tool_output(
    parts: serde_json::Value,
    metadata: codex_history::CodexHarnessMetadata,
    limit: usize,
) -> serde_json::Value {
    let mut item = ResponseItemEnvelope {
        item: serde_json::from_value(
            serde_json::json!({"type":"function_call_output","call_id":"clip","output":parts}),
        )
        .unwrap(),
        metadata: Some(metadata),
    };
    prepare_new_items(
        std::slice::from_mut(&mut item),
        &codex_attachment_store::InlineAttachmentStore,
        "thread",
        /*ephemeral*/ false,
        limit,
    )
    .await;
    let metadata = item.metadata.unwrap();
    assert!(!metadata.producer_bounded_tool_audio);
    assert!(metadata.tool_audio_budget_applied);
    assert_eq!(
        serde_json::to_value(&metadata).unwrap()["tool_audio_budget_applied"],
        true
    );
    serde_json::to_value(item.item).unwrap()["output"].take()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn producer_bounded_audio_keeps_the_same_parts_as_inline_history() {
    use codex_history::CodexHarnessMetadata;
    use serde_json::json;
    let clip = json!({"type":"input_audio","audio_url":"data:audio/wav;base64,UklGRiYAAABXQVZFZm10IBAAAAABAAEAwF0AAIC7AAACABAAZGF0YQIAAAAAAA=="});
    let caption = json!({"type":"input_text","text":"caption ".repeat(100)});
    let policy = TruncationPolicy::Tokens(20);
    let limit = codex_utils_output_truncation::with_serialization_allowance(policy).token_budget();
    let malformed = json!({"type":"input_audio","audio_url":format!("data:audio/wav;base64,{}", "!".repeat(8192))});
    for (parts, keep_clip) in [
        (json!([malformed, clip]), true),
        (json!([{"type":"input_audio","audio_url":""}, clip]), true),
        (json!([clip, caption]), true),
        (json!([caption, clip]), false),
    ] {
        let mut inline = serde_json::from_value(
            json!({"type":"function_call_output","call_id":"clip","output":parts}),
        )
        .unwrap();
        codex_utils_audio::prepare_response_items(std::slice::from_mut(&mut inline));
        let mut history = ContextManager::new(uuid_v7_item_id_generator_for_test());
        history.record_items([&inline], policy);
        let expected =
            serde_json::to_value(history.raw_items().last().unwrap()).unwrap()["output"].take();
        assert_eq!(expected.as_array().unwrap().contains(&clip), keep_clip);
        let metadata = CodexHarnessMetadata {
            producer_bounded_tool_audio: true,
            ..Default::default()
        };
        assert_eq!(prepared_tool_output(parts, metadata, limit).await, expected);
    }
}

#[tokio::test]
async fn zero_cost_invalid_audio_cannot_expand_into_unbounded_error_text() {
    use codex_history::CodexHarnessMetadata;
    use serde_json::json;
    for producer_budgeted in [false, true] {
        let mut parts = vec![json!({"type":"input_text","text":"keep"})];
        parts.extend((0..100).map(|_| json!({"type":"input_audio","audio_url":""})));
        let metadata = CodexHarnessMetadata {
            producer_bounded_tool_audio: producer_budgeted,
            ..Default::default()
        };
        assert_eq!(
            prepared_tool_output(parts.into(), metadata, /*limit*/ 1).await,
            json!([{"type":"input_text","text":"keep"},{"type":"input_text","text":"[omitted 100 text items ...]"}])
        );
    }
}

#[tokio::test]
async fn known_producer_limit_keeps_the_replacement_and_following_caption() {
    use serde_json::json;
    let caption = json!({"type":"input_text","text":"caption"});
    let parts = json!([{"type":"input_audio","audio_url":""}, caption]);
    let metadata = codex_history::CodexHarnessMetadata {
        producer_bounded_tool_audio: true,
        history_truncation_token_limit: Some(100),
        ..Default::default()
    };
    let error = codex_utils_audio::decode_audio("").unwrap_err();
    assert_eq!(
        prepared_tool_output(parts, metadata, /*limit*/ 1).await,
        json!([{"type":"input_text","text":error.placeholder()},caption])
    );
}

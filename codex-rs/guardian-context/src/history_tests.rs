use codex_protocol::models::ExecutedToolCall;
use codex_protocol::models::ImageReference;
use codex_protocol::models::ReasoningItemContent;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;

fn message(text: &str) -> ResponseItem {
    serde_json::from_value(json!({
        "type": "message", "role": "user", "content": [{"type": "input_text", "text": text}]
    }))
    .unwrap()
}

fn tool(text: &str) -> ResponseItem {
    serde_json::from_value(json!({
        "type": "function_call_output", "call_id": "call", "output": text
    }))
    .unwrap()
}

#[test]
fn rollback_keeps_the_earlier_prefix_or_clears_an_evicted_boundary() {
    let items = [message("keep"), message("roll back"), tool("later")];
    let mut history = TranscriptHistory::default();
    history.reset(items.clone().map(Into::into).iter());
    let generation = history.generation();
    history.truncate_before(&items[1].clone().into());
    assert_eq!(history.items().collect::<Vec<_>>(), vec![&items[0]]);
    assert!(history.generation() > generation);
    history.truncate_before(&items[1].clone().into());
    assert_eq!(history.items().count(), 0);
}

#[test]
fn inherited_manual_approvals_stay_out_of_review_after_checkpoint_replay() {
    for content_type in ["input_text", "output_text"] {
        let approval: ResponseItem = serde_json::from_value(json!({
            "type": "message", "role": "developer", "content": [{
                "type": content_type, "text": format!("{}\nApproved action: original action", crate::MANUAL_APPROVAL_DEVELOPER_PREFIX)
            }]
        })).unwrap();
        let mut inherited = ResponseItemEnvelope::new(approval.clone());
        inherited
            .metadata
            .get_or_insert_default()
            .inherited_user_message = true;
        let mut history = TranscriptHistory::default();
        history.reset([inherited.clone(), approval.clone().into()].iter());
        assert_eq!(
            history.items().cloned().collect::<Vec<_>>(),
            vec![approval.clone()]
        );
        let checkpoint = history.checkpoint();
        assert_eq!(
            checkpoint.0[0], inherited,
            "model history keeps the original approval and provenance"
        );
        let mut restored = TranscriptHistory::default();
        restored.reset(checkpoint.0.iter());
        assert_eq!(
            restored.items().cloned().collect::<Vec<_>>(),
            vec![approval]
        );
    }
}

#[test]
fn each_kind_evicts_its_own_oldest_entries_without_reordering() {
    let users = [message("first"), message("second"), message("third")];
    let tools: Vec<_> = (0..MAX_ITEMS_PER_KIND)
        .map(|index| tool(&index.to_string()))
        .collect();
    let mut history = TranscriptHistory::default();
    history.record(&users[0].clone().into());
    history.record(&tool("old output").into());
    history.record(&users[1].clone().into());
    for item in &tools {
        history.record(&item.clone().into());
    }
    history.record(&users[2].clone().into());
    assert_eq!(
        history.items().collect::<Vec<_>>(),
        users[..2]
            .iter()
            .chain(&tools[63..])
            .chain(&users[2..])
            .collect::<Vec<_>>()
    );
    let generation = history.generation();
    let newer_users: Vec<_> = (0..MAX_ITEMS_PER_KIND)
        .map(|index| message(&index.to_string()))
        .collect();
    for item in &newer_users {
        history.record(&item.clone().into());
    }
    assert_eq!(
        history.items().collect::<Vec<_>>(),
        tools[63..].iter().chain(&newer_users).collect::<Vec<_>>()
    );
    assert!(history.generation() > generation);
}

#[test]
fn byte_limits_are_independent_and_oversized_items_do_not_clear_history() {
    let large = tool(&"x".repeat(MAX_BYTES_PER_KIND / 2));
    let keep = message("keep this");
    let mut history = TranscriptHistory::default();
    history.record(&large.clone().into());
    history.record(&keep.clone().into());
    history.record(&large.clone().into());
    assert_eq!(history.items().collect::<Vec<_>>(), vec![&keep, &large]);

    let oversized = "x".repeat(MAX_BYTES_PER_KIND);
    history.record(&message(&oversized).into());
    history.record(&tool(&oversized).into());
    history.record(
        &ResponseItem::Reasoning {
            id: None,
            summary: Vec::new(),
            // Serialization omits this content, but retention must still count it.
            content: Some(vec![ReasoningItemContent::Text { text: oversized }]),
            encrypted_content: None,
            internal_chat_message_metadata_passthrough: None,
        }
        .into(),
    );
    assert_eq!(history.items().collect::<Vec<_>>(), vec![&keep, &large]);
}

#[test]
fn tool_metadata_is_retained_without_changing_retention_size() {
    let before = tool("keep this");
    let mut plain = tool(&"x".repeat(MAX_BYTES_PER_KIND - 4096));
    plain.set_turn_id_if_missing("turn-1");
    let mut recorded = plain.clone();
    recorded.append_executed_tool_calls(vec![ExecutedToolCall::new(
        "command".to_string(),
        json!({"command": "x".repeat(7000)}),
    )]);
    recorded.mark_tool_calls_complete();
    let mut baseline = TranscriptHistory::default();
    let mut with_metadata = TranscriptHistory::default();
    baseline.reset([before.clone().into(), plain.clone().into()].iter());
    with_metadata.reset([before.clone().into(), recorded.clone().into()].iter());
    assert_eq!(
        with_metadata.items().collect::<Vec<_>>(),
        vec![&before, &recorded]
    );
    assert_eq!(
        with_metadata
            .items
            .iter()
            .map(|(_, size)| *size)
            .collect::<Vec<_>>(),
        baseline
            .items
            .iter()
            .map(|(_, size)| *size)
            .collect::<Vec<_>>()
    );
    assert_eq!(with_metadata.generation(), baseline.generation());
}

#[test]
fn oversized_user_images_preserve_text_and_metadata_in_order() {
    let before = tool("earlier tool result");
    let after = message("later message");
    let text_only: ResponseItem = serde_json::from_value(json!({
        "type": "message", "id": "user-1", "role": "user", "phase": "commentary",
        "internal_chat_message_metadata_passthrough": {"turn_id": "turn-1"},
        "content": [
            {"type": "input_text", "text": "Do not publish anything."},
            {"type": "output_text", "text": "Only inspect the attached image."}
        ]
    }))
    .unwrap();
    for image_bytes in [16, MAX_BYTES_PER_KIND] {
        let mut with_image = text_only.clone();
        let ResponseItem::Message { content, .. } = &mut with_image else {
            unreachable!()
        };
        content.insert(
            /*index*/ 1,
            ContentItem::InputImage {
                image: ImageReference::Inline {
                    image_url: format!("data:image/png;base64,{}", "A".repeat(image_bytes)),
                },
                detail: Some(codex_protocol::models::ImageDetail::Original),
            },
        );
        let mut history = TranscriptHistory::default();
        history.record(&before.clone().into());
        history.record(&with_image.clone().into());
        history.record(&after.clone().into());
        let expected = if image_bytes < MAX_BYTES_PER_KIND {
            &with_image
        } else {
            &text_only
        };
        assert_eq!(
            history.items().collect::<Vec<_>>(),
            vec![&before, expected, &after]
        );
    }
}

#[test]
fn rollback_discards_assistant_sources_without_ordering_in_old_backups() {
    let original = ResponseItemEnvelope::new(message("Staging only."));
    let assistant = ResponseItemEnvelope::new(serde_json::from_value(json!({
        "type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "Deploy?"}]
    })).unwrap());
    let boundary = ResponseItemEnvelope {
        item: message("Queued input"),
        metadata: Some(codex_history::CodexHarnessMetadata {
            user_input_order: Some(1),
            ..Default::default()
        }),
    };
    let mut history = TranscriptHistory::default();
    history.reset([&original, &assistant, &boundary]);
    history.truncate_before(&boundary);
    assert_eq!(history.items().collect::<Vec<_>>(), vec![&original.item]);
}

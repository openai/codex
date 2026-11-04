//! Checks reconstructed history against live compaction and persisted replacement items.
#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn reconstruct_history_matches_live_compactions() {
    let (session, turn_context) = make_session_and_context().await;
    let (rollout_items, expected) = sample_rollout(&session, &turn_context).await;

    let reconstruction_turn = session.new_default_turn().await;
    let reconstructed = session
        .reconstruct_history_from_rollout(reconstruction_turn.as_ref(), &rollout_items)
        .await;

    assert_eq!(expected, raw_envelopes(&reconstructed.history));
    assert_eq!(2, reconstructed.window_number);
    assert_eq!(
        reconstructed
            .window_id
            .map(|window_id| Uuid::parse_str(&window_id.to_string())
                .unwrap()
                .get_version_num()),
        Some(7)
    );
}

#[tokio::test]
async fn reconstruct_history_uses_replacement_history_verbatim() {
    let (session, turn_context) = make_session_and_context().await;
    let summary_item = ResponseItem::Message {
        status: None,
        encrypted_content: None,
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: "summary".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: Some(InternalChatMessageMetadataPassthrough {
            turn_id: Some("compact-turn".to_string()),
            ..Default::default()
        }),
    };
    let mut replacement_history = vec![
        ResponseItemEnvelope {
            item: summary_item.clone(),
            metadata: Some(CodexHarnessMetadata::default()),
        },
        ResponseItemEnvelope::new(ResponseItem::Message {
            status: None,
            encrypted_content: None,
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: "stale developer instructions".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        }),
    ];
    let first_window_id = ContextWindowId::new_uuid_v7();
    let previous_window_id = ContextWindowId::new_uuid_v7();
    let window_id = ContextWindowId::new_uuid_v7();
    let rollout_items = vec![RolloutItem::Compacted(Box::new(CompactedItem {
        message: String::new(),
        replacement_history: Some(replacement_history.clone()),
        retained_context: None,
        guardian_history: None,
        mcp_resource_origins: None,
        window_number: Some(42),
        first_window_id: Some(first_window_id.to_string()),
        previous_window_id: Some(previous_window_id.to_string()),
        window_id: Some(window_id.to_string()),
        compaction_response_id: None,
        latest_token_usage_record: None,
        resume_metadata: None,
    }))];

    let reconstructed = session
        .reconstruct_history_from_rollout(&turn_context, &rollout_items)
        .await;

    for (index, envelope) in replacement_history.iter_mut().enumerate() {
        let metadata = envelope.metadata.get_or_insert_default();
        metadata.history_sequence = Some(index as u64 + 1);
        metadata.sampled_through_sequence = Some(0);
    }
    assert_eq!(reconstructed.history, replacement_history);
    assert_eq!(42, reconstructed.window_number);
    assert_eq!(Some(first_window_id), reconstructed.first_window_id);
    assert_eq!(Some(previous_window_id), reconstructed.previous_window_id);
    assert_eq!(Some(window_id), reconstructed.window_id);
}

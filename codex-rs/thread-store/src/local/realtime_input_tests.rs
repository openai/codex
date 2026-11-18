use codex_protocol::ResponseItemId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_rollout::ResponseItemEnvelope;
use codex_rollout::RolloutItem;
use codex_rollout::RolloutRecorder;
use tempfile::TempDir;
use uuid::Uuid;

use super::LocalThreadStore;
use super::test_support::test_config;
use super::test_support::write_session_file_with_history_mode;
use crate::ResumeThreadParams;
use crate::ThreadPersistenceMetadata;
use crate::ThreadStore;

/// A failed directory sync happens after the file contains the accepted input.
/// Confirming that same seal must not append another model-facing input.
#[tokio::test]
async fn retry_after_uncertain_local_input_seal_does_not_append_again() {
    let home = TempDir::new().unwrap();
    let uuid = Uuid::now_v7();
    let thread_id = codex_protocol::ThreadId::from_string(&uuid.to_string()).unwrap();
    let path = write_session_file_with_history_mode(
        home.path(),
        "2025-01-03T12-00-00",
        uuid,
        ThreadHistoryMode::Paginated,
    )
    .unwrap();
    let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
    store
        .resume_thread(ResumeThreadParams {
            history_revision: None,
            thread_id,
            rollout_path: Some(path.clone()),
            history: None,
            include_archived: true,
            metadata: ThreadPersistenceMetadata {
                cwd: Some(home.path().to_path_buf()),
                model_provider: "test-provider".into(),
                memory_mode: ThreadMemoryMode::Enabled,
            },
        })
        .await
        .unwrap();
    let mut response = ResponseItem::Message {
        encrypted_content: None,
        status: None,
        id: Some(ResponseItemId::from_server("msg_voice".into())),
        role: "user".into(),
        content: vec![ContentItem::InputText {
            text: "voice input".into(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };
    response.set_turn_id_if_missing("turn-1");
    let input = ResponseItemEnvelope {
        item: response,
        metadata: Some(
            serde_json::from_value(serde_json::json!({
                "realtime_handoff": {
                    "event_id": "event-1",
                    "realtime_session_id": "call-1",
                    "observed_at_ms": 1,
                    "handoff_id": "handoff-1",
                    "mode": "start",
                    "source_event_ids": []
                }
            }))
            .unwrap(),
        ),
    };
    let parent = path.parent().unwrap();
    let moved = home.path().join("moved");
    std::fs::rename(parent, &moved).unwrap();
    assert!(
        store
            .seal_realtime_input(thread_id, vec![], input.clone())
            .await
            .is_err()
    );
    std::fs::rename(&moved, parent).unwrap();
    store
        .seal_realtime_input(thread_id, vec![], input)
        .await
        .unwrap();
    let (items, _, _) = RolloutRecorder::load_rollout_items(&path).await.unwrap();
    let accepted = items.iter().filter(|item| {
        matches!(item, RolloutItem::ResponseItem(envelope) if envelope.realtime_handoff_item().is_some())
    });
    assert_eq!(accepted.count(), 1);
}

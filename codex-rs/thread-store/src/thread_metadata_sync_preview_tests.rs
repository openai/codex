//! Verify delegated inputs emit a preview-only live metadata patch exactly once.

use super::*;
use crate::ThreadPersistenceMetadata;
use codex_protocol::items::FunctionCallOutputItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::protocol::ItemCompletedEvent;
use pretty_assertions::assert_eq;

#[test]
fn delegated_output_emits_only_first_preview_in_live_patch() {
    let thread_id = ThreadId::new();
    let mut sync = ThreadMetadataSync::for_resume(
        &ResumeThreadParams {
            thread_id,
            rollout_path: None,
            history: None,
            history_revision: None,
            include_archived: false,
            metadata: ThreadPersistenceMetadata {
                cwd: None,
                model_provider: "test-provider".to_string(),
                memory_mode: ThreadMemoryMode::Enabled,
            },
        },
        /*metadata*/ None,
    );
    for (index, text) in ["first task", "later task"].into_iter().enumerate() {
        let item = RolloutItem::EventMsg(EventMsg::ItemCompleted(ItemCompletedEvent {
            thread_id,
            turn_id: "turn".to_string(),
            item: TurnItem::FunctionCallOutput(FunctionCallOutputItem {
                id: format!("output-{index}"),
                name: "create_thread".to_string(),
                namespace: Some("codex_tui".to_string()),
                output: FunctionCallOutputBody::Text(format!(
                    "<codex_delegation>\n  <source_thread_id>source</source_thread_id>\n  <input>{text}</input>\n</codex_delegation>"
                )),
            }),
            started_at_ms: Some(0),
            completed_at_ms: 0,
        }));
        let update = sync.observe_appended_items(&[item]);
        if index == 0 {
            let update = update.expect("first delegated preview patch");
            let expected = ThreadMetadataPatch {
                preview: Some(text.to_string()),
                updated_at: update.patch.updated_at,
                ..Default::default()
            };
            assert_eq!(
                serde_json::to_value(&update.patch).unwrap(),
                serde_json::to_value(expected).unwrap()
            );
            sync.mark_pending_update_applied(&update);
        } else {
            assert_eq!(update.and_then(|update| update.patch.preview), None);
        }
    }
}

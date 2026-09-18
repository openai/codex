use crate::context::GuardianContextMode;
use crate::context::world_state::WorldStateSnapshot;
use crate::context_manager::ContextManager;
use crate::session::tests::make_session_and_context;
use codex_history::InitialHistory;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TokenUsage;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;

#[test_case(GuardianContextMode::Legacy; "legacy")]
#[test_case(GuardianContextMode::ThreadOwned; "thread_owned")]
#[tokio::test]
async fn guardian_checkpoint_preserves_live_context_without_storage(mode: GuardianContextMode) {
    let (mut session, turn) = make_session_and_context().await;
    session.guardian_context_mode = mode;
    // This session has no live store. A checkpoint must still capture the complete context.
    assert!(session.live_thread().is_none());
    let instruction: ResponseItem = serde_json::from_value(json!({
        "type": "message", "id": "user-restriction", "role": "user", "content": [
            {"type": "input_text", "text": "Do not publish the private report."}
        ],
        "internal_chat_message_metadata_passthrough": {"content_item_kinds": ["unknown"]}
    }))
    .unwrap();
    let context = turn.to_turn_context_item();
    let baseline = json!({"test": "baseline"});
    let world_state = WorldStateSnapshot::from(baseline.as_object().unwrap());
    {
        let mut state = session.state.lock().await;
        state.history = ContextManager::with_guardian_context_mode(mode, &SessionSource::default());
        state
            .history
            .record_items([&instruction], turn.model_info().truncation_policy.into());
        // The model window no longer contains the old restriction. Legacy review must
        // preserve it in its retained transcript when this checkpoint is forked.
        if mode == GuardianContextMode::Legacy {
            let compacted: ResponseItem = serde_json::from_value(json!({
                "type": "compaction", "id": "compaction-1", "encrypted_content": "summary",
            }))
            .unwrap();
            state.history.replace_compacted(
                vec![compacted.into()],
                /*reviewer_compaction_hash*/ None,
            );
        }
        let followup: ResponseItem = serde_json::from_value(json!({
            "type": "message", "id": "user-followup", "role": "user", "content": [
                {"type": "input_text", "text": "Review the next action."}
            ],
            "internal_chat_message_metadata_passthrough": {"content_item_kinds": ["unknown"]}
        }))
        .unwrap();
        state
            .history
            .record_items([&followup], turn.model_info().truncation_policy.into());
        state.history.set_reference_context_item(Some(context));
        state.history.set_world_state_baseline(world_state.clone());
        state.history.update_token_info(
            &TokenUsage {
                input_tokens: 123,
                total_tokens: 123,
                ..Default::default()
            },
            Some(32_000),
        );
    }
    let expected = session.clone_history().await;
    if mode == GuardianContextMode::Legacy {
        assert!(
            expected
                .guardian_history_checkpoint()
                .unwrap()
                .0
                .contains(&instruction)
        );
    }
    let items = session.guardian_fork_history().await;
    assert!(
        crate::thread_rollout_truncation::initial_history_has_prior_user_turns(
            &InitialHistory::Forked(items.clone()),
        )
    );
    // Mutating the live reviewer must not change the previously completed checkpoint.
    session
        .replace_history(Vec::new(), /*reference_context_item*/ None)
        .await;
    session
        .record_initial_history(InitialHistory::Forked(items))
        .await;
    let restored = session.clone_history().await;
    assert_eq!(restored.annotated_items(), expected.annotated_items());
    assert_eq!(restored.retained_context(), expected.retained_context());
    assert_eq!(
        restored.guardian_history_checkpoint(),
        expected.guardian_history_checkpoint()
    );
    assert_eq!(
        restored.world_state_checkpoint(),
        expected.world_state_checkpoint()
    );
    assert_eq!(
        serde_json::to_value(restored.reference_context_item()).unwrap(),
        serde_json::to_value(expected.reference_context_item()).unwrap()
    );
    assert_eq!(
        serde_json::to_value(restored.token_info()).unwrap(),
        serde_json::to_value(expected.token_info()).unwrap()
    );
}

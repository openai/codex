use super::*;

use crate::protocol::CompactedItem;
use crate::protocol::InitialHistory;
use crate::protocol::ResumedHistory;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

fn user_message(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: text.to_string(),
        }],
        end_turn: None,
        phase: None,
    }
}

fn assistant_message(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
        end_turn: None,
        phase: None,
    }
}

#[tokio::test]
async fn record_initial_history_resumed_bare_turn_context_does_not_hydrate_previous_model() {
    let (session, turn_context) = make_session_and_context().await;
    let previous_model = "previous-rollout-model";
    let previous_context_item = TurnContextItem {
        turn_id: Some(turn_context.sub_id.clone()),
        cwd: turn_context.cwd.clone(),
        current_date: turn_context.current_date.clone(),
        timezone: turn_context.timezone.clone(),
        approval_policy: turn_context.approval_policy.value(),
        sandbox_policy: turn_context.sandbox_policy.get().clone(),
        network: None,
        model: previous_model.to_string(),
        personality: turn_context.personality,
        collaboration_mode: Some(turn_context.collaboration_mode.clone()),
        effort: turn_context.reasoning_effort,
        summary: turn_context.reasoning_summary,
        user_instructions: None,
        developer_instructions: None,
        final_output_json_schema: None,
        truncation_policy: Some(turn_context.truncation_policy.into()),
    };
    let rollout_items = vec![RolloutItem::TurnContext(previous_context_item)];

    session
        .record_initial_history(InitialHistory::Resumed(ResumedHistory {
            conversation_id: ThreadId::default(),
            history: rollout_items,
            rollout_path: PathBuf::from("/tmp/resume.jsonl"),
        }))
        .await;

    assert_eq!(session.previous_model().await, None);
    assert!(session.reference_context_item().await.is_none());
}

#[tokio::test]
async fn record_initial_history_resumed_hydrates_previous_model_from_lifecycle_turn_with_missing_turn_context_id()
 {
    let (session, turn_context) = make_session_and_context().await;
    let previous_model = "previous-rollout-model";
    let mut previous_context_item = TurnContextItem {
        turn_id: Some(turn_context.sub_id.clone()),
        cwd: turn_context.cwd.clone(),
        current_date: turn_context.current_date.clone(),
        timezone: turn_context.timezone.clone(),
        approval_policy: turn_context.approval_policy.value(),
        sandbox_policy: turn_context.sandbox_policy.get().clone(),
        network: None,
        model: previous_model.to_string(),
        personality: turn_context.personality,
        collaboration_mode: Some(turn_context.collaboration_mode.clone()),
        effort: turn_context.reasoning_effort,
        summary: turn_context.reasoning_summary,
        user_instructions: None,
        developer_instructions: None,
        final_output_json_schema: None,
        truncation_policy: Some(turn_context.truncation_policy.into()),
    };
    let turn_id = previous_context_item
        .turn_id
        .clone()
        .expect("turn context should have turn_id");
    previous_context_item.turn_id = None;

    let rollout_items = vec![
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: turn_id.clone(),
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "seed".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::TurnContext(previous_context_item),
        RolloutItem::EventMsg(EventMsg::TurnComplete(
            codex_protocol::protocol::TurnCompleteEvent {
                turn_id,
                last_agent_message: None,
            },
        )),
    ];

    session
        .record_initial_history(InitialHistory::Resumed(ResumedHistory {
            conversation_id: ThreadId::default(),
            history: rollout_items,
            rollout_path: PathBuf::from("/tmp/resume.jsonl"),
        }))
        .await;

    assert_eq!(
        session.previous_model().await,
        Some(previous_model.to_string())
    );
}

#[tokio::test]
async fn reconstruct_history_rollback_keeps_history_and_metadata_in_sync_for_completed_turns() {
    let (session, turn_context) = make_session_and_context().await;
    let first_context_item = turn_context.to_turn_context_item();
    let first_turn_id = first_context_item
        .turn_id
        .clone()
        .expect("turn context should have turn_id");
    let mut rolled_back_context_item = first_context_item.clone();
    rolled_back_context_item.turn_id = Some("rolled-back-turn".to_string());
    rolled_back_context_item.model = "rolled-back-model".to_string();
    let rolled_back_turn_id = rolled_back_context_item
        .turn_id
        .clone()
        .expect("turn context should have turn_id");
    let turn_one_user = user_message("turn 1 user");
    let turn_one_assistant = assistant_message("turn 1 assistant");
    let turn_two_user = user_message("turn 2 user");
    let turn_two_assistant = assistant_message("turn 2 assistant");

    let rollout_items = vec![
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: first_turn_id.clone(),
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "turn 1 user".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::TurnContext(first_context_item.clone()),
        RolloutItem::ResponseItem(turn_one_user.clone()),
        RolloutItem::ResponseItem(turn_one_assistant.clone()),
        RolloutItem::EventMsg(EventMsg::TurnComplete(
            codex_protocol::protocol::TurnCompleteEvent {
                turn_id: first_turn_id,
                last_agent_message: None,
            },
        )),
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: rolled_back_turn_id.clone(),
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "turn 2 user".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::TurnContext(rolled_back_context_item),
        RolloutItem::ResponseItem(turn_two_user),
        RolloutItem::ResponseItem(turn_two_assistant),
        RolloutItem::EventMsg(EventMsg::TurnComplete(
            codex_protocol::protocol::TurnCompleteEvent {
                turn_id: rolled_back_turn_id,
                last_agent_message: None,
            },
        )),
        RolloutItem::EventMsg(EventMsg::ThreadRolledBack(
            codex_protocol::protocol::ThreadRolledBackEvent { num_turns: 1 },
        )),
    ];

    let reconstructed = session
        .reconstruct_history_from_rollout(&turn_context, &rollout_items)
        .await;

    assert_eq!(
        reconstructed.history,
        vec![turn_one_user, turn_one_assistant]
    );
    assert_eq!(
        reconstructed.previous_model,
        Some(turn_context.model_info.slug.clone())
    );
    assert_eq!(
        serde_json::to_value(reconstructed.reference_context_item)
            .expect("serialize reconstructed reference context item"),
        serde_json::to_value(Some(first_context_item))
            .expect("serialize expected reference context item")
    );
}

#[tokio::test]
async fn reconstruct_history_rollback_keeps_history_and_metadata_in_sync_for_incomplete_turn() {
    let (session, turn_context) = make_session_and_context().await;
    let first_context_item = turn_context.to_turn_context_item();
    let first_turn_id = first_context_item
        .turn_id
        .clone()
        .expect("turn context should have turn_id");
    let incomplete_turn_id = "incomplete-rolled-back-turn".to_string();
    let turn_one_user = user_message("turn 1 user");
    let turn_one_assistant = assistant_message("turn 1 assistant");
    let turn_two_user = user_message("turn 2 user");

    let rollout_items = vec![
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: first_turn_id.clone(),
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "turn 1 user".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::TurnContext(first_context_item.clone()),
        RolloutItem::ResponseItem(turn_one_user.clone()),
        RolloutItem::ResponseItem(turn_one_assistant.clone()),
        RolloutItem::EventMsg(EventMsg::TurnComplete(
            codex_protocol::protocol::TurnCompleteEvent {
                turn_id: first_turn_id,
                last_agent_message: None,
            },
        )),
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: incomplete_turn_id,
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "turn 2 user".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::ResponseItem(turn_two_user),
        RolloutItem::EventMsg(EventMsg::ThreadRolledBack(
            codex_protocol::protocol::ThreadRolledBackEvent { num_turns: 1 },
        )),
    ];

    let reconstructed = session
        .reconstruct_history_from_rollout(&turn_context, &rollout_items)
        .await;

    assert_eq!(
        reconstructed.history,
        vec![turn_one_user, turn_one_assistant]
    );
    assert_eq!(
        reconstructed.previous_model,
        Some(turn_context.model_info.slug.clone())
    );
    assert_eq!(
        serde_json::to_value(reconstructed.reference_context_item)
            .expect("serialize reconstructed reference context item"),
        serde_json::to_value(Some(first_context_item))
            .expect("serialize expected reference context item")
    );
}

#[tokio::test]
async fn reconstruct_history_rollback_skips_non_user_turns_for_history_and_metadata() {
    let (session, turn_context) = make_session_and_context().await;
    let first_context_item = turn_context.to_turn_context_item();
    let first_turn_id = first_context_item
        .turn_id
        .clone()
        .expect("turn context should have turn_id");
    let second_turn_id = "rolled-back-user-turn".to_string();
    let standalone_turn_id = "standalone-turn".to_string();
    let turn_one_user = user_message("turn 1 user");
    let turn_one_assistant = assistant_message("turn 1 assistant");
    let turn_two_user = user_message("turn 2 user");
    let turn_two_assistant = assistant_message("turn 2 assistant");
    let standalone_assistant = assistant_message("standalone assistant");

    let rollout_items = vec![
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: first_turn_id.clone(),
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "turn 1 user".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::TurnContext(first_context_item.clone()),
        RolloutItem::ResponseItem(turn_one_user.clone()),
        RolloutItem::ResponseItem(turn_one_assistant.clone()),
        RolloutItem::EventMsg(EventMsg::TurnComplete(
            codex_protocol::protocol::TurnCompleteEvent {
                turn_id: first_turn_id,
                last_agent_message: None,
            },
        )),
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: second_turn_id.clone(),
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "turn 2 user".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::ResponseItem(turn_two_user),
        RolloutItem::ResponseItem(turn_two_assistant),
        RolloutItem::EventMsg(EventMsg::TurnComplete(
            codex_protocol::protocol::TurnCompleteEvent {
                turn_id: second_turn_id,
                last_agent_message: None,
            },
        )),
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: standalone_turn_id.clone(),
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::ResponseItem(standalone_assistant),
        RolloutItem::EventMsg(EventMsg::TurnComplete(
            codex_protocol::protocol::TurnCompleteEvent {
                turn_id: standalone_turn_id,
                last_agent_message: None,
            },
        )),
        RolloutItem::EventMsg(EventMsg::ThreadRolledBack(
            codex_protocol::protocol::ThreadRolledBackEvent { num_turns: 1 },
        )),
    ];

    let reconstructed = session
        .reconstruct_history_from_rollout(&turn_context, &rollout_items)
        .await;

    assert_eq!(
        reconstructed.history,
        vec![turn_one_user, turn_one_assistant]
    );
    assert_eq!(
        reconstructed.previous_model,
        Some(turn_context.model_info.slug.clone())
    );
    assert_eq!(
        serde_json::to_value(reconstructed.reference_context_item)
            .expect("serialize reconstructed reference context item"),
        serde_json::to_value(Some(first_context_item))
            .expect("serialize expected reference context item")
    );
}

#[tokio::test]
async fn reconstruct_history_rollback_clears_history_and_metadata_when_exceeding_user_turns() {
    let (session, turn_context) = make_session_and_context().await;
    let only_context_item = turn_context.to_turn_context_item();
    let only_turn_id = only_context_item
        .turn_id
        .clone()
        .expect("turn context should have turn_id");
    let rollout_items = vec![
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: only_turn_id.clone(),
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "only user".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::TurnContext(only_context_item),
        RolloutItem::ResponseItem(user_message("only user")),
        RolloutItem::ResponseItem(assistant_message("only assistant")),
        RolloutItem::EventMsg(EventMsg::TurnComplete(
            codex_protocol::protocol::TurnCompleteEvent {
                turn_id: only_turn_id,
                last_agent_message: None,
            },
        )),
        RolloutItem::EventMsg(EventMsg::ThreadRolledBack(
            codex_protocol::protocol::ThreadRolledBackEvent { num_turns: 99 },
        )),
    ];

    let reconstructed = session
        .reconstruct_history_from_rollout(&turn_context, &rollout_items)
        .await;

    assert_eq!(reconstructed.history, Vec::new());
    assert_eq!(reconstructed.previous_model, None);
    assert!(reconstructed.reference_context_item.is_none());
}

#[tokio::test]
async fn record_initial_history_resumed_rollback_skips_only_user_turns() {
    let (session, turn_context) = make_session_and_context().await;
    let previous_context_item = turn_context.to_turn_context_item();
    let user_turn_id = previous_context_item
        .turn_id
        .clone()
        .expect("turn context should have turn_id");
    let standalone_turn_id = "standalone-task-turn".to_string();
    let rollout_items = vec![
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: user_turn_id.clone(),
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "seed".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::TurnContext(previous_context_item),
        RolloutItem::EventMsg(EventMsg::TurnComplete(
            codex_protocol::protocol::TurnCompleteEvent {
                turn_id: user_turn_id,
                last_agent_message: None,
            },
        )),
        // Standalone task turn (no UserMessage) should not consume rollback skips.
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: standalone_turn_id.clone(),
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::TurnComplete(
            codex_protocol::protocol::TurnCompleteEvent {
                turn_id: standalone_turn_id,
                last_agent_message: None,
            },
        )),
        RolloutItem::EventMsg(EventMsg::ThreadRolledBack(
            codex_protocol::protocol::ThreadRolledBackEvent { num_turns: 1 },
        )),
    ];

    session
        .record_initial_history(InitialHistory::Resumed(ResumedHistory {
            conversation_id: ThreadId::default(),
            history: rollout_items,
            rollout_path: PathBuf::from("/tmp/resume.jsonl"),
        }))
        .await;

    assert_eq!(session.previous_model().await, None);
    assert!(session.reference_context_item().await.is_none());
}

#[tokio::test]
async fn record_initial_history_resumed_rollback_drops_incomplete_user_turn_compaction_metadata() {
    let (session, turn_context) = make_session_and_context().await;
    let previous_context_item = turn_context.to_turn_context_item();
    let previous_turn_id = previous_context_item
        .turn_id
        .clone()
        .expect("turn context should have turn_id");
    let incomplete_turn_id = "incomplete-compacted-user-turn".to_string();

    let rollout_items = vec![
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: previous_turn_id.clone(),
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "seed".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::TurnContext(previous_context_item.clone()),
        RolloutItem::EventMsg(EventMsg::TurnComplete(
            codex_protocol::protocol::TurnCompleteEvent {
                turn_id: previous_turn_id,
                last_agent_message: None,
            },
        )),
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: incomplete_turn_id,
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "rolled back".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::Compacted(CompactedItem {
            message: String::new(),
            replacement_history: Some(Vec::new()),
        }),
        RolloutItem::EventMsg(EventMsg::ThreadRolledBack(
            codex_protocol::protocol::ThreadRolledBackEvent { num_turns: 1 },
        )),
    ];

    session
        .record_initial_history(InitialHistory::Resumed(ResumedHistory {
            conversation_id: ThreadId::default(),
            history: rollout_items,
            rollout_path: PathBuf::from("/tmp/resume.jsonl"),
        }))
        .await;

    assert_eq!(
        session.previous_model().await,
        Some(turn_context.model_info.slug.clone())
    );
    assert_eq!(
        serde_json::to_value(session.reference_context_item().await)
            .expect("serialize seeded reference context item"),
        serde_json::to_value(Some(previous_context_item))
            .expect("serialize expected reference context item")
    );
}

#[tokio::test]
async fn record_initial_history_resumed_bare_turn_context_does_not_seed_reference_context_item() {
    let (session, turn_context) = make_session_and_context().await;
    let previous_context_item = turn_context.to_turn_context_item();
    let rollout_items = vec![RolloutItem::TurnContext(previous_context_item.clone())];

    session
        .record_initial_history(InitialHistory::Resumed(ResumedHistory {
            conversation_id: ThreadId::default(),
            history: rollout_items,
            rollout_path: PathBuf::from("/tmp/resume.jsonl"),
        }))
        .await;

    assert!(session.reference_context_item().await.is_none());
}

#[tokio::test]
async fn record_initial_history_resumed_does_not_seed_reference_context_item_after_compaction() {
    let (session, turn_context) = make_session_and_context().await;
    let previous_context_item = turn_context.to_turn_context_item();
    let rollout_items = vec![
        RolloutItem::TurnContext(previous_context_item),
        RolloutItem::Compacted(CompactedItem {
            message: String::new(),
            replacement_history: Some(Vec::new()),
        }),
    ];

    session
        .record_initial_history(InitialHistory::Resumed(ResumedHistory {
            conversation_id: ThreadId::default(),
            history: rollout_items,
            rollout_path: PathBuf::from("/tmp/resume.jsonl"),
        }))
        .await;

    assert_eq!(session.previous_model().await, None);
    assert!(session.reference_context_item().await.is_none());
}

#[tokio::test]
async fn reconstruct_history_legacy_compaction_without_replacement_history_does_not_inject_current_initial_context()
 {
    let (session, turn_context) = make_session_and_context().await;
    let rollout_items = vec![
        RolloutItem::ResponseItem(user_message("before compact")),
        RolloutItem::ResponseItem(assistant_message("assistant reply")),
        RolloutItem::Compacted(CompactedItem {
            message: "legacy summary".to_string(),
            replacement_history: None,
        }),
    ];

    let reconstructed = session
        .reconstruct_history_from_rollout(&turn_context, &rollout_items)
        .await;

    assert_eq!(
        reconstructed.history,
        vec![
            user_message("before compact"),
            user_message("legacy summary"),
        ]
    );
    assert!(reconstructed.reference_context_item.is_none());
}

#[tokio::test]
async fn reconstruct_history_legacy_compaction_without_replacement_history_clears_later_reference_context_item()
 {
    let (session, turn_context) = make_session_and_context().await;
    let current_context_item = turn_context.to_turn_context_item();
    let current_turn_id = current_context_item
        .turn_id
        .clone()
        .expect("turn context should have turn_id");
    let rollout_items = vec![
        RolloutItem::ResponseItem(user_message("before compact")),
        RolloutItem::Compacted(CompactedItem {
            message: "legacy summary".to_string(),
            replacement_history: None,
        }),
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: current_turn_id.clone(),
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "after legacy compact".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::TurnContext(current_context_item),
        RolloutItem::EventMsg(EventMsg::TurnComplete(
            codex_protocol::protocol::TurnCompleteEvent {
                turn_id: current_turn_id,
                last_agent_message: None,
            },
        )),
    ];

    let reconstructed = session
        .reconstruct_history_from_rollout(&turn_context, &rollout_items)
        .await;

    assert!(reconstructed.reference_context_item.is_none());
}

#[tokio::test]
async fn record_initial_history_resumed_turn_context_after_compaction_reestablishes_reference_context_item()
 {
    let (session, turn_context) = make_session_and_context().await;
    let previous_model = "previous-rollout-model";
    let previous_context_item = TurnContextItem {
        turn_id: Some(turn_context.sub_id.clone()),
        cwd: turn_context.cwd.clone(),
        current_date: turn_context.current_date.clone(),
        timezone: turn_context.timezone.clone(),
        approval_policy: turn_context.approval_policy.value(),
        sandbox_policy: turn_context.sandbox_policy.get().clone(),
        network: None,
        model: previous_model.to_string(),
        personality: turn_context.personality,
        collaboration_mode: Some(turn_context.collaboration_mode.clone()),
        effort: turn_context.reasoning_effort,
        summary: turn_context.reasoning_summary,
        user_instructions: None,
        developer_instructions: None,
        final_output_json_schema: None,
        truncation_policy: Some(turn_context.truncation_policy.into()),
    };
    let previous_turn_id = previous_context_item
        .turn_id
        .clone()
        .expect("turn context should have turn_id");
    let rollout_items = vec![
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: previous_turn_id.clone(),
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "seed".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        // Compaction clears baseline until a later TurnContextItem re-establishes it.
        RolloutItem::Compacted(CompactedItem {
            message: String::new(),
            replacement_history: Some(Vec::new()),
        }),
        RolloutItem::TurnContext(previous_context_item),
        RolloutItem::EventMsg(EventMsg::TurnComplete(
            codex_protocol::protocol::TurnCompleteEvent {
                turn_id: previous_turn_id,
                last_agent_message: None,
            },
        )),
    ];

    session
        .record_initial_history(InitialHistory::Resumed(ResumedHistory {
            conversation_id: ThreadId::default(),
            history: rollout_items,
            rollout_path: PathBuf::from("/tmp/resume.jsonl"),
        }))
        .await;

    assert_eq!(
        session.previous_model().await,
        Some(previous_model.to_string())
    );
    assert_eq!(
        serde_json::to_value(session.reference_context_item().await)
            .expect("serialize seeded reference context item"),
        serde_json::to_value(Some(TurnContextItem {
            turn_id: Some(turn_context.sub_id.clone()),
            cwd: turn_context.cwd.clone(),
            current_date: turn_context.current_date.clone(),
            timezone: turn_context.timezone.clone(),
            approval_policy: turn_context.approval_policy.value(),
            sandbox_policy: turn_context.sandbox_policy.get().clone(),
            network: None,
            model: previous_model.to_string(),
            personality: turn_context.personality,
            collaboration_mode: Some(turn_context.collaboration_mode.clone()),
            effort: turn_context.reasoning_effort,
            summary: turn_context.reasoning_summary,
            user_instructions: None,
            developer_instructions: None,
            final_output_json_schema: None,
            truncation_policy: Some(turn_context.truncation_policy.into()),
        }))
        .expect("serialize expected reference context item")
    );
}

#[tokio::test]
async fn record_initial_history_resumed_aborted_turn_without_id_clears_active_turn_for_compaction_accounting()
 {
    let (session, turn_context) = make_session_and_context().await;
    let previous_model = "previous-rollout-model";
    let previous_context_item = TurnContextItem {
        turn_id: Some(turn_context.sub_id.clone()),
        cwd: turn_context.cwd.clone(),
        current_date: turn_context.current_date.clone(),
        timezone: turn_context.timezone.clone(),
        approval_policy: turn_context.approval_policy.value(),
        sandbox_policy: turn_context.sandbox_policy.get().clone(),
        network: None,
        model: previous_model.to_string(),
        personality: turn_context.personality,
        collaboration_mode: Some(turn_context.collaboration_mode.clone()),
        effort: turn_context.reasoning_effort,
        summary: turn_context.reasoning_summary,
        user_instructions: None,
        developer_instructions: None,
        final_output_json_schema: None,
        truncation_policy: Some(turn_context.truncation_policy.into()),
    };
    let previous_turn_id = previous_context_item
        .turn_id
        .clone()
        .expect("turn context should have turn_id");
    let aborted_turn_id = "aborted-turn-without-id".to_string();

    let rollout_items = vec![
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: previous_turn_id.clone(),
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "seed".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::TurnContext(previous_context_item),
        RolloutItem::EventMsg(EventMsg::TurnComplete(
            codex_protocol::protocol::TurnCompleteEvent {
                turn_id: previous_turn_id,
                last_agent_message: None,
            },
        )),
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: aborted_turn_id,
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "aborted".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::EventMsg(EventMsg::TurnAborted(
            codex_protocol::protocol::TurnAbortedEvent {
                turn_id: None,
                reason: TurnAbortReason::Interrupted,
            },
        )),
        RolloutItem::Compacted(CompactedItem {
            message: String::new(),
            replacement_history: Some(Vec::new()),
        }),
    ];

    session
        .record_initial_history(InitialHistory::Resumed(ResumedHistory {
            conversation_id: ThreadId::default(),
            history: rollout_items,
            rollout_path: PathBuf::from("/tmp/resume.jsonl"),
        }))
        .await;

    assert_eq!(
        session.previous_model().await,
        Some(previous_model.to_string())
    );
    assert!(session.reference_context_item().await.is_none());
}

#[tokio::test]
async fn record_initial_history_resumed_unmatched_abort_preserves_active_turn_for_later_turn_context()
 {
    let (session, turn_context) = make_session_and_context().await;
    let previous_context_item = turn_context.to_turn_context_item();
    let previous_turn_id = previous_context_item
        .turn_id
        .clone()
        .expect("turn context should have turn_id");
    let current_model = "current-rollout-model";
    let current_turn_id = "current-turn".to_string();
    let unmatched_abort_turn_id = "other-turn".to_string();
    let current_context_item = TurnContextItem {
        turn_id: Some(current_turn_id.clone()),
        cwd: turn_context.cwd.clone(),
        current_date: turn_context.current_date.clone(),
        timezone: turn_context.timezone.clone(),
        approval_policy: turn_context.approval_policy.value(),
        sandbox_policy: turn_context.sandbox_policy.get().clone(),
        network: None,
        model: current_model.to_string(),
        personality: turn_context.personality,
        collaboration_mode: Some(turn_context.collaboration_mode.clone()),
        effort: turn_context.reasoning_effort,
        summary: turn_context.reasoning_summary,
        user_instructions: None,
        developer_instructions: None,
        final_output_json_schema: None,
        truncation_policy: Some(turn_context.truncation_policy.into()),
    };

    let rollout_items = vec![
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: previous_turn_id.clone(),
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "seed".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::TurnContext(previous_context_item),
        RolloutItem::EventMsg(EventMsg::TurnComplete(
            codex_protocol::protocol::TurnCompleteEvent {
                turn_id: previous_turn_id,
                last_agent_message: None,
            },
        )),
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: current_turn_id.clone(),
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "current".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::EventMsg(EventMsg::TurnAborted(
            codex_protocol::protocol::TurnAbortedEvent {
                turn_id: Some(unmatched_abort_turn_id),
                reason: TurnAbortReason::Interrupted,
            },
        )),
        RolloutItem::TurnContext(current_context_item.clone()),
        RolloutItem::EventMsg(EventMsg::TurnComplete(
            codex_protocol::protocol::TurnCompleteEvent {
                turn_id: current_turn_id,
                last_agent_message: None,
            },
        )),
    ];

    session
        .record_initial_history(InitialHistory::Resumed(ResumedHistory {
            conversation_id: ThreadId::default(),
            history: rollout_items,
            rollout_path: PathBuf::from("/tmp/resume.jsonl"),
        }))
        .await;

    assert_eq!(
        session.previous_model().await,
        Some(current_model.to_string())
    );
    assert_eq!(
        serde_json::to_value(session.reference_context_item().await)
            .expect("serialize seeded reference context item"),
        serde_json::to_value(Some(current_context_item))
            .expect("serialize expected reference context item")
    );
}

#[tokio::test]
async fn record_initial_history_resumed_trailing_incomplete_turn_compaction_clears_reference_context_item()
 {
    let (session, turn_context) = make_session_and_context().await;
    let previous_model = "previous-rollout-model";
    let previous_context_item = TurnContextItem {
        turn_id: Some(turn_context.sub_id.clone()),
        cwd: turn_context.cwd.clone(),
        current_date: turn_context.current_date.clone(),
        timezone: turn_context.timezone.clone(),
        approval_policy: turn_context.approval_policy.value(),
        sandbox_policy: turn_context.sandbox_policy.get().clone(),
        network: None,
        model: previous_model.to_string(),
        personality: turn_context.personality,
        collaboration_mode: Some(turn_context.collaboration_mode.clone()),
        effort: turn_context.reasoning_effort,
        summary: turn_context.reasoning_summary,
        user_instructions: None,
        developer_instructions: None,
        final_output_json_schema: None,
        truncation_policy: Some(turn_context.truncation_policy.into()),
    };
    let previous_turn_id = previous_context_item
        .turn_id
        .clone()
        .expect("turn context should have turn_id");
    let incomplete_turn_id = "trailing-incomplete-turn".to_string();

    let rollout_items = vec![
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: previous_turn_id.clone(),
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "seed".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::TurnContext(previous_context_item),
        RolloutItem::EventMsg(EventMsg::TurnComplete(
            codex_protocol::protocol::TurnCompleteEvent {
                turn_id: previous_turn_id,
                last_agent_message: None,
            },
        )),
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: incomplete_turn_id,
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "incomplete".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::Compacted(CompactedItem {
            message: String::new(),
            replacement_history: Some(Vec::new()),
        }),
    ];

    session
        .record_initial_history(InitialHistory::Resumed(ResumedHistory {
            conversation_id: ThreadId::default(),
            history: rollout_items,
            rollout_path: PathBuf::from("/tmp/resume.jsonl"),
        }))
        .await;

    assert_eq!(
        session.previous_model().await,
        Some(previous_model.to_string())
    );
    assert!(session.reference_context_item().await.is_none());
}

#[tokio::test]
async fn record_initial_history_resumed_trailing_incomplete_turn_preserves_turn_context_item() {
    let (session, turn_context) = make_session_and_context().await;
    let current_context_item = turn_context.to_turn_context_item();
    let current_turn_id = current_context_item
        .turn_id
        .clone()
        .expect("turn context should have turn_id");

    let rollout_items = vec![
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: current_turn_id,
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "incomplete".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::TurnContext(current_context_item.clone()),
    ];

    session
        .record_initial_history(InitialHistory::Resumed(ResumedHistory {
            conversation_id: ThreadId::default(),
            history: rollout_items,
            rollout_path: PathBuf::from("/tmp/resume.jsonl"),
        }))
        .await;

    assert_eq!(
        session.previous_model().await,
        Some(turn_context.model_info.slug.clone())
    );
    assert_eq!(
        serde_json::to_value(session.reference_context_item().await)
            .expect("serialize seeded reference context item"),
        serde_json::to_value(Some(current_context_item))
            .expect("serialize expected reference context item")
    );
}

#[tokio::test]
async fn record_initial_history_resumed_replaced_incomplete_compacted_turn_clears_reference_context_item()
 {
    let (session, turn_context) = make_session_and_context().await;
    let previous_model = "previous-rollout-model";
    let previous_context_item = TurnContextItem {
        turn_id: Some(turn_context.sub_id.clone()),
        cwd: turn_context.cwd.clone(),
        current_date: turn_context.current_date.clone(),
        timezone: turn_context.timezone.clone(),
        approval_policy: turn_context.approval_policy.value(),
        sandbox_policy: turn_context.sandbox_policy.get().clone(),
        network: None,
        model: previous_model.to_string(),
        personality: turn_context.personality,
        collaboration_mode: Some(turn_context.collaboration_mode.clone()),
        effort: turn_context.reasoning_effort,
        summary: turn_context.reasoning_summary,
        user_instructions: None,
        developer_instructions: None,
        final_output_json_schema: None,
        truncation_policy: Some(turn_context.truncation_policy.into()),
    };
    let previous_turn_id = previous_context_item
        .turn_id
        .clone()
        .expect("turn context should have turn_id");
    let compacted_incomplete_turn_id = "compacted-incomplete-turn".to_string();
    let replacing_turn_id = "replacing-turn".to_string();

    let rollout_items = vec![
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: previous_turn_id.clone(),
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "seed".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::TurnContext(previous_context_item),
        RolloutItem::EventMsg(EventMsg::TurnComplete(
            codex_protocol::protocol::TurnCompleteEvent {
                turn_id: previous_turn_id,
                last_agent_message: None,
            },
        )),
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: compacted_incomplete_turn_id,
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
        RolloutItem::EventMsg(EventMsg::UserMessage(
            codex_protocol::protocol::UserMessageEvent {
                message: "compacted".to_string(),
                images: None,
                local_images: Vec::new(),
                text_elements: Vec::new(),
            },
        )),
        RolloutItem::Compacted(CompactedItem {
            message: String::new(),
            replacement_history: Some(Vec::new()),
        }),
        // A newer TurnStarted replaces the incomplete compacted turn without a matching
        // completion/abort for the old one.
        RolloutItem::EventMsg(EventMsg::TurnStarted(
            codex_protocol::protocol::TurnStartedEvent {
                turn_id: replacing_turn_id,
                model_context_window: Some(128_000),
                collaboration_mode_kind: ModeKind::Default,
            },
        )),
    ];

    session
        .record_initial_history(InitialHistory::Resumed(ResumedHistory {
            conversation_id: ThreadId::default(),
            history: rollout_items,
            rollout_path: PathBuf::from("/tmp/resume.jsonl"),
        }))
        .await;

    assert_eq!(
        session.previous_model().await,
        Some(previous_model.to_string())
    );
    assert!(session.reference_context_item().await.is_none());
}

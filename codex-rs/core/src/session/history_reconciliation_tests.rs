use super::history_reconciliation::persisted_history_cursor;
use super::tests::make_session_and_context;
use super::tests::make_session_and_context_with_rx;
use super::*;
use crate::codex_thread::ThreadHistoryReconciliationOutcome;
use crate::config::RolloutBudgetConfig;
use crate::config::TokenBudgetConfig;
use crate::context::ContextualUserFragment;
use crate::context::TokenBudgetReminder;
use crate::state::ActiveTurn;
use crate::state::AutoCompactWindowIds;
use crate::state::PersistedHistoryCursorUncertainty;
use codex_features::Feature;
use codex_protocol::models::ContentItem;
use codex_protocol::models::LocalShellAction;
use codex_protocol::models::LocalShellExecAction;
use codex_protocol::models::LocalShellStatus;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AdditionalContextEntry;
use codex_protocol::protocol::AdditionalContextKind;
use codex_protocol::protocol::CompactedItem;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::ThreadGoal;
use codex_protocol::protocol::ThreadGoalStatus;
use codex_protocol::protocol::ThreadGoalUpdatedEvent;
use codex_protocol::protocol::ThreadRolledBackEvent;
use codex_protocol::protocol::TokenCountEvent;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::protocol::TokenUsageInfo;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::TurnAbortedEvent;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

fn user_message(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: text.to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }
}

fn assistant_message(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }
}

fn turn_started(turn_id: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
        turn_id: turn_id.to_string(),
        trace_id: None,
        started_at: None,
        model_context_window: None,
        collaboration_mode_kind: Default::default(),
    }))
}

fn turn_complete(turn_id: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
        turn_id: turn_id.to_string(),
        last_agent_message: None,
        completed_at: None,
        duration_ms: None,
        time_to_first_token_ms: None,
    }))
}

fn completed_turn(turn_id: &str, user: &str, assistant: &str) -> Vec<RolloutItem> {
    vec![
        turn_started(turn_id),
        RolloutItem::ResponseItem(user_message(user)),
        RolloutItem::ResponseItem(assistant_message(assistant)),
        turn_complete(turn_id),
    ]
}

fn model_history_for_turn(user: &str, assistant: &str) -> Vec<ResponseItem> {
    vec![user_message(user), assistant_message(assistant)]
}

fn session_meta(thread_id: ThreadId) -> RolloutItem {
    RolloutItem::SessionMeta(SessionMetaLine {
        meta: SessionMeta {
            session_id: thread_id.into(),
            id: thread_id,
            ..SessionMeta::default()
        },
        git: None,
    })
}

fn local_shell_call(env: HashMap<String, String>) -> RolloutItem {
    RolloutItem::ResponseItem(ResponseItem::LocalShellCall {
        id: None,
        call_id: Some("shell-call".to_string()),
        status: LocalShellStatus::Completed,
        action: LocalShellAction::Exec(LocalShellExecAction {
            command: vec!["printenv".to_string()],
            timeout_ms: None,
            working_directory: None,
            env: Some(env),
            user: None,
        }),
        internal_chat_message_metadata_passthrough: None,
    })
}

fn token_usage_info(total_tokens: i64) -> TokenUsageInfo {
    TokenUsageInfo {
        total_token_usage: TokenUsage {
            input_tokens: total_tokens,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_output_tokens: 0,
            total_tokens,
        },
        last_token_usage: TokenUsage {
            input_tokens: total_tokens,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_output_tokens: 0,
            total_tokens,
        },
        model_context_window: Some(258_400),
    }
}

#[path = "history_reconciliation/cursor_recovery_tests.rs"]
mod cursor_recovery_tests;
#[path = "history_reconciliation/reminder_state_tests.rs"]
mod reminder_state_tests;
#[path = "history_reconciliation/safety_and_locking_tests.rs"]
mod safety_and_locking_tests;
#[path = "history_reconciliation/window_state_tests.rs"]
mod window_state_tests;

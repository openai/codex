use std::sync::Arc;

use super::Session;
use super::TurnContext;
use crate::protocol::CompactedItem;
use crate::protocol::TurnContextItem;
use crate::truncate::truncate_middle;
use askama::Template;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;

pub const SUMMARIZATION_PROMPT: &str = include_str!("../../templates/compact/prompt.md");
const COMPACT_USER_MESSAGE_MAX_TOKENS: usize = 20_000;

#[derive(Template)]
#[template(path = "compact/history_bridge.md", escape = "none")]
struct HistoryBridgeTemplate<'a> {
    user_messages_text: &'a str,
    summary_text: &'a str,
}

pub(crate) async fn run_inline_auto_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    cancellation_token: CancellationToken,
) {
    persist_turn_context_rollout(&sess, &turn_context).await;

    let input = vec![UserInput::Text {
        text: SUMMARIZATION_PROMPT.to_string(),
    }];

    // Build forked history from parent to seed sub-agent.
    let history_snapshot = sess.clone_history().await.get_history();
    let forked: Vec<RolloutItem> = history_snapshot
        .iter()
        .cloned()
        .map(RolloutItem::ResponseItem)
        .collect();

    // Launch sub-agent one-shot; drain to completion and capture summary.
    let config = turn_context.client.config().as_ref().clone();
    if let Ok(io) = crate::codex_delegate::run_codex_conversation_one_shot(
        crate::codex_delegate::SubAgentRunParams {
            config,
            auth_manager: sess.services.auth_manager.clone(),
            initial_history: Some(codex_protocol::protocol::InitialHistory::Forked(forked)),
            sub_source: codex_protocol::protocol::SubAgentSource::Compact,
            parent_session: Arc::clone(&sess),
            parent_ctx: Arc::clone(&turn_context),
            cancel_token: cancellation_token.child_token(),
        },
        input,
    )
    .await
    {
        let mut summary_text: Option<String> = None;
        while let Ok(event) = io.next_event().await {
            if let crate::protocol::EventMsg::TaskComplete(tc) = event.msg {
                summary_text = tc.last_agent_message;
                break;
            }
            if matches!(event.msg, crate::protocol::EventMsg::TurnAborted(_)) {
                break;
            }
        }
        if let Some(summary) = summary_text {
            apply_compaction(&sess, &turn_context, &summary).await;
            let event =
                crate::protocol::EventMsg::AgentMessage(crate::protocol::AgentMessageEvent {
                    message: "Compact task completed".to_string(),
                });
            sess.send_event(&Arc::clone(&turn_context), event).await;
        }
    }
}

pub fn content_items_to_text(content: &[ContentItem]) -> Option<String> {
    let mut pieces = Vec::new();
    for item in content {
        match item {
            ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                if !text.is_empty() {
                    pieces.push(text.as_str());
                }
            }
            ContentItem::InputImage { .. } => {}
        }
    }
    if pieces.is_empty() {
        None
    } else {
        Some(pieces.join("\n"))
    }
}

pub(crate) fn collect_user_messages(items: &[ResponseItem]) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| match crate::event_mapping::parse_turn_item(item) {
            Some(TurnItem::UserMessage(user)) => Some(user.message()),
            _ => None,
        })
        .collect()
}

pub(crate) fn build_compacted_history(
    initial_context: Vec<ResponseItem>,
    user_messages: &[String],
    summary_text: &str,
) -> Vec<ResponseItem> {
    build_compacted_history_with_limit(
        initial_context,
        user_messages,
        summary_text,
        COMPACT_USER_MESSAGE_MAX_TOKENS * 4,
    )
}

fn build_compacted_history_with_limit(
    mut history: Vec<ResponseItem>,
    user_messages: &[String],
    summary_text: &str,
    max_bytes: usize,
) -> Vec<ResponseItem> {
    let mut user_messages_text = if user_messages.is_empty() {
        "(none)".to_string()
    } else {
        user_messages.join("\n\n")
    };
    // Truncate the concatenated prior user messages so the bridge message
    // stays well under the context window (approx. 4 bytes/token).
    if user_messages_text.len() > max_bytes {
        user_messages_text = truncate_middle(&user_messages_text, max_bytes).0;
    }
    let summary_text = if summary_text.is_empty() {
        "(no summary available)".to_string()
    } else {
        summary_text.to_string()
    };
    let Ok(bridge) = HistoryBridgeTemplate {
        user_messages_text: &user_messages_text,
        summary_text: &summary_text,
    }
    .render() else {
        return vec![];
    };
    history.push(ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText { text: bridge }],
    });
    history
}

// streaming helpers removed; compact now uses the Codex delegate for sampling.

/// Apply compaction to the parent session given a summary text: rebuild the
/// conversation with a bridge message, preserve ghost snapshots, persist the
/// Compacted rollout entry, and replace history.
pub(crate) async fn apply_compaction(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    summary_text: &str,
) {
    let history_snapshot = sess.clone_history().await.get_history();
    let user_messages = collect_user_messages(&history_snapshot);
    let initial_context = sess.build_initial_context(turn_context.as_ref());
    let mut new_history = build_compacted_history(initial_context, &user_messages, summary_text);
    let ghost_snapshots: Vec<ResponseItem> = history_snapshot
        .iter()
        .filter(|item| matches!(item, ResponseItem::GhostSnapshot { .. }))
        .cloned()
        .collect();
    new_history.extend(ghost_snapshots);
    sess.replace_history(new_history).await;

    let rollout_item = RolloutItem::Compacted(CompactedItem {
        message: summary_text.to_string(),
    });
    sess.persist_rollout_items(&[rollout_item]).await;
}

/// Persist a TurnContext rollout entry capturing the model/session context.
pub(crate) async fn persist_turn_context_rollout(sess: &Session, turn_context: &TurnContext) {
    let rollout_item = RolloutItem::TurnContext(TurnContextItem {
        cwd: turn_context.cwd.clone(),
        approval_policy: turn_context.approval_policy,
        sandbox_policy: turn_context.sandbox_policy.clone(),
        model: turn_context.client.get_model(),
        effort: turn_context.client.get_reasoning_effort(),
        summary: turn_context.client.get_reasoning_summary(),
    });
    sess.persist_rollout_items(&[rollout_item]).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn content_items_to_text_joins_non_empty_segments() {
        let items = vec![
            ContentItem::InputText {
                text: "hello".to_string(),
            },
            ContentItem::OutputText {
                text: String::new(),
            },
            ContentItem::OutputText {
                text: "world".to_string(),
            },
        ];

        let joined = content_items_to_text(&items);

        assert_eq!(Some("hello\nworld".to_string()), joined);
    }

    #[test]
    fn content_items_to_text_ignores_image_only_content() {
        let items = vec![ContentItem::InputImage {
            image_url: "file://image.png".to_string(),
        }];

        let joined = content_items_to_text(&items);

        assert_eq!(None, joined);
    }

    #[test]
    fn collect_user_messages_extracts_user_text_only() {
        let items = vec![
            ResponseItem::Message {
                id: Some("assistant".to_string()),
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "ignored".to_string(),
                }],
            },
            ResponseItem::Message {
                id: Some("user".to_string()),
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "first".to_string(),
                }],
            },
            ResponseItem::Other,
        ];

        let collected = collect_user_messages(&items);

        assert_eq!(vec!["first".to_string()], collected);
    }

    #[test]
    fn collect_user_messages_filters_session_prefix_entries() {
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "<user_instructions>do things</user_instructions>".to_string(),
                }],
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "<ENVIRONMENT_CONTEXT>cwd=/tmp</ENVIRONMENT_CONTEXT>".to_string(),
                }],
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "real user message".to_string(),
                }],
            },
        ];

        let collected = collect_user_messages(&items);

        assert_eq!(vec!["real user message".to_string()], collected);
    }

    #[test]
    fn build_compacted_history_truncates_overlong_user_messages() {
        // Use a small truncation limit so the test remains fast while still validating
        // that oversized user content is truncated.
        let max_bytes = 128;
        let big = "X".repeat(max_bytes + 50);
        let history = super::build_compacted_history_with_limit(
            Vec::new(),
            std::slice::from_ref(&big),
            "SUMMARY",
            max_bytes,
        );

        // Expect exactly one bridge message added to history (plus any initial context we provided, which is none).
        assert_eq!(history.len(), 1);

        // Extract the text content of the bridge message.
        let bridge_text = match &history[0] {
            ResponseItem::Message { role, content, .. } if role == "user" => {
                content_items_to_text(content).unwrap_or_default()
            }
            other => panic!("unexpected item in history: {other:?}"),
        };

        // The bridge should contain the truncation marker and not the full original payload.
        assert!(
            bridge_text.contains("tokens truncated"),
            "expected truncation marker in bridge message"
        );
        assert!(
            !bridge_text.contains(&big),
            "bridge should not include the full oversized user text"
        );
        assert!(
            bridge_text.contains("SUMMARY"),
            "bridge should include the provided summary text"
        );
    }
}

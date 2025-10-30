use std::sync::Arc;

use super::Session;
use super::TurnContext;
use super::get_last_assistant_message_from_turn;
use crate::Prompt;
use crate::client_common::ResponseEvent;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::protocol::AgentMessageEvent;
use crate::protocol::CompactedItem;
use crate::protocol::ErrorEvent;
use crate::protocol::EventMsg;
use crate::protocol::TaskStartedEvent;
use crate::protocol::TurnContextItem;
use crate::truncate::truncate_middle;
use crate::util::backoff;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::user_input::UserInput;
use futures::prelude::*;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use serde_json::json;
use tracing::error;
use tracing::warn;
use uuid::Uuid;

pub const SUMMARIZATION_PROMPT: &str = include_str!("../../templates/compact/prompt.md");
const COMPACT_USER_MESSAGE_MAX_TOKENS: usize = 20_000;

pub(crate) async fn run_inline_auto_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
) {
    let prompt = turn_context.compact_prompt().to_string();
    let input = vec![UserInput::Text { text: prompt }];
    run_compact_task_inner(sess, turn_context, input).await;
}

pub(crate) async fn run_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    input: Vec<UserInput>,
) -> Option<String> {
    let start_event = EventMsg::TaskStarted(TaskStartedEvent {
        model_context_window: turn_context.client.get_model_context_window(),
    });
    sess.send_event(&turn_context, start_event).await;
    run_compact_task_inner(sess.clone(), turn_context, input).await;
    None
}

async fn run_compact_task_inner(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    input: Vec<UserInput>,
) {
    let initial_input_for_turn: ResponseInputItem = ResponseInputItem::from(input);
    let output_schema = structured_summary::schema();

    let mut history = sess.clone_history().await;
    history.record_items(&[initial_input_for_turn.into()]);

    let mut truncated_count = 0usize;

    let max_retries = turn_context.client.get_provider().stream_max_retries();
    let mut retries = 0;

    let rollout_item = RolloutItem::TurnContext(TurnContextItem {
        cwd: turn_context.cwd.clone(),
        approval_policy: turn_context.approval_policy,
        sandbox_policy: turn_context.sandbox_policy.clone(),
        model: turn_context.client.get_model(),
        effort: turn_context.client.get_reasoning_effort(),
        summary: turn_context.client.get_reasoning_summary(),
    });
    sess.persist_rollout_items(&[rollout_item]).await;

    loop {
        let turn_input = history.get_history_for_prompt();
        let prompt = Prompt {
            input: turn_input.clone(),
            output_schema: Some(output_schema.clone()),
            ..Default::default()
        };
        let attempt_result = drain_to_completed(&sess, turn_context.as_ref(), &prompt).await;

        match attempt_result {
            Ok(()) => {
                if truncated_count > 0 {
                    sess.notify_background_event(
                        turn_context.as_ref(),
                        format!(
                            "Trimmed {truncated_count} older conversation item(s) before compacting so the prompt fits the model context window."
                        ),
                    )
                    .await;
                }
                break;
            }
            Err(CodexErr::Interrupted) => {
                return;
            }
            Err(e @ CodexErr::ContextWindowExceeded) => {
                if turn_input.len() > 1 {
                    // Trim from the beginning to preserve cache (prefix-based) and keep recent messages intact.
                    error!(
                        "Context window exceeded while compacting; removing oldest history item. Error: {e}"
                    );
                    history.remove_first_item();
                    truncated_count += 1;
                    retries = 0;
                    continue;
                }
                sess.set_total_tokens_full(turn_context.as_ref()).await;
                let event = EventMsg::Error(ErrorEvent {
                    message: e.to_string(),
                });
                sess.send_event(&turn_context, event).await;
                return;
            }
            Err(e) => {
                if retries < max_retries {
                    retries += 1;
                    let delay = backoff(retries);
                    sess.notify_stream_error(
                        turn_context.as_ref(),
                        format!("Reconnecting... {retries}/{max_retries}"),
                    )
                    .await;
                    tokio::time::sleep(delay).await;
                    continue;
                } else {
                    let event = EventMsg::Error(ErrorEvent {
                        message: e.to_string(),
                    });
                    sess.send_event(&turn_context, event).await;
                    return;
                }
            }
        }
    }

    let history_snapshot = sess.clone_history().await.get_history();
    let structured_summary = structured_summary::parse(&history_snapshot);
    let (summary_text, user_messages) = match structured_summary {
        Some(structured) => {
            let structured_summary::Summary {
                intent_user_message,
                summary,
            } = structured;
            (summary, vec![intent_user_message])
        }
        None => {
            let summary =
                get_last_assistant_message_from_turn(&history_snapshot).unwrap_or_default();
            let users = collect_user_messages(&history_snapshot);
            (summary, users)
        }
    };
    let initial_context = sess.build_initial_context(turn_context.as_ref());
    let mut new_history = build_compacted_history(initial_context, &user_messages, &summary_text);
    let ghost_snapshots: Vec<ResponseItem> = history_snapshot
        .iter()
        .filter(|item| matches!(item, ResponseItem::GhostSnapshot { .. }))
        .cloned()
        .collect();
    new_history.extend(ghost_snapshots);
    sess.replace_history(new_history).await;

    let rollout_item = RolloutItem::Compacted(CompactedItem {
        message: summary_text.clone(),
    });
    sess.persist_rollout_items(&[rollout_item]).await;

    let event = EventMsg::AgentMessage(AgentMessageEvent {
        message: "Compact task completed".to_string(),
    });
    sess.send_event(&turn_context, event).await;
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
    let mut selected_messages: Vec<String> = Vec::new();
    if max_bytes > 0 {
        let mut remaining = max_bytes;
        for message in user_messages.iter().rev() {
            if remaining == 0 {
                break;
            }
            if message.len() <= remaining {
                selected_messages.push(message.clone());
                remaining = remaining.saturating_sub(message.len());
            } else {
                let (truncated, _) = truncate_middle(message, remaining);
                selected_messages.push(truncated);
                break;
            }
        }
        selected_messages.reverse();
    }

    for message in selected_messages {
        history.push(ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText { text: message }],
        });
    }

    let summary_text = if summary_text.is_empty() {
        "(no summary available)".to_string()
    } else {
        summary_text.to_string()
    };

    let call_id = Uuid::new_v4().to_string();
    history.push(ResponseItem::CustomToolCall {
        id: None,
        status: Some("completed".to_string()),
        call_id: call_id.clone(),
        name: "compactor".to_string(),
        input: String::new(),
    });
    history.push(ResponseItem::CustomToolCallOutput {
        call_id,
        output: summary_text,
    });

    history
}

async fn drain_to_completed(
    sess: &Session,
    turn_context: &TurnContext,
    prompt: &Prompt,
) -> CodexResult<()> {
    let mut stream = turn_context.client.clone().stream(prompt).await?;
    loop {
        let maybe_event = stream.next().await;
        let Some(event) = maybe_event else {
            return Err(CodexErr::Stream(
                "stream closed before response.completed".into(),
                None,
            ));
        };
        match event {
            Ok(ResponseEvent::OutputItemDone(item)) => {
                sess.record_into_history(std::slice::from_ref(&item)).await;
            }
            Ok(ResponseEvent::RateLimits(snapshot)) => {
                sess.update_rate_limits(turn_context, snapshot).await;
            }
            Ok(ResponseEvent::Completed { token_usage, .. }) => {
                sess.update_token_usage_info(turn_context, token_usage.as_ref())
                    .await;
                return Ok(());
            }
            Ok(_) => continue,
            Err(e) => return Err(e),
        }
    }
}

mod structured_summary {
    use super::*;

    #[derive(Debug, Deserialize)]
    pub struct Summary {
        pub(crate) intent_user_message: String,
        pub(crate) summary: String,
    }

    pub fn schema() -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "intent_user_message": {
                    "type": "string",
                    "description": "One consolidated user message capturing the user's current goal or request."
                },
                "summary": {
                    "type": "string",
                    "description": "A concise status summary describing progress and next steps."
                }
            },
            "required": ["intent_user_message", "summary"],
            "additionalProperties": false
        })
    }

    pub fn parse(responses: &[ResponseItem]) -> Option<Summary> {
        let text = get_last_assistant_message_from_turn(responses)?;
        let parsed: Summary = match serde_json::from_str(text.trim()) {
            Ok(parsed) => parsed,
            Err(err) => {
                warn!(?err, "Failed to parse structured compact summary");
                return None;
            }
        };

        let intent = parsed.intent_user_message.trim();
        if intent.is_empty() {
            warn!("Structured compact summary missing intent_user_message");
            return None;
        }

        Some(Summary {
            intent_user_message: intent.to_string(),
            summary: parsed.summary.trim().to_string(),
        })
    }
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
    fn parse_auto_compact_summary_extracts_trimmed_fields() {
        let payload = r#"
        {
            "intent_user_message": "  intent summary ",
            "summary": " status note "
        }
        "#;
        let responses = vec![ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: payload.to_string(),
            }],
        }];

        let parsed = structured_summary::parse(&responses).expect("structured summary expected");
        assert_eq!(parsed.intent_user_message, "intent summary");
        assert_eq!(parsed.summary, "status note");
    }

    #[test]
    fn parse_auto_compact_summary_requires_intent() {
        let payload = r#"{"intent_user_message":"   ","summary":"just text"}"#;
        let responses = vec![ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: payload.to_string(),
            }],
        }];

        assert!(structured_summary::parse(&responses).is_none());
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
        assert_eq!(history.len(), 3);

        let truncated_message = &history[0];
        let tool_call = &history[1];
        let tool_output = &history[2];

        let truncated_text = match truncated_message {
            ResponseItem::Message { role, content, .. } if role == "user" => {
                content_items_to_text(content).unwrap_or_default()
            }
            other => panic!("unexpected item in history: {other:?}"),
        };

        assert!(
            truncated_text.contains("tokens truncated"),
            "expected truncation marker in truncated user message"
        );
        assert!(
            !truncated_text.contains(&big),
            "truncated user message should not include the full oversized user text"
        );

        match tool_call {
            ResponseItem::CustomToolCall { name, .. } => {
                assert_eq!(name, "compactor");
            }
            other => panic!("expected CustomToolCall, got {other:?}"),
        }

        match tool_output {
            ResponseItem::CustomToolCallOutput { output, .. } => {
                assert_eq!(output, "SUMMARY");
            }
            other => panic!("expected CustomToolCallOutput, got {other:?}"),
        }
    }
}

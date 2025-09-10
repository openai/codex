use std::sync::Arc;

use crate::Prompt;
use crate::client_common::ResponseEvent;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::protocol::AgentMessageEvent;
use crate::protocol::ErrorEvent;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::InputItem;
use crate::protocol::TaskCompleteEvent;
use crate::protocol::TaskStartedEvent;
use crate::protocol::TokenCountEvent;
use crate::protocol::TokenUsage;
use crate::protocol::TokenUsageInfo;
use crate::util::backoff;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use futures::prelude::*;

use super::AgentTask;
use super::MutexExt;
use super::Session;
use super::TurnContext;
use super::get_last_assistant_message_from_turn;

pub(super) const COMPACT_TRIGGER_TEXT: &str = "Start Summarization";
const SUMMARIZATION_PROMPT: &str = include_str!("../prompt_for_compact_command.md");
const AUTO_COMPACT_TOKEN_LIMIT: u64 = 120_000;

pub(super) fn spawn_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    sub_id: String,
    input: Vec<InputItem>,
) {
    let task = AgentTask::compact(
        sess.clone(),
        turn_context,
        sub_id,
        input,
        SUMMARIZATION_PROMPT.to_string(),
    );
    sess.set_task(task);
}

pub(super) fn spawn_auto_compact_task(sess: Arc<Session>, turn_context: Arc<TurnContext>) {
    let sub_id = sess.next_internal_sub_id();
    let input = vec![InputItem::Text {
        text: COMPACT_TRIGGER_TEXT.to_string(),
    }];
    spawn_compact_task(sess, turn_context, sub_id, input);
}

pub(super) async fn run_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    sub_id: String,
    input: Vec<InputItem>,
    compact_instructions: String,
) {
    let model_context_window = turn_context.client.get_model_context_window();
    let start_event = Event {
        id: sub_id.clone(),
        msg: EventMsg::TaskStarted(TaskStartedEvent {
            model_context_window,
        }),
    };
    sess.send_event(start_event).await;

    let initial_input_for_turn: ResponseInputItem = ResponseInputItem::from(input);
    let turn_input: Vec<ResponseItem> =
        sess.turn_input_with_history(vec![initial_input_for_turn.clone().into()]);

    let prompt = Prompt {
        input: turn_input,
        tools: Vec::new(),
        base_instructions_override: Some(compact_instructions.clone()),
    };

    let max_retries = turn_context.client.get_provider().stream_max_retries();
    let mut retries = 0;

    loop {
        let attempt_result =
            drain_to_completed(&sess, turn_context.as_ref(), &sub_id, &prompt).await;

        match attempt_result {
            Ok(()) => break,
            Err(CodexErr::Interrupted) => return,
            Err(e) => {
                if retries < max_retries {
                    retries += 1;
                    let delay = backoff(retries);
                    sess.notify_stream_error(
                        &sub_id,
                        format!(
                            "stream error: {e}; retrying {retries}/{max_retries} in {delay:?}…"
                        ),
                    )
                    .await;
                    tokio::time::sleep(delay).await;
                    continue;
                } else {
                    let event = Event {
                        id: sub_id.clone(),
                        msg: EventMsg::Error(ErrorEvent {
                            message: e.to_string(),
                        }),
                    };
                    sess.send_event(event).await;
                    return;
                }
            }
        }
    }

    sess.remove_task(&sub_id);
    let history_snapshot = {
        let state = sess.state.lock_unchecked();
        state.history.contents()
    };
    let summary_text = get_last_assistant_message_from_turn(&history_snapshot).unwrap_or_default();
    let user_messages = collect_user_messages(&history_snapshot);
    let new_history =
        build_compacted_history(&sess, turn_context.as_ref(), &user_messages, &summary_text);
    {
        let mut state = sess.state.lock_unchecked();
        state.history.replace(new_history);
        state.token_info = None;
        state.token_limit_reached = false;
    }

    let event = Event {
        id: sub_id.clone(),
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "Compact task completed".to_string(),
        }),
    };
    sess.send_event(event).await;
    let event = Event {
        id: sub_id.clone(),
        msg: EventMsg::TaskComplete(TaskCompleteEvent {
            last_agent_message: None,
        }),
    };
    sess.send_event(event).await;
}

pub(super) fn update_token_usage_info(
    sess: &Session,
    turn_context: &TurnContext,
    token_usage: &Option<TokenUsage>,
) -> Option<TokenUsageInfo> {
    let mut state = sess.state.lock_unchecked();
    let info = TokenUsageInfo::new_or_append(
        &state.token_info,
        token_usage,
        turn_context.client.get_model_context_window(),
    );
    if let Some(ref info) = info
        && info.total_token_usage.total_tokens >= AUTO_COMPACT_TOKEN_LIMIT
    {
        state.token_limit_reached = true;
    }
    state.token_info = info.clone();
    info
}

fn content_items_to_text(content: &[ContentItem]) -> Option<String> {
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

fn collect_user_messages(items: &[ResponseItem]) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| match item {
            ResponseItem::Message { role, content, .. } if role == "user" => {
                content_items_to_text(content)
            }
            _ => None,
        })
        .collect()
}

fn build_compacted_history(
    sess: &Session,
    turn_context: &TurnContext,
    user_messages: &[String],
    summary_text: &str,
) -> Vec<ResponseItem> {
    let mut history = sess.build_initial_context(turn_context);
    let user_messages_text = if user_messages.is_empty() {
        "(none)".to_string()
    } else {
        user_messages.join("\n\n")
    };
    let summary_text = if summary_text.is_empty() {
        "(no summary available)".to_string()
    } else {
        summary_text.to_string()
    };
    let bridge = format!(
        "Here are all the user messages {user_messages_text}. Another LLM started working on this task, here is the current state: {summary_text}. Please, finish the task",
    );
    history.push(ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText { text: bridge }],
    });
    history
}

async fn drain_to_completed(
    sess: &Session,
    turn_context: &TurnContext,
    sub_id: &str,
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
                let mut state = sess.state.lock_unchecked();
                state.history.record_items(std::slice::from_ref(&item));
            }
            Ok(ResponseEvent::Completed {
                response_id: _,
                token_usage,
            }) => {
                let info = update_token_usage_info(sess, turn_context, &token_usage);

                sess.tx_event
                    .send(Event {
                        id: sub_id.to_string(),
                        msg: EventMsg::TokenCount(TokenCountEvent { info }),
                    })
                    .await
                    .ok();

                return Ok(());
            }
            Ok(_) => continue,
            Err(e) => return Err(e),
        }
    }
}

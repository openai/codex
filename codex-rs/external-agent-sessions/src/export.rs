use crate::ConversationMessage;
use crate::ImportedExternalAgentSession;
use crate::MessageRole;
use crate::records::conversation_messages;
use crate::records::custom_title_from_records;
use crate::records::project_root_from_records;
use crate::records::read_records;
use crate::summarize_for_label;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AgentMessageEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::TokenCountEvent;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::protocol::TokenUsageInfo;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::protocol::UserMessageEvent;
use codex_utils_output_truncation::approx_tokens_from_byte_count_i64;
use std::io;
use std::path::Path;

pub fn load_session_for_import(path: &Path) -> io::Result<Option<ImportedExternalAgentSession>> {
    let records = read_records(path)?;
    let Some(cwd) = project_root_from_records(&records) else {
        return Ok(None);
    };
    let messages = conversation_messages(&records);
    let rollout_items = rollout_items_from_messages(&messages);
    if rollout_items.is_empty() {
        return Ok(None);
    }
    let title = custom_title_from_records(&records).or_else(|| {
        messages
            .iter()
            .find(|message| message.role == MessageRole::User)
            .map(|message| summarize_for_label(&message.text))
    });
    Ok(Some(ImportedExternalAgentSession {
        cwd,
        title,
        rollout_items,
    }))
}

fn rollout_items_from_messages(messages: &[ConversationMessage]) -> Vec<RolloutItem> {
    let mut items = Vec::new();
    let mut response_items = Vec::new();
    let mut current_turn: Option<(String, Option<String>)> = None;
    let mut user_turn_count = 0usize;

    for message in messages {
        match message.role {
            MessageRole::User => {
                if let Some((turn_id, last_agent_message)) = current_turn.take() {
                    items.push(turn_complete_item(
                        turn_id,
                        last_agent_message,
                        /*completed_at*/ None,
                    ));
                }
                user_turn_count += 1;
                let turn_id = format!("external-import-turn-{user_turn_count}");
                items.push(RolloutItem::EventMsg(EventMsg::TurnStarted(
                    TurnStartedEvent {
                        turn_id: turn_id.clone(),
                        started_at: message.timestamp,
                        model_context_window: None,
                        collaboration_mode_kind: Default::default(),
                    },
                )));
                let response_item = response_item(message);
                response_items.push(response_item.clone());
                items.push(RolloutItem::ResponseItem(response_item));
                items.push(RolloutItem::EventMsg(EventMsg::UserMessage(
                    UserMessageEvent {
                        message: message.text.clone(),
                        images: None,
                        local_images: Vec::new(),
                        text_elements: Vec::new(),
                    },
                )));
                current_turn = Some((turn_id, None));
            }
            MessageRole::Assistant => {
                let Some((_, last_agent_message)) = current_turn.as_mut() else {
                    continue;
                };
                let response_item = response_item(message);
                response_items.push(response_item.clone());
                items.push(RolloutItem::ResponseItem(response_item));
                items.push(RolloutItem::EventMsg(EventMsg::AgentMessage(
                    AgentMessageEvent {
                        message: message.text.clone(),
                        phase: None,
                        memory_citation: None,
                    },
                )));
                *last_agent_message = Some(message.text.clone());
            }
        }
    }

    if let Some((turn_id, last_agent_message)) = current_turn {
        items.push(token_count_item(&response_items));
        let completed_at = messages.last().and_then(|message| message.timestamp);
        items.push(turn_complete_item(
            turn_id,
            last_agent_message,
            completed_at,
        ));
    }

    items
}

fn response_item(message: &ConversationMessage) -> ResponseItem {
    let content = match message.role {
        MessageRole::Assistant => ContentItem::OutputText {
            text: message.text.clone(),
        },
        MessageRole::User => ContentItem::InputText {
            text: message.text.clone(),
        },
    };
    ResponseItem::Message {
        id: None,
        role: match message.role {
            MessageRole::Assistant => "assistant".to_string(),
            MessageRole::User => "user".to_string(),
        },
        content: vec![content],
        phase: None,
    }
}

fn token_count_item(response_items: &[ResponseItem]) -> RolloutItem {
    let last_model_generated = response_items.iter().rposition(
        |item| matches!(item, ResponseItem::Message { role, .. } if role == "assistant"),
    );
    let last_model_visible_tokens = last_model_generated
        .map(|index| estimate_response_items_token_count(&response_items[..=index]))
        .unwrap_or_default();
    let usage = TokenUsage {
        total_tokens: last_model_visible_tokens,
        ..TokenUsage::default()
    };
    RolloutItem::EventMsg(EventMsg::TokenCount(TokenCountEvent {
        info: Some(TokenUsageInfo {
            total_token_usage: usage.clone(),
            last_token_usage: usage,
            model_context_window: None,
        }),
        rate_limits: None,
    }))
}

fn estimate_response_items_token_count(response_items: &[ResponseItem]) -> i64 {
    response_items
        .iter()
        .map(|item| {
            serde_json::to_string(item)
                .map(|serialized| i64::try_from(serialized.len()).unwrap_or(i64::MAX))
                .map(approx_tokens_from_byte_count_i64)
                .unwrap_or_default()
        })
        .fold(0i64, i64::saturating_add)
}

fn turn_complete_item(
    turn_id: String,
    last_agent_message: Option<String>,
    completed_at: Option<i64>,
) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
        turn_id,
        last_agent_message,
        completed_at,
        duration_ms: None,
        time_to_first_token_ms: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::build_turns_from_rollout_items;
    use serde_json::Value as JsonValue;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn builds_visible_turns_for_imported_history() {
        let root = TempDir::new().expect("tempdir");
        let project_root = root.path().join("repo");
        std::fs::create_dir_all(&project_root).expect("project root");
        let path = root.path().join("session.jsonl");
        std::fs::write(
            &path,
            jsonl(&[
                record("user", "first request", &project_root),
                record("assistant", "first answer", &project_root),
                record("user", "second request", &project_root),
            ]),
        )
        .expect("session");

        let imported = load_session_for_import(&path)
            .expect("load")
            .expect("session");
        let turns = build_turns_from_rollout_items(&imported.rollout_items);

        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].items.len(), 2);
        assert_eq!(turns[1].items.len(), 1);
    }

    #[test]
    fn loads_custom_title_for_imported_session() {
        let root = TempDir::new().expect("tempdir");
        let project_root = root.path().join("repo");
        std::fs::create_dir_all(&project_root).expect("project root");
        let path = root.path().join("session.jsonl");
        std::fs::write(
            &path,
            jsonl(&[
                record("user", "first request", &project_root),
                custom_title_record("named by source app"),
            ]),
        )
        .expect("session");

        let imported = load_session_for_import(&path)
            .expect("load")
            .expect("session");

        assert_eq!(imported.title.as_deref(), Some("named by source app"));
    }

    #[test]
    fn emits_token_usage_for_imported_history() {
        let root = TempDir::new().expect("tempdir");
        let project_root = root.path().join("repo");
        std::fs::create_dir_all(&project_root).expect("project root");
        let path = root.path().join("session.jsonl");
        std::fs::write(
            &path,
            jsonl(&[
                record("user", "first request", &project_root),
                record("assistant", "first answer", &project_root),
                record("user", "second request", &project_root),
            ]),
        )
        .expect("session");

        let imported = load_session_for_import(&path)
            .expect("load")
            .expect("session");
        let token_count = imported
            .rollout_items
            .iter()
            .find_map(|item| match item {
                RolloutItem::EventMsg(EventMsg::TokenCount(event)) => event.info.clone(),
                _ => None,
            })
            .expect("token count event");

        assert!(token_count.last_token_usage.total_tokens > 0);
        assert_eq!(token_count.total_token_usage, token_count.last_token_usage);
    }

    fn record(role: &str, text: &str, cwd: &Path) -> JsonValue {
        let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        serde_json::json!({
            "type": role,
            "cwd": cwd,
            "timestamp": timestamp,
            "message": { "content": text }
        })
    }

    fn custom_title_record(title: &str) -> JsonValue {
        serde_json::json!({
            "type": "custom-title",
            "customTitle": title,
        })
    }

    fn jsonl(records: &[JsonValue]) -> String {
        records
            .iter()
            .map(JsonValue::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

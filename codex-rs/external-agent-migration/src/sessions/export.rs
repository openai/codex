use super::ConversationMessage;
use super::ImportedExternalAgentSession;
use super::MessageRole;
use super::records::read_session_import_with_cwd;
use super::summarize_for_label;
use super::title::IMPORTED_SESSION_FALLBACK_TITLE;
use super::title::SessionTitleCandidates;
use super::title::fallback_title_from_user_message;
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
use std::collections::BTreeSet;
use std::io;
use std::path::Path;

const EXTERNAL_SESSION_IMPORTED_MARKER: &str = "<EXTERNAL SESSION IMPORTED>";

#[cfg(test)]
fn load_session_for_import(path: &Path) -> io::Result<Option<ImportedExternalAgentSession>> {
    Ok(
        load_session_for_import_with_content_sha256(path, /*fallback_cwd*/ None)?
            .map(|(session, _content_sha256, _attributed_mcp_server_ids)| session),
    )
}

pub(crate) fn load_session_for_import_with_content_sha256(
    path: &Path,
    fallback_cwd: Option<&Path>,
) -> io::Result<Option<(ImportedExternalAgentSession, String, BTreeSet<String>)>> {
    let parsed = read_session_import_with_cwd(path, fallback_cwd)?;
    let Some(cwd) = parsed.cwd else {
        return Ok(None);
    };
    let attributed_mcp_server_ids = parsed.attributed_mcp_server_ids;
    let messages = parsed.messages;
    let first_user_message_text = messages
        .iter()
        .find(|message| message.role == MessageRole::User)
        .map(|message| message.text.as_str());
    let first_user_message = first_user_message_text.map(summarize_for_label);
    let fallback_title = messages
        .iter()
        .filter(|message| message.role == MessageRole::User)
        .find_map(|message| fallback_title_from_user_message(&message.text))
        .or_else(|| first_user_message_text.map(|_| IMPORTED_SESSION_FALLBACK_TITLE.to_string()));
    let title = SessionTitleCandidates {
        custom_title: parsed.custom_title,
        ai_title: parsed.ai_title,
        fallback_title,
    }
    .select();
    let rollout_items = rollout_items_from_messages(messages);
    if rollout_items.is_empty() {
        return Ok(None);
    }
    Ok(Some((
        ImportedExternalAgentSession {
            cwd,
            title,
            first_user_message,
            rollout_items,
        },
        parsed.content_sha256,
        attributed_mcp_server_ids,
    )))
}

fn rollout_items_from_messages(messages: Vec<ConversationMessage>) -> Vec<RolloutItem> {
    let mut items = Vec::new();
    let mut current_turn = None;
    let mut response_item_bytes = 0i64;
    let mut last_model_visible_tokens = 0i64;
    let mut user_turn_count = 0usize;
    let completed_at = messages.last().and_then(|message| message.timestamp);

    for message in messages {
        match message.role {
            MessageRole::User => {
                let started_at = message.timestamp;
                if let Some((turn_id, previous_started_at)) = current_turn.take() {
                    items.push(turn_complete_item(
                        turn_id,
                        previous_started_at,
                        /*completed_at*/ None,
                    ));
                }
                user_turn_count += 1;
                let turn_id = format!("external-import-turn-{user_turn_count}");
                items.push(RolloutItem::EventMsg(EventMsg::TurnStarted(
                    TurnStartedEvent {
                        turn_id: turn_id.clone(),
                        trace_id: None,
                        started_at,
                        model_context_window: None,
                        collaboration_mode_kind: Default::default(),
                    },
                )));
                items.push(RolloutItem::EventMsg(EventMsg::UserMessage(
                    UserMessageEvent {
                        message: message.text.clone(),
                        ..Default::default()
                    },
                )));
                response_item_bytes =
                    response_item_bytes.saturating_add(message_byte_count(&message));
                items.push(RolloutItem::ResponseItem(response_item(message)));
                current_turn = Some((turn_id, started_at));
            }
            MessageRole::Assistant => {
                if current_turn.is_none() {
                    continue;
                }
                response_item_bytes =
                    response_item_bytes.saturating_add(message_byte_count(&message));
                last_model_visible_tokens = approx_tokens_from_byte_count_i64(response_item_bytes);
                items.push(RolloutItem::EventMsg(EventMsg::AgentMessage(
                    AgentMessageEvent {
                        message: message.text.clone(),
                        phase: None,
                        memory_citation: None,
                    },
                )));
                items.push(RolloutItem::ResponseItem(response_item(message)));
            }
        }
    }

    if let Some((turn_id, started_at)) = current_turn {
        items.push(external_session_imported_marker_item());
        items.push(token_count_item(last_model_visible_tokens));
        items.push(turn_complete_item(turn_id, started_at, completed_at));
    }

    items
}

fn external_session_imported_marker_item() -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::AgentMessage(AgentMessageEvent {
        message: EXTERNAL_SESSION_IMPORTED_MARKER.to_string(),
        phase: None,
        memory_citation: None,
    }))
}

fn response_item(message: ConversationMessage) -> ResponseItem {
    let content = match message.role {
        MessageRole::Assistant => ContentItem::OutputText { text: message.text },
        MessageRole::User => ContentItem::InputText { text: message.text },
    };
    ResponseItem::Message {
        id: None,
        role: match message.role {
            MessageRole::Assistant => "assistant".to_string(),
            MessageRole::User => "user".to_string(),
        },
        content: vec![content],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }
}

fn message_byte_count(message: &ConversationMessage) -> i64 {
    i64::try_from(message.text.len()).unwrap_or(i64::MAX)
}

fn token_count_item(last_model_visible_tokens: i64) -> RolloutItem {
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

fn turn_complete_item(
    turn_id: String,
    started_at: Option<i64>,
    completed_at: Option<i64>,
) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
        turn_id,
        last_agent_message: None,
        error: None,
        started_at,
        completed_at,
        duration_ms: None,
        time_to_first_token_ms: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::ThreadItem;
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
        assert_eq!(turns[1].items.len(), 2);
        assert_eq!(
            turns[1].items[1],
            ThreadItem::AgentMessage {
                id: "item-4".into(),
                text: EXTERNAL_SESSION_IMPORTED_MARKER.into(),
                phase: None,
                memory_citation: None,
            }
        );
    }

    #[test]
    fn adds_import_marker_without_copying_last_agent_message() {
        let root = TempDir::new().expect("tempdir");
        let project_root = root.path().join("repo");
        std::fs::create_dir_all(&project_root).expect("project root");
        let path = root.path().join("session.jsonl");
        std::fs::write(
            &path,
            jsonl(&[
                record("user", "first request", &project_root),
                record("assistant", "first answer", &project_root),
            ]),
        )
        .expect("session");

        let imported = load_session_for_import(&path)
            .expect("load")
            .expect("session");
        let turns = build_turns_from_rollout_items(&imported.rollout_items);

        assert_eq!(turns.len(), 1);
        assert_eq!(
            turns[0].items.last(),
            Some(&ThreadItem::AgentMessage {
                id: "item-3".into(),
                text: EXTERNAL_SESSION_IMPORTED_MARKER.into(),
                phase: None,
                memory_citation: None,
            })
        );
        let last_turn_complete = imported
            .rollout_items
            .iter()
            .rev()
            .find_map(|item| match item {
                RolloutItem::EventMsg(EventMsg::TurnComplete(event)) => Some(event),
                _ => None,
            });
        assert_eq!(
            last_turn_complete.and_then(|event| event.last_agent_message.as_deref()),
            None
        );
    }

    #[test]
    fn stores_imported_messages_as_response_items_and_visible_events() {
        let root = TempDir::new().expect("tempdir");
        let project_root = root.path().join("repo");
        std::fs::create_dir_all(&project_root).expect("project root");
        let path = root.path().join("session.jsonl");
        let request = "r".repeat(1_000);
        let answer = "a".repeat(1_000);
        std::fs::write(
            &path,
            jsonl(&[
                record("user", &request, &project_root),
                record("assistant", &answer, &project_root),
            ]),
        )
        .expect("session");

        let imported = load_session_for_import(&path)
            .expect("load")
            .expect("session");
        let response_message_count = imported
            .rollout_items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    RolloutItem::ResponseItem(ResponseItem::Message { .. })
                )
            })
            .count();
        let visible_message_event_count = imported
            .rollout_items
            .iter()
            .filter(|item| match item {
                RolloutItem::EventMsg(EventMsg::UserMessage(event)) => event.message == request,
                RolloutItem::EventMsg(EventMsg::AgentMessage(event)) => event.message == answer,
                _ => false,
            })
            .count();

        assert_eq!(response_message_count, 2);
        assert_eq!(visible_message_event_count, 2);
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
    fn loads_ai_title_for_imported_session() {
        let root = TempDir::new().expect("tempdir");
        let project_root = root.path().join("repo");
        std::fs::create_dir_all(&project_root).expect("project root");
        let path = root.path().join("session.jsonl");
        std::fs::write(
            &path,
            jsonl(&[
                record("user", "first request", &project_root),
                ai_title_record("generated by source app"),
            ]),
        )
        .expect("session");

        let imported = load_session_for_import(&path)
            .expect("load")
            .expect("session");

        assert_eq!(imported.title.as_deref(), Some("generated by source app"));
    }

    #[test]
    fn loads_custom_title_over_later_ai_title_for_imported_session() {
        let root = TempDir::new().expect("tempdir");
        let project_root = root.path().join("repo");
        std::fs::create_dir_all(&project_root).expect("project root");
        let path = root.path().join("session.jsonl");
        std::fs::write(
            &path,
            jsonl(&[
                record("user", "first request", &project_root),
                custom_title_record("named by source app"),
                ai_title_record("generated by source app"),
            ]),
        )
        .expect("session");

        let imported = load_session_for_import(&path)
            .expect("load")
            .expect("session");

        assert_eq!(imported.title.as_deref(), Some("named by source app"));
    }

    #[test]
    fn sanitizes_only_the_imported_session_fallback_title() {
        let root = TempDir::new().expect("tempdir");
        let project_root = root.path().join("repo");
        std::fs::create_dir_all(&project_root).expect("project root");
        let path = root.path().join("session.jsonl");
        let message = "<system-reminder>\ncontrol context\n</system-reminder>\nFix auth flow";
        std::fs::write(&path, jsonl(&[record("user", message, &project_root)])).expect("session");

        let imported = load_session_for_import(&path)
            .expect("load")
            .expect("session");
        let imported_user_message = imported.rollout_items.iter().find_map(|item| match item {
            RolloutItem::EventMsg(EventMsg::UserMessage(event)) => Some(event.message.as_str()),
            _ => None,
        });

        assert_eq!(imported.title.as_deref(), Some("Fix auth flow"));
        assert_eq!(
            imported.first_user_message.as_deref(),
            Some("<system-reminder>")
        );
        assert_eq!(imported_user_message, Some(message));
    }

    #[test]
    fn skips_control_only_user_messages_when_choosing_fallback_title() {
        let root = TempDir::new().expect("tempdir");
        let project_root = root.path().join("repo");
        std::fs::create_dir_all(&project_root).expect("project root");
        let path = root.path().join("session.jsonl");
        let control_message = "<ide_selection>src/auth.rs:1-5</ide_selection>";
        std::fs::write(
            &path,
            jsonl(&[
                record("user", control_message, &project_root),
                record("user", "Fix auth flow", &project_root),
            ]),
        )
        .expect("session");

        let imported = load_session_for_import(&path)
            .expect("load")
            .expect("session");

        assert_eq!(imported.title.as_deref(), Some("Fix auth flow"));
        assert_eq!(
            imported.first_user_message.as_deref(),
            Some(control_message)
        );
    }

    #[test]
    fn uses_safe_fallback_after_all_user_messages_are_control_only() {
        let root = TempDir::new().expect("tempdir");
        let project_root = root.path().join("repo");
        std::fs::create_dir_all(&project_root).expect("project root");
        let path = root.path().join("session.jsonl");
        std::fs::write(
            &path,
            jsonl(&[
                record(
                    "user",
                    "<ide_selection>src/auth.rs:1-5</ide_selection>",
                    &project_root,
                ),
                record(
                    "user",
                    "<local-command-stderr>tests failed</local-command-stderr>",
                    &project_root,
                ),
            ]),
        )
        .expect("session");

        let imported = load_session_for_import(&path)
            .expect("load")
            .expect("session");

        assert_eq!(
            imported.title.as_deref(),
            Some(IMPORTED_SESSION_FALLBACK_TITLE)
        );
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

    fn ai_title_record(title: &str) -> JsonValue {
        serde_json::json!({
            "type": "ai-title",
            "aiTitle": title,
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

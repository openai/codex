use crate::model::ThreadMetadata;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::is_local_image_close_tag_text;
use codex_protocol::models::is_local_image_open_tag_text;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::TurnContextItem;
use codex_protocol::protocol::USER_MESSAGE_BEGIN;
use serde::Serialize;
use serde_json::Value;

/// Apply a rollout item to the metadata structure.
pub fn apply_rollout_item(
    metadata: &mut ThreadMetadata,
    item: &RolloutItem,
    default_provider: &str,
) {
    match item {
        RolloutItem::SessionMeta(meta_line) => apply_session_meta_from_item(metadata, meta_line),
        RolloutItem::TurnContext(turn_ctx) => apply_turn_context(metadata, turn_ctx),
        RolloutItem::EventMsg(event) => apply_event_msg(metadata, event),
        RolloutItem::ResponseItem(item) => apply_response_item(metadata, item),
        RolloutItem::Compacted(_) => {}
    }
    if metadata.model_provider.is_empty() {
        metadata.model_provider = default_provider.to_string();
    }
}

fn apply_session_meta_from_item(metadata: &mut ThreadMetadata, meta_line: &SessionMetaLine) {
    if metadata.id != meta_line.meta.id {
        // Ignore session_meta lines that don't match the canonical thread ID,
        // e.g., forked rollouts that embed the source session metadata.
        return;
    }
    metadata.id = meta_line.meta.id;
    metadata.source = enum_to_string(&meta_line.meta.source);
    if let Some(provider) = meta_line.meta.model_provider.as_deref() {
        metadata.model_provider = provider.to_string();
    }
    if !meta_line.meta.cwd.as_os_str().is_empty() {
        metadata.cwd = meta_line.meta.cwd.clone();
    }
    if let Some(git) = meta_line.git.as_ref() {
        metadata.git_sha = git.commit_hash.clone();
        metadata.git_branch = git.branch.clone();
        metadata.git_origin_url = git.repository_url.clone();
    }
}

fn apply_turn_context(metadata: &mut ThreadMetadata, turn_ctx: &TurnContextItem) {
    metadata.cwd = turn_ctx.cwd.clone();
    metadata.sandbox_policy = enum_to_string(&turn_ctx.sandbox_policy);
    metadata.approval_mode = enum_to_string(&turn_ctx.approval_policy);
}

fn apply_event_msg(metadata: &mut ThreadMetadata, event: &EventMsg) {
    match event {
        EventMsg::TokenCount(token_count) => {
            if let Some(info) = token_count.info.as_ref() {
                metadata.tokens_used = info.total_token_usage.total_tokens.max(0);
            }
        }
        EventMsg::UserMessage(user) => {
            metadata.has_user_event = true;
            if metadata.title.is_empty() {
                metadata.title = strip_user_message_prefix(user.message.as_str()).to_string();
            }
        }
        _ => {}
    }
}

fn apply_response_item(metadata: &mut ThreadMetadata, item: &ResponseItem) {
    if let Some(text) = extract_user_message_text(item) {
        metadata.has_user_event = true;
        if metadata.title.is_empty() {
            metadata.title = text;
        }
    }
}

fn extract_user_message_text(item: &ResponseItem) -> Option<String> {
    let ResponseItem::Message { role, content, .. } = item else {
        return None;
    };
    if role != "user" {
        return None;
    }
    let texts: Vec<&str> = content
        .iter()
        .filter_map(|content_item| match content_item {
            ContentItem::InputText { text } => Some(text.as_str()),
            ContentItem::InputImage { .. } | ContentItem::OutputText { .. } => None,
        })
        .filter(|text| !is_local_image_open_tag_text(text) && !is_local_image_close_tag_text(text))
        .collect();
    if texts.is_empty() {
        return None;
    }
    let joined = texts.join("\n");
    Some(
        strip_user_message_prefix(joined.as_str())
            .trim()
            .to_string(),
    )
}

fn strip_user_message_prefix(text: &str) -> &str {
    match text.find(USER_MESSAGE_BEGIN) {
        Some(idx) => text[idx + USER_MESSAGE_BEGIN.len()..].trim(),
        None => text.trim(),
    }
}

pub(crate) fn enum_to_string<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::extract_user_message_text;
    use crate::model::ThreadMetadata;
    use chrono::DateTime;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use codex_protocol::protocol::USER_MESSAGE_BEGIN;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn extracts_user_message_text() {
        let item = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputText {
                    text: format!("<prior context> {USER_MESSAGE_BEGIN}actual question"),
                },
                ContentItem::InputImage {
                    image_url: "https://example.com/image.png".to_string(),
                },
            ],
            end_turn: None,
        };
        let actual = extract_user_message_text(&item);
        assert_eq!(actual.as_deref(), Some("actual question"));
    }

    #[test]
    fn diff_fields_detects_changes() {
        let id = ThreadId::from_string(&Uuid::now_v7().to_string()).expect("thread id");
        let created_at = DateTime::<Utc>::from_timestamp(1_735_689_600, 0).expect("timestamp");
        let base = ThreadMetadata {
            id,
            rollout_path: PathBuf::from("/tmp/a.jsonl"),
            created_at,
            updated_at: created_at,
            source: "cli".to_string(),
            model_provider: "openai".to_string(),
            cwd: PathBuf::from("/tmp"),
            title: "hello".to_string(),
            sandbox_policy: "read-only".to_string(),
            approval_mode: "on-request".to_string(),
            tokens_used: 1,
            has_user_event: false,
            archived_at: None,
            git_sha: None,
            git_branch: None,
            git_origin_url: None,
        };
        let mut other = base.clone();
        other.tokens_used = 2;
        other.title = "world".to_string();
        let diffs = base.diff_fields(&other);
        assert_eq!(diffs, vec!["title", "tokens_used"]);
    }
}

use crate::model::ThreadMetadata;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::TurnContextItem;
use codex_protocol::protocol::USER_MESSAGE_BEGIN;
use codex_protocol::protocol::UserMessageEvent;
use serde::Serialize;
use serde_json::Value;

const IMAGE_ONLY_USER_MESSAGE_PLACEHOLDER: &str = "[Image]";

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
    metadata.agent_nickname = meta_line.meta.agent_nickname.clone();
    metadata.agent_role = meta_line.meta.agent_role.clone();
    if let Some(provider) = meta_line.meta.model_provider.as_deref() {
        metadata.model_provider = provider.to_string();
    }
    if !meta_line.meta.cli_version.is_empty() {
        metadata.cli_version = meta_line.meta.cli_version.clone();
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
            if metadata.first_user_message.is_none() {
                metadata.first_user_message = user_message_preview(user);
            }
            if metadata.title.is_empty() {
                let title = strip_user_message_prefix(user.message.as_str());
                if !title.is_empty() {
                    metadata.title = title.to_string();
                }
            }
        }
        _ => {}
    }
}

fn apply_response_item(_metadata: &mut ThreadMetadata, _item: &ResponseItem) {
    // Title and first_user_message are derived from EventMsg::UserMessage only.
}

fn strip_user_message_prefix(text: &str) -> &str {
    match text.find(USER_MESSAGE_BEGIN) {
        Some(idx) => text[idx + USER_MESSAGE_BEGIN.len()..].trim(),
        None => text.trim(),
    }
}

fn user_message_preview(user: &UserMessageEvent) -> Option<String> {
    let message = strip_user_message_prefix(user.message.as_str());
    if !message.is_empty() {
        return Some(message.to_string());
    }
    if user
        .images
        .as_ref()
        .is_some_and(|images| !images.is_empty())
        || !user.local_images.is_empty()
    {
        return Some(IMAGE_ONLY_USER_MESSAGE_PLACEHOLDER.to_string());
    }
    None
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
    use super::apply_rollout_item;
    use crate::model::ThreadMetadata;
    use chrono::DateTime;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::RolloutItem;
    use codex_protocol::protocol::USER_MESSAGE_BEGIN;
    use codex_protocol::protocol::UserMessageEvent;

    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn response_item_user_messages_do_not_set_title_or_first_user_message() {
        let mut metadata = metadata_for_test();
        let item = RolloutItem::ResponseItem(ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "hello from response item".to_string(),
            }],
            end_turn: None,
            phase: None,
        });

        apply_rollout_item(&mut metadata, &item, "test-provider");

        assert_eq!(metadata.first_user_message, None);
        assert_eq!(metadata.title, "");
    }

    #[test]
    fn event_msg_user_messages_set_title_and_first_user_message() {
        let mut metadata = metadata_for_test();
        let item = RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            message: format!("{USER_MESSAGE_BEGIN} actual user request"),
            images: Some(vec![]),
            local_images: vec![],
            text_elements: vec![],
        }));

        apply_rollout_item(&mut metadata, &item, "test-provider");

        assert_eq!(
            metadata.first_user_message.as_deref(),
            Some("actual user request")
        );
        assert_eq!(metadata.title, "actual user request");
    }

    #[test]
    fn event_msg_image_only_user_message_sets_image_placeholder_preview() {
        let mut metadata = metadata_for_test();
        let item = RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            message: String::new(),
            images: Some(vec!["https://example.com/image.png".to_string()]),
            local_images: vec![],
            text_elements: vec![],
        }));

        apply_rollout_item(&mut metadata, &item, "test-provider");

        assert_eq!(
            metadata.first_user_message.as_deref(),
            Some(super::IMAGE_ONLY_USER_MESSAGE_PLACEHOLDER)
        );
        assert_eq!(metadata.title, "");
    }

    #[test]
    fn event_msg_blank_user_message_without_images_keeps_first_user_message_empty() {
        let mut metadata = metadata_for_test();
        let item = RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            message: "   ".to_string(),
            images: Some(vec![]),
            local_images: vec![],
            text_elements: vec![],
        }));

        apply_rollout_item(&mut metadata, &item, "test-provider");

        assert_eq!(metadata.first_user_message, None);
        assert_eq!(metadata.title, "");
    }

    fn metadata_for_test() -> ThreadMetadata {
        let id = ThreadId::from_string(&Uuid::from_u128(42).to_string()).expect("thread id");
        let created_at = DateTime::<Utc>::from_timestamp(1_735_689_600, 0).expect("timestamp");
        ThreadMetadata {
            id,
            rollout_path: PathBuf::from("/tmp/a.jsonl"),
            created_at,
            updated_at: created_at,
            source: "cli".to_string(),
            agent_nickname: None,
            agent_role: None,
            model_provider: "openai".to_string(),
            cwd: PathBuf::from("/tmp"),
            cli_version: "0.0.0".to_string(),
            title: String::new(),
            sandbox_policy: "read-only".to_string(),
            approval_mode: "on-request".to_string(),
            tokens_used: 1,
            first_user_message: None,
            archived_at: None,
            git_sha: None,
            git_branch: None,
            git_origin_url: None,
        }
    }

    #[test]
    fn diff_fields_detects_changes() {
        let mut base = metadata_for_test();
        base.id = ThreadId::from_string(&Uuid::now_v7().to_string()).expect("thread id");
        base.title = "hello".to_string();
        let mut other = base.clone();
        other.tokens_used = 2;
        other.title = "world".to_string();
        let diffs = base.diff_fields(&other);
        assert_eq!(diffs, vec!["title", "tokens_used"]);
    }
}

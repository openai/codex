use super::super::ConversationMessage;
use super::super::MessageRole;
use super::super::export::EXTERNAL_SESSION_IMPORTED_MARKER;
use super::super::export::rollout_items_from_messages;
use super::*;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ContextCompactedEvent;
use codex_protocol::protocol::ThreadRolledBackEvent;
use pretty_assertions::assert_eq;

#[test]
fn returns_the_missing_suffix_from_its_visible_boundary() {
    let history = rollout(&[(MessageRole::User, "first request")]);
    let source = rollout(&[
        (MessageRole::User, "first request"),
        (MessageRole::Assistant, "late answer"),
        (MessageRole::User, "follow-up request"),
    ]);

    let suffix = plan_append(&source, &history).expect("exact prefix should append");

    assert!(matches!(
        suffix.first(),
        Some(RolloutItem::EventMsg(EventMsg::AgentMessage(event)))
            if event.message == "late answer"
    ));
    assert_eq!(
        model_messages(&suffix),
        vec![
            (MessageRole::Assistant, "late answer"),
            (MessageRole::User, "follow-up request"),
        ]
    );
    assert!(!suffix.iter().any(|item| matches!(
        item,
        RolloutItem::EventMsg(EventMsg::AgentMessage(event))
            if event.message == EXTERNAL_SESSION_IMPORTED_MARKER
    )));
}

#[test]
fn requires_a_strict_nonempty_model_prefix() {
    let history = rollout(&[(MessageRole::User, "first request")]);
    let source = rollout(&[
        (MessageRole::User, "first request"),
        (MessageRole::User, "follow-up request"),
    ]);
    assert!(plan_append(&history, &history).is_none());
    assert!(
        plan_append(
            &source,
            &rollout(&[(MessageRole::User, "rewritten request")])
        )
        .is_none()
    );

    let mut metadata_changed = history.clone();
    for item in &mut metadata_changed {
        if let RolloutItem::EventMsg(EventMsg::TurnStarted(event)) = item {
            event.turn_id = "different-turn-id".to_string();
            event.started_at = Some(9_999);
        }
    }
    assert!(model_transcripts_match(&history, &metadata_changed));
    assert!(!model_transcripts_match(&source, &history));
    assert!(plan_append(&source, &metadata_changed).is_some());

    for event in [
        EventMsg::ContextCompacted(ContextCompactedEvent),
        EventMsg::ThreadRolledBack(ThreadRolledBackEvent { num_turns: 1 }),
    ] {
        let mut rewritten = history.clone();
        rewritten.push(RolloutItem::EventMsg(event));
        assert!(plan_append(&source, &rewritten).is_none());
    }
    let mut with_tool_call = history;
    with_tool_call.push(RolloutItem::ResponseItem(
        ResponseItem::FunctionCall {
            id: None,
            name: "native_tool".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            encrypted_function_args: None,
            call_id: "native-call".to_string(),
            internal_chat_message_metadata_passthrough: None,
        }
        .into(),
    ));
    assert!(plan_append(&source, &with_tool_call).is_none());
}

fn rollout(messages: &[(MessageRole, &str)]) -> Vec<RolloutItem> {
    rollout_items_from_messages(
        messages
            .iter()
            .enumerate()
            .map(|(index, &(role, text))| ConversationMessage {
                role,
                text: text.to_string(),
                timestamp: Some(index as i64),
            })
            .collect(),
    )
}

fn model_messages(items: &[RolloutItem]) -> Vec<(MessageRole, &str)> {
    items
        .iter()
        .filter_map(|item| match item {
            RolloutItem::ResponseItem(response_item) => {
                let ResponseItem::Message { role, content, .. } = &response_item.item else {
                    return None;
                };
                match (role.as_str(), content.as_slice()) {
                    ("user", [ContentItem::InputText { text }]) => {
                        Some((MessageRole::User, text.as_str()))
                    }
                    ("assistant", [ContentItem::OutputText { text }]) => {
                        Some((MessageRole::Assistant, text.as_str()))
                    }
                    _ => None,
                }
            }
            _ => None,
        })
        .collect()
}

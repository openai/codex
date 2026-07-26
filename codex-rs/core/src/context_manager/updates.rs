use crate::context::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;

#[derive(Clone, Copy, PartialEq, Eq)]
enum MessageGroup {
    Standalone,
    Mergeable,
}

pub(crate) fn build_developer_update_item(text_sections: Vec<String>) -> Option<ResponseItem> {
    build_text_message("developer", text_sections)
}

pub(crate) fn build_contextual_user_message(text_sections: Vec<String>) -> Option<ResponseItem> {
    build_text_message("user", text_sections)
}

pub(crate) fn merge_contextual_fragments(
    fragments: Vec<Box<dyn ContextualUserFragment>>,
) -> Vec<ResponseItem> {
    let mut messages: Vec<(&str, MessageGroup, Vec<String>)> = Vec::with_capacity(fragments.len());
    for fragment in fragments {
        let role = fragment.role();
        let group = if fragment.requires_separate_message() {
            MessageGroup::Standalone
        } else {
            MessageGroup::Mergeable
        };
        let text = fragment.render();
        match messages.last_mut() {
            Some((previous_role, previous_group, text_sections))
                if *previous_role == role
                    && *previous_group == MessageGroup::Mergeable
                    && group == MessageGroup::Mergeable =>
            {
                text_sections.push(text);
            }
            _ => messages.push((role, group, vec![text])),
        }
    }
    messages
        .into_iter()
        .filter_map(|(role, _, text_sections)| build_text_message(role, text_sections))
        .collect()
}

fn build_text_message(role: &str, text_sections: Vec<String>) -> Option<ResponseItem> {
    if text_sections.is_empty() {
        return None;
    }

    let content = text_sections
        .into_iter()
        .map(|text| ContentItem::InputText { text })
        .collect();

    Some(ResponseItem::Message {
        id: None,
        role: role.to_string(),
        content,
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    })
}

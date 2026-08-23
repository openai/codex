use crate::context::ContextualUserFragment;
use codex_context_fragments::RenderedFragment;
use codex_protocol::models::InternalChatMessageMetadataPassthrough;
use codex_protocol::models::ResponseItem;

#[derive(Clone, Copy, PartialEq, Eq)]
enum MessageGroup {
    Standalone,
    Mergeable,
}

pub(crate) fn build_rendered_message(fragments: Vec<RenderedFragment>) -> Option<ResponseItem> {
    let role = fragments.first()?.role();
    debug_assert!(fragments.iter().all(|fragment| fragment.role() == role));
    let (content, content_item_kinds): (Vec<_>, Vec<_>) = fragments
        .into_iter()
        .map(|fragment| fragment.into_parts().1.into_parts())
        .unzip();

    Some(ResponseItem::Message {
        id: None,
        role: role.to_string(),
        content,
        phase: None,
        internal_chat_message_metadata_passthrough: Some(InternalChatMessageMetadataPassthrough {
            content_item_kinds: Some(content_item_kinds),
            ..Default::default()
        }),
    })
}

pub(crate) fn merge_contextual_fragments(
    fragments: Vec<Box<dyn ContextualUserFragment>>,
) -> Vec<ResponseItem> {
    let mut messages: Vec<(&str, MessageGroup, Vec<RenderedFragment>)> =
        Vec::with_capacity(fragments.len());
    for fragment in fragments {
        let group = if fragment.requires_separate_message() {
            MessageGroup::Standalone
        } else {
            MessageGroup::Mergeable
        };
        let rendered = fragment.render_fragment();
        let role = rendered.role();
        match messages.last_mut() {
            Some((previous_role, previous_group, rendered_fragments))
                if *previous_role == role
                    && *previous_group == MessageGroup::Mergeable
                    && group == MessageGroup::Mergeable =>
            {
                rendered_fragments.push(rendered);
            }
            _ => messages.push((role, group, vec![rendered])),
        }
    }
    messages
        .into_iter()
        .filter_map(|(_, _, fragments)| build_rendered_message(fragments))
        .collect()
}

//! Groups contextual fragments without moving them across explicit response items.

use crate::context::world_state::Placement;
use crate::context::world_state::WorldStateUpdate;
use crate::context::world_state::WorldStateUpdateContent;
use codex_context_fragments::RenderedFragment;
use codex_context_fragments::message_from_parts;
use codex_protocol::models::ResponseItem;

pub(crate) fn build_rendered_message(fragments: Vec<RenderedFragment>) -> Option<ResponseItem> {
    let role = fragments.first()?.role();
    debug_assert!(fragments.iter().all(|fragment| fragment.role() == role));
    let content = fragments
        .into_iter()
        .map(|fragment| fragment.into_parts().1)
        .collect();

    Some(message_from_parts(role, content))
}

pub(crate) fn merge_world_state_updates(updates: Vec<WorldStateUpdate>) -> Vec<ResponseItem> {
    let mut items = Vec::new();
    let mut pending = Vec::<RenderedFragment>::new();
    for update in updates {
        let fragment = match update.content {
            WorldStateUpdateContent::Fragment(fragment) => fragment,
            WorldStateUpdateContent::Item(item) => {
                items.extend(build_rendered_message(std::mem::take(&mut pending)));
                items.push(*item);
                continue;
            }
        };
        let rendered = fragment.render_fragment();
        if update.placement != Placement::Mergeable {
            items.extend(build_rendered_message(std::mem::take(&mut pending)));
            items.extend(build_rendered_message(vec![rendered]));
            continue;
        }
        if pending
            .first()
            .is_some_and(|previous| previous.role() != rendered.role())
        {
            items.extend(build_rendered_message(std::mem::take(&mut pending)));
        }
        pending.push(rendered);
    }
    items.extend(build_rendered_message(pending));
    items
}

#[cfg(test)]
#[path = "updates_tests.rs"]
mod tests;

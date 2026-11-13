//! Identifies standalone retained tool outputs in model history.

use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ResponseItem;

pub(crate) fn is_retained_tool_output(envelope: &ResponseItemEnvelope) -> bool {
    envelope
        .metadata
        .as_ref()
        .is_some_and(|m| m.retain_across_windows.unwrap_or(m.client_authored))
        && matches!(
            envelope.item,
            ResponseItem::FunctionCallOutput {
                call_id: None,
                name: Some(_),
                ..
            }
        )
}

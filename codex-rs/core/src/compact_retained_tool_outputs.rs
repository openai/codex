//! Retains explicitly marked, named function outputs that have no matching call.
//! Outputs share the normal history token budget and are retained whole or dropped.

use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ResponseItem;

pub(crate) fn is_retained_tool_output(envelope: &ResponseItemEnvelope) -> bool {
    envelope
        .metadata
        .as_ref()
        .is_some_and(|metadata| metadata.client_authored)
        && matches!(
            envelope.item,
            ResponseItem::FunctionCallOutput {
                call_id: None,
                name: Some(_),
                ..
            }
        )
}

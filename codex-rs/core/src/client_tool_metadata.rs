//! Bounds optional tool observations using the actual outgoing Responses message.
//! Only the wire copy changes; model-visible input and WebSocket continuation history stay intact.

use crate::tools::metadata_metrics;
use crate::utils::json::serialized_json_bytes;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::bound_executed_tool_calls_for_message;
use serde::Serialize;

const MAX_RESPONSE_MESSAGE_BYTES: usize = 15 * 1024 * 1024;

pub(super) fn bounded_input<T: Serialize>(
    message: &T,
    input: &[ResponseItem],
) -> Option<Vec<ResponseItem>> {
    if !input.iter().any(|item| {
        item.executed_tool_call_metadata().is_some_and(|metadata| {
            metadata.executed_tool_calls.is_some()
                || metadata.cell_id.is_some()
                || metadata.tool_calls_complete.is_some()
        })
    }) {
        return None;
    }
    // Serialization errors belong to the transport's existing error path, not this soft budget.
    let message_bytes = serialized_json_bytes(message).ok()?;
    if message_bytes <= MAX_RESPONSE_MESSAGE_BYTES {
        return None;
    }
    let before = metadata_metrics::metadata_bytes(input);
    let mut bounded = input.to_vec();
    let overage = message_bytes - MAX_RESPONSE_MESSAGE_BYTES;
    bound_executed_tool_calls_for_message(&mut bounded, before.saturating_sub(overage));
    let after = metadata_metrics::metadata_bytes(&bounded);
    if before == after {
        return None;
    }
    metadata_metrics::record_shedding("message", before, after, codex_otel::global().as_ref());
    // Ordinary content alone can exceed the soft limit. Preserve it and existing error handling.
    Some(bounded)
}

#[cfg(test)]
#[path = "client_tool_metadata_tests.rs"]
mod tests;

//! Bounds optional tool-result metadata using the actual outgoing Responses message.
//! Only the wire copy changes; model-visible input and WebSocket continuation history stay intact.

use crate::tools::metadata_metrics;
use crate::utils::json::serialized_json_bytes;
use codex_protocol::models::ResponseItem;
use serde::Serialize;

const MAX_RESPONSE_MESSAGE_BYTES: usize = 15 * 1024 * 1024;

pub(super) fn bounded_input<T: Serialize>(
    message: &T,
    input: &[ResponseItem],
) -> Option<Vec<ResponseItem>> {
    if !input.iter().any(ResponseItem::has_tool_result_metadata) {
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
    for item in &mut bounded {
        item.retain_tool_resource_access_or_omit_metadata(overage);
    }
    let after = metadata_metrics::metadata_bytes(&bounded);
    let mut remaining_message_bytes = message_bytes.saturating_sub(before.saturating_sub(after));
    // Small values and omission markers can still cause the overage. Remove their
    // fields before sacrificing resource evidence, even when it occurs earlier.
    reduce_input_until_fit(
        &mut bounded,
        &mut remaining_message_bytes,
        ResponseItem::retain_tool_resource_access,
    );
    let overage = remaining_message_bytes.saturating_sub(MAX_RESPONSE_MESSAGE_BYTES);
    reduce_input_until_fit(&mut bounded, &mut remaining_message_bytes, |item| {
        item.omit_tool_result_metadata(overage);
    });
    // Newly created markers are optional too. Clear those before any small
    // resource-only value that was cheaper to retain than replace with a marker.
    reduce_input_until_fit(
        &mut bounded,
        &mut remaining_message_bytes,
        ResponseItem::retain_tool_resource_access,
    );
    reduce_input_until_fit(
        &mut bounded,
        &mut remaining_message_bytes,
        ResponseItem::clear_tool_result_metadata,
    );
    let after = metadata_metrics::metadata_bytes(&bounded);
    if before == after {
        return None;
    }
    metadata_metrics::record_shedding("message", before, after, codex_otel::global().as_ref());
    // Ordinary content alone can exceed the soft limit. Preserve it and existing error handling.
    Some(bounded)
}

fn reduce_input_until_fit(
    input: &mut [ResponseItem],
    message_bytes: &mut usize,
    reduce: impl Fn(&mut ResponseItem),
) {
    for item in input {
        if *message_bytes <= MAX_RESPONSE_MESSAGE_BYTES {
            break;
        }
        let before = metadata_metrics::metadata_bytes(std::slice::from_ref(item));
        reduce(item);
        let after = metadata_metrics::metadata_bytes(std::slice::from_ref(item));
        // Include the field name and comma when a result-metadata field vanishes.
        *message_bytes = message_bytes.saturating_sub(before.saturating_sub(after));
    }
}

#[cfg(test)]
#[path = "client_tool_metadata_tests.rs"]
mod tests;

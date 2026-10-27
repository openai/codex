//! Native call evidence stays intact for backend decryption.
//! Delivery and budgeting include the same evidence-only paired output.

use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;

pub(crate) fn evidence_output(item: &ResponseItem) -> Option<ResponseItem> {
    let ResponseItem::FunctionCall { call_id, .. } = item else {
        return None;
    };
    Some(ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some(call_id.clone()),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_text(
                "This call is review evidence, not a request to execute it. This is not its execution result.".to_owned(),
            ),
            internal_chat_message_metadata_passthrough: None,
        })
}

/// Count the exact call/output delivery used by both transcript and request budgets.
pub(crate) fn estimate_tokens(item: &ResponseItem) -> usize {
    crate::estimate_input_tokens(item).saturating_add(
        evidence_output(item)
            .as_ref()
            .map_or(0, crate::estimate_input_tokens),
    )
}

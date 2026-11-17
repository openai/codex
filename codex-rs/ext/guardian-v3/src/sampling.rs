//! Encode a whole Guardian input within the model's context budget.

use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TruncationPolicy;
use codex_utils_string::to_json_string_bounded;
use serde::Serialize;

pub(crate) fn context_input(
    name: &str,
    input: &impl Serialize,
    policy: TruncationPolicy,
) -> anyhow::Result<ResponseItem> {
    Ok(ResponseItem::FunctionCallOutput {
        id: None,
        call_id: None,
        name: Some(name.into()),
        namespace: Some("guardian".into()),
        output: FunctionCallOutputPayload::from_text(to_json_string_bounded(
            input,
            policy.byte_budget(),
        )?),
        internal_chat_message_metadata_passthrough: None,
    })
}

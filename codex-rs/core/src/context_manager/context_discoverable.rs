use crate::truncate::TruncationPolicy;
use crate::truncate::truncate_function_output_items_with_policy;
use crate::truncate::truncate_text;
use codex_protocol::models::CustomToolCallOutput;
use codex_protocol::models::FunctionCallOutput;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;

pub(super) trait ContextDiscoverable {
    fn discoverable_history_item(&self, policy: TruncationPolicy) -> ResponseItem;
}

impl ContextDiscoverable for FunctionCallOutput {
    fn discoverable_history_item(&self, policy: TruncationPolicy) -> ResponseItem {
        let body = match &self.output.body {
            FunctionCallOutputBody::Text(content) => {
                FunctionCallOutputBody::Text(truncate_text(content, policy))
            }
            FunctionCallOutputBody::ContentItems(items) => FunctionCallOutputBody::ContentItems(
                truncate_function_output_items_with_policy(items, policy),
            ),
        };

        ResponseItem::FunctionCallOutput(FunctionCallOutput {
            call_id: self.call_id.clone(),
            output: FunctionCallOutputPayload {
                body,
                success: self.output.success,
            },
        })
    }
}

impl ContextDiscoverable for CustomToolCallOutput {
    fn discoverable_history_item(&self, policy: TruncationPolicy) -> ResponseItem {
        ResponseItem::CustomToolCallOutput(CustomToolCallOutput {
            call_id: self.call_id.clone(),
            output: truncate_text(&self.output, policy),
        })
    }
}

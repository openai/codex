use codex_extension_api::ToolOutput;
use codex_extension_api::ToolPayload;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use serde_json::Value;

pub(crate) struct SearchOutput {
    output: String,
}

impl SearchOutput {
    pub(crate) fn new(output: String) -> Self {
        Self { output }
    }
}

impl ToolOutput for SearchOutput {
    fn log_preview(&self) -> String {
        "[standalone web search output]".to_string()
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        // TODO: Make standalone search honor memories.disable_on_external_context,
        // as hosted web search does.
        ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputText {
                    text: self.output.clone(),
                },
            ]),
        }
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> Value {
        Value::String(self.output.clone())
    }
}

#[cfg(test)]
mod tests {
    use codex_extension_api::ToolPayload;
    use codex_protocol::models::FunctionCallOutputContentItem;
    use codex_protocol::models::FunctionCallOutputPayload;
    use codex_protocol::models::ResponseInputItem;
    use pretty_assertions::assert_eq;

    use super::SearchOutput;
    use super::ToolOutput;

    #[test]
    fn emits_plaintext_function_call_output() {
        let output = SearchOutput::new("search output".to_string());

        assert_eq!(
            output.to_response_item(
                "call-1",
                &ToolPayload::Function {
                    arguments: "{}".to_string(),
                },
            ),
            ResponseInputItem::FunctionCallOutput {
                call_id: "call-1".to_string(),
                output: FunctionCallOutputPayload::from_content_items(vec![
                    FunctionCallOutputContentItem::InputText {
                        text: "search output".to_string(),
                    },
                ]),
            }
        );
    }
}

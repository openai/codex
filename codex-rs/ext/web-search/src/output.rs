use codex_extension_api::ToolOutput;
use codex_extension_api::ToolPayload;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;

pub(crate) struct EncryptedSearchOutput {
    encrypted_output: String,
}

impl EncryptedSearchOutput {
    pub(crate) fn new(encrypted_output: String) -> Self {
        Self { encrypted_output }
    }
}

impl ToolOutput for EncryptedSearchOutput {
    fn log_preview(&self) -> String {
        "[encrypted standalone web search output]".to_string()
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::EncryptedContent {
                    encrypted_content: self.encrypted_output.clone(),
                },
            ]),
        }
    }
}

#[cfg(test)]
mod tests {
    use codex_extension_api::ToolPayload;
    use codex_protocol::models::FunctionCallOutputContentItem;
    use codex_protocol::models::FunctionCallOutputPayload;
    use codex_protocol::models::ResponseInputItem;
    use pretty_assertions::assert_eq;

    use super::EncryptedSearchOutput;
    use super::ToolOutput;

    #[test]
    fn emits_encrypted_function_call_output() {
        let output = EncryptedSearchOutput::new("encrypted-search-output".to_string());

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
                    FunctionCallOutputContentItem::EncryptedContent {
                        encrypted_content: "encrypted-search-output".to_string(),
                    },
                ]),
            }
        );
    }
}

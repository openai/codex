use codex_extension_api::ToolOutput;
use codex_extension_api::ToolPayload;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;

pub(crate) struct SearchOutput {
    payload: FunctionCallOutputPayload,
}

impl SearchOutput {
    pub(crate) fn new(result: Result<String, String>) -> Self {
        let payload = match result {
            Ok(text) => FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputText { text },
            ]),
            Err(error) => FunctionCallOutputPayload {
                success: Some(false),
                ..FunctionCallOutputPayload::from_text(error)
            },
        };
        Self { payload }
    }
}

impl ToolOutput for SearchOutput {
    fn log_output(&self) -> String {
        "[standalone web search output]".to_string()
    }

    fn success_for_logging(&self) -> bool {
        self.payload.success.unwrap_or(true)
    }

    fn contains_external_context(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output: self.payload.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::SearchOutput;
    use super::ToolOutput;

    #[test]
    fn reports_search_status_for_logging_and_downstream_handling() {
        for (result, success) in [
            (Ok("found".into()), None),
            (Err("failed".into()), Some(false)),
        ] {
            let output = SearchOutput::new(result);
            assert_eq!(output.payload.success, success);
            assert_eq!(output.success_for_logging(), success.unwrap_or(true));
        }
    }
}

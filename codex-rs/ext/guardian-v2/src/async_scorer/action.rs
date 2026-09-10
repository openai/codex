//! Renders complete planned-action JSON or rejects it for synchronous review.
//! Action arguments cannot be shortened to fit the asynchronous classifier budget.

use codex_extension_api::ToolName;
use codex_extension_api::ToolPayload;
use codex_protocol::protocol::TruncationPolicy;
use serde_json::json;

pub(super) struct GuardianAction {
    pub(super) tool_name: ToolName,
    pub(super) payload: ToolPayload,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum ActionRenderError {
    #[error("Guardian action exceeds the {max_tokens}-token review limit")]
    TooLarge { max_tokens: usize },
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
}

impl GuardianAction {
    pub(super) fn render(self, max_action_tokens: usize) -> Result<String, ActionRenderError> {
        let arguments = match self.payload {
            ToolPayload::Function { arguments } => {
                serde_json::from_str(&arguments).unwrap_or(serde_json::Value::String(arguments))
            }
            ToolPayload::Custom { input } => serde_json::Value::String(input),
            ToolPayload::ToolSearch { arguments } => json!(arguments),
        };
        let mut action = match arguments {
            serde_json::Value::Object(arguments) => arguments,
            arguments => serde_json::Map::from_iter([("arguments".to_owned(), arguments)]),
        };
        action.insert(
            "tool".to_owned(),
            serde_json::Value::String(self.tool_name.to_string()),
        );

        action.sort_keys();
        action
            .values_mut()
            .for_each(serde_json::Value::sort_all_objects);
        let max_action_bytes = TruncationPolicy::Tokens(max_action_tokens).byte_budget();
        let rendered = serde_json::to_string_pretty(&action)?;
        if rendered.len().saturating_add(1) <= max_action_bytes {
            return Ok(rendered);
        }

        Err(ActionRenderError::TooLarge {
            max_tokens: max_action_tokens,
        })
    }
}

#[cfg(test)]
#[path = "action_tests.rs"]
mod tests;

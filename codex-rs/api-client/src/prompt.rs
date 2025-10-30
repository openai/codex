use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use serde_json::Value;

use crate::Reasoning;
use crate::TextControls;

#[derive(Debug, Clone, Default)]
pub struct Prompt {
    pub instructions: String,
    pub input: Vec<ResponseItem>,
    pub tools: Vec<Value>,
    pub parallel_tool_calls: bool,
    pub output_schema: Option<Value>,
    pub reasoning: Option<Reasoning>,
    pub text_controls: Option<TextControls>,
    pub store_response: bool,
    pub prompt_cache_key: Option<String>,
    pub previous_response_id: Option<String>,
    pub session_source: Option<SessionSource>,
}

impl Prompt {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        instructions: String,
        input: Vec<ResponseItem>,
        tools: Vec<Value>,
        parallel_tool_calls: bool,
        output_schema: Option<Value>,
        reasoning: Option<Reasoning>,
        text_controls: Option<TextControls>,
        store_response: bool,
        prompt_cache_key: Option<String>,
        previous_response_id: Option<String>,
        session_source: Option<SessionSource>,
    ) -> Self {
        Self {
            instructions,
            input,
            tools,
            parallel_tool_calls,
            output_schema,
            reasoning,
            text_controls,
            store_response,
            prompt_cache_key,
            previous_response_id,
            session_source,
        }
    }
}

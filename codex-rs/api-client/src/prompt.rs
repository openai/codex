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
    pub prompt_cache_key: Option<String>,
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
        prompt_cache_key: Option<String>,
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
            prompt_cache_key,
            session_source,
        }
    }
}

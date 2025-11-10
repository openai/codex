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

    /// Start building a `Prompt` with a fluent API.
    pub fn builder() -> PromptBuilder {
        PromptBuilder::default()
    }
}

#[derive(Debug, Clone, Default)]
pub struct PromptBuilder {
    instructions: String,
    input: Vec<ResponseItem>,
    tools: Vec<Value>,
    parallel_tool_calls: bool,
    output_schema: Option<Value>,
    reasoning: Option<Reasoning>,
    text_controls: Option<TextControls>,
    prompt_cache_key: Option<String>,
    session_source: Option<SessionSource>,
}

impl PromptBuilder {
    pub fn instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = instructions.into();
        self
    }

    pub fn input(mut self, input: Vec<ResponseItem>) -> Self {
        self.input = input;
        self
    }

    pub fn tools(mut self, tools: Vec<Value>) -> Self {
        self.tools = tools;
        self
    }

    pub fn parallel_tool_calls(mut self, enabled: bool) -> Self {
        self.parallel_tool_calls = enabled;
        self
    }

    pub fn output_schema(mut self, schema: Option<Value>) -> Self {
        self.output_schema = schema;
        self
    }

    pub fn reasoning(mut self, reasoning: Option<Reasoning>) -> Self {
        self.reasoning = reasoning;
        self
    }

    pub fn text_controls(mut self, text_controls: Option<TextControls>) -> Self {
        self.text_controls = text_controls;
        self
    }

    pub fn prompt_cache_key(mut self, key: Option<String>) -> Self {
        self.prompt_cache_key = key;
        self
    }

    pub fn session_source(mut self, session: Option<SessionSource>) -> Self {
        self.session_source = session;
        self
    }

    pub fn build(self) -> Prompt {
        Prompt {
            instructions: self.instructions,
            input: self.input,
            tools: self.tools,
            parallel_tool_calls: self.parallel_tool_calls,
            output_schema: self.output_schema,
            reasoning: self.reasoning,
            text_controls: self.text_controls,
            prompt_cache_key: self.prompt_cache_key,
            session_source: self.session_source,
        }
    }
}

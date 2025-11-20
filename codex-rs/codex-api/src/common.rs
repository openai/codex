use crate::error::ApiError;
use codex_protocol::config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::config_types::Verbosity as VerbosityConfig;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::TokenUsage;
use futures::Stream;
use serde::Serialize;
use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashSet;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::sync::mpsc;

pub const APPLY_PATCH_TOOL_INSTRUCTIONS: &str =
    "When using the apply_patch tool, respond with valid patch.";

#[derive(Default, Debug, Clone)]
pub struct Prompt {
    pub input: Vec<ResponseItem>,
    pub tools: Vec<tools::ToolSpec>,
    pub parallel_tool_calls: bool,
    pub base_instructions_override: Option<String>,
    pub output_schema: Option<Value>,
}

impl Prompt {
    pub fn get_full_instructions<'a>(&'a self, base: &'a str) -> Cow<'a, str> {
        if let Some(override_text) = &self.base_instructions_override {
            Cow::Borrowed(override_text.as_str())
        } else {
            Cow::Owned(format!("{base}\n{APPLY_PATCH_TOOL_INSTRUCTIONS}"))
        }
    }

    pub fn get_formatted_input(&self) -> Vec<ResponseItem> {
        let mut input = self.input.clone();
        reserialize_shell_outputs(&mut input);
        input
    }
}

fn reserialize_shell_outputs(items: &mut [ResponseItem]) {
    let mut shell_call_ids: HashSet<String> = HashSet::new();

    items.iter_mut().for_each(|item| match item {
        ResponseItem::LocalShellCall { call_id, id, .. } => {
            if let Some(identifier) = call_id.clone().or_else(|| id.clone()) {
                shell_call_ids.insert(identifier);
            }
        }
        ResponseItem::CustomToolCall {
            id: _,
            status: _,
            call_id,
            name,
            input: _,
        } => {
            if name == "apply_patch" {
                shell_call_ids.insert(call_id.clone());
            }
        }
        ResponseItem::CustomToolCallOutput { call_id, output } => {
            if shell_call_ids.remove(call_id)
                && serde_json::from_str::<serde_json::Value>(output).is_ok()
            {
                // keep as is
            }
        }
        ResponseItem::FunctionCall { name, call_id, .. }
            if name == "shell" || name == "container.exec" =>
        {
            shell_call_ids.insert(call_id.clone());
        }
        ResponseItem::FunctionCallOutput { call_id, .. } => {
            shell_call_ids.remove(call_id);
        }
        _ => {}
    })
}

#[derive(Debug)]
pub enum ResponseEvent {
    Created,
    OutputItemDone(ResponseItem),
    OutputItemAdded(ResponseItem),
    Completed {
        response_id: String,
        token_usage: Option<TokenUsage>,
    },
    OutputTextDelta(String),
    ReasoningSummaryDelta {
        delta: String,
        summary_index: i64,
    },
    ReasoningContentDelta {
        delta: String,
        content_index: i64,
    },
    ReasoningSummaryPartAdded {
        summary_index: i64,
    },
    RateLimits(RateLimitSnapshot),
}

#[derive(Debug, Serialize)]
pub struct Reasoning {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<ReasoningEffortConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<ReasoningSummaryConfig>,
}

#[derive(Debug, Serialize, Default, Clone)]
#[serde(rename_all = "snake_case")]
pub enum TextFormatType {
    #[default]
    JsonSchema,
}

#[derive(Debug, Serialize, Default, Clone)]
pub struct TextFormat {
    pub r#type: TextFormatType,
    pub strict: bool,
    pub schema: Value,
    pub name: String,
}

#[derive(Debug, Serialize, Default, Clone)]
pub struct TextControls {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbosity: Option<OpenAiVerbosity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<TextFormat>,
}

#[derive(Debug, Serialize, Default, Clone)]
#[serde(rename_all = "lowercase")]
pub enum OpenAiVerbosity {
    Low,
    #[default]
    Medium,
    High,
}

impl From<VerbosityConfig> for OpenAiVerbosity {
    fn from(v: VerbosityConfig) -> Self {
        match v {
            VerbosityConfig::Low => OpenAiVerbosity::Low,
            VerbosityConfig::Medium => OpenAiVerbosity::Medium,
            VerbosityConfig::High => OpenAiVerbosity::High,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ResponsesApiRequest<'a> {
    pub model: &'a str,
    pub instructions: &'a str,
    pub input: &'a Vec<ResponseItem>,
    pub tools: &'a [serde_json::Value],
    pub tool_choice: &'static str,
    pub parallel_tool_calls: bool,
    pub reasoning: Option<Reasoning>,
    pub store: bool,
    pub stream: bool,
    pub include: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<TextControls>,
}

pub mod tools {
    use serde::Deserialize;
    use serde::Serialize;

    #[derive(Debug, Clone, Serialize, PartialEq)]
    #[serde(tag = "type")]
    pub enum ToolSpec {
        #[serde(rename = "function")]
        Function(ResponsesApiTool),
        #[serde(rename = "local_shell")]
        LocalShell {},
        #[serde(rename = "web_search")]
        WebSearch {},
        #[serde(rename = "custom")]
        Freeform(FreeformTool),
    }

    impl ToolSpec {
        pub fn name(&self) -> &str {
            match self {
                ToolSpec::Function(tool) => tool.name.as_str(),
                ToolSpec::LocalShell {} => "local_shell",
                ToolSpec::WebSearch {} => "web_search",
                ToolSpec::Freeform(tool) => tool.name.as_str(),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct FreeformTool {
        pub name: String,
        pub description: String,
        pub format: FreeformToolFormat,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct FreeformToolFormat {
        pub r#type: String,
        pub syntax: String,
        pub definition: String,
    }

    #[derive(Debug, Clone, Serialize, PartialEq)]
    pub struct ResponsesApiTool {
        pub name: String,
        pub description: String,
        pub strict: bool,
        pub parameters: serde_json::Value,
    }
}

pub fn create_text_param_for_request(
    verbosity: Option<VerbosityConfig>,
    output_schema: &Option<Value>,
) -> Option<TextControls> {
    if verbosity.is_none() && output_schema.is_none() {
        return None;
    }

    Some(TextControls {
        verbosity: verbosity.map(std::convert::Into::into),
        format: output_schema.as_ref().map(|schema| TextFormat {
            r#type: TextFormatType::JsonSchema,
            strict: true,
            schema: schema.clone(),
            name: "codex_output_schema".to_string(),
        }),
    })
}

pub struct ResponseStream {
    pub rx_event: mpsc::Receiver<Result<ResponseEvent, ApiError>>,
}

impl Stream for ResponseStream {
    type Item = Result<ResponseEvent, ApiError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx_event.poll_recv(cx)
    }
}

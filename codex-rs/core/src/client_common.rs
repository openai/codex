use crate::config::Config;
use crate::config_types::ReasoningEffort as ReasoningEffortConfig;
use crate::config_types::ReasoningSummary as ReasoningSummaryConfig;
use crate::error::Result;
use crate::models::ResponseItem;
use crate::protocol::TokenUsage;
use codex_apply_patch::APPLY_PATCH_TOOL_INSTRUCTIONS;
use futures::Stream;
use serde::Serialize;
use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashMap;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::sync::mpsc;

/// The `instructions` field in the payload sent to a model should always start
/// with this content.
const BASE_INSTRUCTIONS: &str = include_str!("../prompt.md");

/// API request payload for a single model turn.
#[derive(Default, Debug, Clone)]
pub struct Prompt {
    /// Conversation context input items.
    pub input: Vec<ResponseItem>,
    /// Optional instructions from the user to amend to the built-in agent
    /// instructions.
    pub user_instructions: Option<String>,
    /// Whether to store response on server side (disable_response_storage = !store).
    pub store: bool,

    /// Additional tools sourced from external MCP servers. Note each key is
    /// the "fully qualified" tool name (i.e., prefixed with the server name),
    /// which should be reported to the model in place of Tool::name.
    pub extra_tools: HashMap<String, mcp_types::Tool>,

    /// Optional override for the built-in BASE_INSTRUCTIONS.
    pub base_instructions_override: Option<String>,
}

impl Prompt {
    pub(crate) fn get_full_instructions(&self, model: &str) -> Cow<'_, str> {
        let base = self
            .base_instructions_override
            .as_deref()
            .unwrap_or(BASE_INSTRUCTIONS);
        let mut sections: Vec<&str> = vec![base];
        if let Some(ref user) = self.user_instructions {
            sections.push(user);
        }
        if model.starts_with("gpt-4.1") {
            sections.push(APPLY_PATCH_TOOL_INSTRUCTIONS);
        }
        Cow::Owned(sections.join("\n"))
    }
}

#[derive(Debug)]
pub enum ResponseEvent {
    Created,
    OutputItemDone(ResponseItem),
    Completed {
        response_id: String,
        token_usage: Option<TokenUsage>,
    },
    OutputTextDelta(String),
    ReasoningSummaryDelta(String),
}

#[derive(Debug, Serialize)]
pub(crate) struct Reasoning {
    pub(crate) effort: OpenAiReasoningEffort,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) summary: Option<OpenAiReasoningSummary>,
}

/// See https://platform.openai.com/docs/guides/reasoning?api-mode=responses#get-started-with-reasoning
#[derive(Debug, Serialize, Default, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OpenAiReasoningEffort {
    Low,
    #[default]
    Medium,
    High,
}

impl From<ReasoningEffortConfig> for Option<OpenAiReasoningEffort> {
    fn from(effort: ReasoningEffortConfig) -> Self {
        match effort {
            ReasoningEffortConfig::Low => Some(OpenAiReasoningEffort::Low),
            ReasoningEffortConfig::Medium => Some(OpenAiReasoningEffort::Medium),
            ReasoningEffortConfig::High => Some(OpenAiReasoningEffort::High),
            ReasoningEffortConfig::None => None,
        }
    }
}

/// A summary of the reasoning performed by the model. This can be useful for
/// debugging and understanding the model's reasoning process.
/// See https://platform.openai.com/docs/guides/reasoning?api-mode=responses#reasoning-summaries
#[derive(Debug, Serialize, Default, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OpenAiReasoningSummary {
    #[default]
    Auto,
    Concise,
    Detailed,
}

impl From<ReasoningSummaryConfig> for Option<OpenAiReasoningSummary> {
    fn from(summary: ReasoningSummaryConfig) -> Self {
        match summary {
            ReasoningSummaryConfig::Auto => Some(OpenAiReasoningSummary::Auto),
            ReasoningSummaryConfig::Concise => Some(OpenAiReasoningSummary::Concise),
            ReasoningSummaryConfig::Detailed => Some(OpenAiReasoningSummary::Detailed),
            ReasoningSummaryConfig::None => None,
        }
    }
}

/// Request object that is serialized as JSON and POST'ed when using the
/// Responses API.
#[derive(Debug, Serialize)]
pub(crate) struct ResponsesApiRequest<'a> {
    pub(crate) model: &'a str,
    pub(crate) instructions: &'a str,
    // TODO(mbolin): ResponseItem::Other should not be serialized. Currently,
    // we code defensively to avoid this case, but perhaps we should use a
    // separate enum for serialization.
    #[serde(serialize_with = "serialize_sanitized_input")]
    pub(crate) input: &'a Vec<crate::models::ResponseItem>,
    pub(crate) tools: &'a [serde_json::Value],
    pub(crate) tool_choice: &'static str,
    pub(crate) parallel_tool_calls: bool,
    pub(crate) reasoning: Option<Reasoning>,
    /// true when using the Responses API.
    pub(crate) store: bool,
    pub(crate) stream: bool,
    pub(crate) include: Vec<String>,
}

// Custom serializer that strips internal-only fields before sending to the LLM.
#[allow(clippy::ptr_arg)]
fn serialize_sanitized_input<S>(
    input: &Vec<crate::models::ResponseItem>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::ser::Serializer,
{
    let sanitized: Vec<Value> = input.iter().map(sanitize_response_item).collect();

    serde::Serialize::serialize(&sanitized, serializer)
}

// Top-level dispatcher to apply a sanitizer per ResponseItem variant.
fn sanitize_response_item(item: &crate::models::ResponseItem) -> Value {
    use crate::models::ResponseItem as RI;
    match item {
        RI::FunctionCallOutput { .. } => sanitize_function_call_output(item),
        // For other variants, no special handling for now.
        _ => serde_json::to_value(item).unwrap_or(Value::Null),
    }
}

// Special-case sanitizer for FunctionCallOutput: replace the `output` object with its `content` string.
fn sanitize_function_call_output(item: &crate::models::ResponseItem) -> Value {
    let mut v = serde_json::to_value(item).unwrap_or(Value::Null);
    if let Some(obj) = v.as_object_mut() {
        if let Some(output) = obj.get_mut("output") {
            if let Some(content) = output.get("content") {
                *output = content.clone();
            }
        }
    }
    v
}

pub(crate) fn create_reasoning_param_for_request(
    config: &Config,
    effort: ReasoningEffortConfig,
    summary: ReasoningSummaryConfig,
) -> Option<Reasoning> {
    if model_supports_reasoning_summaries(config) {
        let effort: Option<OpenAiReasoningEffort> = effort.into();
        let effort = effort?;
        Some(Reasoning {
            effort,
            summary: summary.into(),
        })
    } else {
        None
    }
}

pub fn model_supports_reasoning_summaries(config: &Config) -> bool {
    // Currently, we hardcode this rule to decide whether to enable reasoning.
    // We expect reasoning to apply only to OpenAI models, but we do not want
    // users to have to mess with their config to disable reasoning for models
    // that do not support it, such as `gpt-4.1`.
    //
    // Though if a user is using Codex with non-OpenAI models that, say, happen
    // to start with "o", then they can set `model_reasoning_effort = "none"` in
    // config.toml to disable reasoning.
    //
    // Converseley, if a user has a non-OpenAI provider that supports reasoning,
    // they can set the top-level `model_supports_reasoning_summaries = true`
    // config option to enable reasoning.
    if config.model_supports_reasoning_summaries {
        return true;
    }

    let model = &config.model;
    model.starts_with("o") || model.starts_with("codex")
}

pub(crate) struct ResponseStream {
    pub(crate) rx_event: mpsc::Receiver<Result<ResponseEvent>>,
}

impl Stream for ResponseStream {
    type Item = Result<ResponseEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx_event.poll_recv(cx)
    }
}

#[cfg(test)]
mod sanitize_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::models::ContentItem;
    use crate::models::FunctionCallOutputPayload;
    use crate::models::ResponseItem;

    fn make_req(input: &Vec<ResponseItem>) -> serde_json::Value {
        let req = ResponsesApiRequest {
            model: "test-model",
            instructions: "instr",
            input,
            tools: &[],
            tool_choice: "auto",
            parallel_tool_calls: false,
            reasoning: None,
            store: false,
            stream: false,
            include: vec![],
        };
        serde_json::to_value(&req).unwrap()
    }

    #[test]
    fn function_call_output_output_is_replaced_by_content() {
        let items = vec![ResponseItem::FunctionCallOutput {
            call_id: "c1".into(),
            output: FunctionCallOutputPayload {
                content: "hello".into(),
                success: Some(true),
                is_user_feedback: true,
            },
        }];

        let v = make_req(&items);
        let input0 = &v["input"][0];
        assert_eq!(input0["type"].as_str().unwrap(), "function_call_output");
        assert!(input0["output"].is_string());
        assert_eq!(input0["output"].as_str().unwrap(), "hello");
        // The object fields like is_user_feedback should be stripped at this stage
        // (because output is now just a string).
    }

    #[test]
    fn non_function_call_items_are_unchanged() {
        let items = vec![ResponseItem::Message {
            id: None,
            role: "user".into(),
            content: vec![ContentItem::InputText { text: "Hi".into() }],
        }];

        let v = make_req(&items);
        let input0 = &v["input"][0];
        assert_eq!(input0["type"].as_str().unwrap(), "message");
        assert_eq!(input0["role"].as_str().unwrap(), "user");
        assert_eq!(input0["content"][0]["text"].as_str().unwrap(), "Hi");
    }
}

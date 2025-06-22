use crate::config_types::ReasoningEffort as ReasoningEffortConfig;
use crate::config_types::ReasoningSummary as ReasoningSummaryConfig;
use crate::error::Result;
use crate::models::ResponseItem;
use codex_apply_patch::APPLY_PATCH_TOOL_INSTRUCTIONS;
use futures::Stream;
use serde::Serialize;
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
    /// Optional previous response ID (when storage is enabled).
    pub prev_id: Option<String>,
    /// Optional instructions from the user to amend to the built-in agent
    /// instructions.
    pub user_instructions: Option<String>,
    /// Whether to store response on server side (disable_response_storage = !store).
    pub store: bool,

    /// Additional tools sourced from external MCP servers. Note each key is
    /// the "fully qualified" tool name (i.e., prefixed with the server name),
    /// which should be reported to the model in place of Tool::name.
    pub extra_tools: HashMap<String, mcp_types::Tool>,
}

impl Prompt {
    pub(crate) fn get_full_instructions(&self, model: &str) -> Cow<str> {
        let mut sections: Vec<&str> = vec![BASE_INSTRUCTIONS];
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
    OutputItemDone(ResponseItem),
    Completed { response_id: String },
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
    pub(crate) input: &'a Vec<ResponseItem>,
    pub(crate) tools: &'a [serde_json::Value],
    pub(crate) tool_choice: &'static str,
    pub(crate) parallel_tool_calls: bool,
    pub(crate) reasoning: Option<Reasoning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) previous_response_id: Option<String>,
    /// true when using the Responses API.
    pub(crate) store: bool,
    pub(crate) stream: bool,
}

pub(crate) fn create_reasoning_param_for_request(
    model: &str,
    effort: ReasoningEffortConfig,
    summary: ReasoningSummaryConfig,
) -> Option<Reasoning> {
    let effort: Option<OpenAiReasoningEffort> = effort.into();
    let effort = effort?;

    if model_supports_reasoning_summaries(model) {
        Some(Reasoning {
            effort,
            summary: summary.into(),
        })
    } else {
        None
    }
}

pub fn model_supports_reasoning_summaries(model: &str) -> bool {
    // Currently, we hardcode this rule to decide whether enable reasoning.
    // We expect reasoning to apply only to OpenAI models, but we do not want
    // users to have to mess with their config to disable reasoning for models
    // that do not support it, such as `gpt-4.1`.
    //
    // Though if a user is using Codex with non-OpenAI models that, say, happen
    // to start with "o", then they can set `model_reasoning_effort = "none` in
    // config.toml to disable reasoning.
    //
    // Ultimately, this should also be configurable in config.toml, but we
    // need to have defaults that "just work." Perhaps we could have a
    // "reasoning models pattern" as part of ModelProviderInfo?
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

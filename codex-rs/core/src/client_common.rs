use crate::config_types::ReasoningEffort as ReasoningEffortConfig;
use crate::config_types::ReasoningSummary as ReasoningSummaryConfig;
use crate::error::Result;
use crate::models::ResponseItem;
use crate::protocol::TokenUsage;
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

/// Options that influence how the full instructions are composed for a request.
#[derive(Debug, Default, Clone)]
pub struct InstructionsConfig {
    pub include_plan_tool: bool,
    pub extra_sections: Vec<&'static str>,
}

impl InstructionsConfig {
    pub fn for_model(model: &str, include_plan_tool: bool) -> Self {
        let mut extra_sections = Vec::new();
        if model.starts_with("gpt-4.1") {
            extra_sections.push(APPLY_PATCH_TOOL_INSTRUCTIONS);
        }
        Self {
            include_plan_tool,
            extra_sections,
        }
    }
}

impl Prompt {
    pub(crate) fn get_full_instructions(&self, cfg: &InstructionsConfig) -> Cow<'_, str> {
        let mut base = self
            .base_instructions_override
            .as_deref()
            .unwrap_or(BASE_INSTRUCTIONS)
            .to_string();

        if !cfg.include_plan_tool {
            // Remove the plan-tool section if present. Prefer explicit markers
            // for robustness, but fall back to trimming from the "Plan updates"
            // heading if markers are missing.
            let start_marker = "<!-- PLAN_TOOL:START -->";
            let end_marker = "<!-- PLAN_TOOL:END -->";
            if let (Some(start), Some(end)) = (base.find(start_marker), base.find(end_marker)) {
                if end > start {
                    let mut edited = String::with_capacity(base.len());
                    edited.push_str(&base[..start]);
                    edited.push_str(&base[end + end_marker.len()..]);
                    base = edited;
                }
            } else if let Some(idx) = base
                .find("\n\nPlan updates")
                .or_else(|| base.find("\nPlan updates"))
                .or_else(|| base.find("Plan updates"))
            {
                base.truncate(idx);
            }
            base = base.trim_end().to_string();
        }

        let mut sections: Vec<&str> = vec![&base];
        for s in &cfg.extra_sections {
            sections.push(s);
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
    pub(crate) input: &'a Vec<ResponseItem>,
    pub(crate) tools: &'a [serde_json::Value],
    pub(crate) tool_choice: &'static str,
    pub(crate) parallel_tool_calls: bool,
    pub(crate) reasoning: Option<Reasoning>,
    /// true when using the Responses API.
    pub(crate) store: bool,
    pub(crate) stream: bool,
    pub(crate) include: Vec<String>,
}

use crate::config::Config;

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
mod tests {
    use super::*;

    #[test]
    fn get_full_instructions_no_user_content() {
        let prompt = Prompt {
            user_instructions: Some("custom instruction".to_string()),
            ..Default::default()
        };
        let expected = format!("{BASE_INSTRUCTIONS}\n{APPLY_PATCH_TOOL_INSTRUCTIONS}");
        let cfg = InstructionsConfig::for_model("gpt-4.1", true);
        let full = prompt.get_full_instructions(&cfg);
        assert_eq!(full, expected);
    }

    #[test]
    fn plan_section_removed_when_disabled() {
        let prompt = Prompt::default();
        let cfg = InstructionsConfig::for_model("gpt-4.1", false);
        let full = prompt.get_full_instructions(&cfg);
        assert!(!full.contains("Plan updates"));
        assert!(!full.contains("update_plan"));
        assert!(full.contains(APPLY_PATCH_TOOL_INSTRUCTIONS));
    }
}

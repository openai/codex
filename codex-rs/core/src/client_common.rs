use crate::config_types::ReasoningEffort as ReasoningEffortConfig;
use crate::config_types::ReasoningSummary as ReasoningSummaryConfig;
use crate::error::Result;
use crate::model_family::ModelFamily;
use crate::models::ContentItem;
use crate::models::ResponseItem;
use crate::openai_tools::OpenAiTool;
use crate::protocol::AskForApproval;
use crate::protocol::SandboxPolicy;
use crate::protocol::TokenUsage;
use codex_apply_patch::APPLY_PATCH_TOOL_INSTRUCTIONS;
use futures::Stream;
use serde::Serialize;
use std::borrow::Cow;
use std::fmt::Display;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::sync::mpsc;

/// The `instructions` field in the payload sent to a model should always start
/// with this content.
const BASE_INSTRUCTIONS: &str = include_str!("../prompt.md");

/// wraps environment context message in a tag for the model to parse more easily.
const ENVIRONMENT_CONTEXT_START: &str = "<environment_context>\n\n";
const ENVIRONMENT_CONTEXT_END: &str = "\n\n</environment_context>";

/// wraps user instructions message in a tag for the model to parse more easily.
const USER_INSTRUCTIONS_START: &str = "<user_instructions>\n\n";
const USER_INSTRUCTIONS_END: &str = "\n\n</user_instructions>";

#[derive(Debug, Clone)]
pub(crate) struct EnvironmentContext {
    pub cwd: PathBuf,
    pub approval_policy: AskForApproval,
    pub sandbox_policy: SandboxPolicy,
}

impl Display for EnvironmentContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "Current working directory: {}",
            self.cwd.to_string_lossy()
        )?;
        writeln!(f, "Approval policy: {}", self.approval_policy)?;
        writeln!(f, "Sandbox policy: {}", self.sandbox_policy)?;

        let network_access = match self.sandbox_policy.clone() {
            SandboxPolicy::DangerFullAccess => "enabled",
            SandboxPolicy::ReadOnly => "restricted",
            SandboxPolicy::WorkspaceWrite { network_access, .. } => {
                if network_access {
                    "enabled"
                } else {
                    "restricted"
                }
            }
        };
        writeln!(f, "Network access: {network_access}")?;
        Ok(())
    }
}

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

    /// A list of key-value pairs that will be added as a developer message
    /// for the model to use
    pub environment_context: Option<EnvironmentContext>,

    /// Tools available to the model, including additional tools sourced from
    /// external MCP servers.
    pub tools: Vec<OpenAiTool>,

    /// Optional override for the built-in BASE_INSTRUCTIONS.
    pub base_instructions_override: Option<String>,
}

impl Prompt {
    pub(crate) fn get_full_instructions(&self, model: &ModelFamily) -> Cow<'_, str> {
        let base = self
            .base_instructions_override
            .as_deref()
            .unwrap_or(BASE_INSTRUCTIONS);
        let mut sections: Vec<&str> = vec![base];
        if model.needs_special_apply_patch_instructions {
            sections.push(APPLY_PATCH_TOOL_INSTRUCTIONS);
        }
        Cow::Owned(sections.join("\n"))
    }

    fn get_formatted_user_instructions(&self) -> Option<String> {
        self.user_instructions
            .as_ref()
            .map(|ui| format!("{USER_INSTRUCTIONS_START}{ui}{USER_INSTRUCTIONS_END}"))
    }

    fn get_formatted_environment_context(&self) -> Option<String> {
        self.environment_context
            .as_ref()
            .map(|ec| format!("{ENVIRONMENT_CONTEXT_START}{ec}{ENVIRONMENT_CONTEXT_END}"))
    }

    pub(crate) fn get_formatted_input(&self) -> Vec<ResponseItem> {
        let mut input_with_instructions = Vec::with_capacity(self.input.len() + 2);
        if let Some(ec) = self.get_formatted_environment_context() {
            input_with_instructions.push(ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText { text: ec }],
            });
        }
        if let Some(ui) = self.get_formatted_user_instructions() {
            input_with_instructions.push(ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText { text: ui }],
            });
        }
        input_with_instructions.extend(self.input.clone());
        input_with_instructions
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
    ReasoningContentDelta(String),
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) prompt_cache_key: Option<String>,
}

pub(crate) fn create_reasoning_param_for_request(
    model_family: &ModelFamily,
    effort: ReasoningEffortConfig,
    summary: ReasoningSummaryConfig,
) -> Option<Reasoning> {
    if model_family.supports_reasoning_summaries {
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
    #![allow(clippy::expect_used)]
    use crate::model_family::find_family_for_model;

    use super::*;

    #[test]
    fn get_full_instructions_no_user_content() {
        let prompt = Prompt {
            user_instructions: Some("custom instruction".to_string()),
            ..Default::default()
        };
        let expected = format!("{BASE_INSTRUCTIONS}\n{APPLY_PATCH_TOOL_INSTRUCTIONS}");
        let model_family = find_family_for_model("gpt-4.1").expect("known model slug");
        let full = prompt.get_full_instructions(&model_family);
        assert_eq!(full, expected);
    }
}

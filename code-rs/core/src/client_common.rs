use crate::config_types::ReasoningEffort as ReasoningEffortConfig;
use crate::config_types::ReasoningSummary as ReasoningSummaryConfig;
use crate::config_types::TextVerbosity as TextVerbosityConfig;
use crate::environment_context::EnvironmentContext;
use crate::error::Result;
use crate::model_family::ModelFamily;
use crate::openai_tools::OpenAiTool;
use crate::protocol::RateLimitSnapshotEvent;
use crate::protocol::TokenUsage;
use code_apply_patch::APPLY_PATCH_TOOL_INSTRUCTIONS;
use code_protocol::models::ContentItem;
use code_protocol::models::ResponseItem;
use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::borrow::Cow;
use std::ops::Deref;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Additional prompt for Code. Can not edit Codex instructions.
const ADDITIONAL_INSTRUCTIONS: &str = include_str!("../prompt_coder.md");

/// wraps environment context message in a tag for the model to parse more easily.
const ENVIRONMENT_CONTEXT_START: &str = "<environment_context>\n\n";
const ENVIRONMENT_CONTEXT_END: &str = "\n\n</environment_context>";

/// wraps user instructions message in a tag for the model to parse more easily.
const USER_INSTRUCTIONS_START: &str = "<user_instructions>\n\n";
const USER_INSTRUCTIONS_END: &str = "\n\n</user_instructions>";
/// Review thread system prompt. Edit `core/src/review_prompt.md` to customize.
#[allow(dead_code)]
pub const REVIEW_PROMPT: &str = include_str!("../review_prompt.md");

/// API request payload for a single model turn
#[derive(Debug, Clone)]
pub struct Prompt {
    /// Conversation context input items.
    pub input: Vec<ResponseItem>,

    /// Whether to store response on server side (disable_response_storage = !store).
    pub store: bool,

    /// Model instructions that are appended to the base instructions.
    pub user_instructions: Option<String>,

    /// A list of key-value pairs that will be added as a developer message
    /// for the model to use
    pub(crate) environment_context: Option<EnvironmentContext>,

    /// Tools available to the model, including additional tools sourced from
    /// external MCP servers.
    pub(crate) tools: Vec<OpenAiTool>,

    /// Status items to be added at the end of the input
    /// These are generated fresh for each request (screenshots, system status)
    pub status_items: Vec<ResponseItem>,

    /// Optional override for the built-in BASE_INSTRUCTIONS.
    pub base_instructions_override: Option<String>,

    /// Whether to prepend the default developer instructions block.
    pub include_additional_instructions: bool,

    /// Optional `text.format` for structured outputs (used by side-channel requests).
    pub text_format: Option<TextFormat>,

    /// Optional per-request model slug override.
    pub model_override: Option<String>,

    /// Optional per-request model family override matching `model_override`.
    pub model_family_override: Option<ModelFamily>,
    /// Optional the output schema for the model's response.
    pub output_schema: Option<Value>,
    /// Optional tag used to route debug logs into helper-specific directories.
    pub log_tag: Option<String>,
    /// Optional override for session/conversation identifiers used for caching.
    pub session_id_override: Option<Uuid>,
}

impl Default for Prompt {
    fn default() -> Self {
        Self {
            input: Vec::new(),
            store: false,
            user_instructions: None,
            environment_context: None,
            tools: Vec::new(),
            status_items: Vec::new(),
            base_instructions_override: None,
            include_additional_instructions: true,
            text_format: None,
            model_override: None,
            model_family_override: None,
            output_schema: None,
            log_tag: None,
            session_id_override: None,
        }
    }
}

impl Prompt {
    pub(crate) fn get_full_instructions<'a>(&'a self, model: &'a ModelFamily) -> Cow<'a, str> {
        let effective_model = self.model_family_override.as_ref().unwrap_or(model);
        let base = self
            .base_instructions_override
            .as_deref()
            .unwrap_or(effective_model.base_instructions.deref());
        let _sections: Vec<&str> = vec![base];
        // When there are no custom instructions, add apply_patch_tool_instructions if:
        // - the model needs special instructions (4.1)
        // AND
        // - there is no apply_patch tool present
        let is_apply_patch_tool_present = self.tools.iter().any(|tool| match tool {
            OpenAiTool::Function(f) => f.name == "apply_patch",
            OpenAiTool::Freeform(f) => f.name == "apply_patch",
            _ => false,
        });
        if self.base_instructions_override.is_none()
            && effective_model.needs_special_apply_patch_instructions
            && !is_apply_patch_tool_present
        {
            Cow::Owned(format!("{base}\n{APPLY_PATCH_TOOL_INSTRUCTIONS}"))
        } else {
            Cow::Borrowed(base)
        }
    }

    pub fn set_log_tag<S: Into<String>>(&mut self, tag: S) {
        self.log_tag = Some(tag.into());
    }

    fn get_formatted_user_instructions(&self) -> Option<String> {
        self.user_instructions
            .as_ref()
            .map(|ui| format!("{USER_INSTRUCTIONS_START}{ui}{USER_INSTRUCTIONS_END}"))
    }

    fn get_formatted_environment_context(&self) -> Option<String> {
        self.environment_context.as_ref().map(|ec| {
            let ec_str = serde_json::to_string_pretty(ec).unwrap_or_else(|_| format!("{:?}", ec));
            format!("{ENVIRONMENT_CONTEXT_START}{ec_str}{ENVIRONMENT_CONTEXT_END}")
        })
    }

    pub(crate) fn get_formatted_input(&self) -> Vec<ResponseItem> {
        let mut input_with_instructions =
            Vec::with_capacity(self.input.len() + self.status_items.len() + 3);
        if self.include_additional_instructions {
            input_with_instructions.push(ResponseItem::Message {
                id: None,
                role: "developer".to_string(),
                content: vec![ContentItem::InputText {
                    text: ADDITIONAL_INSTRUCTIONS.to_string(),
                }],
            });
            if let Some(ec) = self.get_formatted_environment_context() {
                let has_environment_context = self.input.iter().any(|item| {
                    matches!(item, ResponseItem::Message { role, content, .. }
                        if role == "user"
                            && content.iter().any(|c| matches!(c,
                                ContentItem::InputText { text } if text.contains(ENVIRONMENT_CONTEXT_START.trim())
                            )))
                });
                if !has_environment_context {
                    input_with_instructions.push(ResponseItem::Message {
                        id: None,
                        role: "user".to_string(),
                        content: vec![ContentItem::InputText { text: ec }],
                    });
                }
            }
            if let Some(ui) = self.get_formatted_user_instructions() {
                let has_user_instructions = self.input.iter().any(|item| {
                    matches!(item, ResponseItem::Message { role, content, .. }
                        if role == "user"
                            && content.iter().any(|c| matches!(c,
                                ContentItem::InputText { text } if text.contains(USER_INSTRUCTIONS_START)
                            )))
                });
                if !has_user_instructions {
                    input_with_instructions.push(ResponseItem::Message {
                        id: None,
                        role: "user".to_string(),
                        content: vec![ContentItem::InputText { text: ui }],
                    });
                }
            }
        }
        // Deduplicate function call outputs before adding to input
        let mut seen_call_ids = std::collections::HashSet::new();
        for item in &self.input {
            match item {
                ResponseItem::FunctionCallOutput { call_id, .. } => {
                    if !seen_call_ids.insert(call_id.clone()) {
                        // Skip duplicate function call output
                        tracing::debug!(
                            "Filtering duplicate FunctionCallOutput with call_id: {} from input",
                            call_id
                        );
                        continue;
                    }
                }
                _ => {}
            }
            input_with_instructions.push(item.clone());
        }

        // Add status items at the end so they're fresh for each request
        input_with_instructions.extend(self.status_items.clone());

        // Limit screenshots to maximum 5 (keep first and last 4)
        limit_screenshots_in_input(&mut input_with_instructions);

        input_with_instructions
    }

    pub fn set_tools(&mut self, tools: Vec<OpenAiTool>) {
        self.tools = tools;
    }

    /// Creates a formatted user instructions message from a string
    #[allow(dead_code)]
    pub(crate) fn format_user_instructions_message(ui: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: format!("{USER_INSTRUCTIONS_START}{ui}{USER_INSTRUCTIONS_END}"),
            }],
        }
    }
}

#[derive(Debug)]
pub enum ResponseEvent {
    Created,
    OutputItemDone { item: ResponseItem, sequence_number: Option<u64>, output_index: Option<u32> },
    Completed {
        response_id: String,
        token_usage: Option<TokenUsage>,
    },
    OutputTextDelta {
        delta: String,
        item_id: Option<String>,
        sequence_number: Option<u64>,
        output_index: Option<u32>,
    },
    ReasoningSummaryDelta {
        delta: String,
        item_id: Option<String>,
        sequence_number: Option<u64>,
        output_index: Option<u32>,
        summary_index: Option<u32>,
    },
    ReasoningContentDelta {
        delta: String,
        item_id: Option<String>,
        sequence_number: Option<u64>,
        output_index: Option<u32>,
        content_index: Option<u32>,
    },
    ReasoningSummaryPartAdded,
    WebSearchCallBegin {
        call_id: String,
    },
    WebSearchCallCompleted {
        call_id: String,
        query: Option<String>,
    },
    RateLimits(RateLimitSnapshotEvent),
}

#[derive(Debug, Serialize)]
pub(crate) struct Reasoning {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) effort: Option<ReasoningEffortConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) summary: Option<ReasoningSummaryConfig>,
}

/// Text configuration for verbosity/format in OpenAI API responses.
#[derive(Debug)]
pub(crate) struct Text {
    pub(crate) verbosity: OpenAiTextVerbosity,
    pub(crate) format: Option<TextFormat>,
}

impl serde::Serialize for Text {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("verbosity", &self.verbosity)?;
        if let Some(fmt) = &self.format {
            map.serialize_entry("format", fmt)?;
        }
        map.end()
    }
}

/// OpenAI text verbosity level for serialization.
#[derive(Debug, Serialize, Default, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OpenAiTextVerbosity {
    Low,
    #[default]
    Medium,
    High,
}

impl From<TextVerbosityConfig> for OpenAiTextVerbosity {
    fn from(verbosity: TextVerbosityConfig) -> Self {
        match verbosity {
            TextVerbosityConfig::Low => OpenAiTextVerbosity::Low,
            TextVerbosityConfig::Medium => OpenAiTextVerbosity::Medium,
            TextVerbosityConfig::High => OpenAiTextVerbosity::High,
        }
    }
}

/// Optional structured output format for `text.format` in the Responses API.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TextFormat {
    #[serde(rename = "type")]
    pub r#type: String, // e.g. "json_schema"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<serde_json::Value>,
}

/// Limits the number of screenshots in the input to a maximum of 5.
/// Keeps the first screenshot and the last 4 screenshots.
/// Replaces removed screenshots with a placeholder message.
fn limit_screenshots_in_input(input: &mut Vec<ResponseItem>) {
    // Find all screenshot positions
    let mut screenshot_positions = Vec::new();
    
    for (idx, item) in input.iter().enumerate() {
        if let ResponseItem::Message { content, .. } = item {
            let has_screenshot = content
                .iter()
                .any(|c| matches!(c, ContentItem::InputImage { .. }));
            if has_screenshot {
                screenshot_positions.push(idx);
            }
        }
    }
    
    // If we have 5 or fewer screenshots, no action needed
    if screenshot_positions.len() <= 5 {
        return;
    }
    
    // Determine which screenshots to keep
    let mut positions_to_keep = std::collections::HashSet::new();
    
    // Keep the first screenshot
    if let Some(&first) = screenshot_positions.first() {
        positions_to_keep.insert(first);
    }
    
    // Keep the last 4 screenshots
    let last_four_start = screenshot_positions.len().saturating_sub(4);
    for &pos in &screenshot_positions[last_four_start..] {
        positions_to_keep.insert(pos);
    }
    
    // Replace screenshots that should be removed
    for &pos in &screenshot_positions {
        if !positions_to_keep.contains(&pos) {
            if let Some(ResponseItem::Message { content, .. }) = input.get_mut(pos) {
                // Replace image content with placeholder message
                let mut new_content = Vec::new();
                for item in content.iter() {
                    match item {
                        ContentItem::InputImage { .. } => {
                            new_content.push(ContentItem::InputText {
                                text: "[screenshot no longer available]".to_string(),
                            });
                        }
                        other => new_content.push(other.clone()),
                    }
                }
                *content = new_content;
            }
        }
    }
    
    tracing::debug!(
        "Limited screenshots from {} to {} (kept first and last 4)",
        screenshot_positions.len(),
        positions_to_keep.len()
    );
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
    pub(crate) text: Option<Text>,
    /// true when using the Responses API.
    pub(crate) store: bool,
    pub(crate) stream: bool,
    pub(crate) include: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) prompt_cache_key: Option<String>,
}

pub(crate) fn create_reasoning_param_for_request(
    model_family: &ModelFamily,
    effort: Option<ReasoningEffortConfig>,
    summary: ReasoningSummaryConfig,
) -> Option<Reasoning> {
    if !model_family.supports_reasoning_summaries {
        return None;
    }

    Some(Reasoning {
        effort,
        summary: Some(summary),
    })
}

// Removed legacy TextControls helper; use `Text` with `OpenAiTextVerbosity` instead.

pub struct ResponseStream {
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
    use crate::model_family::find_family_for_model;
    use pretty_assertions::assert_eq;

    use super::*;

    struct InstructionsTestCase {
        pub slug: &'static str,
        pub expects_apply_patch_instructions: bool,
    }
    #[test]
    fn get_full_instructions_no_user_content() {
        let prompt = Prompt {
            ..Default::default()
        };
        let test_cases = vec![
            InstructionsTestCase {
                slug: "gpt-3.5",
                expects_apply_patch_instructions: true,
            },
            InstructionsTestCase {
                slug: "gpt-4.1",
                expects_apply_patch_instructions: true,
            },
            InstructionsTestCase {
                slug: "gpt-4o",
                expects_apply_patch_instructions: true,
            },
            InstructionsTestCase {
                slug: "gpt-5",
                expects_apply_patch_instructions: true,
            },
            InstructionsTestCase {
                slug: "codex-mini-latest",
                expects_apply_patch_instructions: true,
            },
            InstructionsTestCase {
                slug: "gpt-oss:120b",
                expects_apply_patch_instructions: false,
            },
            InstructionsTestCase {
                slug: "gpt-5-codex",
                expects_apply_patch_instructions: false,
            },
        ];
        for test_case in test_cases {
            let model_family = find_family_for_model(test_case.slug).expect("known model slug");
            let expected = if test_case.expects_apply_patch_instructions {
                format!(
                    "{}\n{}",
                    model_family.clone().base_instructions,
                    APPLY_PATCH_TOOL_INSTRUCTIONS
                )
            } else {
                model_family.clone().base_instructions
            };

            let full = prompt.get_full_instructions(&model_family);
            assert_eq!(full, expected);
        }
    }

    #[test]
    fn serializes_text_verbosity_when_set() {
        let input: Vec<ResponseItem> = vec![];
        let tools: Vec<serde_json::Value> = vec![];
        let req = ResponsesApiRequest {
            model: "gpt-5",
            instructions: "i",
            input: &input,
            tools: &tools,
            tool_choice: "auto",
            parallel_tool_calls: false,
            reasoning: None,
            store: false,
            stream: true,
            include: vec![],
            prompt_cache_key: None,
            text: Some(Text { verbosity: OpenAiTextVerbosity::Low, format: None }),
        };

        let v = serde_json::to_value(&req).expect("json");
        assert_eq!(
            v.get("text")
                .and_then(|t| t.get("verbosity"))
                .and_then(|s| s.as_str()),
            Some("low")
        );
    }

    #[test]
    fn serializes_text_schema_with_strict_format() {
        let input: Vec<ResponseItem> = vec![];
        let tools: Vec<serde_json::Value> = vec![];
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "answer": {"type": "string"}
            },
            "required": ["answer"],
        });
        let req = ResponsesApiRequest {
            model: "gpt-5",
            instructions: "i",
            input: &input,
            tools: &tools,
            tool_choice: "auto",
            parallel_tool_calls: false,
            reasoning: None,
            store: false,
            stream: true,
            include: vec![],
            prompt_cache_key: None,
            text: Some(Text {
                verbosity: OpenAiTextVerbosity::Medium,
                format: Some(TextFormat {
                    r#type: "json_schema".to_string(),
                    name: Some("code_output_schema".to_string()),
                    strict: Some(true),
                    schema: Some(schema.clone()),
                }),
            }),
        };

        let v = serde_json::to_value(&req).expect("json");
        let text = v.get("text").expect("text field");
        assert_eq!(
            text.get("verbosity").and_then(|v| v.as_str()),
            Some("medium")
        );
        let format = text.get("format").expect("format field");

        assert_eq!(
            format.get("name"),
            Some(&serde_json::Value::String("code_output_schema".into()))
        );
        assert_eq!(
            format.get("type"),
            Some(&serde_json::Value::String("json_schema".into()))
        );
        assert_eq!(format.get("strict"), Some(&serde_json::Value::Bool(true)));
        assert_eq!(format.get("schema"), Some(&schema));
    }

    #[test]
    fn omits_text_when_not_set() {
        let input: Vec<ResponseItem> = vec![];
        let tools: Vec<serde_json::Value> = vec![];
        let req = ResponsesApiRequest {
            model: "gpt-5",
            instructions: "i",
            input: &input,
            tools: &tools,
            tool_choice: "auto",
            parallel_tool_calls: false,
            reasoning: None,
            store: false,
            stream: true,
            include: vec![],
            prompt_cache_key: None,
            text: None,
        };

        let v = serde_json::to_value(&req).expect("json");
        assert!(v.get("text").is_none());
    }
}

use crate::client_common::tools::ToolSpec;
use crate::error::Result;
use crate::model_family::ModelFamily;
use crate::protocol::RateLimitSnapshot;
use crate::protocol::TokenUsage;
use codex_apply_patch::APPLY_PATCH_TOOL_INSTRUCTIONS;
use codex_protocol::config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::config_types::Verbosity as VerbosityConfig;
use codex_protocol::models::ResponseItem;
use futures::Stream;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashSet;
use std::ops::Deref;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::sync::mpsc;

/// Review thread system prompt. Edit `core/src/review_prompt.md` to customize.
pub const REVIEW_PROMPT: &str = include_str!("../review_prompt.md");

/// API request payload for a single model turn
#[derive(Default, Debug, Clone)]
pub struct Prompt {
    /// Conversation context input items.
    pub input: Vec<ResponseItem>,

    /// Tools available to the model, including additional tools sourced from
    /// external MCP servers.
    pub(crate) tools: Vec<ToolSpec>,

    /// Whether parallel tool calls are permitted for this prompt.
    pub(crate) parallel_tool_calls: bool,

    /// Optional override for the built-in BASE_INSTRUCTIONS.
    pub base_instructions_override: Option<String>,

    /// Optional the output schema for the model's response.
    pub output_schema: Option<Value>,
}

impl Prompt {
    pub(crate) fn get_full_instructions<'a>(&'a self, model: &'a ModelFamily) -> Cow<'a, str> {
        let base = self
            .base_instructions_override
            .as_deref()
            .unwrap_or(model.base_instructions.deref());
        // When there are no custom instructions, add apply_patch_tool_instructions if:
        // - the model needs special instructions (4.1)
        // AND
        // - there is no apply_patch tool present
        let is_apply_patch_tool_present = self.tools.iter().any(|tool| match tool {
            ToolSpec::Function(f) => f.name == "apply_patch",
            ToolSpec::Freeform(f) => f.name == "apply_patch",
            _ => false,
        });
        if self.base_instructions_override.is_none()
            && model.needs_special_apply_patch_instructions
            && !is_apply_patch_tool_present
        {
            Cow::Owned(format!("{base}\n{APPLY_PATCH_TOOL_INSTRUCTIONS}"))
        } else {
            Cow::Borrowed(base)
        }
    }

    pub(crate) fn get_formatted_input(&self) -> Vec<ResponseItem> {
        let mut input = self.input.clone();

        // when using the *Freeform* apply_patch tool specifically, tool outputs
        // should be structured text, not json. Do NOT reserialize when using
        // the Function tool - note that this differs from the check above for
        // instructions. We declare the result as a named variable for clarity.
        let is_freeform_apply_patch_tool_present = self.tools.iter().any(|tool| match tool {
            ToolSpec::Freeform(f) => f.name == "apply_patch",
            _ => false,
        });
        if is_freeform_apply_patch_tool_present {
            reserialize_shell_outputs(&mut input);
        }

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
                && let Some(structured) = parse_structured_shell_output(output)
            {
                *output = structured
            }
        }
        ResponseItem::FunctionCall { name, call_id, .. }
            if is_shell_tool_name(name) || name == "apply_patch" =>
        {
            shell_call_ids.insert(call_id.clone());
        }
        ResponseItem::FunctionCallOutput { call_id, output } => {
            if shell_call_ids.remove(call_id)
                && let Some(structured) = parse_structured_shell_output(&output.content)
            {
                output.content = structured
            }
        }
        _ => {}
    })
}

fn is_shell_tool_name(name: &str) -> bool {
    matches!(name, "shell" | "container.exec")
}

#[derive(Deserialize)]
struct ExecOutputJson {
    output: String,
    metadata: ExecOutputMetadataJson,
}

#[derive(Deserialize)]
struct ExecOutputMetadataJson {
    exit_code: i32,
    duration_seconds: f32,
}

fn parse_structured_shell_output(raw: &str) -> Option<String> {
    let parsed: ExecOutputJson = serde_json::from_str(raw).ok()?;
    Some(build_structured_output(&parsed))
}

fn build_structured_output(parsed: &ExecOutputJson) -> String {
    let mut sections = Vec::new();
    sections.push(format!("Exit code: {}", parsed.metadata.exit_code));
    sections.push(format!(
        "Wall time: {} seconds",
        parsed.metadata.duration_seconds
    ));

    let mut output = parsed.output.clone();
    if let Some(total_lines) = extract_total_output_lines(&parsed.output) {
        sections.push(format!("Total output lines: {total_lines}"));
        if let Some(stripped) = strip_total_output_header(&output) {
            output = stripped.to_string();
        }
    }

    sections.push("Output:".to_string());
    sections.push(output);

    sections.join("\n")
}

fn extract_total_output_lines(output: &str) -> Option<u32> {
    let marker_start = output.find("[... omitted ")?;
    let marker = &output[marker_start..];
    let (_, after_of) = marker.split_once(" of ")?;
    let (total_segment, _) = after_of.split_once(' ')?;
    total_segment.parse::<u32>().ok()
}

fn strip_total_output_header(output: &str) -> Option<&str> {
    let after_prefix = output.strip_prefix("Total output lines: ")?;
    let (_, remainder) = after_prefix.split_once('\n')?;
    let remainder = remainder.strip_prefix('\n').unwrap_or(remainder);
    Some(remainder)
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
    ReasoningSummaryPartAdded,
    WebSearchCallBegin {
        call_id: String,
    },
    RateLimits(RateLimitSnapshot),
}

#[derive(Debug, Serialize)]
pub(crate) struct Reasoning {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) effort: Option<ReasoningEffortConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) summary: Option<ReasoningSummaryConfig>,
}

#[derive(Debug, Serialize, Default, Clone)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TextFormatType {
    #[default]
    JsonSchema,
}

#[derive(Debug, Serialize, Default, Clone)]
pub(crate) struct TextFormat {
    pub(crate) r#type: TextFormatType,
    pub(crate) strict: bool,
    pub(crate) schema: Value,
    pub(crate) name: String,
}

/// Controls under the `text` field in the Responses API for GPT-5.
#[derive(Debug, Serialize, Default, Clone)]
pub(crate) struct TextControls {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) verbosity: Option<OpenAiVerbosity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) format: Option<TextFormat>,
}

#[derive(Debug, Serialize, Default, Clone)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OpenAiVerbosity {
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
    pub(crate) store: bool,
    pub(crate) stream: bool,
    pub(crate) include: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) prompt_cache_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) text: Option<TextControls>,
}

pub(crate) mod tools {
    use crate::openai_tools::JsonSchema;
    use serde::Deserialize;
    use serde::Serialize;

    /// When serialized as JSON, this produces a valid "Tool" in the OpenAI
    /// Responses API.
    #[derive(Debug, Clone, Serialize, PartialEq)]
    #[serde(tag = "type")]
    pub(crate) enum ToolSpec {
        #[serde(rename = "function")]
        Function(ResponsesApiTool),
        #[serde(rename = "local_shell")]
        LocalShell {},
        // TODO: Understand why we get an error on web_search although the API docs say it's supported.
        // https://platform.openai.com/docs/guides/tools-web-search?api-mode=responses#:~:text=%7B%20type%3A%20%22web_search%22%20%7D%2C
        #[serde(rename = "web_search")]
        WebSearch {},
        #[serde(rename = "custom")]
        Freeform(FreeformTool),
    }

    impl ToolSpec {
        pub(crate) fn name(&self) -> &str {
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
        pub(crate) name: String,
        pub(crate) description: String,
        pub(crate) format: FreeformToolFormat,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct FreeformToolFormat {
        pub(crate) r#type: String,
        pub(crate) syntax: String,
        pub(crate) definition: String,
    }

    #[derive(Debug, Clone, Serialize, PartialEq)]
    pub struct ResponsesApiTool {
        pub(crate) name: String,
        pub(crate) description: String,
        /// TODO: Validation. When strict is set to true, the JSON schema,
        /// `required` and `additional_properties` must be present. All fields in
        /// `properties` must be present in `required`.
        pub(crate) strict: bool,
        pub(crate) parameters: JsonSchema,
    }
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

pub(crate) fn create_text_param_for_request(
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
            parallel_tool_calls: true,
            reasoning: None,
            store: false,
            stream: true,
            include: vec![],
            prompt_cache_key: None,
            text: Some(TextControls {
                verbosity: Some(OpenAiVerbosity::Low),
                format: None,
            }),
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
        let text_controls =
            create_text_param_for_request(None, &Some(schema.clone())).expect("text controls");

        let req = ResponsesApiRequest {
            model: "gpt-5",
            instructions: "i",
            input: &input,
            tools: &tools,
            tool_choice: "auto",
            parallel_tool_calls: true,
            reasoning: None,
            store: false,
            stream: true,
            include: vec![],
            prompt_cache_key: None,
            text: Some(text_controls),
        };

        let v = serde_json::to_value(&req).expect("json");
        let text = v.get("text").expect("text field");
        assert!(text.get("verbosity").is_none());
        let format = text.get("format").expect("format field");

        assert_eq!(
            format.get("name"),
            Some(&serde_json::Value::String("codex_output_schema".into()))
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
            parallel_tool_calls: true,
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

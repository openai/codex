use crate::client_common::tools::ToolSpec;
use crate::config::types::Personality;
use crate::error::Result;
pub use codex_api::common::ResponseEvent;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::ResponseItem;
use futures::Stream;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::sync::mpsc;

/// Review thread system prompt. Edit `core/src/review_prompt.md` to customize.
pub const REVIEW_PROMPT: &str = include_str!("../review_prompt.md");

// Centralized templates for review-related user messages
pub const REVIEW_EXIT_SUCCESS_TMPL: &str = include_str!("../templates/review/exit_success.xml");
pub const REVIEW_EXIT_INTERRUPTED_TMPL: &str =
    include_str!("../templates/review/exit_interrupted.xml");

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

    pub base_instructions: BaseInstructions,

    /// Optionally specify the personality of the model.
    pub personality: Option<Personality>,

    /// Optional the output schema for the model's response.
    pub output_schema: Option<Value>,
}

impl Prompt {
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
        ResponseItem::FunctionCall { name, call_id, .. }
            if is_shell_tool_name(name) || name == "apply_patch" =>
        {
            shell_call_ids.insert(call_id.clone());
        }
        ResponseItem::FunctionCallOutput {
            call_id, output, ..
        }
        | ResponseItem::CustomToolCallOutput {
            call_id, output, ..
        } => {
            if shell_call_ids.remove(call_id)
                && let Some(structured) = output
                    .text_content()
                    .and_then(parse_structured_shell_output)
            {
                output.body = FunctionCallOutputBody::Text(structured);
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
    if let Some((stripped, total_lines)) = strip_total_output_header(&parsed.output) {
        sections.push(format!("Total output lines: {total_lines}"));
        output = stripped.to_string();
    }

    sections.push("Output:".to_string());
    sections.push(output);

    sections.join("\n")
}

fn strip_total_output_header(output: &str) -> Option<(&str, u32)> {
    let after_prefix = output.strip_prefix("Total output lines: ")?;
    let (total_segment, remainder) = after_prefix.split_once('\n')?;
    let total_lines = total_segment.parse::<u32>().ok()?;
    let remainder = remainder.strip_prefix('\n').unwrap_or(remainder);
    Some((remainder, total_lines))
}

pub(crate) mod tools {
    use crate::tools::spec::JsonSchema;
    use codex_protocol::config_types::WebSearchContextSize;
    use codex_protocol::config_types::WebSearchFilters as ConfigWebSearchFilters;
    use codex_protocol::config_types::WebSearchUserLocation as ConfigWebSearchUserLocation;
    use codex_protocol::config_types::WebSearchUserLocationType;
    use serde::Deserialize;
    use serde::Serialize;
    use serde_json::Value;

    /// When serialized as JSON, this produces a valid "Tool" in the OpenAI
    /// Responses API.
    #[derive(Debug, Clone, Serialize, PartialEq)]
    #[serde(tag = "type")]
    pub(crate) enum ToolSpec {
        #[serde(rename = "function")]
        Function(ResponsesApiTool),
        #[serde(rename = "tool_search")]
        ToolSearch {
            execution: String,
            description: String,
            parameters: JsonSchema,
        },
        #[serde(rename = "local_shell")]
        LocalShell {},
        #[serde(rename = "image_generation")]
        ImageGeneration { output_format: String },
        // TODO: Understand why we get an error on web_search although the API docs say it's supported.
        // https://platform.openai.com/docs/guides/tools-web-search?api-mode=responses#:~:text=%7B%20type%3A%20%22web_search%22%20%7D%2C
        // The `external_web_access` field determines whether the web search is over cached or live content.
        // https://platform.openai.com/docs/guides/tools-web-search#live-internet-access
        #[serde(rename = "web_search")]
        WebSearch {
            #[serde(skip_serializing_if = "Option::is_none")]
            external_web_access: Option<bool>,
            #[serde(skip_serializing_if = "Option::is_none")]
            filters: Option<ResponsesApiWebSearchFilters>,
            #[serde(skip_serializing_if = "Option::is_none")]
            user_location: Option<ResponsesApiWebSearchUserLocation>,
            #[serde(skip_serializing_if = "Option::is_none")]
            search_context_size: Option<WebSearchContextSize>,
            #[serde(skip_serializing_if = "Option::is_none")]
            search_content_types: Option<Vec<String>>,
        },
        #[serde(rename = "custom")]
        Freeform(FreeformTool),
    }

    impl ToolSpec {
        pub(crate) fn name(&self) -> &str {
            match self {
                ToolSpec::Function(tool) => tool.name.as_str(),
                ToolSpec::ToolSearch { .. } => "tool_search_tool",
                ToolSpec::LocalShell {} => "local_shell",
                ToolSpec::ImageGeneration { .. } => "image_generation",
                ToolSpec::WebSearch { .. } => "web_search",
                ToolSpec::Freeform(tool) => tool.name.as_str(),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, PartialEq)]
    pub(crate) struct ResponsesApiWebSearchFilters {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub(crate) allowed_domains: Option<Vec<String>>,
    }

    impl From<ConfigWebSearchFilters> for ResponsesApiWebSearchFilters {
        fn from(filters: ConfigWebSearchFilters) -> Self {
            Self {
                allowed_domains: filters.allowed_domains,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, PartialEq)]
    pub(crate) struct ResponsesApiWebSearchUserLocation {
        #[serde(rename = "type")]
        pub(crate) r#type: WebSearchUserLocationType,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub(crate) country: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub(crate) region: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub(crate) city: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub(crate) timezone: Option<String>,
    }

    impl From<ConfigWebSearchUserLocation> for ResponsesApiWebSearchUserLocation {
        fn from(user_location: ConfigWebSearchUserLocation) -> Self {
            Self {
                r#type: user_location.r#type,
                country: user_location.country,
                region: user_location.region,
                city: user_location.city,
                timezone: user_location.timezone,
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
        #[serde(skip_serializing_if = "Option::is_none")]
        pub(crate) defer_loading: Option<bool>,
        pub(crate) parameters: JsonSchema,
        #[serde(skip)]
        pub(crate) output_schema: Option<Value>,
    }

    #[derive(Debug, Clone, Serialize, PartialEq)]
    #[serde(tag = "type")]
    pub(crate) enum ToolSearchOutputTool {
        #[allow(dead_code)]
        #[serde(rename = "function")]
        Function(ResponsesApiTool),
        #[serde(rename = "namespace")]
        Namespace(ResponsesApiNamespace),
    }

    #[derive(Debug, Clone, Serialize, PartialEq)]
    pub(crate) struct ResponsesApiNamespace {
        pub(crate) name: String,
        pub(crate) description: String,
        pub(crate) tools: Vec<ResponsesApiNamespaceTool>,
    }

    #[derive(Debug, Clone, Serialize, PartialEq)]
    #[serde(tag = "type")]
    pub(crate) enum ResponsesApiNamespaceTool {
        #[serde(rename = "function")]
        Function(ResponsesApiTool),
    }
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
#[path = "client_common_tests.rs"]
mod tests;

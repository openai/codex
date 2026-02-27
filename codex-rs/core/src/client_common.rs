use crate::client_common::tools::ToolSpec;
use crate::config::types::Personality;
use crate::error::CodexErr;
use crate::error::Result;
use base64::Engine as _;
use base64::prelude::BASE64_STANDARD;
pub use codex_api::common::ResponseEvent;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
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

// See the Responses API image input size limits in the Images and Vision guide:
// https://platform.openai.com/docs/guides/images-vision?api-mode=responses&format=file
const RESPONSES_API_MAX_INLINE_IMAGE_BYTES: usize = 50_000_000;
const RESPONSES_API_MAX_INLINE_IMAGE_BYTES_LABEL: &str = "50 MB";
const INLINE_TOOL_IMAGE_OMITTED_PLACEHOLDER: &str = "Codex omitted this tool-returned image because the current request would exceed the Responses API 50 MB total image limit. Request fewer images at a time or inspect them in smaller batches.";

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
    pub(crate) fn get_formatted_input(&self) -> Result<Vec<ResponseItem>> {
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

        enforce_inline_image_request_budget(&mut input, RESPONSES_API_MAX_INLINE_IMAGE_BYTES)?;

        Ok(input)
    }
}

fn enforce_inline_image_request_budget(
    items: &mut [ResponseItem],
    max_inline_image_bytes: usize,
) -> Result<()> {
    let mut inline_image_bytes = total_inline_image_bytes(items);
    let mut omitted_model_generated_image = false;

    if inline_image_bytes <= max_inline_image_bytes {
        return Ok(());
    }

    for item in items.iter_mut().rev() {
        if inline_image_bytes <= max_inline_image_bytes {
            return Ok(());
        }

        let Some(content_items) = tool_output_content_items_mut(item) else {
            continue;
        };

        for content_item in content_items.iter_mut().rev() {
            if inline_image_bytes <= max_inline_image_bytes {
                return Ok(());
            }

            let FunctionCallOutputContentItem::InputImage { image_url, .. } = content_item else {
                continue;
            };
            let Some(image_bytes) = inline_image_data_url_bytes(image_url) else {
                continue;
            };

            *content_item = FunctionCallOutputContentItem::InputText {
                text: INLINE_TOOL_IMAGE_OMITTED_PLACEHOLDER.to_string(),
            };
            inline_image_bytes = inline_image_bytes.saturating_sub(image_bytes);
            omitted_model_generated_image = true;
        }
    }

    Err(CodexErr::InvalidRequest(
        inline_image_request_budget_exceeded_message(
            inline_image_bytes,
            max_inline_image_bytes,
            omitted_model_generated_image,
        ),
    ))
}

fn total_inline_image_bytes(items: &[ResponseItem]) -> usize {
    items
        .iter()
        .map(response_item_inline_image_bytes)
        .sum::<usize>()
}

fn response_item_inline_image_bytes(item: &ResponseItem) -> usize {
    match item {
        ResponseItem::Message { content, .. } => content
            .iter()
            .filter_map(|content_item| match content_item {
                ContentItem::InputImage { image_url } => inline_image_data_url_bytes(image_url),
                ContentItem::InputText { .. } | ContentItem::OutputText { .. } => None,
            })
            .sum::<usize>(),
        ResponseItem::FunctionCallOutput { output, .. }
        | ResponseItem::CustomToolCallOutput { output, .. } => output
            .content_items()
            .map(|content_items| {
                content_items
                    .iter()
                    .filter_map(|content_item| match content_item {
                        FunctionCallOutputContentItem::InputImage { image_url, .. } => {
                            inline_image_data_url_bytes(image_url)
                        }
                        FunctionCallOutputContentItem::InputText { .. } => None,
                    })
                    .sum::<usize>()
            })
            .unwrap_or_default(),
        _ => 0,
    }
}

fn tool_output_content_items_mut(
    item: &mut ResponseItem,
) -> Option<&mut Vec<FunctionCallOutputContentItem>> {
    match item {
        ResponseItem::FunctionCallOutput { output, .. }
        | ResponseItem::CustomToolCallOutput { output, .. } => output.content_items_mut(),
        _ => None,
    }
}

fn inline_image_data_url_bytes(url: &str) -> Option<usize> {
    let payload = parse_base64_image_data_url(url)?;
    Some(BASE64_STANDARD.decode(payload).ok()?.len())
}

fn parse_base64_image_data_url(url: &str) -> Option<&str> {
    if !url
        .get(.."data:".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
    {
        return None;
    }
    let comma_index = url.find(',')?;
    let metadata = &url[..comma_index];
    let payload = &url[comma_index + 1..];
    let metadata_without_scheme = &metadata["data:".len()..];
    let mut metadata_parts = metadata_without_scheme.split(';');
    let mime_type = metadata_parts.next().unwrap_or_default();
    let has_base64_marker = metadata_parts.any(|part| part.eq_ignore_ascii_case("base64"));
    if !mime_type
        .get(.."image/".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("image/"))
    {
        return None;
    }
    if !has_base64_marker {
        return None;
    }
    Some(payload)
}

fn inline_image_request_budget_exceeded_message(
    inline_image_bytes: usize,
    max_inline_image_bytes: usize,
    omitted_model_generated_image: bool,
) -> String {
    let limit_label = if max_inline_image_bytes == RESPONSES_API_MAX_INLINE_IMAGE_BYTES {
        RESPONSES_API_MAX_INLINE_IMAGE_BYTES_LABEL.to_string()
    } else {
        format!("{max_inline_image_bytes} bytes")
    };

    if omitted_model_generated_image {
        format!(
            "Codex could not send this turn because inline images still total {inline_image_bytes} bytes after omitting all model-generated tool images, exceeding the Responses API {limit_label} total image limit for a single request. Remove some attached images or start a new thread without earlier image attachments."
        )
    } else {
        format!(
            "Codex could not send this turn because inline images total {inline_image_bytes} bytes, exceeding the Responses API {limit_label} total image limit for a single request. Remove some attached images or start a new thread without earlier image attachments."
        )
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
        ResponseItem::FunctionCallOutput { call_id, output }
        | ResponseItem::CustomToolCallOutput { call_id, output } => {
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
        // The `external_web_access` field determines whether the web search is over cached or live content.
        // https://platform.openai.com/docs/guides/tools-web-search#live-internet-access
        #[serde(rename = "web_search")]
        WebSearch {
            #[serde(skip_serializing_if = "Option::is_none")]
            external_web_access: Option<bool>,
        },
        #[serde(rename = "custom")]
        Freeform(FreeformTool),
    }

    impl ToolSpec {
        pub(crate) fn name(&self) -> &str {
            match self {
                ToolSpec::Function(tool) => tool.name.as_str(),
                ToolSpec::LocalShell {} => "local_shell",
                ToolSpec::WebSearch { .. } => "web_search",
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
    use base64::Engine as _;
    use base64::prelude::BASE64_STANDARD;
    use codex_api::ResponsesApiRequest;
    use codex_api::common::OpenAiVerbosity;
    use codex_api::common::TextControls;
    use codex_api::create_text_param_for_request;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::FunctionCallOutputContentItem;
    use codex_protocol::models::FunctionCallOutputPayload;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn serializes_text_verbosity_when_set() {
        let input: Vec<ResponseItem> = vec![];
        let tools: Vec<serde_json::Value> = vec![];
        let req = ResponsesApiRequest {
            model: "gpt-5.1".to_string(),
            instructions: "i".to_string(),
            input,
            tools,
            tool_choice: "auto".to_string(),
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
            model: "gpt-5.1".to_string(),
            instructions: "i".to_string(),
            input,
            tools,
            tool_choice: "auto".to_string(),
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
            model: "gpt-5.1".to_string(),
            instructions: "i".to_string(),
            input,
            tools,
            tool_choice: "auto".to_string(),
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

    #[test]
    fn reserializes_shell_outputs_for_function_and_custom_tool_calls() {
        let raw_output = r#"{"output":"hello","metadata":{"exit_code":0,"duration_seconds":0.5}}"#;
        let expected_output = "Exit code: 0\nWall time: 0.5 seconds\nOutput:\nhello";
        let mut items = vec![
            ResponseItem::FunctionCall {
                id: None,
                name: "shell".to_string(),
                arguments: "{}".to_string(),
                call_id: "call-1".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call-1".to_string(),
                output: FunctionCallOutputPayload::from_text(raw_output.to_string()),
            },
            ResponseItem::CustomToolCall {
                id: None,
                status: None,
                call_id: "call-2".to_string(),
                name: "apply_patch".to_string(),
                input: "*** Begin Patch".to_string(),
            },
            ResponseItem::CustomToolCallOutput {
                call_id: "call-2".to_string(),
                output: FunctionCallOutputPayload::from_text(raw_output.to_string()),
            },
        ];

        reserialize_shell_outputs(&mut items);

        assert_eq!(
            items,
            vec![
                ResponseItem::FunctionCall {
                    id: None,
                    name: "shell".to_string(),
                    arguments: "{}".to_string(),
                    call_id: "call-1".to_string(),
                },
                ResponseItem::FunctionCallOutput {
                    call_id: "call-1".to_string(),
                    output: FunctionCallOutputPayload::from_text(expected_output.to_string()),
                },
                ResponseItem::CustomToolCall {
                    id: None,
                    status: None,
                    call_id: "call-2".to_string(),
                    name: "apply_patch".to_string(),
                    input: "*** Begin Patch".to_string(),
                },
                ResponseItem::CustomToolCallOutput {
                    call_id: "call-2".to_string(),
                    output: FunctionCallOutputPayload::from_text(expected_output.to_string()),
                },
            ]
        );
    }

    #[test]
    fn rewrites_newest_tool_images_until_request_is_within_budget() {
        let mut items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputImage {
                    image_url: image_data_url(&[1, 2, 3, 4]),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call-1".to_string(),
                output: FunctionCallOutputPayload::from_content_items(vec![
                    FunctionCallOutputContentItem::InputImage {
                        image_url: image_data_url(&[5, 6, 7, 8]),
                        detail: None,
                    },
                ]),
            },
            ResponseItem::CustomToolCallOutput {
                call_id: "call-2".to_string(),
                output: FunctionCallOutputPayload::from_content_items(vec![
                    FunctionCallOutputContentItem::InputImage {
                        image_url: image_data_url(&[9, 10, 11, 12]),
                        detail: None,
                    },
                ]),
            },
        ];

        enforce_inline_image_request_budget(&mut items, 8).expect("request should fit");

        assert_eq!(
            items,
            vec![
                ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![ContentItem::InputImage {
                        image_url: image_data_url(&[1, 2, 3, 4]),
                    }],
                    end_turn: None,
                    phase: None,
                },
                ResponseItem::FunctionCallOutput {
                    call_id: "call-1".to_string(),
                    output: FunctionCallOutputPayload::from_content_items(vec![
                        FunctionCallOutputContentItem::InputImage {
                            image_url: image_data_url(&[5, 6, 7, 8]),
                            detail: None,
                        },
                    ]),
                },
                ResponseItem::CustomToolCallOutput {
                    call_id: "call-2".to_string(),
                    output: FunctionCallOutputPayload::from_content_items(vec![
                        FunctionCallOutputContentItem::InputText {
                            text: INLINE_TOOL_IMAGE_OMITTED_PLACEHOLDER.to_string(),
                        },
                    ]),
                },
            ]
        );
    }

    #[test]
    fn errors_when_user_images_still_exceed_request_budget() {
        let mut items = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputImage {
                image_url: image_data_url(&[1, 2, 3, 4]),
            }],
            end_turn: None,
            phase: None,
        }];

        let err = enforce_inline_image_request_budget(&mut items, 3).expect_err("should fail");

        assert_eq!(
            err.to_string(),
            "Codex could not send this turn because inline images total 4 bytes, exceeding the Responses API 3 bytes total image limit for a single request. Remove some attached images or start a new thread without earlier image attachments."
        );
    }

    #[test]
    fn errors_after_omitting_tool_images_if_user_images_still_exceed_budget() {
        let mut items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputImage {
                    image_url: image_data_url(&[1, 2, 3, 4]),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call-1".to_string(),
                output: FunctionCallOutputPayload::from_content_items(vec![
                    FunctionCallOutputContentItem::InputImage {
                        image_url: image_data_url(&[5, 6, 7, 8]),
                        detail: None,
                    },
                ]),
            },
        ];

        let err = enforce_inline_image_request_budget(&mut items, 3).expect_err("should fail");

        assert_eq!(
            err.to_string(),
            "Codex could not send this turn because inline images still total 4 bytes after omitting all model-generated tool images, exceeding the Responses API 3 bytes total image limit for a single request. Remove some attached images or start a new thread without earlier image attachments."
        );
        assert_eq!(
            items,
            vec![
                ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![ContentItem::InputImage {
                        image_url: image_data_url(&[1, 2, 3, 4]),
                    }],
                    end_turn: None,
                    phase: None,
                },
                ResponseItem::FunctionCallOutput {
                    call_id: "call-1".to_string(),
                    output: FunctionCallOutputPayload::from_content_items(vec![
                        FunctionCallOutputContentItem::InputText {
                            text: INLINE_TOOL_IMAGE_OMITTED_PLACEHOLDER.to_string(),
                        },
                    ]),
                },
            ]
        );
    }

    fn image_data_url(bytes: &[u8]) -> String {
        format!("data:image/png;base64,{}", BASE64_STANDARD.encode(bytes))
    }
}

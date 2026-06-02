use codex_api::ImageBackground;
use codex_api::ImageEditRequest;
use codex_api::ImageGenerationRequest;
use codex_api::ImageQuality;
use codex_api::ImageUrl;
use codex_extension_api::ExtensionTurnItem;
use codex_extension_api::FunctionCallError;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolName;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolPayload;
use codex_extension_api::ToolSpec;
use codex_extension_api::parse_tool_input_schema;
use codex_protocol::items::ImageGenerationItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::DEFAULT_IMAGE_DETAIL;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolExposure;
use codex_tools::default_namespace_description;
use schemars::JsonSchema;
use schemars::r#gen::SchemaSettings;
use serde::Deserialize;
use serde_json::Map;
use serde_json::Value;

use crate::IMAGE_GEN_NAMESPACE;
use crate::IMAGEGEN_TOOL_NAME;
use crate::backend::CodexImagesBackend;

const IMAGE_MODEL: &str = "gpt-image-2";
const MAX_EDIT_IMAGES: usize = 5;
const IMAGEGEN_DESCRIPTION: &str = include_str!("../imagegen_description.md");

#[derive(Clone)]
pub(crate) struct ImageGenerationTool {
    backend: CodexImagesBackend,
}

impl ImageGenerationTool {
    /// Creates an image-generation tool backed by an image API executor.
    pub(crate) fn new(backend: CodexImagesBackend) -> Self {
        Self { backend }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ImagegenArgs {
    prompt: String,
    action: ImagegenAction,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum ImagegenAction {
    Generate,
    Edit,
}

#[async_trait::async_trait]
impl ToolExecutor<ToolCall> for ImageGenerationTool {
    /// Keeps the tool in the existing image-generation Responses namespace.
    fn tool_name(&self) -> ToolName {
        ToolName::namespaced(IMAGE_GEN_NAMESPACE, IMAGEGEN_TOOL_NAME)
    }

    /// Advertises the model contract: a rewritten prompt and semantic action.
    fn spec(&self) -> ToolSpec {
        imagegen_tool_spec()
    }

    /// Exposes image generation directly and through the nested code-mode tool surface.
    fn exposure(&self) -> ToolExposure {
        ToolExposure::Direct
    }

    /// Executes the selected image operation and returns the completed image result.
    async fn handle(&self, call: ToolCall) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let args = parse_args(&call)?;
        let request = request_for_action(&args, call.conversation_history.items())?;
        call.turn_item_emitter
            .emit_started(ExtensionTurnItem::ImageGeneration(ImageGenerationItem {
                id: call.call_id.clone(),
                status: "in_progress".to_string(),
                revised_prompt: None,
                result: String::new(),
                saved_path: None,
            }))
            .await;
        let response = match request {
            ImageRequest::Generate(request) => self.backend.generate(request).await,
            ImageRequest::Edit(request) => self.backend.edit(request).await,
        }
        .map_err(|err| {
            FunctionCallError::RespondToModel(format!("image generation failed: {err}"))
        })?;
        let Some(result) = response.data.into_iter().next().map(|data| data.b64_json) else {
            return Err(FunctionCallError::RespondToModel(
                "image generation returned no image data".to_string(),
            ));
        };
        call.turn_item_emitter
            .emit_completed(ExtensionTurnItem::ImageGeneration(ImageGenerationItem {
                id: call.call_id.clone(),
                status: "completed".to_string(),
                revised_prompt: Some(args.prompt),
                result: result.clone(),
                saved_path: None,
            }))
            .await;
        Ok(Box::new(GeneratedImageOutput { result }))
    }
}

#[derive(Debug, PartialEq)]
enum ImageRequest {
    Generate(ImageGenerationRequest),
    Edit(ImageEditRequest),
}

/// Maps the model-selected action to the fixed image API request parameters.
fn request_for_action(
    args: &ImagegenArgs,
    history: &[ResponseItem],
) -> Result<ImageRequest, FunctionCallError> {
    match args.action {
        ImagegenAction::Generate => Ok(ImageRequest::Generate(ImageGenerationRequest {
            prompt: args.prompt.clone(),
            background: Some(ImageBackground::Auto),
            model: IMAGE_MODEL.to_string(),
            n: None,
            quality: Some(ImageQuality::Auto),
            size: Some("auto".to_string()),
        })),
        ImagegenAction::Edit => {
            let images = edit_images(history);
            if images.is_empty() {
                return Err(FunctionCallError::RespondToModel(
                    "image edit requested without any usable image in conversation history"
                        .to_string(),
                ));
            }
            Ok(ImageRequest::Edit(ImageEditRequest {
                images,
                prompt: args.prompt.clone(),
                background: Some(ImageBackground::Auto),
                model: IMAGE_MODEL.to_string(),
                n: None,
                quality: Some(ImageQuality::Auto),
                size: Some("auto".to_string()),
            }))
        }
    }
}

/// Selects edit context using the hosted imagegen anchor and truncation behavior.
fn edit_images(history: &[ResponseItem]) -> Vec<ImageUrl> {
    let latest_uploaded_images = history.iter().enumerate().rev().find_map(|(index, item)| {
        let ResponseItem::Message { role, content, .. } = item else {
            return None;
        };
        if role != "user" {
            return None;
        }
        let images = content
            .iter()
            .filter_map(|item| match item {
                ContentItem::InputImage { image_url, .. } => Some(ImageUrl {
                    image_url: image_url.clone(),
                }),
                ContentItem::InputText { .. } | ContentItem::OutputText { .. } => None,
            })
            .collect::<Vec<_>>();
        (!images.is_empty()).then_some((index, images))
    });
    let (user_images, follow_up_start) = latest_uploaded_images
        .map_or_else(|| (Vec::new(), 0), |(index, images)| (images, index + 1));
    let mut generated_images = Vec::new();
    for item in &history[follow_up_start..] {
        match item {
            ResponseItem::ImageGenerationCall { result, .. } if !result.is_empty() => {
                generated_images.push(ImageUrl {
                    image_url: format!("data:image/png;base64,{result}"),
                });
            }
            ResponseItem::FunctionCallOutput { call_id, output }
                if history.iter().any(|item| {
                    matches!(
                        item,
                        ResponseItem::FunctionCall {
                            name,
                            namespace: Some(namespace),
                            call_id: function_call_id,
                            ..
                        } if function_call_id == call_id
                            && name == IMAGEGEN_TOOL_NAME
                            && namespace == IMAGE_GEN_NAMESPACE
                    )
                }) =>
            {
                generated_images.extend(output.content_items().into_iter().flatten().filter_map(
                    |item| match item {
                        FunctionCallOutputContentItem::InputImage { image_url, .. } => {
                            Some(ImageUrl {
                                image_url: image_url.clone(),
                            })
                        }
                        FunctionCallOutputContentItem::InputText { .. }
                        | FunctionCallOutputContentItem::EncryptedContent { .. } => None,
                    },
                ));
            }
            ResponseItem::Message { .. }
            | ResponseItem::Reasoning { .. }
            | ResponseItem::LocalShellCall { .. }
            | ResponseItem::FunctionCall { .. }
            | ResponseItem::ToolSearchCall { .. }
            | ResponseItem::FunctionCallOutput { .. }
            | ResponseItem::CustomToolCall { .. }
            | ResponseItem::CustomToolCallOutput { .. }
            | ResponseItem::ToolSearchOutput { .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::ImageGenerationCall { .. }
            | ResponseItem::Compaction { .. }
            | ResponseItem::CompactionTrigger
            | ResponseItem::ContextCompaction { .. }
            | ResponseItem::Other => {}
        }
    }
    truncate_images(user_images, generated_images)
}

/// Truncates edit inputs while preserving the newest generated image when possible.
fn truncate_images(
    mut user_images: Vec<ImageUrl>,
    mut generated_images: Vec<ImageUrl>,
) -> Vec<ImageUrl> {
    let mut excess = (user_images.len() + generated_images.len()).saturating_sub(MAX_EDIT_IMAGES);
    let drop_generated = excess.min(generated_images.len().saturating_sub(1));
    generated_images.drain(..drop_generated);
    excess -= drop_generated;
    let drop_user = excess.min(user_images.len());
    user_images.drain(..drop_user);
    excess -= drop_user;
    generated_images.drain(..excess);

    user_images.extend(generated_images);
    user_images
}

/// Parses the strict model-facing arguments for an image-generation call.
fn parse_args(call: &ToolCall) -> Result<ImagegenArgs, FunctionCallError> {
    serde_json::from_str(call.function_arguments()?)
        .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))
}

/// Builds the namespace function schema exposed to the model.
fn imagegen_tool_spec() -> ToolSpec {
    let mut schema_value = serde_json::to_value(
        SchemaSettings::draft2019_09()
            .with(|settings| settings.inline_subschemas = true)
            .into_generator()
            .into_root_schema_for::<ImagegenArgs>(),
    )
    .unwrap_or_else(|err| panic!("imagegen schema should serialize: {err}"));
    let Value::Object(ref mut schema) = schema_value else {
        unreachable!("imagegen root schema must be an object");
    };
    let mut input_schema = Map::new();
    for key in ["properties", "required", "type", "additionalProperties"] {
        if let Some(value) = schema.remove(key) {
            input_schema.insert(key.to_string(), value);
        }
    }
    ToolSpec::Namespace(ResponsesApiNamespace {
        name: IMAGE_GEN_NAMESPACE.to_string(),
        description: default_namespace_description(IMAGE_GEN_NAMESPACE),
        tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
            name: IMAGEGEN_TOOL_NAME.to_string(),
            description: IMAGEGEN_DESCRIPTION.to_string(),
            strict: false,
            parameters: parse_tool_input_schema(&Value::Object(input_schema))
                .unwrap_or_else(|err| panic!("imagegen input schema should parse: {err}")),
            output_schema: None,
            defer_loading: None,
        })],
    })
}

struct GeneratedImageOutput {
    result: String,
}

impl ToolOutput for GeneratedImageOutput {
    /// Avoids copying image bytes into tool-call telemetry.
    fn log_preview(&self) -> String {
        "[generated image]".to_string()
    }

    /// Reports a completed images request as successful tool execution.
    fn success_for_logging(&self) -> bool {
        true
    }

    /// Returns generated bytes for model follow-up.
    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        let content = vec![FunctionCallOutputContentItem::InputImage {
            image_url: format!("data:image/png;base64,{}", self.result),
            detail: Some(DEFAULT_IMAGE_DETAIL),
        }];
        ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output: FunctionCallOutputPayload {
                body: FunctionCallOutputBody::ContentItems(content),
                success: Some(true),
            },
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

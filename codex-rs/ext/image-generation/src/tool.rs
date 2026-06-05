use std::collections::HashSet;

use codex_api::ImageBackground;
use codex_api::ImageEditRequest;
use codex_api::ImageGenerationRequest;
use codex_api::ImageQuality;
use codex_api::ImageUrl;
use codex_core::context::extension_image_generation_output_hint;
use codex_core::image_generation_artifact_path;
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
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_image::PromptImageMode;
use codex_utils_image::load_for_prompt_bytes;
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
    codex_home: AbsolutePathBuf,
    thread_id: String,
}

impl ImageGenerationTool {
    /// Creates an image-generation tool backed by an image API executor.
    pub(crate) fn new(
        backend: CodexImagesBackend,
        codex_home: AbsolutePathBuf,
        thread_id: String,
    ) -> Self {
        Self {
            backend,
            codex_home,
            thread_id,
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ImagegenArgs {
    prompt: String,
    #[schemars(length(max = 5))]
    referenced_image_paths: Option<Vec<AbsolutePathBuf>>,
    #[schemars(range(min = 1, max = 5))]
    num_last_images_to_include: Option<usize>,
}

#[async_trait::async_trait]
impl ToolExecutor<ToolCall> for ImageGenerationTool {
    /// Keeps the tool in the existing image-generation Responses namespace.
    fn tool_name(&self) -> ToolName {
        ToolName::namespaced(IMAGE_GEN_NAMESPACE, IMAGEGEN_TOOL_NAME)
    }

    /// Advertises the model contract: a rewritten prompt and optional edit references.
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
        let request = request_for_args(&args, call.conversation_history.items())?;
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
        let output_path =
            image_generation_artifact_path(&self.codex_home, &self.thread_id, &call.call_id);
        let output_dir = output_path
            .parent()
            .unwrap_or_else(|| self.codex_home.clone());
        let output_hint =
            extension_image_generation_output_hint(output_dir.display(), output_path.display());
        Ok(Box::new(GeneratedImageOutput {
            result,
            output_hint,
        }))
    }
}

#[derive(Debug, PartialEq)]
enum ImageRequest {
    Generate(ImageGenerationRequest),
    Edit(ImageEditRequest),
}

/// Builds a generation or edit request from the mutually exclusive image selectors.
fn request_for_args(
    args: &ImagegenArgs,
    history: &[ResponseItem],
) -> Result<ImageRequest, FunctionCallError> {
    let paths = args.referenced_image_paths.as_deref().unwrap_or_default();
    if paths.len() > MAX_EDIT_IMAGES {
        return Err(FunctionCallError::RespondToModel(format!(
            "`referenced_image_paths` must contain at most {MAX_EDIT_IMAGES} paths"
        )));
    }
    let images = match (paths.is_empty(), args.num_last_images_to_include) {
        (true, None) => {
            return Ok(ImageRequest::Generate(ImageGenerationRequest {
                prompt: args.prompt.clone(),
                background: Some(ImageBackground::Auto),
                model: IMAGE_MODEL.to_string(),
                n: None,
                quality: Some(ImageQuality::Auto),
                size: Some("auto".to_string()),
            }));
        }
        (false, None) => paths.iter().map(image_url).collect::<Result<Vec<_>, _>>()?,
        (true, Some(count)) => {
            if !(1..=MAX_EDIT_IMAGES).contains(&count) {
                return Err(FunctionCallError::RespondToModel(format!(
                    "`num_last_images_to_include` must be between 1 and {MAX_EDIT_IMAGES}"
                )));
            }
            let images = recent_images(history, count);
            if images.len() != count {
                return Err(FunctionCallError::RespondToModel(format!(
                    "requested the last {count} conversation images, but only {} were available",
                    images.len()
                )));
            }
            images
        }
        (false, Some(_)) => {
            return Err(FunctionCallError::RespondToModel(
                "provide only one of `referenced_image_paths` or \
                 `num_last_images_to_include`"
                    .to_string(),
            ));
        }
    };

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

/// Selects the newest requested images while preserving their conversation order.
fn recent_images(history: &[ResponseItem], count: usize) -> Vec<ImageUrl> {
    let mut function_call_ids = HashSet::new();
    let mut custom_tool_call_ids = HashSet::new();
    for item in history {
        match item {
            ResponseItem::FunctionCall { call_id, .. } => {
                function_call_ids.insert(call_id.as_str());
            }
            ResponseItem::CustomToolCall { call_id, .. } => {
                custom_tool_call_ids.insert(call_id.as_str());
            }
            ResponseItem::Message { .. }
            | ResponseItem::Reasoning { .. }
            | ResponseItem::LocalShellCall { .. }
            | ResponseItem::ToolSearchCall { .. }
            | ResponseItem::FunctionCallOutput { .. }
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

    let mut images = Vec::with_capacity(count);
    'history: for item in history.iter().rev() {
        let mut image_urls = Vec::new();
        match item {
            ResponseItem::Message { content, .. } => {
                image_urls.extend(content.iter().rev().filter_map(|item| match item {
                    ContentItem::InputImage { image_url, .. } => Some(image_url.clone()),
                    ContentItem::InputText { .. } | ContentItem::OutputText { .. } => None,
                }));
            }
            ResponseItem::FunctionCallOutput { call_id, output }
                if function_call_ids.contains(call_id.as_str()) =>
            {
                image_urls.extend(output_image_urls(output));
            }
            ResponseItem::CustomToolCallOutput {
                call_id, output, ..
            } if custom_tool_call_ids.contains(call_id.as_str()) => {
                image_urls.extend(output_image_urls(output));
            }
            ResponseItem::ImageGenerationCall { result, .. } if !result.is_empty() => {
                image_urls.push(format!("data:image/png;base64,{result}"));
            }
            ResponseItem::Reasoning { .. }
            | ResponseItem::LocalShellCall { .. }
            | ResponseItem::FunctionCall { .. }
            | ResponseItem::ToolSearchCall { .. }
            | ResponseItem::CustomToolCall { .. }
            | ResponseItem::FunctionCallOutput { .. }
            | ResponseItem::CustomToolCallOutput { .. }
            | ResponseItem::ToolSearchOutput { .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::ImageGenerationCall { .. }
            | ResponseItem::Compaction { .. }
            | ResponseItem::CompactionTrigger
            | ResponseItem::ContextCompaction { .. }
            | ResponseItem::Other => {}
        }
        for image_url in image_urls {
            images.push(ImageUrl { image_url });
            if images.len() == count {
                break 'history;
            }
        }
    }
    images.reverse();
    images
}

/// Extracts image URLs from a tool output in newest-first order.
fn output_image_urls(output: &FunctionCallOutputPayload) -> impl Iterator<Item = String> + '_ {
    output
        .content_items()
        .into_iter()
        .flatten()
        .rev()
        .filter_map(|item| match item {
            FunctionCallOutputContentItem::InputImage { image_url, .. } => Some(image_url.clone()),
            FunctionCallOutputContentItem::InputText { .. }
            | FunctionCallOutputContentItem::EncryptedContent { .. } => None,
        })
}

fn image_url(path: &AbsolutePathBuf) -> Result<ImageUrl, FunctionCallError> {
    let bytes = std::fs::read(path).map_err(|error| {
        FunctionCallError::RespondToModel(format!(
            "unable to read referenced image at `{}`: {error}",
            path.display()
        ))
    })?;
    let image = load_for_prompt_bytes(path.as_path(), bytes, PromptImageMode::Original).map_err(
        |error| {
            FunctionCallError::RespondToModel(format!(
                "unable to process referenced image at `{}`: {error}",
                path.display()
            ))
        },
    )?;
    Ok(ImageUrl {
        image_url: image.into_data_url(),
    })
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
    output_hint: Option<String>,
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

    /// Returns the object consumed by the code-mode `generatedImage()` helper.
    fn code_mode_result(&self, _payload: &ToolPayload) -> Value {
        let mut result = Map::from_iter([(
            "image_url".to_string(),
            Value::String(format!("data:image/png;base64,{}", self.result)),
        )]);
        if let Some(output_hint) = &self.output_hint {
            result.insert(
                "output_hint".to_string(),
                Value::String(output_hint.clone()),
            );
        }
        Value::Object(result)
    }

    /// Returns generated bytes and persisted-artifact context for model follow-up.
    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        let mut content = vec![FunctionCallOutputContentItem::InputImage {
            image_url: format!("data:image/png;base64,{}", self.result),
            detail: Some(DEFAULT_IMAGE_DETAIL),
        }];
        if let Some(output_hint) = &self.output_hint {
            content.push(FunctionCallOutputContentItem::InputText {
                text: output_hint.clone(),
            });
        }
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

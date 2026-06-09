use std::borrow::Cow;
use std::path::Path;
use std::sync::LazyLock;

use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ResponseItem;
use codex_utils_image::ImageProcessingError;
use codex_utils_image::PromptImageMode;
use codex_utils_image::load_for_prompt_bytes;
use reqwest::Client;
use thiserror::Error;
use tracing::warn;
use url::Url;

const IMAGE_MATERIALIZATION_ERROR_PLACEHOLDER: &str =
    "image content omitted because it could not be downloaded or processed";

static IMAGE_URL_CLIENT: LazyLock<Client> = LazyLock::new(Client::new);

#[derive(Debug, Error)]
pub(crate) enum ImageUrlMaterializationError {
    #[error("failed to download image: {0}")]
    Download(#[from] reqwest::Error),
    #[error("failed to process downloaded image: {0}")]
    Processing(#[from] ImageProcessingError),
}

/// Downloads an HTTP(S) image and returns it as an inline data URL.
///
/// Non-HTTP(S) values, including existing data URLs, are returned unchanged.
pub(crate) async fn materialize_http_image_url<'a>(
    client: &Client,
    image_url: &'a str,
) -> Result<Cow<'a, str>, ImageUrlMaterializationError> {
    let Ok(url) = Url::parse(image_url) else {
        return Ok(Cow::Borrowed(image_url));
    };
    if !matches!(url.scheme(), "http" | "https") {
        return Ok(Cow::Borrowed(image_url));
    }

    let response = client.get(url.as_str()).send().await?.error_for_status()?;
    let bytes = response.bytes().await?;
    let image = load_for_prompt_bytes(
        Path::new(url.path()),
        bytes.to_vec(),
        PromptImageMode::Original,
    )?;

    Ok(Cow::Owned(image.into_data_url()))
}

/// Materializes HTTP(S) image URLs in a newly recorded batch.
///
/// Batches without remote images remain borrowed, avoiding copies of existing
/// inline image data.
pub(crate) async fn materialize_conversation_item_images<'a>(
    items: &'a [ResponseItem],
) -> Cow<'a, [ResponseItem]> {
    if !items.iter().any(response_item_has_http_image) {
        return Cow::Borrowed(items);
    }

    let mut items = items.to_vec();
    for item in &mut items {
        match item {
            ResponseItem::Message { content, .. } => materialize_content_items(content).await,
            ResponseItem::FunctionCallOutput { output, .. }
            | ResponseItem::CustomToolCallOutput { output, .. } => {
                if let Some(content_items) = output.content_items_mut() {
                    materialize_function_call_output_items(content_items).await;
                }
            }
            ResponseItem::Reasoning { .. }
            | ResponseItem::AgentMessage { .. }
            | ResponseItem::LocalShellCall { .. }
            | ResponseItem::FunctionCall { .. }
            | ResponseItem::ToolSearchCall { .. }
            | ResponseItem::CustomToolCall { .. }
            | ResponseItem::ToolSearchOutput { .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::ImageGenerationCall { .. }
            | ResponseItem::Compaction { .. }
            | ResponseItem::CompactionTrigger
            | ResponseItem::ContextCompaction { .. }
            | ResponseItem::Other => {}
        }
    }

    Cow::Owned(items)
}

fn response_item_has_http_image(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { content, .. } => content.iter().any(|item| match item {
            ContentItem::InputImage { image_url, .. } => is_http_url(image_url),
            ContentItem::InputText { .. } | ContentItem::OutputText { .. } => false,
        }),
        ResponseItem::FunctionCallOutput { output, .. }
        | ResponseItem::CustomToolCallOutput { output, .. } => output
            .content_items()
            .is_some_and(|items| items.iter().any(function_call_output_item_has_http_image)),
        ResponseItem::Reasoning { .. }
        | ResponseItem::AgentMessage { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::ToolSearchCall { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::ToolSearchOutput { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::Compaction { .. }
        | ResponseItem::CompactionTrigger
        | ResponseItem::ContextCompaction { .. }
        | ResponseItem::Other => false,
    }
}

fn function_call_output_item_has_http_image(item: &FunctionCallOutputContentItem) -> bool {
    match item {
        FunctionCallOutputContentItem::InputImage { image_url, .. } => is_http_url(image_url),
        FunctionCallOutputContentItem::InputText { .. }
        | FunctionCallOutputContentItem::EncryptedContent { .. } => false,
    }
}

fn is_http_url(value: &str) -> bool {
    Url::parse(value).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
}

async fn materialize_content_items(items: &mut [ContentItem]) {
    for item in items {
        let result = match item {
            ContentItem::InputImage { image_url, .. } => materialize_image_url(image_url).await,
            ContentItem::InputText { .. } | ContentItem::OutputText { .. } => continue,
        };
        if let Err(err) = result {
            warn!(error = %err, "failed to materialize remote message image");
            *item = ContentItem::InputText {
                text: IMAGE_MATERIALIZATION_ERROR_PLACEHOLDER.to_string(),
            };
        }
    }
}

async fn materialize_function_call_output_items(items: &mut [FunctionCallOutputContentItem]) {
    for item in items {
        let result = match item {
            FunctionCallOutputContentItem::InputImage { image_url, .. } => {
                materialize_image_url(image_url).await
            }
            FunctionCallOutputContentItem::InputText { .. }
            | FunctionCallOutputContentItem::EncryptedContent { .. } => continue,
        };
        if let Err(err) = result {
            warn!(error = %err, "failed to materialize remote tool output image");
            *item = FunctionCallOutputContentItem::InputText {
                text: IMAGE_MATERIALIZATION_ERROR_PLACEHOLDER.to_string(),
            };
        }
    }
}

async fn materialize_image_url(image_url: &mut String) -> Result<(), ImageUrlMaterializationError> {
    if let Cow::Owned(materialized_url) =
        materialize_http_image_url(&IMAGE_URL_CLIENT, image_url).await?
    {
        *image_url = materialized_url;
    }
    Ok(())
}

use std::borrow::Cow;
use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_login::default_client::build_reqwest_client;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ResponseItem;
use codex_utils_image::PromptImageMode;
use codex_utils_image::load_for_prompt_bytes;
use reqwest::Client;
use reqwest::Response;
use tracing::warn;
use url::Url;

const IMAGE_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_IMAGE_DOWNLOAD_BYTES: u64 = 50 * 1024 * 1024;
const IMAGE_MATERIALIZATION_ERROR_PLACEHOLDER: &str =
    "image content omitted because it could not be downloaded or processed";

static IMAGE_URL_CLIENT: LazyLock<Client> = LazyLock::new(build_reqwest_client);

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
            ResponseItem::Message { content, .. } => {
                for content_item in content {
                    let result = match content_item {
                        ContentItem::InputImage { image_url, .. } => {
                            materialize_image_url(image_url).await
                        }
                        ContentItem::InputText { .. } | ContentItem::OutputText { .. } => continue,
                    };
                    if let Err(err) = result {
                        warn!(error = %err, "failed to materialize remote message image");
                        *content_item = ContentItem::InputText {
                            text: IMAGE_MATERIALIZATION_ERROR_PLACEHOLDER.to_string(),
                        };
                    }
                }
            }
            ResponseItem::FunctionCallOutput { output, .. }
            | ResponseItem::CustomToolCallOutput { output, .. } => {
                if let Some(content_items) = output.content_items_mut() {
                    for content_item in content_items {
                        let result = match content_item {
                            FunctionCallOutputContentItem::InputImage { image_url, .. } => {
                                materialize_image_url(image_url).await
                            }
                            FunctionCallOutputContentItem::InputText { .. }
                            | FunctionCallOutputContentItem::EncryptedContent { .. } => continue,
                        };
                        if let Err(err) = result {
                            warn!(error = %err, "failed to materialize remote tool output image");
                            *content_item = FunctionCallOutputContentItem::InputText {
                                text: IMAGE_MATERIALIZATION_ERROR_PLACEHOLDER.to_string(),
                            };
                        }
                    }
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
            ContentItem::InputImage { image_url, .. } => parse_http_url(image_url).is_some(),
            ContentItem::InputText { .. } | ContentItem::OutputText { .. } => false,
        }),
        ResponseItem::FunctionCallOutput { output, .. }
        | ResponseItem::CustomToolCallOutput { output, .. } => {
            output.content_items().is_some_and(|items| {
                items.iter().any(|item| match item {
                    FunctionCallOutputContentItem::InputImage { image_url, .. } => {
                        parse_http_url(image_url).is_some()
                    }
                    FunctionCallOutputContentItem::InputText { .. }
                    | FunctionCallOutputContentItem::EncryptedContent { .. } => false,
                })
            })
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
        | ResponseItem::Other => false,
    }
}

fn parse_http_url(value: &str) -> Option<Url> {
    // Avoid parsing potentially large inline data URLs on the common no-op path.
    let (scheme, _) = value.split_once(':')?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    Url::parse(value).ok()
}

async fn materialize_image_url(image_url: &mut String) -> Result<()> {
    let Some(url) = parse_http_url(image_url) else {
        return Ok(());
    };
    let response = IMAGE_URL_CLIENT
        .get(url)
        .timeout(IMAGE_DOWNLOAD_TIMEOUT)
        .send()
        .await
        .map_err(reqwest::Error::without_url)
        .context("failed to download image")?
        .error_for_status()
        .map_err(reqwest::Error::without_url)
        .context("failed to download image")?;
    let bytes = read_response_body_with_limit(response).await?;
    let image = load_for_prompt_bytes(
        Path::new("<remote image>"),
        bytes,
        PromptImageMode::Original,
    )
    .context("failed to process downloaded image")?;
    *image_url = image.into_data_url();
    Ok(())
}

async fn read_response_body_with_limit(mut response: Response) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_IMAGE_DOWNLOAD_BYTES)
    {
        bail!("downloaded image exceeded the maximum size of {MAX_IMAGE_DOWNLOAD_BYTES} bytes");
    }

    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(reqwest::Error::without_url)
        .context("failed to download image")?
    {
        let next_length = body.len() as u64 + chunk.len() as u64;
        if next_length > MAX_IMAGE_DOWNLOAD_BYTES {
            bail!("downloaded image exceeded the maximum size of {MAX_IMAGE_DOWNLOAD_BYTES} bytes");
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

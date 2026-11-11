//! Converts individual MCP content blocks into model-visible tool output.

use super::DEFAULT_IMAGE_DETAIL;
use super::FunctionCallOutputContentItem;
use super::ImageDetail;
use super::ImageReference;
use crate::mcp::Resource;
use crate::mcp::ResourceContent;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum McpContent<'a> {
    Text {
        text: String,
    },
    Image(#[serde(borrow)] McpMedia<'a>),
    Audio(#[serde(borrow)] McpMedia<'a>),
    ResourceLink {
        #[serde(flatten)]
        _link: Resource,
    },
    Resource {
        #[serde(rename = "resource")]
        _resource: ResourceContent,
    },
}

#[derive(Deserialize)]
struct McpMedia<'a> {
    data: &'a str,
    #[serde(rename = "mimeType", alias = "mime_type")]
    mime_type: Option<&'a str>,
}

impl McpMedia<'_> {
    fn data_url(&self) -> String {
        if self
            .data
            .get(.."data:".len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
        {
            self.data.to_owned()
        } else {
            let mime_type = self.mime_type.unwrap_or("application/octet-stream");
            format!("data:{mime_type};base64,{}", self.data)
        }
    }
}

impl FunctionCallOutputContentItem {
    /// Projects one MCP ContentBlock using its wire discriminator and Codex metadata.
    pub fn from_mcp_content_block(block: &Value) -> serde_json::Result<Self> {
        let meta = block.get("_meta");
        Ok(match McpContent::deserialize(block)? {
            McpContent::Text { text } => {
                if meta
                    .and_then(|meta| meta.get("codex/encryptedContent"))
                    .and_then(Value::as_bool)
                    == Some(true)
                {
                    Self::EncryptedContent {
                        encrypted_content: text,
                    }
                } else {
                    Self::InputText { text }
                }
            }
            McpContent::Image(media) => Self::InputImage {
                image: ImageReference::Inline {
                    image_url: media.data_url(),
                },
                detail: Some(
                    meta.and_then(|meta| meta.get("codex/imageDetail"))
                        .filter(|detail| detail.is_string())
                        .and_then(|detail| ImageDetail::deserialize(detail).ok())
                        .unwrap_or(DEFAULT_IMAGE_DETAIL),
                ),
            },
            McpContent::Audio(media) => Self::inline_audio(media.data_url()),
            McpContent::ResourceLink { .. } | McpContent::Resource { .. } => Self::InputText {
                text: serde_json::to_string(block)?,
            },
        })
    }
}

pub(super) fn convert_mcp_content_to_items(
    contents: &[Value],
) -> Vec<FunctionCallOutputContentItem> {
    contents
        .iter()
        .map(|block| {
            FunctionCallOutputContentItem::from_mcp_content_block(block).unwrap_or_else(|_| {
                FunctionCallOutputContentItem::InputText {
                    text: serde_json::to_string(block).unwrap_or_else(|_| "<content>".to_string()),
                }
            })
        })
        .collect()
}

//! Bounded image selection shared by Guardian consumers.
//! Keeps source order and evicts oldest images using the existing count/byte caps.
//! Consumers still choose source visibility and model-specific image detail.

use crate::ContextSection;
use crate::SectionContributor;
use crate::SectionError;
use crate::SectionInput;
use crate::SectionScope;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ImageReference;
use codex_protocol::models::ResponseItem;
use std::collections::VecDeque;

const MAX_TRANSCRIPT_IMAGES: usize = 4;
const MAX_TRANSCRIPT_IMAGE_BYTES: usize = 8 * 1024 * 1024;

/// Request-local image policy and a frozen view of captured REPL screenshots.
#[derive(Clone, Copy)]
pub struct TranscriptImageInput<'a> {
    pub enabled: bool,
    pub include_tool_outputs: bool,
    pub node_repl_images: &'a [ContentItem],
}

/// Selected images and byte omissions for the consumer's existing telemetry.
#[derive(Clone, Default, PartialEq)]
pub struct TranscriptImages {
    pub images: Vec<ContentItem>,
    pub omitted_bytes: usize,
}

impl std::fmt::Debug for TranscriptImages {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TranscriptImages")
            .field("count", &self.images.len())
            .field("omitted_bytes", &self.omitted_bytes)
            .finish_non_exhaustive()
    }
}

impl TranscriptImages {
    /// Selects images without changing their detail or allocating a second history.
    pub fn collect<'a>(
        items: impl IntoIterator<Item = &'a ResponseItem>,
        input: TranscriptImageInput<'_>,
    ) -> Self {
        if !input.enabled {
            return TranscriptImages {
                images: Vec::new(),
                omitted_bytes: 0,
            };
        }

        let mut images = VecDeque::new();
        let mut image_bytes = 0usize;
        let mut omitted_bytes = 0usize;
        let mut include_image = |image_url: &str, detail: Option<ImageDetail>| {
            if image_url.len() > MAX_TRANSCRIPT_IMAGE_BYTES {
                omitted_bytes = omitted_bytes.saturating_add(image_url.len());
                return;
            }
            while images.len() >= MAX_TRANSCRIPT_IMAGES
                || image_bytes + image_url.len() > MAX_TRANSCRIPT_IMAGE_BYTES
            {
                let Some(ContentItem::InputImage {
                    image: ImageReference::Inline { image_url },
                    ..
                }) = images.pop_front()
                else {
                    break;
                };
                image_bytes -= image_url.len();
                omitted_bytes = omitted_bytes.saturating_add(image_url.len());
            }
            image_bytes += image_url.len();
            images.push_back(ContentItem::InputImage {
                image: ImageReference::Inline {
                    image_url: image_url.to_owned(),
                },
                detail,
            });
        };

        // TODO(kc) Include file-backed images once Guardian can admit them without resolving or
        // applying byte-based checks in Core.
        for item in items {
            match item {
                ResponseItem::Message { role, content, .. }
                    if matches!(role.as_str(), "user" | "assistant") =>
                {
                    for item in content {
                        match item {
                            ContentItem::InputImage {
                                image: ImageReference::Inline { image_url },
                                detail,
                            } => include_image(image_url, *detail),
                            ContentItem::InputImage {
                                image: ImageReference::File { .. },
                                ..
                            }
                            | ContentItem::InputText { .. }
                            | ContentItem::InputAudio { .. }
                            | ContentItem::OutputText { .. } => {}
                        }
                    }
                }
                ResponseItem::FunctionCallOutput { output, .. }
                | ResponseItem::CustomToolCallOutput { output, .. }
                    if input.include_tool_outputs =>
                {
                    if let Some(content) = output.content_items() {
                        for item in content {
                            match item {
                                FunctionCallOutputContentItem::InputImage {
                                    image: ImageReference::Inline { image_url },
                                    detail,
                                } => include_image(image_url, *detail),
                                FunctionCallOutputContentItem::InputImage {
                                    image: ImageReference::File { .. },
                                    ..
                                }
                                | FunctionCallOutputContentItem::InputText { .. }
                                | FunctionCallOutputContentItem::InputAudio { .. }
                                | FunctionCallOutputContentItem::EncryptedContent { .. } => {}
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if input.include_tool_outputs {
            for image in input.node_repl_images {
                match image {
                    ContentItem::InputImage {
                        image: ImageReference::Inline { image_url },
                        detail,
                    } => include_image(image_url, *detail),
                    ContentItem::InputImage {
                        image: ImageReference::File { .. },
                        ..
                    }
                    | ContentItem::InputText { .. }
                    | ContentItem::InputAudio { .. }
                    | ContentItem::OutputText { .. } => {}
                }
            }
        }

        TranscriptImages {
            images: images.into_iter().collect(),
            omitted_bytes,
        }
    }
}

pub(crate) struct TranscriptImagesSection;
impl SectionContributor for TranscriptImagesSection {
    fn scope(&self) -> SectionScope {
        SectionScope::Shared
    }
    fn contribute(&self, input: &SectionInput<'_>) -> Result<Option<ContextSection>, SectionError> {
        Ok(input.images.map(|images| {
            ContextSection::TranscriptImages(TranscriptImages::collect(
                input.history.items(),
                images,
            ))
        }))
    }
}

#[cfg(test)]
#[path = "images_tests.rs"]
mod tests;

//! Projects unavailable images and tool audio into placeholders without changing tool attribution.

use std::collections::HashSet;

use codex_attachment_store::AttachmentStore;
use codex_context_fragments::AnnotatedContent;
use codex_context_fragments::set_annotated_content;
use codex_context_fragments::to_annotated_content;
use codex_protocol::models::AudioReference;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ContentItemMetadata;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ImageReference;
use codex_protocol::models::ResponseItem;
use futures::StreamExt;
use futures::stream;

const MAX_CONCURRENT_FILE_AVAILABILITY_CHECKS: usize = 8;

/// Checks distinct file IDs concurrently and replaces only media the store confirms is unavailable.
pub(crate) async fn replace_unresolvable_media_files<'a>(
    items: impl Iterator<Item = &'a mut ResponseItem>,
    store: &dyn AttachmentStore,
) {
    let items: Vec<_> = items.collect();
    let mut file_ids = HashSet::new();
    for item in &items {
        match item {
            ResponseItem::Message { content, .. } => {
                for part in content {
                    if let ContentItem::InputImage {
                        image: ImageReference::File { file_id },
                        ..
                    } = part
                    {
                        file_ids.insert(file_id.clone());
                    }
                }
            }
            ResponseItem::FunctionCallOutput { output, .. }
            | ResponseItem::CustomToolCallOutput { output, .. } => {
                if let Some(content) = output.content_items() {
                    for part in content {
                        if let FunctionCallOutputContentItem::InputImage {
                            image: ImageReference::File { file_id },
                            ..
                        }
                        | FunctionCallOutputContentItem::InputAudio {
                            audio: AudioReference::File { file_id },
                        } = part
                        {
                            file_ids.insert(file_id.clone());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let mut checks = stream::iter(file_ids)
        .map(|file_id| async move {
            let resolvable = store.is_resolvable(&file_id).await;
            (file_id, resolvable)
        })
        .buffer_unordered(MAX_CONCURRENT_FILE_AVAILABILITY_CHECKS);
    let mut unavailable_file_ids = HashSet::new();
    while let Some((file_id, resolvable)) = checks.next().await {
        if !resolvable {
            unavailable_file_ids.insert(file_id);
        }
    }
    if unavailable_file_ids.is_empty() {
        return;
    }

    for item in items {
        match item {
            ResponseItem::Message { content, .. } => {
                let mut unavailable = Vec::new();
                for (index, part) in content.iter().enumerate() {
                    if let ContentItem::InputImage {
                        image: ImageReference::File { file_id },
                        ..
                    } = part
                        && unavailable_file_ids.contains(file_id)
                    {
                        unavailable.push(index);
                    }
                }
                if unavailable.is_empty() {
                    continue;
                }
                let Some(mut content) = to_annotated_content(item) else {
                    continue;
                };
                for index in unavailable {
                    content[index] = AnnotatedContent::text(
                        "[image unavailable]",
                        ContentItemKind("images.unavailable".to_string()),
                        ContentItemMetadata::harness(),
                    );
                }
                let _ = set_annotated_content(item, content);
            }
            ResponseItem::FunctionCallOutput { output, .. }
            | ResponseItem::CustomToolCallOutput { output, .. } => {
                if let Some(content) = output.content_items_mut() {
                    for part in content {
                        let (file_id, placeholder) = match part {
                            FunctionCallOutputContentItem::InputImage {
                                image: ImageReference::File { file_id },
                                ..
                            } => (file_id, "[image unavailable]"),
                            FunctionCallOutputContentItem::InputAudio {
                                audio: AudioReference::File { file_id },
                            } => (
                                file_id,
                                "audio content omitted because its file is unavailable",
                            ),
                            _ => continue,
                        };
                        if unavailable_file_ids.contains(file_id) {
                            *part = FunctionCallOutputContentItem::InputText {
                                text: placeholder.into(),
                            };
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

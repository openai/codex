//! Non-mutating request projection for uploaded audio rejected by Responses.
//! Provider errors are attributed only against the immutable submitted request.

use std::collections::BTreeSet;
use std::time::Duration;

use codex_attachment_store::AttachmentStore;
use codex_history::AcceptedFileAudio;
use codex_history::CodexHarnessMetadata;
use codex_history::ResponseItemEnvelope;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::models::AudioReference;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ImageReference;
use codex_protocol::models::ResponseItem;
use futures::StreamExt;
use futures::stream;

/// Exact file identity proven by a terminal provider error and its submitted request.
#[derive(Clone)]
pub(crate) struct RejectedAudioFileIds(pub(crate) Vec<String>);

pub(crate) fn pending_audio_confirmation(
    previous: Option<&AcceptedFileAudio>,
    turn_id: &str,
    model: &str,
    provider: &str,
    items: &[ResponseItem],
) -> Option<AcceptedFileAudio> {
    let ids = items
        .iter()
        .filter_map(|item| match item {
            ResponseItem::Message { content, .. } => Some(content.iter()),
            _ => None,
        })
        .flatten()
        .filter_map(|part| match part {
            ContentItem::InputAudio {
                audio: AudioReference::File { file_id },
            } => Some(file_id),
            _ => None,
        });
    if ids
        .clone()
        .all(|id| previous.is_some_and(|known| known.file_ids.contains(id)))
    {
        return None;
    }
    Some(AcceptedFileAudio {
        turn_id: turn_id.to_owned(),
        model: model.to_owned(),
        provider: provider.to_owned(),
        file_ids: ids.cloned().collect(),
    })
}

enum FileErrorSource<'a> {
    Audio(&'a str),
    InlineAudio,
    ToolFileAudio,
    FileImage,
    Opaque,
}

fn visit_sources<'a>(item: &'a ResponseItem, mut visit: impl FnMut(FileErrorSource<'a>)) {
    match item {
        ResponseItem::Message { content, .. } => {
            for part in content {
                match part {
                    ContentItem::InputAudio {
                        audio: AudioReference::File { file_id },
                    } => visit(FileErrorSource::Audio(file_id)),
                    ContentItem::InputAudio {
                        audio: AudioReference::Inline { .. },
                    } => visit(FileErrorSource::InlineAudio),
                    ContentItem::InputImage {
                        image: ImageReference::File { .. },
                        ..
                    } => visit(FileErrorSource::FileImage),
                    _ => {}
                }
            }
        }
        ResponseItem::FunctionCallOutput { output, .. }
        | ResponseItem::CustomToolCallOutput { output, .. } => {
            for part in output.content_items().unwrap_or_default() {
                match part {
                    FunctionCallOutputContentItem::InputAudio {
                        audio: AudioReference::File { .. },
                    } => visit(FileErrorSource::ToolFileAudio),
                    FunctionCallOutputContentItem::InputAudio {
                        audio: AudioReference::Inline { .. },
                    } => visit(FileErrorSource::InlineAudio),
                    FunctionCallOutputContentItem::InputImage {
                        image: ImageReference::File { .. },
                        ..
                    } => visit(FileErrorSource::FileImage),
                    _ => {}
                }
            }
        }
        ResponseItem::Compaction { .. } | ResponseItem::ContextCompaction { .. } => {
            visit(FileErrorSource::Opaque)
        }
        _ => {}
    }
}

/// Older opaque checkpoints carry no provenance and continue to compete for attribution.
pub(crate) fn checkpoint_input_has_no_file_error_sources(items: &[ResponseItemEnvelope]) -> bool {
    items.iter().all(|envelope| {
        let mut clear = true;
        visit_sources(&envelope.item, |source| {
            clear &= matches!(source, FileErrorSource::Opaque)
                && clear_checkpoint(&envelope.item, envelope.metadata.as_ref()).is_some();
        });
        clear
    })
}

/// Only the host marker on this ciphertext proves that the checkpoint had no competing files.
pub(crate) fn clear_checkpoint<'a>(
    item: &'a ResponseItem,
    metadata: Option<&CodexHarnessMetadata>,
) -> Option<&'a str> {
    metadata.filter(|metadata| metadata.compaction_without_file_error_sources)?;
    opaque_content(item)
}

fn opaque_content(item: &ResponseItem) -> Option<&str> {
    match item {
        ResponseItem::Compaction {
            encrypted_content, ..
        } => Some(encrypted_content),
        ResponseItem::ContextCompaction {
            encrypted_content, ..
        } => encrypted_content.as_deref(),
        _ => None,
    }
}

/// Audio IDs and competing sources in the immutable submitted request. The prompt itself
/// can be released while the provider streams.
pub(crate) struct SubmittedFileAudio {
    file_ids: Vec<String>,
    candidate: Option<usize>,
    has_competing_audio: bool,
    has_competing_file: bool,
}

impl SubmittedFileAudio {
    pub(crate) fn from_request<'a>(
        items: &[ResponseItem],
        accepted: Option<&AcceptedFileAudio>,
        clear_checkpoints: impl Iterator<Item = &'a str> + Clone,
    ) -> Option<Self> {
        let mut file_ids = Vec::new();
        let mut has_competing_audio = false;
        let mut has_competing_file = false;
        let mut unknown_checkpoint = false;
        for item in items {
            visit_sources(item, |source| match source {
                FileErrorSource::Audio(id) => file_ids.push(id),
                FileErrorSource::InlineAudio => has_competing_audio = true,
                FileErrorSource::ToolFileAudio => {
                    has_competing_audio = true;
                    has_competing_file = true;
                }
                FileErrorSource::FileImage => has_competing_file = true,
                FileErrorSource::Opaque => {
                    unknown_checkpoint |= !opaque_content(item).is_some_and(|ciphertext| {
                        clear_checkpoints.clone().any(|known| known == ciphertext)
                    });
                }
            });
        }
        if file_ids.is_empty() {
            return None;
        }
        let candidate = if unknown_checkpoint {
            None
        } else if file_ids.len() == 1 {
            Some(0)
        } else {
            let mut unseen = file_ids
                .iter()
                .enumerate()
                .filter(|(_, id)| !accepted.is_some_and(|known| known.file_ids.contains(**id)));
            let first = unseen.next().map(|(index, _)| index);
            first.filter(|_| unseen.next().is_none())
        };
        Some(Self {
            file_ids: file_ids.into_iter().map(str::to_owned).collect(),
            candidate,
            has_competing_audio,
            has_competing_file,
        })
    }

    fn rejected_index(&self, error: &CodexErrorDetails) -> Option<usize> {
        let index = self.candidate?;
        let unique_file = self.file_ids.len() == 1;
        let attributable = match error {
            CodexErrorDetails::InvalidAudioInput { per_recording, .. } => {
                !self.has_competing_audio && (unique_file || *per_recording)
            }
            _ => unique_file && !self.has_competing_file && is_unknown_file_error(error),
        };
        attributable.then_some(index)
    }

    pub(crate) fn matches_rejection(&self, error: &CodexErrorDetails) -> bool {
        self.rejected_index(error).is_some()
    }

    pub(crate) fn rejected_file_id(mut self, error: &CodexErrorDetails) -> Option<String> {
        let index = self.rejected_index(error)?;
        Some(self.file_ids.swap_remove(index))
    }

    pub(crate) async fn rejected_file_ids(
        mut self,
        error: &CodexErrorDetails,
        store: &dyn AttachmentStore,
    ) -> Vec<String> {
        if let Some(index) = self.rejected_index(error) {
            return vec![self.file_ids.swap_remove(index)];
        }
        if is_unknown_file_error(error) {
            // A file that decoded successfully may since have expired. Probe only IDs from
            // this submission; later queued input cannot be blamed for the provider's 404.
            self.file_ids.sort_unstable();
            self.file_ids.dedup();
            return unavailable_file_audio_ids(self.file_ids, store)
                .await
                .into_iter()
                .collect();
        }
        Vec::new()
    }
}

fn is_unknown_file_error(error: &CodexErrorDetails) -> bool {
    const UNKNOWN_FILE: &str = "Invalid input: Unknown `file_id` in input content.";
    matches!(error, CodexErrorDetails::UnexpectedStatus(response)
        if response.status == http::StatusCode::NOT_FOUND
            && (response.body.trim() == UNKNOWN_FILE
                || response.extract_error_message().as_deref() == Some(UNKNOWN_FILE)))
}

/// A long-lived thread can reach compaction after an earlier upload has expired.
/// Only an authoritative negative availability check permits omitting the file.
pub(crate) async fn unavailable_file_audio(
    items: &[ResponseItem],
    store: &dyn AttachmentStore,
) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for item in items {
        if let ResponseItem::Message { content, .. } = item {
            for part in content {
                if let ContentItem::InputAudio {
                    audio: AudioReference::File { file_id },
                } = part
                {
                    ids.insert(file_id.clone());
                }
            }
        }
    }
    unavailable_file_audio_ids(ids, store).await
}

async fn unavailable_file_audio_ids(
    ids: impl IntoIterator<Item = String>,
    store: &dyn AttachmentStore,
) -> BTreeSet<String> {
    let mut checks = stream::iter(ids)
        .map(|id| async move { (!store.is_resolvable(&id).await).then_some(id) })
        .buffer_unordered(8);
    let mut missing = BTreeSet::new();
    // Bound the entire preflight. Responses can still validate unchecked IDs.
    let _ = tokio::time::timeout(Duration::from_millis(250), async {
        while let Some(result) = checks.next().await {
            missing.extend(result);
        }
    })
    .await;
    missing
}

/// Keep per-item metadata paired with the submitted input when an audio-only
/// message disappears. The resulting projection also becomes the checkpoint.
pub(crate) fn project_compaction_audio(
    input: &mut Vec<ResponseItem>,
    metadata: &mut Vec<Option<CodexHarnessMetadata>>,
    unavailable: &BTreeSet<String>,
) {
    if unavailable.is_empty() {
        return;
    }
    debug_assert_eq!(input.len(), metadata.len());
    let mut envelopes = input
        .drain(..)
        .zip(metadata.drain(..))
        .map(|(item, metadata)| ResponseItemEnvelope { item, metadata })
        .collect();
    project_file_audio(&mut envelopes, unavailable);
    for envelope in envelopes {
        input.push(envelope.item);
        metadata.push(envelope.metadata);
    }
}

pub(super) fn project_file_audio(
    items: &mut Vec<ResponseItemEnvelope>,
    rejected_file_ids: &BTreeSet<String>,
) {
    items.retain_mut(|envelope| {
        let ResponseItem::Message {
            role,
            content,
            internal_chat_message_metadata_passthrough,
            ..
        } = &mut envelope.item
        else {
            return true;
        };
        let keep = content
            .iter()
            .map(|part| {
                !matches!(part, ContentItem::InputAudio {
                    audio: AudioReference::File { file_id },
                } if rejected_file_ids.contains(file_id))
            })
            .collect::<Vec<_>>();
        if keep.iter().all(|keep| *keep) {
            return true;
        }
        retain_aligned(content, &keep);
        if let Some(metadata) = internal_chat_message_metadata_passthrough {
            if let Some(kinds) = &mut metadata.content_item_kinds {
                retain_aligned(kinds, &keep);
            }
            if let Some(sources) = &mut metadata.content_item_metadata {
                retain_aligned(sources, &keep);
            }
        }
        role != "user" || !content.is_empty()
    });
}

fn retain_aligned<T>(items: &mut Vec<T>, keep: &[bool]) {
    if items.len() != keep.len() {
        items.clear();
        return;
    }
    let mut index = 0;
    items.retain(|_| {
        let retain = keep[index];
        index += 1;
        retain
    });
}

#[cfg(test)]
#[path = "history_rejected_audio_tests.rs"]
mod tests;

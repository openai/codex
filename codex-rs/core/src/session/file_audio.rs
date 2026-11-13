//! Caches trusted file-audio durations for one session. Metadata lookup is best effort
//! and time-bounded. Accepted input is recorded before lookup, and unmeasured
//! files reserve the full audio token budget.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use codex_attachment_store::AttachmentStore;
use codex_attachment_store::FormatSpecificMetadata;
use codex_attachment_store::ResolveRequest;
use codex_attachment_store::UploadRequest;
use codex_attachment_store::UploadResult;
use codex_history::ResponseItemEnvelope;
use codex_protocol::models::AudioReference;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ResponseItem;
use codex_utils_audio::estimate_file_audio_tokens;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::approx_token_count;
use codex_utils_output_truncation::truncate_measured_tool_output;
use futures::StreamExt;
use futures::stream;

use crate::session::Session;

/// Thread-local metadata. It is never serialized into a rollout or a provider request.
#[derive(Clone, Debug, Default)]
pub(crate) struct FileAudioDurations(BTreeMap<String, Duration>);

impl FileAudioDurations {
    pub(crate) fn tokens(&self, file_id: &str) -> usize {
        estimate_file_audio_tokens(self.0.get(file_id).copied())
    }
}

pub(crate) async fn snapshot(session: &Session) -> Arc<FileAudioDurations> {
    Arc::clone(&session.state.lock().await.history.file_audio_durations)
}

pub(super) fn file_ids(items: &[ResponseItemEnvelope]) -> impl Iterator<Item = &String> {
    items
        .iter()
        .flat_map(|envelope| envelope.item.file_audio_ids())
}

/// Only fresh results pass through the store. A replay keeps the representation in the rollout.
pub(super) async fn prepare_new_items(
    items: &mut [ResponseItemEnvelope],
    store: &dyn AttachmentStore,
    thread_id: &str,
    ephemeral: bool,
    default_token_limit: usize,
) {
    for ResponseItemEnvelope { item, metadata } in items {
        let (ResponseItem::FunctionCallOutput { output, .. }
        | ResponseItem::CustomToolCallOutput { output, .. }) = item
        else {
            codex_utils_audio::prepare_response_items(std::slice::from_mut(item));
            continue;
        };
        if !output.contains_audio() {
            continue;
        }
        let metadata = metadata.get_or_insert_default();
        // The producer's output limit can be larger than the history limit (e.g. Code Mode).
        // Apply the latter to the whole result while audio still has its inline cost.
        let limit = metadata
            .history_truncation_token_limit
            .unwrap_or(default_token_limit);
        let producer_budgeted = std::mem::take(&mut metadata.producer_bounded_tool_audio);
        // Validate and measure on a worker. Only clips retained by history need their
        // decoded bytes materialized for upload, one at a time.
        let Some(parts) = output.content_items_mut() else {
            continue;
        };
        let parts = std::mem::take(parts);
        let preserve_producer_limit =
            producer_budgeted && metadata.history_truncation_token_limit.is_none();
        let Ok((mut admitted, repair_limit)) = tokio::task::spawn_blocking(move || {
            admit_tool_audio(parts, limit, preserve_producer_limit)
        })
        .await
        else {
            *output = codex_protocol::models::FunctionCallOutputPayload::from_text(
                "audio content omitted because it could not be processed".into(),
            );
            continue;
        };
        let mut replaced_with_text = false;
        for (part, charge) in &mut admitted {
            if let FunctionCallOutputContentItem::InputAudio {
                audio: AudioReference::Inline { audio_url },
            } = part
            {
                let url = std::mem::take(audio_url);
                *part = store_tool_audio(store, thread_id, ephemeral, url).await;
                if matches!(part, FunctionCallOutputContentItem::InputText { .. }) {
                    *charge = 0;
                    replaced_with_text = true;
                }
            }
        }
        if replaced_with_text {
            admitted =
                truncate_measured_tool_output(admitted, TruncationPolicy::Tokens(repair_limit));
        }
        output.body = codex_protocol::models::FunctionCallOutputBody::ContentItems(
            admitted.into_iter().map(|(part, _)| part).collect(),
        );
        metadata.tool_audio_budget_applied = true;
    }
}

fn admit_tool_audio(
    parts: Vec<FunctionCallOutputContentItem>,
    history_limit: usize,
    preserve_producer_limit: bool,
) -> (Vec<(FunctionCallOutputContentItem, usize)>, usize) {
    let mut measured = Vec::with_capacity(parts.len());
    for mut part in parts {
        let charge = match &mut part {
            FunctionCallOutputContentItem::InputAudio {
                audio: AudioReference::Inline { audio_url },
            } => match codex_utils_audio::prepare_and_measure_inline_audio(audio_url) {
                Ok(tokens) => tokens,
                Err(error) => {
                    part = FunctionCallOutputContentItem::InputText {
                        text: error.placeholder().into(),
                    };
                    0
                }
            },
            FunctionCallOutputContentItem::InputAudio {
                audio: AudioReference::File { .. },
            } => estimate_file_audio_tokens(/*duration*/ None),
            _ => 0,
        };
        measured.push((part, charge));
    }
    let admitted = truncate_measured_tool_output(measured, TruncationPolicy::Tokens(history_limit));
    // Admission matches inline history. Only storage can introduce a new placeholder
    // after that point. With no explicit producer limit, keep such repairs within
    // the size of the result the model would otherwise have received.
    let repair_limit = if preserve_producer_limit {
        admitted
            .iter()
            .fold(0_usize, |sum, (part, charge)| {
                let cost = match part {
                    FunctionCallOutputContentItem::InputText { text } => approx_token_count(text),
                    _ => *charge,
                };
                sum.saturating_add(cost)
            })
            .min(history_limit)
    } else {
        history_limit
    };
    (admitted, repair_limit)
}

async fn store_tool_audio(
    store: &dyn AttachmentStore,
    thread_id: &str,
    ephemeral: bool,
    inline_url: String,
) -> FunctionCallOutputContentItem {
    let (mime, data, inline_url) = match tokio::task::spawn_blocking(move || {
        codex_utils_audio::decode_audio(&inline_url).map(|(mime, data)| (mime, data, inline_url))
    })
    .await
    {
        Ok(Ok(decoded)) => decoded,
        Ok(Err(error)) => {
            return FunctionCallOutputContentItem::InputText {
                text: error.placeholder().into(),
            };
        }
        Err(_) => {
            return FunctionCallOutputContentItem::InputText {
                text: "audio content omitted because it could not be processed".into(),
            };
        }
    };
    // These tags come from Core. MCP servers and Code Mode never supply attachment IDs or policy tags.
    let tags = HashMap::from([
        ("usage".to_string(), "tool_audio".to_string()),
        ("mime_type".to_string(), mime.to_string()),
    ]);
    let result = store
        .upload(UploadRequest {
            thread_id: thread_id.to_owned(),
            ephemeral,
            file_name: None,
            tags,
            data,
        })
        .await;
    tool_audio_after_upload(result, mime, inline_url)
}

fn tool_audio_after_upload(
    result: Result<UploadResult, codex_attachment_store::AttachmentStoreError>,
    mime: &str,
    mut inline_url: String,
) -> FunctionCallOutputContentItem {
    match result {
        Ok(UploadResult::File { file_id }) if !file_id.trim().is_empty() => {
            FunctionCallOutputContentItem::InputAudio {
                audio: AudioReference::File { file_id },
            }
        }
        Ok(UploadResult::Inline { bytes }) => FunctionCallOutputContentItem::inline_audio(
            codex_utils_audio::audio_data_url(mime, &bytes),
        ),
        Err(error) if error.kind() == codex_attachment_store::AttachmentStoreErrorKind::Backend => {
            tracing::warn!(kind = ?error.kind(), "tool audio upload failed; using inline audio");
            // decode_audio validated the base64 and normalized the MIME alias.
            if let Some(separator) = inline_url.find(',') {
                inline_url.replace_range(..separator, &format!("data:{mime};base64"));
            }
            FunctionCallOutputContentItem::inline_audio(inline_url)
        }
        Err(error)
            if error.kind()
                == codex_attachment_store::AttachmentStoreErrorKind::InvalidAttachment =>
        {
            FunctionCallOutputContentItem::InputText {
                text: "audio content omitted because the attachment store rejected the clip".into(),
            }
        }
        Ok(UploadResult::File { .. }) | Err(_) => {
            tracing::warn!("could not store tool audio attachment");
            FunctionCallOutputContentItem::InputText {
                text: "audio content omitted because it could not be stored".to_owned(),
            }
        }
    }
}

pub(super) async fn resolve_pending(session: &Session) {
    let ids = std::mem::take(&mut session.state.lock().await.pending_audio_duration_ids);
    resolve(session, ids).await;
}

pub(super) async fn resolve(session: &Session, mut file_ids: BTreeSet<String>) {
    if file_ids.is_empty() {
        return;
    }
    {
        let state = session.state.lock().await;
        file_ids.retain(|id| !state.history.file_audio_durations.0.contains_key(id));
    }
    let mut lookups = stream::iter(file_ids)
        .map(|id| async move {
            let result = session
                .services
                .image_store
                .resolve(ResolveRequest {
                    file_id: &id,
                    tags: HashMap::new(),
                    download_url_ttl: None,
                })
                .await;
            (id, result)
        })
        .buffer_unordered(8);
    let mut resolved = BTreeMap::new();
    // Metadata is optional. Bound the entire batch, keep completed lookups on timeout,
    // and allow future attachments to retry a missing value.
    let _ = tokio::time::timeout(Duration::from_millis(250), async {
        while let Some((id, result)) = lookups.next().await {
            if let Ok(metadata) = result
                && let Some(FormatSpecificMetadata::Audio(audio)) = metadata.format_specific
                && let Some(duration_ms) = audio.duration_ms
                && duration_ms > 0
            {
                resolved.insert(id, Duration::from_millis(duration_ms));
            }
        }
    })
    .await;
    if !resolved.is_empty() {
        let mut state = session.state.lock().await;
        Arc::make_mut(&mut state.history.file_audio_durations)
            .0
            .extend(resolved);
    }
}

#[cfg(test)]
#[path = "file_audio_tests.rs"]
mod tests;

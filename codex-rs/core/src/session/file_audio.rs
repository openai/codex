//! Caches trusted file-audio durations for one session. Metadata lookup is best effort
//! and time-bounded. Accepted input is recorded before lookup, and unmeasured
//! files reserve the full audio token budget.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use codex_attachment_store::FormatSpecificMetadata;
use codex_attachment_store::ResolveRequest;
use codex_history::ResponseItemEnvelope;
use codex_protocol::models::AudioReference;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_utils_audio::estimate_file_audio_tokens;
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
        .filter_map(|envelope| match &envelope.item {
            ResponseItem::Message { content, .. } => Some(content),
            _ => None,
        })
        .flatten()
        .filter_map(|part| match part {
            ContentItem::InputAudio {
                audio: AudioReference::File { file_id },
            } => Some(file_id),
            _ => None,
        })
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

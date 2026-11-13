//! Deduplicates active client context and tracks its history item IDs across compaction.

use std::collections::BTreeMap;
use std::collections::HashMap;

use crate::context::AdditionalContextDeveloperFragment;
use crate::context::AdditionalContextToolOutput;
use crate::context::AdditionalContextUserFragment;
use crate::context::ContextualUserFragment;
use codex_context_fragments::MAX_ADDITIONAL_CONTEXT_VALUE_TOKENS;
use codex_history::CodexHarnessMetadata;
use codex_history::ResponseItemEnvelope;
use codex_otel::ADDITIONAL_CONTEXT_COUNT_METRIC;
use codex_otel::ADDITIONAL_CONTEXT_VALUE_BYTES_BUCKETS;
use codex_otel::ADDITIONAL_CONTEXT_VALUE_BYTES_METRIC;
use codex_otel::SessionTelemetry;
use codex_protocol::ItemIdGenerator;
use codex_protocol::ResponseItemId;
use codex_protocol::protocol::AdditionalContextEntry;
use codex_protocol::protocol::AdditionalContextKind;
use codex_utils_string::truncate_middle_with_token_budget;

pub(crate) enum AdditionalContextFormat {
    Messages,
    ToolOutputs,
}

#[derive(Debug, Clone, Eq)]
enum StoredAdditionalContext {
    Message(AdditionalContextEntry),
    ToolOutput(AdditionalContextToolOutput),
}

impl PartialEq for StoredAdditionalContext {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Message(left), Self::Message(right)) => {
                left.kind == right.kind && left.value == right.value
            }
            (Self::ToolOutput(left), Self::ToolOutput(right)) => left == right,
            (Self::Message(_), Self::ToolOutput(_)) | (Self::ToolOutput(_), Self::Message(_)) => {
                false
            }
        }
    }
}

impl StoredAdditionalContext {
    fn new(key: &str, entry: AdditionalContextEntry, format: &AdditionalContextFormat) -> Self {
        match (format, entry.kind) {
            (AdditionalContextFormat::ToolOutputs, AdditionalContextKind::Untrusted) => {
                Self::ToolOutput(AdditionalContextToolOutput::new(
                    key,
                    entry.name,
                    &entry.value,
                ))
            }
            (AdditionalContextFormat::Messages, AdditionalContextKind::Untrusted)
            | (_, AdditionalContextKind::Application) => Self::Message(entry),
        }
    }

    fn envelope(&self, key: &str) -> ResponseItemEnvelope {
        match self {
            Self::ToolOutput(output) => ResponseItemEnvelope {
                item: output.clone().into(),
                metadata: Some(CodexHarnessMetadata {
                    additional_context_tool_output: true,
                    ..Default::default()
                }),
            },
            Self::Message(entry) => ResponseItemEnvelope::new(match entry.kind {
                AdditionalContextKind::Untrusted => ContextualUserFragment::into(
                    AdditionalContextUserFragment::new(key.to_owned(), entry.value.clone()),
                ),
                AdditionalContextKind::Application => ContextualUserFragment::into(
                    AdditionalContextDeveloperFragment::new(key.to_owned(), entry.value.clone()),
                ),
            }),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AdditionalContextStore {
    values: BTreeMap<String, (StoredAdditionalContext, ResponseItemId)>,
}

impl AdditionalContextStore {
    pub(crate) fn merge(
        &mut self,
        values: BTreeMap<String, AdditionalContextEntry>,
        format: AdditionalContextFormat,
        telemetry: &SessionTelemetry,
        item_id_generator: &ItemIdGenerator,
    ) -> Vec<ResponseItemEnvelope> {
        self.values.retain(|key, _| values.contains_key(key));
        self.extend(values, format, telemetry, item_id_generator)
    }

    /// Adds a producer's updates without dropping unrelated client context.
    pub(crate) fn extend(
        &mut self,
        values: BTreeMap<String, AdditionalContextEntry>,
        format: AdditionalContextFormat,
        telemetry: &SessionTelemetry,
        item_id_generator: &ItemIdGenerator,
    ) -> Vec<ResponseItemEnvelope> {
        let mut fragments = Vec::new();
        for (key, entry) in values {
            Self::record_metrics(&entry, telemetry);
            let value = StoredAdditionalContext::new(&key, entry, &format);
            if self
                .values
                .get(&key)
                .is_some_and(|(previous, _)| previous == &value)
            {
                continue;
            }
            let mut envelope = value.envelope(&key);
            let Some(prefix) = envelope.id_prefix() else {
                unreachable!("additional context has an item ID")
            };
            let id = item_id_generator(prefix);
            envelope.set_id(Some(id.clone()));
            self.values.insert(key, (value, id));
            fragments.push(envelope);
        }
        fragments
    }

    /// Refresh retained active items in place and return the missing ones in history order.
    /// Queued updates are absent from the replaced prefix and arrive with their input.
    pub(crate) fn restore_compacted_items(
        &self,
        replaced: &[ResponseItemEnvelope],
        retained: &mut [ResponseItemEnvelope],
    ) -> Vec<ResponseItemEnvelope> {
        if self.values.is_empty() {
            return Vec::new();
        }
        let mut active: HashMap<_, _> = self.values.values().map(|(_, id)| (id, None)).collect();
        for (index, item) in retained.iter().enumerate() {
            if let Some(slot) = item.id().and_then(|id| active.get_mut(id)) {
                *slot = Some(index);
            }
        }
        let mut missing = Vec::new();
        for item in replaced {
            let Some(slot) = item.id().and_then(|id| active.remove(id)) else {
                continue;
            };
            if let Some(index) = slot {
                // Generic client-message retention may have shortened the active value.
                // Keep its position and checkpoint metadata, with the original bounded payload.
                retained[index].item.clone_from(&item.item);
            } else {
                missing.push(item.clone());
            }
        }
        missing
    }

    fn record_metrics(entry: &AdditionalContextEntry, telemetry: &SessionTelemetry) {
        let (bounded_value, original_tokens) =
            truncate_middle_with_token_budget(&entry.value, MAX_ADDITIONAL_CONTEXT_VALUE_TOKENS);
        let kind = match entry.kind {
            AdditionalContextKind::Untrusted => "untrusted",
            AdditionalContextKind::Application => "application",
        };
        let truncated = original_tokens.is_some().to_string();
        let mut tags = vec![("kind", kind), ("truncated", truncated.as_str())];
        if let Some(name) = entry.name.as_deref() {
            tags.push(("name", name));
        }
        telemetry.counter(ADDITIONAL_CONTEXT_COUNT_METRIC, /*inc*/ 1, &tags);
        for (stage, bytes) in [
            ("before", entry.value.len()),
            ("after", bounded_value.len()),
        ] {
            let mut size_tags = tags.clone();
            size_tags.push(("stage", stage));
            telemetry.histogram_with_boundaries(
                ADDITIONAL_CONTEXT_VALUE_BYTES_METRIC,
                i64::try_from(bytes).unwrap_or(i64::MAX),
                ADDITIONAL_CONTEXT_VALUE_BYTES_BUCKETS.as_slice(),
                &size_tags,
            );
        }
    }
}

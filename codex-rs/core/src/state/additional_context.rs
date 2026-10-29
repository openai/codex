//! Deduplicates the active client context using its selected model-visible representation.

use std::collections::BTreeMap;

use crate::context::AdditionalContextDeveloperFragment;
use crate::context::AdditionalContextToolOutput;
use crate::context::AdditionalContextUserFragment;
use crate::context::ContextualUserFragment;
use codex_context_fragments::MAX_ADDITIONAL_CONTEXT_VALUE_TOKENS;
use codex_otel::ADDITIONAL_CONTEXT_COUNT_METRIC;
use codex_otel::ADDITIONAL_CONTEXT_VALUE_BYTES_BUCKETS;
use codex_otel::ADDITIONAL_CONTEXT_VALUE_BYTES_METRIC;
use codex_otel::SessionTelemetry;
use codex_protocol::models::ResponseItem;
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AdditionalContextStore {
    values: BTreeMap<String, StoredAdditionalContext>,
}

impl AdditionalContextStore {
    pub(crate) fn merge(
        &mut self,
        values: BTreeMap<String, AdditionalContextEntry>,
        format: AdditionalContextFormat,
        telemetry: &SessionTelemetry,
    ) -> Vec<ResponseItem> {
        for entry in values.values() {
            Self::record_metrics(entry, telemetry);
        }
        let values =
            values
                .into_iter()
                .map(|(key, entry)| {
                    let value = match (&format, entry.kind) {
                        (
                            AdditionalContextFormat::ToolOutputs,
                            AdditionalContextKind::Untrusted,
                        ) => StoredAdditionalContext::ToolOutput(AdditionalContextToolOutput::new(
                            &key,
                            entry.name,
                            &entry.value,
                        )),
                        (AdditionalContextFormat::Messages, AdditionalContextKind::Untrusted)
                        | (AdditionalContextFormat::Messages, AdditionalContextKind::Application)
                        | (
                            AdditionalContextFormat::ToolOutputs,
                            AdditionalContextKind::Application,
                        ) => StoredAdditionalContext::Message(entry),
                    };
                    (key, value)
                })
                .collect::<BTreeMap<_, _>>();
        let fragments = values
            .iter()
            .filter(|(key, value)| self.values.get(*key) != Some(*value))
            .map(|(key, entry)| match entry {
                StoredAdditionalContext::ToolOutput(output) => output.clone().into(),
                StoredAdditionalContext::Message(entry) => match entry.kind {
                    AdditionalContextKind::Untrusted => ContextualUserFragment::into(
                        AdditionalContextUserFragment::new(key.clone(), entry.value.clone()),
                    ),
                    AdditionalContextKind::Application => ContextualUserFragment::into(
                        AdditionalContextDeveloperFragment::new(key.clone(), entry.value.clone()),
                    ),
                },
            })
            .collect();
        self.values = values;
        fragments
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
                ADDITIONAL_CONTEXT_VALUE_BYTES_BUCKETS,
                &size_tags,
            );
        }
    }
}

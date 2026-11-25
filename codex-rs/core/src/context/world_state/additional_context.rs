//! Renders client input as standalone items. WorldState tracks their fingerprints and history IDs.

use super::PreviousSectionState;
use super::SectionTransition;
use super::WorldStateHash;
use super::WorldStateSection;
use super::WorldStateSnapshot;
use super::WorldStateUpdate;
use codex_context_fragments::AnnotatedContent;
use codex_context_fragments::RenderedFragment;
use codex_history::ResponseItemEnvelope;
use codex_otel::ADDITIONAL_CONTEXT_COUNT_METRIC;
use codex_otel::ADDITIONAL_CONTEXT_VALUE_BYTES_BUCKETS;
use codex_otel::ADDITIONAL_CONTEXT_VALUE_BYTES_METRIC;
use codex_otel::SessionTelemetry;
use codex_protocol::ResponseItemId;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ContentItemMetadata;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AdditionalContextEntry;
use codex_protocol::protocol::AdditionalContextKind;
use codex_thread_context_ids::ThreadContextIds;
use codex_utils_string::truncate_middle_with_token_budget;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::collections::HashMap;

const MAX_ADDITIONAL_CONTEXT_VALUE_TOKENS: usize = 10_000;
const MAX_DERIVED_TOOL_NAME_CHARS: usize = 128;

/// A delivered client submission replaces the active sources. Extension inputs only update their own keys.
pub(crate) struct AdditionalContextState {
    items: Vec<(String, ResponseItem)>,
    ids: ThreadContextIds,
    extend: bool,
}

type Snapshot = BTreeMap<String, (WorldStateHash, ResponseItemId)>;

impl AdditionalContextState {
    pub(crate) fn new(
        entries: &BTreeMap<String, AdditionalContextEntry>,
        tool_outputs: bool,
        telemetry: &SessionTelemetry,
        ids: &ThreadContextIds,
    ) -> Self {
        let items = entries
            .iter()
            .map(|(key, entry)| {
                let (value, original_tokens) = truncate_middle_with_token_budget(
                    &entry.value,
                    MAX_ADDITIONAL_CONTEXT_VALUE_TOKENS,
                );
                Self::record_metrics(entry, value.len(), original_tokens.is_some(), telemetry);
                let item = match entry.kind {
                    AdditionalContextKind::Untrusted if tool_outputs => {
                        ResponseItem::FunctionCallOutput {
                            id: None,
                            call_id: None,
                            name: Some(
                                entry
                                    .name
                                    .clone()
                                    .unwrap_or_else(|| tool_name_from_key(key)),
                            ),
                            namespace: Some("agent".to_owned()),
                            output: FunctionCallOutputPayload::from_text(value),
                            internal_chat_message_metadata_passthrough: None,
                        }
                    }
                    AdditionalContextKind::Untrusted => message(
                        "user",
                        key,
                        format!("<external_{key}>{value}</external_{key}>"),
                    ),
                    AdditionalContextKind::Application => {
                        message("developer", key, format!("<{key}>{value}</{key}>"))
                    }
                };
                (key.clone(), item)
            })
            .collect();
        Self {
            items,
            ids: ids.clone(),
            extend: false,
        }
    }

    pub(crate) fn extending(mut self) -> Self {
        self.extend = true;
        self
    }

    pub(crate) fn matches_user_text(text: &str) -> bool {
        text.trim()
            .strip_prefix("<external_")
            .and_then(|rest| rest.split_once('>'))
            .is_some_and(|(key, rest)| rest.ends_with(&format!("</external_{key}>")))
    }

    fn record_metrics(
        entry: &AdditionalContextEntry,
        bounded_bytes: usize,
        truncated: bool,
        telemetry: &SessionTelemetry,
    ) {
        let kind = match entry.kind {
            AdditionalContextKind::Untrusted => "untrusted",
            AdditionalContextKind::Application => "application",
        };
        let truncated = truncated.to_string();
        let mut tags = vec![("kind", kind), ("truncated", truncated.as_str())];
        if let Some(name) = entry.name.as_deref() {
            tags.push(("name", name));
        }
        telemetry.counter(ADDITIONAL_CONTEXT_COUNT_METRIC, /*inc*/ 1, &tags);
        for (stage, bytes) in [("before", entry.value.len()), ("after", bounded_bytes)] {
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

impl WorldStateSection for AdditionalContextState {
    const ID: &'static str = "additional_context";
    type Snapshot = Snapshot;

    fn should_persist(&self) -> bool {
        !self.items.is_empty()
    }

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> SectionTransition<Self::Snapshot> {
        let previous = match previous {
            PreviousSectionState::Known(previous) => Some(previous),
            PreviousSectionState::Absent | PreviousSectionState::Unknown => None,
        };
        let mut snapshot = previous
            .filter(|_| self.extend)
            .cloned()
            .unwrap_or_default();
        let mut updates = Vec::new();
        for (key, item) in &self.items {
            let hash = WorldStateHash::from_json(&serde_json::json!(item));
            if let Some((old_hash, id)) = previous.and_then(|entries| entries.get(key))
                && *old_hash == hash
            {
                snapshot.insert(key.clone(), (hash, id.clone()));
                continue;
            }
            let Some(prefix) = item.id_prefix() else {
                unreachable!("additional context has an item ID")
            };
            let id = self.ids.next_item(prefix);
            let mut item = item.clone();
            item.set_id(Some(id.clone()));
            updates.push(WorldStateUpdate::item(item));
            snapshot.insert(key.clone(), (hash, id));
        }
        (Some(snapshot), updates)
    }
}

impl WorldStateSnapshot {
    /// Steps have no new client submission. Carry the delivered section forward unchanged.
    pub(crate) fn retain_input_context(&mut self, previous: &Self) {
        if let Some(context) = previous.section(AdditionalContextState::ID) {
            self.insert_section(AdditionalContextState::ID, context.clone());
        }
    }

    /// Keep active items in history order, refreshing ones shortened by generic retention.
    pub(crate) fn restore_input_context(
        &self,
        replaced: &[ResponseItemEnvelope],
        retained: &mut [ResponseItemEnvelope],
    ) -> Vec<ResponseItemEnvelope> {
        let Some(context) = self
            .section(AdditionalContextState::ID)
            .and_then(|section| Snapshot::deserialize(section).ok())
        else {
            return Vec::new();
        };
        let mut active: HashMap<_, _> = context.into_values().map(|(_, id)| (id, None)).collect();
        for item in retained {
            if let Some(slot) = item.id().and_then(|id| active.get_mut(id)) {
                *slot = Some(item);
            }
        }
        let mut missing = Vec::new();
        for item in replaced {
            match item.id().and_then(|id| active.remove(id)) {
                Some(Some(existing)) => existing.item.clone_from(&item.item),
                Some(None) => missing.push(item.clone()),
                None => {}
            }
        }
        missing
    }
}

fn message(role: &'static str, key: &str, text: String) -> ResponseItem {
    let content = AnnotatedContent::text(
        text,
        ContentItemKind(format!("additional_content.{key}")),
        ContentItemMetadata::harness(),
    );
    RenderedFragment::new(role, content).into()
}

fn tool_name_from_key(key: &str) -> String {
    let derived: String = key
        .chars()
        .map(|ch| if ch == ' ' { '_' } else { ch })
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .take(MAX_DERIVED_TOOL_NAME_CHARS)
        .collect();
    if derived.is_empty() {
        "untrusted_content".to_owned()
    } else {
        derived
    }
}

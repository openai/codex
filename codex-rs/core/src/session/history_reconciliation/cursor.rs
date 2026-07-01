//! Logical persisted-rollout cursors used to prove append and rewrite boundaries.

use crate::state::PersistedHistoryCursor;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_rollout::is_persisted_rollout_item;
use serde_json::Value;
use sha1::Digest;
use sha1::Sha1;

fn is_persisted_non_metadata_item(item: &RolloutItem) -> bool {
    is_persisted_rollout_item(item) && !matches!(item, RolloutItem::SessionMeta(_))
}

pub(super) fn is_persisted_history_rewrite_item(item: &RolloutItem) -> bool {
    matches!(
        item,
        RolloutItem::Compacted(_) | RolloutItem::EventMsg(EventMsg::ThreadRolledBack(_))
    )
}

pub(in crate::session) fn empty_persisted_history_cursor() -> PersistedHistoryCursor {
    PersistedHistoryCursor {
        item_count: 0,
        fingerprint: [0; 20],
    }
}

pub(in crate::session) fn persisted_history_cursor(
    items: &[RolloutItem],
) -> Option<PersistedHistoryCursor> {
    advance_persisted_history_cursor(empty_persisted_history_cursor(), items)
}

pub(super) fn advance_persisted_history_cursor(
    mut cursor: PersistedHistoryCursor,
    items: &[RolloutItem],
) -> Option<PersistedHistoryCursor> {
    for item in items
        .iter()
        .filter(|item| is_persisted_non_metadata_item(item))
    {
        let encoded = canonical_rollout_item_bytes(item)?;
        let encoded_len = u64::try_from(encoded.len()).ok()?;
        let mut hasher = Sha1::new();
        hasher.update(cursor.fingerprint);
        hasher.update(encoded_len.to_le_bytes());
        hasher.update(encoded);
        cursor.fingerprint = hasher.finalize().into();
        cursor.item_count = cursor.item_count.saturating_add(1);
    }
    Some(cursor)
}

fn canonical_rollout_item_bytes(item: &RolloutItem) -> Option<Vec<u8>> {
    let value = serde_json::to_value(item).ok()?;
    serde_json::to_vec(&canonicalize_json_value(value)).ok()
}

fn canonicalize_json_value(value: Value) -> Value {
    match value {
        Value::Array(values) => {
            Value::Array(values.into_iter().map(canonicalize_json_value).collect())
        }
        Value::Object(values) => {
            let mut entries = values.into_iter().collect::<Vec<_>>();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            let mut sorted = serde_json::Map::new();
            for (key, value) in entries {
                sorted.insert(key, canonicalize_json_value(value));
            }
            Value::Object(sorted)
        }
        primitive => primitive,
    }
}

pub(super) enum PersistedCursorComparison<'a> {
    Matched(&'a [RolloutItem]),
    Mismatched,
    Shorter,
}

/// Validates the known logical prefix, deliberately ignoring metadata-only `SessionMeta`
/// appends, then maps it back to a raw rollout suffix.
pub(super) fn persisted_suffix_after_cursor(
    items: &[RolloutItem],
    known_cursor: PersistedHistoryCursor,
) -> PersistedCursorComparison<'_> {
    let mut loaded_cursor = empty_persisted_history_cursor();
    if known_cursor.item_count == 0 {
        return if loaded_cursor == known_cursor {
            PersistedCursorComparison::Matched(items)
        } else {
            PersistedCursorComparison::Mismatched
        };
    }
    for (raw_index, item) in items.iter().enumerate() {
        if !is_persisted_non_metadata_item(item) {
            continue;
        }
        let Some(next_cursor) =
            advance_persisted_history_cursor(loaded_cursor, std::slice::from_ref(item))
        else {
            return PersistedCursorComparison::Mismatched;
        };
        loaded_cursor = next_cursor;
        if loaded_cursor.item_count == known_cursor.item_count {
            return if loaded_cursor.fingerprint == known_cursor.fingerprint {
                PersistedCursorComparison::Matched(&items[raw_index + 1..])
            } else {
                PersistedCursorComparison::Mismatched
            };
        }
    }
    PersistedCursorComparison::Shorter
}

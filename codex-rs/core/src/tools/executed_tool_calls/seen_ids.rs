//! Tracks seen call and runtime cell IDs with bounded memory. Bits are never cleared:
//! collisions can withhold a proof, but cannot make an observed ID appear fresh again.

use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use codex_code_mode::CellId;
use codex_history::InitialHistory;
use codex_history::RolloutItem;
use codex_protocol::models::ResponseItem;

const ID_FILTER_WORDS: usize = 8 * 1024;
const MAX_HISTORY_ENTRIES: usize = 100_000;
const MAX_HISTORY_ID_BYTES: usize = 4 * 1024 * 1024;
// IDs are untrusted strings. A longer ID cannot prove freshness without unbounded hashing.
const MAX_ID_BYTES: usize = 1024;

pub(super) struct SeenIds {
    bits: Box<[u64]>,
    history_ids_indexed: bool,
}

impl Default for SeenIds {
    fn default() -> Self {
        Self {
            bits: vec![0; ID_FILTER_WORDS].into_boxed_slice(),
            history_ids_indexed: true,
        }
    }
}

impl SeenIds {
    pub(super) fn unobserved_history() -> Self {
        // Recording started after session initialization, so earlier IDs are unknown.
        Self {
            history_ids_indexed: false,
            ..Self::default()
        }
    }

    pub(super) fn from_history(history: &InitialHistory) -> Self {
        // Index the supplied history, including paginated context. Missing older pages
        // do not invalidate a new cell whose calls and outputs are observed locally.
        let mut tracker = Self::default();
        let mut entries_left = MAX_HISTORY_ENTRIES;
        let mut id_bytes_left = MAX_HISTORY_ID_BYTES;
        'history: for item in history.get_rollout_items() {
            if entries_left == 0 {
                tracker.history_ids_indexed = false;
                break;
            }
            entries_left -= 1;
            match item {
                RolloutItem::ResponseItem(item)
                    if !tracker.observe_history_item(item, &mut id_bytes_left) =>
                {
                    tracker.history_ids_indexed = false;
                    break;
                }
                RolloutItem::Compacted(item) => {
                    for item in item.replacement_history.as_deref().unwrap_or_default() {
                        if entries_left == 0 {
                            tracker.history_ids_indexed = false;
                            break 'history;
                        }
                        entries_left -= 1;
                        if !tracker.observe_history_item(item, &mut id_bytes_left) {
                            tracker.history_ids_indexed = false;
                            break 'history;
                        }
                    }
                }
                _ => {}
            }
        }
        tracker
    }

    /// Fresh relative to IDs observed in the supplied history and this recorder.
    pub(super) fn observe_call_id(&mut self, id: &str) -> bool {
        self.observe(/*namespace*/ 0, id)
    }

    pub(super) fn observe_runtime_cell_id(&mut self, id: &CellId) -> bool {
        // Runtime handles are tracked only within this recorder's lifetime.
        self.observe(/*namespace*/ 1, id.as_str())
    }

    pub(super) fn history_ids_indexed(&self) -> bool {
        self.history_ids_indexed
    }

    fn observe_history_item(&mut self, item: &ResponseItem, bytes_left: &mut usize) -> bool {
        let call_id = super::input_call_id(item).or_else(|| super::output_call_id(item));
        // Metadata cell_id identifies the originating exec call, not a runtime handle.
        let exec_origin = item
            .executed_tool_call_metadata()
            .and_then(|metadata| metadata.cell_id.as_deref());
        for id in [call_id, exec_origin].into_iter().flatten() {
            if id.len() > MAX_ID_BYTES || id.len() > *bytes_left {
                return false;
            }
            *bytes_left -= id.len();
            self.observe_call_id(id);
        }
        true
    }

    fn observe(&mut self, namespace: u8, id: &str) -> bool {
        if id.len() > MAX_ID_BYTES {
            return false;
        }
        let mut hasher = DefaultHasher::new();
        (namespace, id).hash(&mut hasher);
        let hash = hasher.finish();
        let first = hash as usize;
        let step = (hash >> 32) as usize | 1;
        let mut fresh = false;
        for probe in 0_usize..4 {
            let bit = first.wrapping_add(probe.wrapping_mul(step)) % (ID_FILTER_WORDS * 64);
            let word = &mut self.bits[bit / 64];
            let mask = 1_u64 << (bit % 64);
            fresh |= *word & mask == 0;
            *word |= mask;
        }
        fresh
    }
}

#[cfg(test)]
#[path = "seen_ids_tests.rs"]
mod tests;

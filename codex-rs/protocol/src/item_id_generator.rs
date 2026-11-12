//! Response item ID allocation for a thread and its consumers.

use crate::ResponseItemId;
use std::sync::Arc;

/// Generates IDs for an item kind. Clones share the owning thread's sequence.
pub type ItemIdGenerator = Arc<dyn Fn(&str) -> ResponseItemId + Send + Sync>;

#[cfg(test)]
pub(super) fn uuid_v7_item_id_generator_for_test() -> ItemIdGenerator {
    Arc::new(|prefix| ResponseItemId::with_suffix(prefix, uuid::Uuid::now_v7()))
}

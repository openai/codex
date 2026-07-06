use chrono::DateTime;
use chrono::Utc;
use codex_protocol::ThreadId;
use serde::Deserialize;
use serde::Serialize;

use crate::SortDirection;

/// Optional filters for listing persisted items.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListItemsFilters {
    /// Optional turn id to filter by. When omitted, returns items across the thread.
    pub turn_id: Option<String>,

    /// Optional item update timestamp filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<UpdatedAtFilter>,
}

/// Filters item snapshots to updates after an exclusive timestamp watermark.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatedAtFilter {
    /// Exclusive lower bound: `updated_at > gt`.
    pub gt: DateTime<Utc>,
}

/// Parameters for listing persisted items within a thread.
///
/// Callers must reuse the same filters when following a page cursor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListItemsParams {
    /// Thread id to read.
    pub thread_id: ThreadId,
    /// Whether archived threads are eligible.
    pub include_archived: bool,
    /// Filters applied to the listed items.
    ///
    /// Flattening preserves the existing serialized `turn_id` field.
    #[serde(flatten)]
    pub filters: ListItemsFilters,
    /// Opaque cursor returned by a previous list call.
    pub cursor: Option<String>,
    /// Maximum number of items to return.
    pub page_size: usize,
    /// Sort direction requested by the caller.
    pub sort_direction: SortDirection,
}

/// A projected app-server `ThreadItem` snapshot within a turn.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredThreadItem {
    pub turn_id: Option<String>,
    pub item_key: String,
    pub item_ordinal: u64,
    pub item_created_at_ms: i64,
    pub materialized_thread_item_json: Vec<u8>,
    /// Storage-assigned update timestamp used by incremental consumers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
}

/// A page of persisted items within a thread, optionally filtered to a turn.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemPage {
    /// Items returned for this page.
    pub items: Vec<StoredThreadItem>,
    /// Opaque cursor to continue listing.
    pub next_cursor: Option<String>,
    /// Opaque cursor for fetching in the opposite direction.
    pub backwards_cursor: Option<String>,
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    #[test]
    fn list_items_filters_preserve_flattened_turn_id_shape() {
        let params = ListItemsParams {
            thread_id: ThreadId::default(),
            include_archived: true,
            filters: ListItemsFilters {
                turn_id: Some("turn-1".to_string()),
                updated_at: None,
            },
            cursor: Some("next-page".to_string()),
            page_size: 25,
            sort_direction: SortDirection::Asc,
        };

        let value = serde_json::to_value(&params).expect("serialize list items params");
        assert_eq!(value["turn_id"], json!("turn-1"));
        assert_eq!(value.get("filters"), None);
        assert_eq!(value.get("updated_at"), None);

        let decoded: ListItemsParams =
            serde_json::from_value(value).expect("deserialize legacy-shaped list items params");
        assert_eq!(decoded, params);
    }

    #[test]
    fn stored_item_accepts_legacy_payload_without_updated_at() {
        let value = json!({
            "turn_id": "turn-1",
            "item_key": "item-1",
            "item_ordinal": 1,
            "item_created_at_ms": 2,
            "materialized_thread_item_json": [123, 125]
        });

        let item: StoredThreadItem =
            serde_json::from_value(value).expect("deserialize legacy stored item");
        assert_eq!(item.updated_at, None);
    }
}

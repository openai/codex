//! Protocol fixtures and text assertions shared by test targets.

use codex_protocol::ItemIdGenerator;
use codex_protocol::ResponseItemId;
use codex_protocol::models::ContentItem;
use std::sync::Arc;

pub fn uuid_v7_item_id_generator_for_test() -> ItemIdGenerator {
    Arc::new(|prefix| ResponseItemId::with_suffix(prefix, uuid::Uuid::now_v7()))
}

/// Assertion helpers for tests that require text rather than media content.
/// Implementations should fail at the calling assertion for non-text content.
pub trait ContentItemTestExt {
    /// Extracts text, panicking at the call site when the content is not text.
    #[track_caller]
    fn expect_text(self) -> String;
}

impl ContentItemTestExt for ContentItem {
    #[track_caller]
    fn expect_text(self) -> String {
        match self.into_text() {
            Some(text) => text,
            None => panic!("expected text content"),
        }
    }
}

//! Protocol fixtures and text assertions shared by test targets.

use codex_protocol::models::ContentItem;

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

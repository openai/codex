//! Helpers for extension-owned hidden context messages.

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;

/// Marker pair for extension-owned hidden model-visible context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HiddenContextMarker {
    start_marker: &'static str,
    end_marker: &'static str,
}

impl HiddenContextMarker {
    /// Creates a marker pair for one extension-owned hidden context shape.
    pub const fn new(start_marker: &'static str, end_marker: &'static str) -> Self {
        Self {
            start_marker,
            end_marker,
        }
    }

    /// Renders this marker pair as a user-role response input item.
    pub fn response_input_item(&self, body: impl AsRef<str>) -> ResponseInputItem {
        ResponseInputItem::Message {
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: format!(
                    "{}\n{}\n{}",
                    self.start_marker,
                    body.as_ref(),
                    self.end_marker
                ),
            }],
            phase: None,
        }
    }
}

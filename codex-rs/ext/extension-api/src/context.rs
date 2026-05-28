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
}

/// Extension-owned hidden context body with the marker pair used to wrap it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenContext {
    marker: HiddenContextMarker,
    body: String,
}

impl HiddenContext {
    /// Creates one hidden context body with its wrapping marker pair.
    pub fn new(marker: HiddenContextMarker, body: impl Into<String>) -> Self {
        Self {
            marker,
            body: body.into(),
        }
    }

    /// Returns the unwrapped hidden context body.
    pub fn body(&self) -> &str {
        &self.body
    }

    /// Splits this context into its marker pair and unwrapped body.
    pub fn into_parts(self) -> (HiddenContextMarker, String) {
        (self.marker, self.body)
    }

    /// Renders this hidden context as a user-role response input item.
    pub fn into_response_input_item(self) -> ResponseInputItem {
        ResponseInputItem::Message {
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: format!(
                    "{}\n{}\n{}",
                    self.marker.start_marker, self.body, self.marker.end_marker
                ),
            }],
            phase: None,
        }
    }
}

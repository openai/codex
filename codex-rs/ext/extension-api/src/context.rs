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

    /// Returns true when text is wrapped in this marker pair.
    pub fn matches_text(self, text: &str) -> bool {
        if self.start_marker.is_empty() || self.end_marker.is_empty() {
            return false;
        }

        let trimmed = text.trim_start();
        let starts_with_marker = trimmed
            .get(..self.start_marker.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(self.start_marker));
        let trimmed = trimmed.trim_end();
        let ends_with_marker = trimmed
            .get(trimmed.len().saturating_sub(self.end_marker.len())..)
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(self.end_marker));
        starts_with_marker && ends_with_marker
    }
}

/// Compile-time registration for extension-owned hidden context markers.
///
/// Extensions use this to reserve their hidden context wire tags without adding
/// feature-specific tags to core parsing code.
pub struct HiddenContextMarkerRegistration {
    pub marker: HiddenContextMarker,
}

inventory::collect!(HiddenContextMarkerRegistration);

/// Returns all hidden context markers registered by linked extensions.
pub fn registered_hidden_context_markers() -> impl Iterator<Item = HiddenContextMarker> {
    inventory::iter::<HiddenContextMarkerRegistration>
        .into_iter()
        .map(|registration| registration.marker)
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

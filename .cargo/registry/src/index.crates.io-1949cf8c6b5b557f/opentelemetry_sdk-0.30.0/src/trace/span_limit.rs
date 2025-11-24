/// # Span limit
/// Erroneous code can add unintended attributes, events, and links to a span. If these collections
/// are unbounded, they can quickly exhaust available memory, resulting in crashes that are
/// difficult to recover from safely.
///
/// To protected against those errors. Users can use span limit to configure
///  - Maximum allowed span attribute count
///  - Maximum allowed span event count
///  - Maximum allowed span link count
///  - Maximum allowed attribute per span event count
///  - Maximum allowed attribute per span link count
///
/// If the limit has been breached. The attributes, events or links will be dropped based on their
/// index in the collection. The one added to collections later will be dropped first.
pub(crate) const DEFAULT_MAX_EVENT_PER_SPAN: u32 = 128;
pub(crate) const DEFAULT_MAX_ATTRIBUTES_PER_SPAN: u32 = 128;
pub(crate) const DEFAULT_MAX_LINKS_PER_SPAN: u32 = 128;
pub(crate) const DEFAULT_MAX_ATTRIBUTES_PER_EVENT: u32 = 128;
pub(crate) const DEFAULT_MAX_ATTRIBUTES_PER_LINK: u32 = 128;

/// Span limit configuration to keep attributes, events and links to a span in a reasonable number.
#[derive(Copy, Clone, Debug)]
pub struct SpanLimits {
    /// The max events that can be added to a `Span`.
    pub max_events_per_span: u32,
    /// The max attributes that can be added to a `Span`.
    pub max_attributes_per_span: u32,
    /// The max links that can be added to a `Span`.
    pub max_links_per_span: u32,
    /// The max attributes that can be added into an `Event`
    pub max_attributes_per_event: u32,
    /// The max attributes that can be added into a `Link`
    pub max_attributes_per_link: u32,
}

impl Default for SpanLimits {
    fn default() -> Self {
        SpanLimits {
            max_events_per_span: DEFAULT_MAX_EVENT_PER_SPAN,
            max_attributes_per_span: DEFAULT_MAX_ATTRIBUTES_PER_SPAN,
            max_links_per_span: DEFAULT_MAX_LINKS_PER_SPAN,
            max_attributes_per_link: DEFAULT_MAX_ATTRIBUTES_PER_LINK,
            max_attributes_per_event: DEFAULT_MAX_ATTRIBUTES_PER_EVENT,
        }
    }
}

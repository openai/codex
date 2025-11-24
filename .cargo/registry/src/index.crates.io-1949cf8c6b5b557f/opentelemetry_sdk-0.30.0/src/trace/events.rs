//! # Span Events

use std::ops::Deref;

use opentelemetry::trace::Event;
/// Stores span events along with dropped count.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SpanEvents {
    /// The events stored as a vector. Could be empty if there are no events.
    pub events: Vec<Event>,
    /// The number of Events dropped from the span.
    pub dropped_count: u32,
}

impl Deref for SpanEvents {
    type Target = [Event];

    fn deref(&self) -> &Self::Target {
        &self.events
    }
}

impl IntoIterator for SpanEvents {
    type Item = Event;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.events.into_iter()
    }
}

impl SpanEvents {
    pub(crate) fn add_event(&mut self, event: Event) {
        self.events.push(event);
    }
}

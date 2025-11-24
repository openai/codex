use std::ops::{Deref, DerefMut};

#[cfg(feature = "web-time")]
use web_time::Instant;

#[cfg(not(feature = "web-time"))]
use std::time::Instant;

use crate::event::Event;

/// A debounced event is emitted after a short delay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebouncedEvent {
    /// The original event.
    pub event: Event,

    /// The time at which the event occurred.
    pub time: Instant,
}

impl DebouncedEvent {
    pub fn new(event: Event, time: Instant) -> Self {
        Self { event, time }
    }
}

impl Deref for DebouncedEvent {
    type Target = Event;

    fn deref(&self) -> &Self::Target {
        &self.event
    }
}

impl DerefMut for DebouncedEvent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.event
    }
}

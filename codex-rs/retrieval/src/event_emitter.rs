//! Global event broadcaster for the retrieval system.
//!
//! Provides a singleton event emitter that broadcasts `RetrievalEvent`s to all
//! registered consumers. This enables decoupled event handling where:
//! - Core retrieval code emits events without knowing consumers
//! - Multiple consumers (TUI, CLI, logging) can receive events concurrently
//!
//! # Usage
//!
//! ```ignore
//! use codex_retrieval::event_emitter::{EventEmitter, subscribe};
//! use codex_retrieval::events::{RetrievalEvent, EventConsumer};
//!
//! // Subscribe to events
//! let mut rx = subscribe();
//!
//! // Emit an event (from anywhere in the crate)
//! EventEmitter::emit(RetrievalEvent::SearchStarted { ... });
//!
//! // Receive events
//! while let Ok(event) = rx.recv() {
//!     println!("Received: {:?}", event);
//! }
//! ```

use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::RwLock;

use tokio::sync::broadcast;

use crate::events::EventConsumer;
use crate::events::RetrievalEvent;

/// Default channel capacity for event broadcast.
const DEFAULT_CHANNEL_CAPACITY: usize = 256;

/// Global event emitter singleton.
static EMITTER: OnceLock<EventEmitter> = OnceLock::new();

/// Event emitter for broadcasting retrieval events.
///
/// The emitter uses a tokio broadcast channel to support multiple receivers.
/// Events are cloned to each receiver, so ensure event types are cheap to clone.
pub struct EventEmitter {
    /// Broadcast sender for events.
    sender: broadcast::Sender<RetrievalEvent>,

    /// Registered synchronous consumers (for non-async contexts).
    sync_consumers: RwLock<Vec<Arc<RwLock<dyn EventConsumer>>>>,

    /// Whether event emission is enabled.
    enabled: RwLock<bool>,
}

impl EventEmitter {
    /// Create a new event emitter with default capacity.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CHANNEL_CAPACITY)
    }

    /// Create a new event emitter with specified channel capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            sync_consumers: RwLock::new(Vec::new()),
            enabled: RwLock::new(true),
        }
    }

    /// Get the global event emitter instance.
    pub fn global() -> &'static EventEmitter {
        EMITTER.get_or_init(EventEmitter::new)
    }

    /// Emit an event to all subscribers.
    ///
    /// This is the primary method for emitting events. It:
    /// 1. Sends to all async broadcast receivers
    /// 2. Calls all registered sync consumers
    ///
    /// Returns the number of async receivers that received the event.
    pub fn emit(event: RetrievalEvent) -> i32 {
        Self::global().emit_internal(event)
    }

    /// Subscribe to events (async receiver).
    ///
    /// Returns a broadcast receiver that will receive all future events.
    /// Note: Events emitted before subscription are not received.
    pub fn subscribe() -> broadcast::Receiver<RetrievalEvent> {
        Self::global().sender.subscribe()
    }

    /// Register a synchronous event consumer.
    ///
    /// The consumer's `on_event` method will be called for each emitted event.
    /// Use this for consumers that need synchronous notification (e.g., logging).
    pub fn register_consumer(consumer: Arc<RwLock<dyn EventConsumer>>) {
        if let Ok(mut consumers) = Self::global().sync_consumers.write() {
            consumers.push(consumer);
        }
    }

    /// Clear all registered synchronous consumers.
    pub fn clear_consumers() {
        if let Ok(mut consumers) = Self::global().sync_consumers.write() {
            consumers.clear();
        }
    }

    /// Enable or disable event emission.
    ///
    /// When disabled, `emit()` becomes a no-op. Useful for benchmarking
    /// or high-performance scenarios where event overhead is unacceptable.
    pub fn set_enabled(enabled: bool) {
        if let Ok(mut flag) = Self::global().enabled.write() {
            *flag = enabled;
        }
    }

    /// Check if event emission is enabled.
    pub fn is_enabled() -> bool {
        Self::global()
            .enabled
            .read()
            .map(|flag| *flag)
            .unwrap_or(true)
    }

    /// Get the number of active async subscribers.
    pub fn subscriber_count() -> usize {
        Self::global().sender.receiver_count()
    }

    /// Internal emit implementation.
    fn emit_internal(&self, event: RetrievalEvent) -> i32 {
        // Check if emission is enabled
        if let Ok(enabled) = self.enabled.read() {
            if !*enabled {
                return 0;
            }
        }

        // Send to async receivers
        let async_count = self.sender.send(event.clone()).unwrap_or(0);

        // Notify sync consumers
        if let Ok(consumers) = self.sync_consumers.read() {
            for consumer in consumers.iter() {
                if let Ok(mut c) = consumer.write() {
                    c.on_event(&event);
                }
            }
        }

        async_count as i32
    }
}

impl Default for EventEmitter {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function to subscribe to events.
///
/// Equivalent to `EventEmitter::subscribe()`.
pub fn subscribe() -> broadcast::Receiver<RetrievalEvent> {
    EventEmitter::subscribe()
}

/// Convenience function to emit an event.
///
/// Equivalent to `EventEmitter::emit(event)`.
pub fn emit(event: RetrievalEvent) -> i32 {
    EventEmitter::emit(event)
}

// ============================================================================
// Scoped Emitter for Testing
// ============================================================================

/// A scoped event collector for testing.
///
/// Collects all events emitted during its lifetime. Useful for verifying
/// that operations emit the expected events.
///
/// # Example
///
/// ```ignore
/// let collector = ScopedEventCollector::new();
///
/// // Do something that emits events
/// service.search("query").await?;
///
/// let events = collector.events();
/// assert!(events.iter().any(|e| matches!(e, RetrievalEvent::SearchStarted { .. })));
/// ```
pub struct ScopedEventCollector {
    receiver: std::sync::Mutex<broadcast::Receiver<RetrievalEvent>>,
    collected: std::sync::Mutex<Vec<RetrievalEvent>>,
}

impl ScopedEventCollector {
    /// Create a new scoped collector.
    pub fn new() -> Self {
        Self {
            receiver: std::sync::Mutex::new(EventEmitter::subscribe()),
            collected: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Collect all pending events.
    ///
    /// Call this after operations complete to gather emitted events.
    pub fn collect(&self) {
        // Drain the receiver we were given
        if let Ok(mut receiver) = self.receiver.lock() {
            loop {
                match receiver.try_recv() {
                    Ok(event) => {
                        if let Ok(mut collected) = self.collected.lock() {
                            collected.push(event);
                        }
                    }
                    Err(broadcast::error::TryRecvError::Empty) => break,
                    Err(broadcast::error::TryRecvError::Lagged(n)) => {
                        tracing::warn!(lagged = n, "Event collector lagged, some events missed");
                    }
                    Err(broadcast::error::TryRecvError::Closed) => break,
                }
            }
        }
    }

    /// Get all collected events.
    pub fn events(&self) -> Vec<RetrievalEvent> {
        self.collect();
        self.collected.lock().map(|v| v.clone()).unwrap_or_default()
    }

    /// Check if any event matches a predicate.
    pub fn has_event<F>(&self, predicate: F) -> bool
    where
        F: Fn(&RetrievalEvent) -> bool,
    {
        self.events().iter().any(predicate)
    }

    /// Count events matching a predicate.
    pub fn count_events<F>(&self, predicate: F) -> usize
    where
        F: Fn(&RetrievalEvent) -> bool,
    {
        self.events().iter().filter(|e| predicate(e)).count()
    }

    /// Get events of a specific type.
    pub fn events_of_type(&self, event_type: &str) -> Vec<RetrievalEvent> {
        self.events()
            .into_iter()
            .filter(|e| e.event_type() == event_type)
            .collect()
    }
}

impl Default for ScopedEventCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::SearchMode;

    #[test]
    fn test_emit_and_subscribe() {
        let mut rx = EventEmitter::subscribe();

        let event = RetrievalEvent::SearchStarted {
            query_id: "q-1".to_string(),
            query: "test".to_string(),
            mode: SearchMode::Hybrid,
            limit: 10,
        };

        let count = EventEmitter::emit(event.clone());
        assert!(count >= 1);

        // Receive the event
        let received = rx.try_recv().unwrap();
        assert_eq!(received.event_type(), "search_started");
    }

    #[test]
    fn test_enable_disable() {
        // Ensure enabled by default
        assert!(EventEmitter::is_enabled());

        // Disable
        EventEmitter::set_enabled(false);
        assert!(!EventEmitter::is_enabled());

        // Emit should be no-op when disabled
        let event = RetrievalEvent::SessionEnded {
            session_id: "s-1".to_string(),
            duration_ms: 100,
        };
        let count = EventEmitter::emit(event);
        assert_eq!(count, 0);

        // Re-enable
        EventEmitter::set_enabled(true);
        assert!(EventEmitter::is_enabled());
    }

    #[test]
    fn test_subscriber_count() {
        let initial = EventEmitter::subscriber_count();

        let _rx1 = EventEmitter::subscribe();
        assert_eq!(EventEmitter::subscriber_count(), initial + 1);

        let _rx2 = EventEmitter::subscribe();
        assert_eq!(EventEmitter::subscriber_count(), initial + 2);

        drop(_rx1);
        // Note: receiver_count may not immediately reflect dropped receivers
    }

    #[test]
    fn test_scoped_collector() {
        let collector = ScopedEventCollector::new();

        // Emit some events
        emit(RetrievalEvent::SearchStarted {
            query_id: "q-1".to_string(),
            query: "test1".to_string(),
            mode: SearchMode::Bm25,
            limit: 5,
        });

        emit(RetrievalEvent::SearchCompleted {
            query_id: "q-1".to_string(),
            results: vec![],
            total_duration_ms: 50,
        });

        // Check collected events
        let events = collector.events();
        assert!(events.len() >= 2);
        assert!(collector.has_event(|e| matches!(e, RetrievalEvent::SearchStarted { .. })));
        assert!(collector.has_event(|e| matches!(e, RetrievalEvent::SearchCompleted { .. })));
    }

    #[test]
    fn test_events_of_type() {
        let collector = ScopedEventCollector::new();

        emit(RetrievalEvent::SearchStarted {
            query_id: "q-1".to_string(),
            query: "test".to_string(),
            mode: SearchMode::Vector,
            limit: 10,
        });

        emit(RetrievalEvent::SessionEnded {
            session_id: "s-1".to_string(),
            duration_ms: 100,
        });

        let search_events = collector.events_of_type("search_started");
        assert!(!search_events.is_empty());

        let session_events = collector.events_of_type("session_ended");
        assert!(!session_events.is_empty());
    }
}

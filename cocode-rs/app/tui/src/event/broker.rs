//! Event broker for controlling stdin reading.
//!
//! The [`EventBroker`] manages the crossterm event stream, allowing it to be
//! paused and resumed. This is essential for:
//!
//! - Launching external editors (vim, nano, etc.)
//! - Running external processes that need stdin
//! - Preventing stdin conflicts during subprocess execution

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

/// Controls stdin reading for the TUI.
///
/// The broker allows pausing and resuming the crossterm event stream,
/// which is necessary when launching external editors or processes
/// that need to read from stdin.
///
/// # Example
///
/// ```ignore
/// let broker = EventBroker::new();
///
/// // Pause stdin reading before launching editor
/// broker.pause();
///
/// // Run external editor
/// run_editor().await;
///
/// // Resume stdin reading
/// broker.resume();
/// ```
#[derive(Debug)]
pub struct EventBroker {
    /// Whether events are currently paused.
    paused: AtomicBool,
    /// Depth counter for nested pause/resume calls.
    pause_depth: std::sync::atomic::AtomicI32,
}

impl EventBroker {
    /// Create a new event broker.
    pub fn new() -> Self {
        Self {
            paused: AtomicBool::new(false),
            pause_depth: std::sync::atomic::AtomicI32::new(0),
        }
    }

    /// Pause stdin reading.
    ///
    /// This increments the pause depth. Stdin remains paused until
    /// `resume()` is called an equal number of times.
    pub fn pause(&self) {
        let prev_depth = self.pause_depth.fetch_add(1, Ordering::SeqCst);
        if prev_depth == 0 {
            self.paused.store(true, Ordering::SeqCst);
            tracing::debug!("EventBroker: paused stdin reading");
        }
    }

    /// Resume stdin reading.
    ///
    /// This decrements the pause depth. Stdin resumes when the depth
    /// reaches zero.
    pub fn resume(&self) {
        let prev_depth = self.pause_depth.fetch_sub(1, Ordering::SeqCst);
        if prev_depth == 1 {
            self.paused.store(false, Ordering::SeqCst);
            tracing::debug!("EventBroker: resumed stdin reading");
        } else if prev_depth <= 0 {
            // Underflow protection - reset to 0
            self.pause_depth.store(0, Ordering::SeqCst);
            self.paused.store(false, Ordering::SeqCst);
            tracing::warn!("EventBroker: resume called more times than pause");
        }
    }

    /// Check if events are currently paused.
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    /// Get the current pause depth.
    pub fn pause_depth(&self) -> i32 {
        self.pause_depth.load(Ordering::SeqCst)
    }

    /// Force resume, resetting the pause depth to 0.
    ///
    /// Use this when you need to ensure stdin is resumed regardless
    /// of the current pause depth (e.g., on panic recovery).
    pub fn force_resume(&self) {
        self.pause_depth.store(0, Ordering::SeqCst);
        self.paused.store(false, Ordering::SeqCst);
        tracing::debug!("EventBroker: force resumed stdin reading");
    }
}

impl Default for EventBroker {
    fn default() -> Self {
        Self::new()
    }
}

/// A guard that pauses events on creation and resumes on drop.
///
/// This is useful for ensuring events are resumed even if a function
/// returns early or panics.
#[allow(dead_code)]
pub struct PauseGuard {
    broker: Arc<EventBroker>,
}

#[allow(dead_code)]
impl PauseGuard {
    /// Create a new pause guard, pausing the broker.
    pub fn new(broker: Arc<EventBroker>) -> Self {
        broker.pause();
        Self { broker }
    }
}

impl Drop for PauseGuard {
    fn drop(&mut self) {
        self.broker.resume();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_broker_pause_resume() {
        let broker = EventBroker::new();

        assert!(!broker.is_paused());
        assert_eq!(broker.pause_depth(), 0);

        broker.pause();
        assert!(broker.is_paused());
        assert_eq!(broker.pause_depth(), 1);

        broker.resume();
        assert!(!broker.is_paused());
        assert_eq!(broker.pause_depth(), 0);
    }

    #[test]
    fn test_broker_nested_pause() {
        let broker = EventBroker::new();

        broker.pause();
        assert!(broker.is_paused());
        assert_eq!(broker.pause_depth(), 1);

        broker.pause();
        assert!(broker.is_paused());
        assert_eq!(broker.pause_depth(), 2);

        broker.resume();
        assert!(broker.is_paused()); // Still paused
        assert_eq!(broker.pause_depth(), 1);

        broker.resume();
        assert!(!broker.is_paused());
        assert_eq!(broker.pause_depth(), 0);
    }

    #[test]
    fn test_broker_force_resume() {
        let broker = EventBroker::new();

        broker.pause();
        broker.pause();
        assert_eq!(broker.pause_depth(), 2);

        broker.force_resume();
        assert!(!broker.is_paused());
        assert_eq!(broker.pause_depth(), 0);
    }

    #[test]
    fn test_broker_underflow_protection() {
        let broker = EventBroker::new();

        // Resume without pause should not underflow
        broker.resume();
        assert!(!broker.is_paused());
        assert_eq!(broker.pause_depth(), 0);

        broker.resume();
        assert_eq!(broker.pause_depth(), 0);
    }

    #[test]
    fn test_pause_guard() {
        let broker = Arc::new(EventBroker::new());

        assert!(!broker.is_paused());

        {
            let _guard = PauseGuard::new(broker.clone());
            assert!(broker.is_paused());
        }

        // Guard dropped, should be resumed
        assert!(!broker.is_paused());
    }
}

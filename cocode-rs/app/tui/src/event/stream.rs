//! Async event stream for the TUI.
//!
//! This module provides [`TuiEventStream`], an async stream that combines:
//! - Crossterm terminal events (keyboard, mouse, resize)
//! - Draw requests from the frame scheduler
//! - Periodic tick events for animations
//! - Agent events from the core loop

use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

use crossterm::event::Event as CrosstermEvent;
use crossterm::event::EventStream;
use crossterm::event::KeyEventKind;
use futures::Stream;
use tokio::sync::broadcast;
use tokio::time::Interval;
use tokio::time::interval;

use super::TuiEvent;
use super::broker::EventBroker;

/// Configuration for the event stream.
pub struct EventStreamConfig {
    /// Interval between tick events (for animations).
    pub tick_interval: Duration,
    /// Interval between draw events (frame rate).
    pub draw_interval: Duration,
}

impl Default for EventStreamConfig {
    fn default() -> Self {
        Self {
            tick_interval: Duration::from_millis(250),
            draw_interval: Duration::from_millis(16), // ~60 FPS
        }
    }
}

/// Async stream of TUI events.
///
/// This stream combines multiple event sources into a single unified stream:
/// - Terminal events from crossterm
/// - Draw requests from the frame scheduler
/// - Tick events for animations
///
/// The stream respects the [`EventBroker`]'s pause state, suspending
/// terminal event reading when paused.
pub struct TuiEventStream {
    /// The event broker controlling stdin reading.
    broker: Arc<EventBroker>,
    /// Crossterm event stream (recreated when resumed).
    event_stream: Option<EventStream>,
    /// Receiver for draw requests.
    draw_rx: broadcast::Receiver<()>,
    /// Tick interval timer.
    tick_interval: Interval,
    /// Whether the terminal is focused.
    terminal_focused: Arc<AtomicBool>,
    /// Whether we're in the process of polling.
    #[allow(dead_code)]
    polling: bool,
}

impl TuiEventStream {
    /// Create a new event stream.
    ///
    /// # Arguments
    ///
    /// * `broker` - The event broker for pause/resume control
    /// * `draw_rx` - Receiver for draw requests
    /// * `terminal_focused` - Atomic flag tracking terminal focus
    pub fn new(
        broker: Arc<EventBroker>,
        draw_rx: broadcast::Receiver<()>,
        terminal_focused: Arc<AtomicBool>,
    ) -> Self {
        Self::with_config(
            broker,
            draw_rx,
            terminal_focused,
            EventStreamConfig::default(),
        )
    }

    /// Create a new event stream with custom configuration.
    pub fn with_config(
        broker: Arc<EventBroker>,
        draw_rx: broadcast::Receiver<()>,
        terminal_focused: Arc<AtomicBool>,
        config: EventStreamConfig,
    ) -> Self {
        let event_stream = if !broker.is_paused() {
            Some(EventStream::new())
        } else {
            None
        };

        Self {
            broker,
            event_stream,
            draw_rx,
            tick_interval: interval(config.tick_interval),
            terminal_focused,
            polling: false,
        }
    }

    /// Check if the event stream should be active.
    fn should_read_events(&self) -> bool {
        !self.broker.is_paused()
    }

    /// Ensure the event stream exists if we should be reading.
    fn ensure_event_stream(&mut self) {
        if self.should_read_events() && self.event_stream.is_none() {
            self.event_stream = Some(EventStream::new());
        } else if !self.should_read_events() && self.event_stream.is_some() {
            self.event_stream = None;
        }
    }

    /// Convert a crossterm event to a TUI event.
    fn convert_event(&self, event: CrosstermEvent) -> Option<TuiEvent> {
        match event {
            CrosstermEvent::Key(key) => {
                // Only process key press events (not release)
                if key.kind == KeyEventKind::Press {
                    Some(TuiEvent::Key(key))
                } else {
                    None
                }
            }
            CrosstermEvent::Mouse(mouse) => Some(TuiEvent::Mouse(mouse)),
            CrosstermEvent::Resize(width, height) => Some(TuiEvent::Resize { width, height }),
            CrosstermEvent::FocusGained => {
                self.terminal_focused.store(true, Ordering::Relaxed);
                Some(TuiEvent::FocusChanged { focused: true })
            }
            CrosstermEvent::FocusLost => {
                self.terminal_focused.store(false, Ordering::Relaxed);
                Some(TuiEvent::FocusChanged { focused: false })
            }
            CrosstermEvent::Paste(text) => Some(TuiEvent::Paste(text)),
        }
    }
}

impl Stream for TuiEventStream {
    type Item = TuiEvent;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Ensure event stream state matches broker state
        self.ensure_event_stream();

        // Check for draw requests first (highest priority)
        if let Ok(()) = self.draw_rx.try_recv() {
            return Poll::Ready(Some(TuiEvent::Draw));
        }

        // Check tick interval
        if self.tick_interval.poll_tick(cx).is_ready() {
            return Poll::Ready(Some(TuiEvent::Tick));
        }

        // Poll terminal events if not paused
        if let Some(ref mut stream) = self.event_stream {
            // Pin the stream to poll it
            let stream = Pin::new(stream);
            match stream.poll_next(cx) {
                Poll::Ready(Some(Ok(event))) => {
                    if let Some(tui_event) = self.convert_event(event) {
                        return Poll::Ready(Some(tui_event));
                    }
                    // Event was filtered, wake up to try again
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
                Poll::Ready(Some(Err(e))) => {
                    tracing::error!("Crossterm event error: {e}");
                    // Continue polling
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
                Poll::Ready(None) => {
                    // Stream ended (shouldn't happen normally)
                    return Poll::Ready(None);
                }
                Poll::Pending => {}
            }
        }

        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_stream_config_default() {
        let config = EventStreamConfig::default();
        assert_eq!(config.tick_interval, Duration::from_millis(250));
        assert_eq!(config.draw_interval, Duration::from_millis(16));
    }

    #[test]
    fn test_broker_pause_state() {
        // Test broker pause/resume logic without creating EventStream
        // (EventStream requires a real terminal)
        let broker = Arc::new(EventBroker::new());

        assert!(!broker.is_paused());

        broker.pause();
        assert!(broker.is_paused());

        broker.resume();
        assert!(!broker.is_paused());
    }
}

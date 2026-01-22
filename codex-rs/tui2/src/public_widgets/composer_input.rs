//! Public wrapper around the internal ChatComposer for simple, reusable text input.
//!
//! This exposes a minimal interface suitable for other crates (e.g.,
//! codex-cloud-tasks) to reuse the mature composer behavior: multi-line input,
//! paste heuristics, Enter-to-submit, and Shift+Enter for newline.

use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::time::Duration;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::ChatComposer;
use crate::bottom_pane::InputResult;
use crate::render::renderable::Renderable;

/// Action returned from feeding a key event into the ComposerInput.
pub enum ComposerAction {
    /// The user submitted the current text (typically via Enter). Contains the submitted text.
    Submitted(String),
    /// No submission occurred; UI may need to redraw if `needs_redraw()` returned true.
    None,
}

/// A minimal, public wrapper for the internal `ChatComposer` that behaves as a
/// reusable text input field with submit semantics.
pub struct ComposerInput {
    inner: ChatComposer,
    _tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    rx: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
}

impl ComposerInput {
    /// Create a new composer input with a neutral placeholder.
    pub fn new() -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let sender = AppEventSender::new(tx.clone());
        // `enhanced_keys_supported=true` enables Shift+Enter newline hint/behavior.
        let inner = ChatComposer::new(true, sender, true, "Compose new task".to_string(), false);
        Self { inner, _tx: tx, rx }
    }

    /// Returns true if the input is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Clear the input text.
    pub fn clear(&mut self) {
        self.inner.set_text_content(String::new());
    }

    /// Feed a key event into the composer and return a high-level action.
    pub fn input(&mut self, key: KeyEvent) -> ComposerAction {
        let action = match self.inner.handle_key_event(key).0 {
            InputResult::Submitted(text) => ComposerAction::Submitted(text),
            _ => ComposerAction::None,
        };
        self.drain_app_events();
        action
    }

    pub fn handle_paste(&mut self, pasted: String) -> bool {
        let handled = self.inner.handle_paste(pasted);
        self.drain_app_events();
        handled
    }

    /// Override the footer hint items displayed under the composer.
    /// Each tuple is rendered as "<key> <label>", with keys styled.
    pub fn set_hint_items(&mut self, items: Vec<(impl Into<String>, impl Into<String>)>) {
        let mapped: Vec<(String, String)> = items
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        self.inner.set_footer_hint_override(Some(mapped));
    }

    /// Clear any previously set custom hint items and restore the default hints.
    pub fn clear_hint_items(&mut self) {
        self.inner.set_footer_hint_override(None);
    }

    /// Desired height (in rows) for a given width.
    pub fn desired_height(&self, width: u16) -> u16 {
        self.inner.desired_height(width)
    }

    /// Compute the on-screen cursor position for the given area.
    pub fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.inner.cursor_pos(area)
    }

    /// Render the input into the provided buffer at `area`.
    pub fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        self.inner.render(area, buf);
    }

    /// Return true if a paste-burst detection is currently active.
    pub fn is_in_paste_burst(&self) -> bool {
        self.inner.is_in_paste_burst()
    }

    /// Flush a pending paste-burst if the inter-key timeout has elapsed.
    /// Returns true if text changed and a redraw is warranted.
    pub fn flush_paste_burst_if_due(&mut self) -> bool {
        let flushed = self.inner.flush_paste_burst_if_due();
        self.drain_app_events();
        flushed
    }

    /// Recommended delay to schedule the next micro-flush frame while a
    /// paste-burst is active.
    pub fn recommended_flush_delay() -> Duration {
        crate::bottom_pane::ChatComposer::recommended_paste_flush_delay()
    }

    fn drain_app_events(&mut self) {
        while self.rx.try_recv().is_ok() {}
    }
}

impl Default for ComposerInput {
    fn default() -> Self {
        Self::new()
    }
}

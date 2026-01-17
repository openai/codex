//! Clipboard copy helpers for the TUI transcript and composer actions.
//!
//! This module abstracts clipboard writes behind a small trait so UI components can request
//! copy-to-clipboard without depending directly on the platform backend. The primary
//! implementation uses `arboard` when available and falls back to explicit errors on platforms or
//! environments where clipboard access is unavailable.

use tracing::error;

/// Errors surfaced when a clipboard write cannot be completed.
#[derive(Debug)]
pub enum ClipboardError {
    /// Clipboard access could not be initialized or is unavailable.
    ClipboardUnavailable(String),
    /// Clipboard access succeeded, but the write failed.
    WriteFailed(String),
}

impl std::fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClipboardError::ClipboardUnavailable(msg) => {
                write!(f, "clipboard unavailable: {msg}")
            }
            ClipboardError::WriteFailed(msg) => write!(f, "failed to write to clipboard: {msg}"),
        }
    }
}

impl std::error::Error for ClipboardError {}

/// Minimal clipboard API used by the TUI for copy actions.
pub trait ClipboardManager {
    /// Write `text` to the clipboard or return a reason why it failed.
    fn set_text(&mut self, text: String) -> Result<(), ClipboardError>;
}

/// Clipboard manager backed by `arboard` on non-Android targets.
#[cfg(not(target_os = "android"))]
pub struct ArboardClipboardManager {
    /// Lazily initialized clipboard handle, if available.
    inner: Option<arboard::Clipboard>,
}

#[cfg(not(target_os = "android"))]
impl ArboardClipboardManager {
    /// Create a new clipboard manager, logging and disabling copy on failure.
    pub fn new() -> Self {
        match arboard::Clipboard::new() {
            Ok(cb) => Self { inner: Some(cb) },
            Err(err) => {
                error!(error = %err, "failed to initialize clipboard");
                Self { inner: None }
            }
        }
    }
}

#[cfg(not(target_os = "android"))]
impl ClipboardManager for ArboardClipboardManager {
    /// Write text to the system clipboard when the handle is available.
    fn set_text(&mut self, text: String) -> Result<(), ClipboardError> {
        let Some(cb) = &mut self.inner else {
            return Err(ClipboardError::ClipboardUnavailable(
                "clipboard is not available in this environment".to_string(),
            ));
        };
        cb.set_text(text)
            .map_err(|e| ClipboardError::WriteFailed(e.to_string()))
    }
}

/// Stub clipboard manager for Android builds that do not support text copy.
#[cfg(target_os = "android")]
pub struct ArboardClipboardManager;

#[cfg(target_os = "android")]
impl ArboardClipboardManager {
    /// Construct the Android stub clipboard manager.
    pub fn new() -> Self {
        ArboardClipboardManager
    }
}

#[cfg(target_os = "android")]
impl ClipboardManager for ArboardClipboardManager {
    /// Always returns a `ClipboardUnavailable` error on Android targets.
    fn set_text(&mut self, _text: String) -> Result<(), ClipboardError> {
        Err(ClipboardError::ClipboardUnavailable(
            "clipboard text copy is unsupported on Android".to_string(),
        ))
    }
}

/// Copy text to the clipboard using the platform-specific clipboard manager.
pub fn copy_text(text: String) -> Result<(), ClipboardError> {
    let mut manager = ArboardClipboardManager::new();
    manager.set_text(text)
}

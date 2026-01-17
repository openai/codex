//! Public widget implementations exposed for reuse across TUI surfaces.
//!
//! This module collects small, reusable widgets that are safe to embed outside
//! the main TUI, keeping their interfaces stable for other crates.

/// Minimal text input wrapper with submit semantics.
pub mod composer_input;

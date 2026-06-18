//! The unified mention popup used by default in the TUI.
//!
//! The `mentions_v2` feature flag remains temporarily as a rollback path: disabling it restores
//! the legacy split mention and file-search popups.

mod candidate;
mod filter;
mod footer;
mod popup;
mod render;
mod search_catalog;
mod search_mode;

pub(crate) use candidate::Selection as MentionV2Selection;
pub(crate) use popup::Popup as MentionV2Popup;
pub(crate) use search_catalog::build_search_catalog;

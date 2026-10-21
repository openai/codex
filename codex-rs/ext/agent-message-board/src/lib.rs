//! Installs agent message-board tools over the shared board interface.
//! Storage, backend behavior and tool implementations live in `codex-message-board`.

mod extension;

pub use extension::install;

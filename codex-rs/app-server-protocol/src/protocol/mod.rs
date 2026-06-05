// Module declarations for the app-server protocol namespace.
// Exposes protocol pieces used by `lib.rs` via `pub use protocol::common::*;`.

pub mod common;
pub mod event_mapping;
pub mod item_builders;
mod lifecycle_projection;
mod mappers;
mod serde_helpers;
pub mod thread_history;
mod thread_history_builder;
mod thread_item_projection;
mod turn_summary_projection;
pub mod v1;
pub mod v2;

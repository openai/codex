// Module declarations for the app-server protocol namespace.
// Exposes protocol pieces used by `lib.rs` via `pub use protocol::common::*;`.

pub mod common;
pub mod thread_history;
pub mod v1;
pub mod v2;

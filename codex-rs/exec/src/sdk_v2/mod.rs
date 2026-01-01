//! SDK v2 protocol support for bidirectional communication.
//!
//! This module provides the server-side implementation of the SDK v2 protocol,
//! enabling bidirectional communication between the Codex CLI and SDK clients
//! (Python, TypeScript, etc.) via stdin/stdout JSON-lines.

mod control;
mod entry_point;
mod runner;
mod transport;

pub use entry_point::detect_entry_point;
pub use entry_point::is_sdk_mode;
pub use entry_point::EntryPoint;
pub use runner::run_sdk_mode;

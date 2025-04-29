//! Root of the `codex-core` library.

// Prevent accidental direct writes to stdout/stderr in library code. All
// user‑visible output must go through the appropriate abstraction (e.g.,
// the TUI or the tracing stack).
#![deny(clippy::print_stdout, clippy::print_stderr)]

mod client;
pub mod codex;
pub mod codex_wrapper;
pub mod config;
pub mod error;
pub mod exec;
mod flags;
mod is_safe_command;
#[cfg(target_os = "linux")]
pub mod linux;
mod models;
pub mod protocol;
mod safety;
pub mod util;
mod zdr_transcript;

pub use codex::Codex;

#[cfg(feature = "cli")]
mod approval_mode_cli_arg;
#[cfg(feature = "cli")]
pub use approval_mode_cli_arg::ApprovalModeCliArg;
#[cfg(feature = "cli")]
pub use approval_mode_cli_arg::SandboxModeCliArg;

//! Background shell execution module.
//!
//! This module provides support for running shell commands in the background,
//! allowing the LLM to continue working while long-running commands execute.
//!
//! ## Features
//!
//! - **Background execution**: Run shell commands without blocking
//! - **Output tracking**: Track stdout/stderr from background shells
//! - **Kill support**: Terminate running shells via KillShell tool
//! - **System reminder integration**: Notify LLM of shell status changes
//!
//! ## Usage
//!
//! Background shells are managed through the `BackgroundShellStore`:
//!
//! ```ignore
//! let store = BackgroundShellStore::new();
//!
//! // Phase 1: Register pending (before spawn)
//! let shell_id = store.register_pending("cargo test".into(), "Run tests".into());
//!
//! // Phase 2: Spawn and set running
//! let handle = tokio::spawn(async { /* execute shell */ });
//! store.set_running(&shell_id, handle);
//!
//! // Later: Get output
//! let output = store.get_output(&shell_id, true, Duration::from_secs(60)).await;
//!
//! // Or kill it
//! store.kill(&shell_id)?;
//! ```

mod store;
mod types;

pub use store::BackgroundShellStore;
pub use store::SharedBackgroundShellStore;
pub use types::BackgroundShell;
pub use types::OutputBuffer;
pub use types::SharedOutputBuffer;
pub use types::ShellOutput;
pub use types::ShellResult;
pub use types::ShellStatus;

use std::sync::Arc;
use std::sync::OnceLock;

/// Global background shell store.
///
/// This provides a single store for all background shells in the current process.
/// Shell IDs are globally unique (UUID-based), so there's no conflict.
static GLOBAL_SHELL_STORE: OnceLock<SharedBackgroundShellStore> = OnceLock::new();

/// Get or initialize the global background shell store.
pub fn get_global_shell_store() -> SharedBackgroundShellStore {
    GLOBAL_SHELL_STORE
        .get_or_init(|| Arc::new(BackgroundShellStore::new()))
        .clone()
}

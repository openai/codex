//! Shell snapshot capture and restoration.
//!
//! This module provides functionality to capture a user's shell environment
//! (aliases, functions, exports, options) and restore them before each command
//! execution. This ensures that commands run with the same environment as
//! the user's interactive shell.
//!
//! ## Overview
//!
//! Shell snapshotting works by:
//! 1. Executing a shell-specific script that captures the environment
//! 2. Writing the captured state to a temporary file
//! 3. Sourcing that file before each command execution
//!
//! ## Supported Shells
//!
//! - **zsh**: Full support (functions, setopt, aliases, exports)
//! - **bash**: Full support (functions, set -o, aliases, exports)
//! - **sh**: Basic support (functions if available, aliases, exports)
//! - **PowerShell**: Limited support
//! - **cmd**: Not supported

mod cleanup;
mod scripts;
mod shell_snapshot;

pub use cleanup::cleanup_stale_snapshots;
pub use scripts::EXCLUDED_EXPORT_VARS;
pub use scripts::bash_snapshot_script;
pub use scripts::powershell_snapshot_script;
pub use scripts::sh_snapshot_script;
pub use scripts::zsh_snapshot_script;
pub use shell_snapshot::ShellSnapshot;
pub use shell_snapshot::SnapshotConfig;

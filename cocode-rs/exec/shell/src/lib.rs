//! Shell command execution for the cocode agent.
//!
//! This crate provides shell command execution with:
//! - Timeout support
//! - Output capture and truncation
//! - Background task management
//! - Read-only command detection

pub mod background;
pub mod command;
pub mod executor;
pub mod readonly;

pub use background::{BackgroundProcess, BackgroundTaskRegistry};
pub use command::{CommandInput, CommandResult};
pub use executor::ShellExecutor;
pub use readonly::{is_git_read_only, is_read_only_command};

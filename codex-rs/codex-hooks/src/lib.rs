//! Hook system for Codex - aligned with Claude Code's hook architecture.
//!
//! This crate provides a comprehensive hook system that allows customization
//! of behavior through external commands and native Rust callbacks at various
//! points during tool usage and session lifecycle.
//!
//! ## Hook Events
//!
//! 12 hook events are supported, aligned with Claude Code:
//! - `PreToolUse` - Before tool execution
//! - `PostToolUse` - After successful tool execution
//! - `PostToolUseFailure` - After failed tool execution
//! - `SessionStart` - Session begins
//! - `SessionEnd` - Session ends
//! - `Stop` - User interruption (Ctrl+C)
//! - `SubagentStart` - Subagent spawns
//! - `SubagentStop` - Subagent ends
//! - `UserPromptSubmit` - User sends message
//! - `Notification` - System notification
//! - `PreCompact` - Before context compaction
//! - `PermissionRequest` - Before permission prompt
//!
//! ## Hook Types (MVP)
//!
//! - `Command` - Shell command execution (stdin JSON, exit codes)
//! - `Callback` - Native Rust function callback
//!
//! ## Example
//!
//! ```rust,ignore
//! use codex_hooks::{HookRegistry, HookEventType, callback_from_fn};
//!
//! // Register a callback hook
//! let registry = HookRegistry::new();
//! registry.register_callback(
//!     HookEventType::PreToolUse,
//!     "Bash|Write",
//!     callback_from_fn(|input, _tool_use_id, _cancel, _index| async move {
//!         println!("Tool: {}", input.event_data.tool_name());
//!         Ok(HookOutput::default())
//!     }),
//! );
//! ```

pub mod config;
mod error;
mod executor;
mod executors;
mod input;
pub mod loader;
mod matcher;
mod output;
mod registry;
mod types;

pub use error::*;
pub use executor::*;
pub use executors::callback::BlockingCallback;
pub use executors::callback::NoOpCallback;
pub use executors::callback::SystemMessageCallback;
pub use executors::callback::callback_from_fn;
pub use executors::callback::callback_from_fn_named;
pub use executors::command::CommandConfig;
pub use input::*;
pub use matcher::*;
pub use output::*;
pub use registry::*;
pub use types::*;

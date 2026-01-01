//! Codex SDK Protocol Types
//!
//! This crate is the single source of truth for all protocol types used in SDK communication.
//! It generates JSON Schema that can be used to generate types for multiple languages.
//!
//! # Modules
//!
//! - `events` - Thread events (ThreadEvent, ThreadItem, etc.)
//! - `control` - Control protocol (ControlRequest, ControlResponse)
//! - `config` - Configuration types (AgentOptions, PermissionMode)
//! - `messages` - Message types (UserMessage, AssistantMessage)
//! - `hooks` - Hook types (HookEvent, HookInput, HookOutput)

pub mod config;
pub mod control;
pub mod events;
pub mod hooks;
pub mod messages;
pub mod schema_gen;

/// Protocol version for SDK-CLI negotiation.
pub const PROTOCOL_VERSION: &str = "1.0";

/// Re-export commonly used types
pub use config::*;
pub use control::*;
pub use events::*;
pub use hooks::*;
pub use messages::*;

//! Lifecycle hooks system for Codex.
//!
//! This module provides a comprehensive lifecycle hooks system that allows external
//! scripts, webhooks, and integrations to be triggered at specific points in the
//! Codex execution lifecycle.
//!
//! # Architecture
//!
//! The hooks system is built around the following core components:
//!
//! - **Hook Manager**: Central registry and coordinator for all lifecycle hooks
//! - **Hook Registry**: Stores hook definitions and manages event routing
//! - **Hook Executors**: Execute different types of hooks (scripts, webhooks, MCP tools)
//! - **Hook Context**: Provides execution context and data to hooks
//!
//! # Hook Types
//!
//! The system supports several types of hooks:
//!
//! - **Script Hooks**: Execute shell scripts or commands with event context
//! - **Webhook Hooks**: Send HTTP requests to external APIs with event data
//! - **MCP Tool Hooks**: Call MCP tools as lifecycle hooks
//! - **Custom Executable Hooks**: Execute any binary with event data
//!
//! # Lifecycle Events
//!
//! Hooks can be triggered by various lifecycle events:
//!
//! - Session lifecycle (start, end)
//! - Task lifecycle (start, complete)
//! - Execution lifecycle (before/after command execution)
//! - Patch lifecycle (before/after patch application)
//! - MCP tool lifecycle (before/after tool calls)
//! - Agent interactions (messages, reasoning)
//! - Error handling
//!
//! # Usage
//!
//! ```rust
//! use codex_core::hooks::{HookManager, LifecycleEvent};
//!
//! // Initialize the hook manager with configuration
//! let hook_manager = HookManager::new(config.hooks).await?;
//!
//! // Trigger a lifecycle event
//! let event = LifecycleEvent::TaskStart {
//!     task_id: "task_123".to_string(),
//!     prompt: "Create a new file".to_string(),
//! };
//! hook_manager.trigger_event(event).await?;
//! ```
//!
//! # Configuration
//!
//! Hooks are configured via the `hooks.toml` configuration file:
//!
//! ```toml
//! [hooks]
//! enabled = true
//! timeout_seconds = 30
//!
//! [[hooks.task]]
//! event = "task.start"
//! type = "script"
//! command = ["./scripts/log-task-start.sh"]
//! ```

pub mod config;
pub mod context;
pub mod executor;
pub mod manager;
pub mod registry;
pub mod types;

// Re-export commonly used types
pub use config::{HookConfig, HooksConfig};
pub use context::{HookContext, HookExecutionContext};
pub use executor::{HookExecutor, HookExecutorResult};
pub use manager::HookManager;
pub use registry::{HookRegistry, HookRegistryStatistics};
pub use types::{
    HookError, HookResult, HookType, LifecycleEvent, LifecycleEventType, HookExecutionMode,
};

/// Result type for hook operations
pub type Result<T> = std::result::Result<T, HookError>;

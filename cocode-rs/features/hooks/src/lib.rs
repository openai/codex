//! Hook system for cocode.
//!
//! This crate provides a hook mechanism that allows external code to intercept
//! and react to events in the agent loop. Hooks can be configured to run before
//! or after tool calls, on session lifecycle events, and more.
//!
//! # Architecture
//!
//! - **Events** (`HookEventType`): Defines when hooks fire.
//! - **Context** (`HookContext`): Data available to a hook at execution time.
//! - **Definitions** (`HookDefinition`): Describes a single hook (event, matcher, handler).
//! - **Matchers** (`HookMatcher`): Determines whether a hook applies to a given value.
//! - **Handlers**: The action a hook performs (command, prompt, agent, webhook, inline).
//! - **Registry** (`HookRegistry`): Stores hooks and dispatches them on events.
//! - **Scope** (`HookScope`): Priority ordering for hooks.
//! - **Source** (`HookSource`): Identifies where a hook was registered from.
//! - **Aggregator** (`HookAggregator`): Combines hooks from multiple sources.
//! - **Settings** (`HookSettings`): Global hook settings.
//! - **AsyncTracker** (`AsyncHookTracker`): Tracks background hook tasks.
//! - **Config**: Loading hook definitions from TOML files.

pub mod aggregator;
pub mod async_tracker;
pub mod config;
pub mod context;
pub mod definition;
pub mod error;
pub mod event;
pub mod handlers;
pub mod matcher;
pub mod registry;
pub mod result;
pub mod scope;
pub mod settings;

// Re-exports
pub use aggregator::HookAggregator;
pub use aggregator::aggregate_hooks;
pub use async_tracker::AsyncHookTracker;
pub use async_tracker::CompletedAsyncHook;
pub use async_tracker::PendingAsyncHook;
pub use config::load_hooks_from_toml;
pub use context::HookContext;
pub use definition::HookDefinition;
pub use definition::HookHandler;
pub use error::HookError;
pub use event::HookEventType;
pub use handlers::command::HookOutput;
pub use handlers::inline::InlineHandler;
pub use matcher::HookMatcher;
pub use registry::HookRegistry;
pub use result::HookOutcome;
pub use result::HookResult;
pub use scope::HookScope;
pub use scope::HookSource;
pub use settings::HookSettings;

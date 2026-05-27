//! Extension crate for persisted thread goals.
//!
//! The app-server host installs this extension to contribute goal tools,
//! lifecycle accounting, and idle continuation work without teaching
//! `codex-core` session code about goal-specific runtime state.

mod accounting;
mod events;
mod extension;
mod metrics;
mod runtime;
mod spec;
mod steering;
mod tool;

pub use extension::GoalExtension;
pub use extension::install_with_backend;
pub use runtime::GoalRuntimeHandle;
pub use spec::CREATE_GOAL_TOOL_NAME;
pub use spec::GET_GOAL_TOOL_NAME;
pub use spec::UPDATE_GOAL_TOOL_NAME;
pub use tool::CreateGoalRequest;

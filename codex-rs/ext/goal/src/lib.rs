//! Extension crate sketch for the `/goal` feature.
//!
//! This crate is intentionally not wired into the host yet. It contains the
//! goal tool specs, extension registration shape, and the parts of runtime
//! accounting that can be represented with today's extension API.

mod accounting;
mod extension;
mod spec;
mod tool;

pub use extension::GoalExtension;
pub use extension::GoalExtensionConfig;
pub use extension::GoalToolBackend;
pub use extension::NoGoalToolBackend;
pub use extension::install;
pub use extension::install_with_backend;
pub use spec::CREATE_GOAL_TOOL_NAME;
pub use spec::GET_GOAL_TOOL_NAME;
pub use spec::UPDATE_GOAL_TOOL_NAME;
pub use tool::CreateGoalRequest;

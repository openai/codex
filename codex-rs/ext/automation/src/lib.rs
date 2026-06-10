//! Extension crate for scheduled automations and heartbeats.

mod extension;
mod spec;
mod tool;

pub use extension::install;
pub use spec::AUTOMATION_UPDATE_TOOL_NAME;

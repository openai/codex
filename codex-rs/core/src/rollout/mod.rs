//! Rollout module: persistence and discovery of session rollout files.

pub(crate) const SESSIONS_SUBDIR: &str = "sessions";

pub mod list;
pub(crate) mod policy;
pub mod recorder;

pub use recorder::RolloutRecorder;
pub use recorder::RolloutRecorderParams;
pub use recorder::SessionMeta;
pub use recorder::SessionStateSnapshot;

#[cfg(test)]
pub mod tests;

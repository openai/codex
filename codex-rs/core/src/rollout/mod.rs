//! Rollout module: persistence and discovery of session rollout files.

use codex_protocol::protocol::SessionSource;

pub const SESSIONS_SUBDIR: &str = "sessions";
pub const ARCHIVED_SESSIONS_SUBDIR: &str = "archived_sessions";
pub const INTERACTIVE_SESSION_SOURCES: &[SessionSource] =
    &[SessionSource::Cli, SessionSource::VSCode];

pub mod list;
pub(crate) mod policy;
pub mod recorder;

pub use codex_protocol::protocol::SessionMeta;
pub use list::find_conversation_path_by_id_str;
pub use recorder::RolloutRecorder;
pub use recorder::RolloutRecorderParams;

#[cfg(test)]
pub mod tests;

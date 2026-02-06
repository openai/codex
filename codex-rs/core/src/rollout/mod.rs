//! Rollout module: persistence and discovery of session rollout files.

use codex_protocol::protocol::SessionSource;

pub const SESSIONS_SUBDIR: &str = "sessions";
pub const ARCHIVED_SESSIONS_SUBDIR: &str = "archived_sessions";
pub const INTERACTIVE_SESSION_SOURCES: &[SessionSource] =
    &[SessionSource::Cli, SessionSource::VSCode];

pub(crate) mod error;
pub mod list;
pub(crate) mod metadata;
pub(crate) mod policy;
pub mod recorder;
pub(crate) mod session_index;
pub(crate) mod truncation;

pub use codex_protocol::protocol::SessionMeta;
pub(crate) use error::map_session_init_error;
pub use list::find_archived_thread_path_by_id_str;
pub use list::find_thread_path_by_id_str;
#[deprecated(note = "use find_thread_path_by_id_str")]
pub use list::find_thread_path_by_id_str as find_conversation_path_by_id_str;
pub use list::rollout_date_parts;
pub use recorder::RolloutRecorder;
pub use recorder::RolloutRecorderParams;
pub use session_index::find_thread_name_by_id;
pub use session_index::find_thread_path_by_name_str;

#[cfg(test)]
pub mod tests;

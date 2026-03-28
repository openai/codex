//! Rollout persistence and discovery for Codex session files.

use std::sync::LazyLock;

use codex_protocol::protocol::SessionSource;

pub mod config;
pub mod list;
pub mod metadata;
pub mod policy;
pub mod recorder;
pub mod session_index;
pub mod state_db;

pub(crate) mod default_client {
    pub use codex_login::default_client::*;
}

pub(crate) use codex_protocol::protocol;

pub const SESSIONS_SUBDIR: &str = "sessions";
pub const ARCHIVED_SESSIONS_SUBDIR: &str = "archived_sessions";
pub static INTERACTIVE_SESSION_SOURCES: LazyLock<Vec<SessionSource>> = LazyLock::new(|| {
    vec![
        SessionSource::Cli,
        SessionSource::VSCode,
        SessionSource::Custom("atlas".to_string()),
        SessionSource::Custom("chatgpt".to_string()),
    ]
});

pub use codex_protocol::protocol::SessionMeta;
pub use config::RolloutConfig;
pub use config::RolloutConfigView;
pub use list::find_archived_thread_path_by_id_str;
pub use list::find_thread_path_by_id_str;
#[deprecated(note = "use find_thread_path_by_id_str")]
pub use list::find_thread_path_by_id_str as find_conversation_path_by_id_str;
pub use list::resolve_fork_reference_rollout_path;
pub use list::rollout_date_parts;
pub use policy::EventPersistenceMode;
pub use recorder::RolloutRecorder;
pub use recorder::RolloutRecorderParams;
pub use session_index::append_thread_name;
pub use session_index::find_thread_name_by_id;
pub use session_index::find_thread_names_by_ids;
pub use session_index::find_thread_path_by_name_str;
pub use state_db::StateDbHandle;

#[cfg(test)]
mod tests;

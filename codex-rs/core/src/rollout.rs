use crate::config::Config;

impl codex_rollout::RolloutConfigView for Config {
    fn codex_home(&self) -> &std::path::Path {
        self.codex_home.as_path()
    }

    fn sqlite_home(&self) -> &std::path::Path {
        self.sqlite_home.as_path()
    }

    fn cwd(&self) -> &std::path::Path {
        self.cwd.as_path()
    }

    fn model_provider_id(&self) -> &str {
        self.model_provider_id.as_str()
    }

    fn generate_memories(&self) -> bool {
        self.memories.generate_memories
    }
}

pub use codex_rollout::ARCHIVED_SESSIONS_SUBDIR;
pub use codex_rollout::INTERACTIVE_SESSION_SOURCES;
pub use codex_rollout::RolloutRecorder;
pub use codex_rollout::RolloutRecorderParams;
pub use codex_rollout::SESSIONS_SUBDIR;
pub use codex_rollout::SessionMeta;
pub use codex_rollout::append_thread_name;
pub use codex_rollout::find_archived_thread_path_by_id_str;
#[deprecated(note = "use find_thread_path_by_id_str")]
pub use codex_rollout::find_conversation_path_by_id_str;
pub use codex_rollout::find_thread_name_by_id;
pub use codex_rollout::find_thread_path_by_id_str;
pub use codex_rollout::find_thread_path_by_name_str;
pub use codex_rollout::rollout_date_parts;

pub mod list {
    pub use codex_rollout::list::*;
}

pub(crate) mod metadata {
    pub(crate) use codex_rollout::metadata::builder_from_items;
}

pub mod policy {
    pub use codex_rollout::policy::*;
}

pub mod recorder {
    pub use codex_rollout::recorder::*;
}

pub mod session_index {
    pub use codex_rollout::session_index::*;
}

pub(crate) use crate::session_rollout_init_error::map_session_init_error;

pub(crate) mod truncation {
    pub(crate) use crate::thread_rollout_truncation::*;
}

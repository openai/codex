use crate::config::Config;
pub use codex_rollout::ARCHIVED_SESSIONS_SUBDIR;
pub use codex_rollout::Cursor;
pub use codex_rollout::EventPersistenceMode;
pub use codex_rollout::INTERACTIVE_SESSION_SOURCES;
pub use codex_rollout::RolloutRecorder;
pub use codex_rollout::RolloutRecorderParams;
pub use codex_rollout::SESSIONS_SUBDIR;
pub use codex_rollout::SessionMeta;
pub use codex_rollout::ThreadItem;
pub use codex_rollout::ThreadSortDirection;
pub use codex_rollout::ThreadSortKey;
pub use codex_rollout::ThreadsPage;
pub use codex_rollout::append_thread_name;
pub use codex_rollout::find_archived_thread_path_by_id_str;
#[deprecated(note = "use find_thread_path_by_id_str")]
pub use codex_rollout::find_conversation_path_by_id_str;
pub use codex_rollout::find_thread_name_by_id;
pub use codex_rollout::find_thread_names_by_ids;
pub use codex_rollout::find_thread_path_by_id_str;
pub use codex_rollout::find_thread_path_by_name_str;
pub use codex_rollout::parse_cursor;
pub use codex_rollout::read_head_for_summary;
pub use codex_rollout::read_session_meta_line;
pub use codex_rollout::rollout_date_parts;

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

pub(crate) mod list {
    pub use codex_rollout::ThreadListConfig;
    pub use codex_rollout::ThreadListLayout;
    pub use codex_rollout::ThreadSortKey;
    pub use codex_rollout::find_thread_path_by_id_str;
    pub use codex_rollout::get_threads_in_root;
}

pub(crate) mod metadata {
    pub(crate) use codex_rollout::builder_from_items;
}

pub(crate) mod policy {
    pub use codex_rollout::EventPersistenceMode;
    pub use codex_rollout::should_persist_response_item_for_memories;
}

pub(crate) mod recorder {
    pub use codex_rollout::RolloutRecorder;
}

pub(crate) mod session_index {
    pub use codex_rollout::append_thread_name;
    pub use codex_rollout::find_thread_name_by_id;
}

pub(crate) use crate::session_rollout_init_error::map_session_init_error;

pub(crate) mod truncation {
    pub(crate) use crate::thread_rollout_truncation::*;
}

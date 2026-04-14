#[cfg(not(target_arch = "wasm32"))]
mod native {
    use crate::config::Config;

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
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::collections::HashMap;
    use std::collections::HashSet;
    use std::ffi::OsStr;
    use std::io;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::LazyLock;

    use codex_protocol::ThreadId;
    use codex_protocol::dynamic_tools::DynamicToolSpec;
    use codex_protocol::models::BaseInstructions;
    use codex_protocol::protocol::InitialHistory;
    use codex_protocol::protocol::RolloutItem;
    use codex_protocol::protocol::SessionMeta;
    use codex_protocol::protocol::SessionMetaLine;
    use codex_protocol::protocol::SessionSource;

    use crate::config::Config;
    use crate::state_db::StateDbHandle;

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

    pub use session_index::append_thread_name;
    pub use session_index::find_thread_name_by_id;
    pub use session_index::find_thread_path_by_name_str;

    #[deprecated(note = "use find_thread_path_by_id_str")]
    pub async fn find_conversation_path_by_id_str(
        codex_home: &Path,
        id_str: &str,
    ) -> io::Result<Option<PathBuf>> {
        find_thread_path_by_id_str(codex_home, id_str).await
    }

    pub async fn find_thread_path_by_id_str(
        _codex_home: &Path,
        _id_str: &str,
    ) -> io::Result<Option<PathBuf>> {
        Ok(None)
    }

    pub async fn find_archived_thread_path_by_id_str(
        _codex_home: &Path,
        _id_str: &str,
    ) -> io::Result<Option<PathBuf>> {
        Ok(None)
    }

    pub fn rollout_date_parts(_file_name: &OsStr) -> Option<(String, String, String)> {
        None
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub enum EventPersistenceMode {
        #[default]
        Limited,
        Extended,
    }

    pub mod policy {
        pub use super::EventPersistenceMode;
    }

    #[derive(Clone)]
    pub struct RolloutRecorder {
        rollout_path: PathBuf,
        state_db: Option<StateDbHandle>,
        _event_persistence_mode: EventPersistenceMode,
    }

    #[derive(Clone)]
    pub enum RolloutRecorderParams {
        Create {
            conversation_id: ThreadId,
            forked_from_id: Option<ThreadId>,
            source: SessionSource,
            base_instructions: BaseInstructions,
            dynamic_tools: Vec<DynamicToolSpec>,
            event_persistence_mode: EventPersistenceMode,
        },
        Resume {
            path: PathBuf,
            event_persistence_mode: EventPersistenceMode,
        },
    }

    impl RolloutRecorderParams {
        pub fn new(
            conversation_id: ThreadId,
            forked_from_id: Option<ThreadId>,
            source: SessionSource,
            base_instructions: BaseInstructions,
            dynamic_tools: Vec<DynamicToolSpec>,
            event_persistence_mode: EventPersistenceMode,
        ) -> Self {
            Self::Create {
                conversation_id,
                forked_from_id,
                source,
                base_instructions,
                dynamic_tools,
                event_persistence_mode,
            }
        }

        pub fn resume(path: PathBuf, event_persistence_mode: EventPersistenceMode) -> Self {
            Self::Resume {
                path,
                event_persistence_mode,
            }
        }
    }

    impl RolloutRecorder {
        #[allow(clippy::too_many_arguments)]
        pub async fn list_threads(
            _config: &Config,
            _page_size: usize,
            _cursor: Option<&list::Cursor>,
            _sort_key: list::ThreadSortKey,
            _allowed_sources: &[SessionSource],
            _model_providers: Option<&[String]>,
            _default_provider: &str,
            _search_term: Option<&str>,
        ) -> io::Result<list::ThreadsPage> {
            Ok(list::ThreadsPage::default())
        }

        #[allow(clippy::too_many_arguments)]
        pub async fn list_archived_threads(
            _config: &Config,
            _page_size: usize,
            _cursor: Option<&list::Cursor>,
            _sort_key: list::ThreadSortKey,
            _allowed_sources: &[SessionSource],
            _model_providers: Option<&[String]>,
            _default_provider: &str,
            _search_term: Option<&str>,
        ) -> io::Result<list::ThreadsPage> {
            Ok(list::ThreadsPage::default())
        }

        pub async fn new(
            config: &Config,
            params: RolloutRecorderParams,
            state_db: Option<StateDbHandle>,
            _state_builder: Option<metadata::ThreadMetadataBuilder>,
        ) -> io::Result<Self> {
            let rollout_path = match params {
                RolloutRecorderParams::Create {
                    conversation_id, ..
                } => config
                    .codex_home
                    .join(SESSIONS_SUBDIR)
                    .join(format!("browser-{}.jsonl", conversation_id)),
                RolloutRecorderParams::Resume { path, .. } => path,
            };
            Ok(Self {
                rollout_path,
                state_db,
                _event_persistence_mode: EventPersistenceMode::Limited,
            })
        }

        pub fn rollout_path(&self) -> &Path {
            self.rollout_path.as_path()
        }

        pub fn state_db(&self) -> Option<StateDbHandle> {
            self.state_db.clone()
        }

        pub async fn record_items(&self, _items: &[RolloutItem]) -> io::Result<()> {
            Ok(())
        }

        pub async fn persist(&self) -> io::Result<()> {
            Ok(())
        }

        pub async fn flush(&self) -> io::Result<()> {
            Ok(())
        }

        pub async fn load_rollout_items(
            _path: &Path,
        ) -> io::Result<(Vec<RolloutItem>, Option<ThreadId>, usize)> {
            Err(io::Error::other(
                "rollout loading is unavailable on wasm32 without a browser persistence backend",
            ))
        }

        pub async fn get_rollout_history(_path: &Path) -> io::Result<InitialHistory> {
            Err(io::Error::other(
                "rollout history is unavailable on wasm32 without a browser persistence backend",
            ))
        }

        pub async fn shutdown(&self) -> io::Result<()> {
            Ok(())
        }
    }

    pub mod recorder {
        pub use super::RolloutRecorder;
        pub use super::RolloutRecorderParams;
    }

    pub mod metadata {
        use std::path::Path;

        use codex_protocol::protocol::RolloutItem;

        #[derive(Clone, Debug)]
        pub struct ThreadMetadataBuilder;

        pub fn builder_from_items(
            _items: &[RolloutItem],
            _rollout_path: &Path,
        ) -> Option<ThreadMetadataBuilder> {
            None
        }
    }

    pub mod list {
        use std::io;
        use std::path::Path;
        use std::path::PathBuf;

        use codex_protocol::ThreadId;
        use codex_protocol::protocol::SessionMetaLine;
        use codex_protocol::protocol::SessionSource;

        #[derive(Debug, Default, PartialEq)]
        pub struct ThreadsPage {
            pub items: Vec<ThreadItem>,
            pub next_cursor: Option<Cursor>,
            pub num_scanned_files: usize,
            pub reached_scan_cap: bool,
        }

        #[derive(Debug, PartialEq, Default)]
        pub struct ThreadItem {
            pub path: PathBuf,
            pub thread_id: Option<ThreadId>,
            pub first_user_message: Option<String>,
            pub cwd: Option<PathBuf>,
            pub git_branch: Option<String>,
            pub git_sha: Option<String>,
            pub git_origin_url: Option<String>,
            pub source: Option<SessionSource>,
            pub agent_nickname: Option<String>,
            pub agent_role: Option<String>,
            pub model_provider: Option<String>,
            pub cli_version: Option<String>,
            pub created_at: Option<String>,
            pub updated_at: Option<String>,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ThreadSortKey {
            CreatedAt,
            UpdatedAt,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ThreadListLayout {
            NestedByDate,
            Flat,
        }

        pub struct ThreadListConfig<'a> {
            pub allowed_sources: &'a [SessionSource],
            pub model_providers: Option<&'a [String]>,
            pub default_provider: &'a str,
            pub layout: ThreadListLayout,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct Cursor(String);

        pub fn parse_cursor(cursor: &str) -> Option<Cursor> {
            if cursor.is_empty() {
                None
            } else {
                Some(Cursor(cursor.to_string()))
            }
        }

        pub async fn get_threads(
            _codex_home: &Path,
            _page_size: usize,
            _cursor: Option<&Cursor>,
            _sort_key: ThreadSortKey,
            _allowed_sources: &[SessionSource],
            _model_providers: Option<&[String]>,
            _default_provider: &str,
        ) -> io::Result<ThreadsPage> {
            Ok(ThreadsPage::default())
        }

        pub async fn get_threads_in_root(
            _root: PathBuf,
            _page_size: usize,
            _cursor: Option<&Cursor>,
            _sort_key: ThreadSortKey,
            _config: ThreadListConfig<'_>,
        ) -> io::Result<ThreadsPage> {
            Ok(ThreadsPage::default())
        }

        pub async fn read_head_for_summary(_path: &Path) -> io::Result<Vec<serde_json::Value>> {
            Ok(Vec::new())
        }

        pub async fn read_session_meta_line(_path: &Path) -> io::Result<SessionMetaLine> {
            Ok(SessionMetaLine {
                meta: Default::default(),
                git: None,
            })
        }

        pub async fn find_thread_path_by_id_str(
            _codex_home: &Path,
            _id_str: &str,
        ) -> io::Result<Option<PathBuf>> {
            Ok(None)
        }

        pub async fn find_archived_thread_path_by_id_str(
            _codex_home: &Path,
            _id_str: &str,
        ) -> io::Result<Option<PathBuf>> {
            Ok(None)
        }

        pub fn rollout_date_parts(
            _file_name: &std::ffi::OsStr,
        ) -> Option<(String, String, String)> {
            None
        }
    }

    pub mod session_index {
        use std::collections::HashMap;
        use std::collections::HashSet;
        use std::io;
        use std::path::Path;
        use std::path::PathBuf;

        use codex_protocol::ThreadId;

        pub async fn append_thread_name(
            _codex_home: &Path,
            _thread_id: ThreadId,
            _name: &str,
        ) -> io::Result<()> {
            Ok(())
        }

        pub async fn find_thread_name_by_id(
            _codex_home: &Path,
            _thread_id: &ThreadId,
        ) -> io::Result<Option<String>> {
            Ok(None)
        }

        pub async fn find_thread_names_by_ids(
            _codex_home: &Path,
            _thread_ids: &HashSet<ThreadId>,
        ) -> io::Result<HashMap<ThreadId, String>> {
            Ok(HashMap::new())
        }

        pub async fn find_thread_path_by_name_str(
            _codex_home: &Path,
            _name: &str,
        ) -> io::Result<Option<PathBuf>> {
            Ok(None)
        }
    }

    pub(crate) fn map_session_init_error(
        err: &anyhow::Error,
        _codex_home: &Path,
    ) -> crate::error::CodexErr {
        crate::error::CodexErr::Fatal(err.to_string())
    }

    pub(crate) mod truncation {}
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
pub use wasm::*;

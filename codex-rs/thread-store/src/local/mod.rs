use async_trait::async_trait;
use chrono::DateTime;
use chrono::Utc;
use codex_git_utils::GitSha;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GitInfo;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_rollout::RolloutConfig;
use codex_rollout::RolloutRecorder;
use codex_rollout::ThreadItem;
use codex_rollout::parse_cursor;

use crate::AppendThreadItemsParams;
use crate::ArchiveThreadParams;
use crate::CreateThreadParams;
use crate::ListThreadsParams;
use crate::LoadThreadHistoryParams;
use crate::ReadThreadParams;
use crate::ResumeThreadRecorderParams;
use crate::SetThreadNameParams;
use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::ThreadPage;
use crate::ThreadRecorder;
use crate::ThreadSortKey;
use crate::ThreadStore;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use crate::UpdateThreadMetadataParams;

/// Local filesystem/SQLite-backed implementation of [`ThreadStore`].
#[derive(Clone, Debug)]
pub struct LocalThreadStore {
    config: RolloutConfig,
}

impl LocalThreadStore {
    /// Create a local store from the rollout configuration used by existing local persistence.
    pub fn new(config: RolloutConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ThreadStore for LocalThreadStore {
    async fn create_thread(
        &self,
        _params: CreateThreadParams,
    ) -> ThreadStoreResult<Box<dyn ThreadRecorder>> {
        unsupported("create_thread")
    }

    async fn resume_thread_recorder(
        &self,
        _params: ResumeThreadRecorderParams,
    ) -> ThreadStoreResult<Box<dyn ThreadRecorder>> {
        unsupported("resume_thread_recorder")
    }

    async fn append_items(&self, _params: AppendThreadItemsParams) -> ThreadStoreResult<()> {
        unsupported("append_items")
    }

    async fn load_history(
        &self,
        _params: LoadThreadHistoryParams,
    ) -> ThreadStoreResult<StoredThreadHistory> {
        unsupported("load_history")
    }

    async fn read_thread(&self, _params: ReadThreadParams) -> ThreadStoreResult<StoredThread> {
        unsupported("read_thread")
    }

    async fn list_threads(&self, params: ListThreadsParams) -> ThreadStoreResult<ThreadPage> {
        let cursor = params
            .cursor
            .as_deref()
            .map(|cursor| {
                parse_cursor(cursor).ok_or_else(|| ThreadStoreError::InvalidRequest {
                    message: format!("invalid cursor: {cursor}"),
                })
            })
            .transpose()?;
        let sort_key = match params.sort_key {
            ThreadSortKey::CreatedAt => codex_rollout::ThreadSortKey::CreatedAt,
            ThreadSortKey::UpdatedAt => codex_rollout::ThreadSortKey::UpdatedAt,
        };
        let page = list_rollout_threads(&self.config, &params, cursor.as_ref(), sort_key).await?;

        let next_cursor = page
            .next_cursor
            .as_ref()
            .and_then(|cursor| serde_json::to_value(cursor).ok())
            .and_then(|value| value.as_str().map(str::to_owned));
        let items = page
            .items
            .into_iter()
            .filter_map(|item| {
                stored_thread_from_rollout_item(
                    item,
                    params.archived,
                    self.config.model_provider_id.as_str(),
                )
            })
            .collect::<Vec<_>>();

        Ok(ThreadPage { items, next_cursor })
    }

    async fn set_thread_name(&self, _params: SetThreadNameParams) -> ThreadStoreResult<()> {
        unsupported("set_thread_name")
    }

    async fn update_thread_metadata(
        &self,
        _params: UpdateThreadMetadataParams,
    ) -> ThreadStoreResult<StoredThread> {
        unsupported("update_thread_metadata")
    }

    async fn archive_thread(&self, _params: ArchiveThreadParams) -> ThreadStoreResult<()> {
        unsupported("archive_thread")
    }

    async fn unarchive_thread(
        &self,
        _params: ArchiveThreadParams,
    ) -> ThreadStoreResult<StoredThread> {
        unsupported("unarchive_thread")
    }
}

fn unsupported<T>(operation: &str) -> ThreadStoreResult<T> {
    Err(ThreadStoreError::Internal {
        message: format!("local thread store does not implement {operation} in this slice"),
    })
}

async fn list_rollout_threads(
    config: &RolloutConfig,
    params: &ListThreadsParams,
    cursor: Option<&codex_rollout::Cursor>,
    sort_key: codex_rollout::ThreadSortKey,
) -> ThreadStoreResult<codex_rollout::ThreadsPage> {
    let page = if params.archived {
        RolloutRecorder::list_archived_threads(
            config,
            params.page_size,
            cursor,
            sort_key,
            params.allowed_sources.as_slice(),
            params.model_providers.as_deref(),
            config.model_provider_id.as_str(),
            params.search_term.as_deref(),
        )
        .await
    } else {
        RolloutRecorder::list_threads(
            config,
            params.page_size,
            cursor,
            sort_key,
            params.allowed_sources.as_slice(),
            params.model_providers.as_deref(),
            config.model_provider_id.as_str(),
            params.search_term.as_deref(),
        )
        .await
    };
    page.map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to list threads: {err}"),
    })
}

fn stored_thread_from_rollout_item(
    item: ThreadItem,
    archived: bool,
    default_provider: &str,
) -> Option<StoredThread> {
    let thread_id = item
        .thread_id
        .or_else(|| thread_id_from_rollout_path(item.path.as_path()))?;
    let created_at = parse_rfc3339(item.created_at.as_deref()).unwrap_or_else(Utc::now);
    let updated_at = parse_rfc3339(item.updated_at.as_deref()).unwrap_or(created_at);
    let archived_at = archived.then_some(updated_at);
    let git_info = git_info_from_parts(
        item.git_sha.clone(),
        item.git_branch.clone(),
        item.git_origin_url.clone(),
    );
    let source = item.source.unwrap_or(SessionSource::Unknown);
    let preview = item.first_user_message.clone().unwrap_or_default();

    Some(StoredThread {
        thread_id,
        rollout_path: Some(item.path),
        forked_from_id: None,
        preview,
        name: None,
        model_provider: item
            .model_provider
            .filter(|provider| !provider.is_empty())
            .unwrap_or_else(|| default_provider.to_string()),
        model: None,
        reasoning_effort: None,
        created_at,
        updated_at,
        archived_at,
        cwd: item.cwd.unwrap_or_default(),
        cli_version: item.cli_version.unwrap_or_default(),
        source,
        agent_nickname: item.agent_nickname,
        agent_role: item.agent_role,
        agent_path: None,
        git_info,
        approval_mode: AskForApproval::OnRequest,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        token_usage: None,
        first_user_message: item.first_user_message,
        history: None,
    })
}

fn parse_rfc3339(value: Option<&str>) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value?)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn git_info_from_parts(
    sha: Option<String>,
    branch: Option<String>,
    origin_url: Option<String>,
) -> Option<GitInfo> {
    if sha.is_none() && branch.is_none() && origin_url.is_none() {
        return None;
    }
    Some(GitInfo {
        commit_hash: sha.as_deref().map(GitSha::new),
        branch,
        repository_url: origin_url,
    })
}

fn thread_id_from_rollout_path(path: &std::path::Path) -> Option<codex_protocol::ThreadId> {
    let file_name = path.file_name()?.to_str()?;
    let stem = file_name.strip_suffix(".jsonl")?;
    if stem.len() < 37 {
        return None;
    }
    let uuid_start = stem.len().saturating_sub(36);
    if !stem[..uuid_start].ends_with('-') {
        return None;
    }
    codex_protocol::ThreadId::from_string(&stem[uuid_start..]).ok()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::path::Path;
    use std::path::PathBuf;

    use codex_protocol::ThreadId;
    use codex_protocol::protocol::SessionSource;
    use codex_rollout::ARCHIVED_SESSIONS_SUBDIR;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::*;

    fn test_config(codex_home: &Path) -> RolloutConfig {
        RolloutConfig {
            codex_home: codex_home.to_path_buf(),
            sqlite_home: codex_home.to_path_buf(),
            cwd: codex_home.to_path_buf(),
            model_provider_id: "test-provider".to_string(),
            generate_memories: true,
        }
    }

    fn write_session_file(root: &Path, ts: &str, uuid: Uuid) -> std::io::Result<PathBuf> {
        write_session_file_with(
            root,
            root.join("sessions/2025/01/03"),
            ts,
            uuid,
            "Hello from user",
            Some("test-provider"),
        )
    }

    fn write_archived_session_file(root: &Path, ts: &str, uuid: Uuid) -> std::io::Result<PathBuf> {
        write_session_file_with(
            root,
            root.join(ARCHIVED_SESSIONS_SUBDIR),
            ts,
            uuid,
            "Archived user message",
            Some("test-provider"),
        )
    }

    fn write_session_file_with(
        root: &Path,
        day_dir: PathBuf,
        ts: &str,
        uuid: Uuid,
        first_user_message: &str,
        model_provider: Option<&str>,
    ) -> std::io::Result<PathBuf> {
        fs::create_dir_all(&day_dir)?;
        let path = day_dir.join(format!("rollout-{ts}-{uuid}.jsonl"));
        let mut file = fs::File::create(&path)?;
        let meta = serde_json::json!({
            "timestamp": ts,
            "type": "session_meta",
            "payload": {
                "id": uuid,
                "timestamp": ts,
                "cwd": root,
                "originator": "test_originator",
                "cli_version": "test_version",
                "source": "cli",
                "model_provider": model_provider,
                "git": {
                    "commit_hash": "abcdef",
                    "branch": "main",
                    "repository_url": "https://example.com/repo.git"
                }
            },
        });
        writeln!(file, "{meta}")?;
        let user_event = serde_json::json!({
            "timestamp": ts,
            "type": "event_msg",
            "payload": {
                "type": "user_message",
                "message": first_user_message,
                "kind": "plain",
            },
        });
        writeln!(file, "{user_event}")?;
        Ok(path)
    }

    #[tokio::test]
    async fn list_threads_uses_default_provider_when_rollout_omits_provider() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()));
        write_session_file_with(
            home.path(),
            home.path().join("sessions/2025/01/03"),
            "2025-01-03T12-00-00",
            Uuid::from_u128(102),
            "Hello from user",
            /*model_provider*/ None,
        )
        .expect("session file");

        let page = store
            .list_threads(ListThreadsParams {
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                allowed_sources: Vec::new(),
                model_providers: None,
                archived: false,
                search_term: None,
            })
            .await
            .expect("thread listing");

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].model_provider, "test-provider");
    }

    #[tokio::test]
    async fn list_threads_preserves_sqlite_title_search_results() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let store = LocalThreadStore::new(config.clone());
        let uuid = Uuid::from_u128(103);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let rollout_path = home.path().join("rollout-title-search.jsonl");
        fs::write(&rollout_path, "").expect("placeholder rollout file");

        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        runtime
            .mark_backfill_complete(/*last_watermark*/ None)
            .await
            .expect("backfill should be complete");
        let created_at = Utc::now();
        let mut builder = codex_state::ThreadMetadataBuilder::new(
            thread_id,
            rollout_path,
            created_at,
            SessionSource::Cli,
        );
        builder.model_provider = Some(config.model_provider_id.clone());
        builder.cwd = home.path().to_path_buf();
        builder.cli_version = Some("test_version".to_string());
        let mut metadata = builder.build(config.model_provider_id.as_str());
        metadata.title = "needle title".to_string();
        metadata.first_user_message = Some("plain preview".to_string());
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("state db upsert should succeed");

        let page = store
            .list_threads(ListThreadsParams {
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                allowed_sources: Vec::new(),
                model_providers: None,
                archived: false,
                search_term: Some("needle".to_string()),
            })
            .await
            .expect("thread listing");

        let ids = page
            .items
            .iter()
            .map(|item| item.thread_id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec![thread_id]);
        assert_eq!(
            page.items[0].first_user_message.as_deref(),
            Some("plain preview")
        );
    }

    #[tokio::test]
    async fn list_threads_selects_active_or_archived_collection() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()));
        let active_uuid = Uuid::from_u128(105);
        let archived_uuid = Uuid::from_u128(106);
        write_session_file(home.path(), "2025-01-03T12-00-00", active_uuid)
            .expect("active session file");
        write_archived_session_file(home.path(), "2025-01-03T13-00-00", archived_uuid)
            .expect("archived session file");

        let active = store
            .list_threads(ListThreadsParams {
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                allowed_sources: Vec::new(),
                model_providers: None,
                archived: false,
                search_term: None,
            })
            .await
            .expect("active listing");
        let archived = store
            .list_threads(ListThreadsParams {
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                allowed_sources: Vec::new(),
                model_providers: None,
                archived: true,
                search_term: None,
            })
            .await
            .expect("archived listing");

        let active_id = ThreadId::from_string(&active_uuid.to_string()).expect("valid thread id");
        let archived_id =
            ThreadId::from_string(&archived_uuid.to_string()).expect("valid thread id");
        assert_eq!(
            active
                .items
                .iter()
                .map(|item| item.thread_id)
                .collect::<Vec<_>>(),
            vec![active_id]
        );
        assert_eq!(
            archived
                .items
                .iter()
                .map(|item| item.thread_id)
                .collect::<Vec<_>>(),
            vec![archived_id]
        );
        assert_eq!(active.items[0].archived_at, None);
        assert_eq!(
            archived.items[0].archived_at,
            Some(archived.items[0].updated_at)
        );
    }

    #[tokio::test]
    async fn list_threads_returns_local_rollout_summary() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let store = LocalThreadStore::new(config);
        let uuid = Uuid::from_u128(101);
        let path =
            write_session_file(home.path(), "2025-01-03T12-00-00", uuid).expect("session file");

        let page = store
            .list_threads(ListThreadsParams {
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                allowed_sources: vec![SessionSource::Cli],
                model_providers: Some(vec!["test-provider".to_string()]),
                archived: false,
                search_term: None,
            })
            .await
            .expect("thread listing");

        let thread_id =
            codex_protocol::ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        assert_eq!(page.next_cursor, None);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].thread_id, thread_id);
        assert_eq!(page.items[0].rollout_path, Some(path));
        assert_eq!(page.items[0].preview, "Hello from user");
        assert_eq!(
            page.items[0].first_user_message.as_deref(),
            Some("Hello from user")
        );
        assert_eq!(page.items[0].model_provider, "test-provider");
        assert_eq!(page.items[0].cli_version, "test_version");
        assert_eq!(page.items[0].source, SessionSource::Cli);
    }

    #[tokio::test]
    async fn list_threads_rejects_invalid_cursor() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()));

        let err = store
            .list_threads(ListThreadsParams {
                page_size: 10,
                cursor: Some("not-a-cursor".to_string()),
                sort_key: ThreadSortKey::CreatedAt,
                allowed_sources: Vec::new(),
                model_providers: None,
                archived: false,
                search_term: None,
            })
            .await
            .expect_err("invalid cursor should fail");

        assert!(matches!(err, ThreadStoreError::InvalidRequest { .. }));
    }
}

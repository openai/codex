use super::with_thread_spawn_agent_metadata;
use chrono::SecondsFormat;
use codex_app_server_protocol::ConversationGitInfo;
use codex_app_server_protocol::ConversationSummary;
use codex_app_server_protocol::GitInfo as ApiGitInfo;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::build_turns_from_rollout_items;
use codex_core::path_utils;
use codex_thread_store::ArchiveThreadParams as StoreArchiveThreadParams;
use codex_thread_store::ListThreadsParams as StoreListThreadsParams;
use codex_thread_store::LocalThreadStore;
use codex_thread_store::ReadThreadParams as StoreReadThreadParams;
use codex_thread_store::StoredThread;
use codex_thread_store::ThreadPage;
use codex_thread_store::ThreadStore;
use codex_thread_store::ThreadStoreResult;
use codex_thread_store::UpdateThreadMetadataParams as StoreUpdateThreadMetadataParams;
use codex_utils_absolute_path::AbsolutePathBuf;
use tracing::warn;

/// App-server boundary around `ThreadStore`. Adapts `StoredThread` to `Thread`.
pub(crate) struct ThreadStoreAdapter<S = LocalThreadStore> {
    store: S,
    fallback_provider: String,
    fallback_cwd: AbsolutePathBuf,
}

pub(crate) struct ThreadSummaryPage {
    pub(crate) items: Vec<ConversationSummary>,
    pub(crate) next_cursor: Option<String>,
}

impl<S> ThreadStoreAdapter<S> {
    pub(crate) fn new(store: S, fallback_provider: String, fallback_cwd: AbsolutePathBuf) -> Self {
        Self {
            store,
            fallback_provider,
            fallback_cwd,
        }
    }
}

impl<S> ThreadStoreAdapter<S>
where
    S: ThreadStore,
{
    pub(crate) async fn read_thread(
        &self,
        params: StoreReadThreadParams,
    ) -> ThreadStoreResult<Thread> {
        let include_turns = params.include_history;
        let stored_thread = self.store.read_thread(params).await?;
        Ok(self.thread_from_stored_thread(stored_thread, include_turns))
    }

    pub(crate) async fn list_thread_summaries(
        &self,
        params: StoreListThreadsParams,
    ) -> ThreadStoreResult<ThreadSummaryPage> {
        let page = self.store.list_threads(params).await?;
        Ok(self.thread_summary_page_from_store(page))
    }

    pub(crate) async fn update_thread_metadata(
        &self,
        params: StoreUpdateThreadMetadataParams,
    ) -> ThreadStoreResult<Thread> {
        let stored_thread = self.store.update_thread_metadata(params).await?;
        Ok(self.thread_from_stored_thread(stored_thread, /*include_turns*/ false))
    }

    pub(crate) async fn archive_thread(
        &self,
        params: StoreArchiveThreadParams,
    ) -> ThreadStoreResult<()> {
        self.store.archive_thread(params).await
    }

    pub(crate) async fn unarchive_thread(
        &self,
        params: StoreArchiveThreadParams,
    ) -> ThreadStoreResult<Thread> {
        let stored_thread = self.store.unarchive_thread(params).await?;
        Ok(self.thread_from_stored_thread(stored_thread, /*include_turns*/ false))
    }
}

impl<S> ThreadStoreAdapter<S> {
    fn thread_from_stored_thread(
        &self,
        stored_thread: StoredThread,
        include_turns: bool,
    ) -> Thread {
        let path = stored_thread.rollout_path;
        let git_info = stored_thread.git_info.map(|info| ApiGitInfo {
            sha: info.commit_hash.map(|sha| sha.0),
            branch: info.branch,
            origin_url: info.repository_url,
        });
        let cwd = AbsolutePathBuf::relative_to_current_dir(
            path_utils::normalize_for_native_workdir(stored_thread.cwd),
        )
        .unwrap_or_else(|err| {
            warn!("failed to normalize thread cwd while reading stored thread: {err}");
            self.fallback_cwd.clone()
        });
        let source = with_thread_spawn_agent_metadata(
            stored_thread.source,
            stored_thread.agent_nickname.clone(),
            stored_thread.agent_role.clone(),
        );
        let mut thread = Thread {
            id: stored_thread.thread_id.to_string(),
            forked_from_id: stored_thread.forked_from_id.map(|id| id.to_string()),
            preview: stored_thread
                .first_user_message
                .unwrap_or(stored_thread.preview),
            ephemeral: false,
            model_provider: if stored_thread.model_provider.is_empty() {
                self.fallback_provider.clone()
            } else {
                stored_thread.model_provider
            },
            created_at: stored_thread.created_at.timestamp(),
            updated_at: stored_thread.updated_at.timestamp(),
            status: ThreadStatus::NotLoaded,
            path,
            cwd,
            cli_version: stored_thread.cli_version,
            agent_nickname: source.get_nickname(),
            agent_role: source.get_agent_role(),
            source: source.into(),
            git_info,
            name: stored_thread.name,
            turns: Vec::new(),
        };
        if include_turns && let Some(history) = stored_thread.history {
            thread.turns = build_turns_from_rollout_items(&history.items);
        }
        thread
    }

    fn thread_summary_page_from_store(&self, page: ThreadPage) -> ThreadSummaryPage {
        let items = page
            .items
            .into_iter()
            .filter_map(|stored_thread| self.summary_from_stored_thread(stored_thread))
            .collect();
        ThreadSummaryPage {
            items,
            next_cursor: page.next_cursor,
        }
    }

    fn summary_from_stored_thread(
        &self,
        stored_thread: StoredThread,
    ) -> Option<ConversationSummary> {
        let path = stored_thread.rollout_path?;
        let source = with_thread_spawn_agent_metadata(
            stored_thread.source,
            stored_thread.agent_nickname.clone(),
            stored_thread.agent_role.clone(),
        );
        let git_info = stored_thread.git_info.map(|git| ConversationGitInfo {
            sha: git.commit_hash.map(|sha| sha.0),
            branch: git.branch,
            origin_url: git.repository_url,
        });
        Some(ConversationSummary {
            conversation_id: stored_thread.thread_id,
            path,
            preview: stored_thread
                .first_user_message
                .unwrap_or(stored_thread.preview),
            // Preserve millisecond precision from the thread store so thread/list cursors
            // round-trip the same ordering key used by pagination queries.
            timestamp: Some(
                stored_thread
                    .created_at
                    .to_rfc3339_opts(SecondsFormat::Millis, true),
            ),
            updated_at: Some(
                stored_thread
                    .updated_at
                    .to_rfc3339_opts(SecondsFormat::Millis, true),
            ),
            model_provider: if stored_thread.model_provider.is_empty() {
                self.fallback_provider.clone()
            } else {
                stored_thread.model_provider
            },
            cwd: stored_thread.cwd,
            cli_version: stored_thread.cli_version,
            source,
            git_info,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::AskForApproval;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::RolloutItem;
    use codex_protocol::protocol::SandboxPolicy;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::UserMessageEvent;
    use codex_thread_store::StoredThreadHistory;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn summary_preserves_millisecond_precision() {
        let created_at =
            DateTime::parse_from_rfc3339("2025-01-02T03:04:05.678Z").expect("valid timestamp");
        let updated_at =
            DateTime::parse_from_rfc3339("2025-01-02T03:04:06.789Z").expect("valid timestamp");
        let stored_thread = stored_thread(
            created_at.with_timezone(&Utc),
            updated_at.with_timezone(&Utc),
        );

        let adapter = ThreadStoreAdapter::new(
            (),
            "fallback".to_string(),
            AbsolutePathBuf::from_absolute_path_checked("/fallback").expect("absolute path"),
        );
        let summary = adapter
            .summary_from_stored_thread(stored_thread)
            .expect("summary should exist");

        assert_eq!(
            summary.timestamp.as_deref(),
            Some("2025-01-02T03:04:05.678Z")
        );
        assert_eq!(
            summary.updated_at.as_deref(),
            Some("2025-01-02T03:04:06.789Z")
        );
    }

    #[test]
    fn thread_conversion_includes_history_when_requested() {
        let created_at =
            DateTime::parse_from_rfc3339("2025-01-02T03:04:05.678Z").expect("valid timestamp");
        let updated_at =
            DateTime::parse_from_rfc3339("2025-01-02T03:04:06.789Z").expect("valid timestamp");
        let mut stored_thread = stored_thread(
            created_at.with_timezone(&Utc),
            updated_at.with_timezone(&Utc),
        );
        stored_thread.history = Some(StoredThreadHistory {
            thread_id: stored_thread.thread_id,
            items: vec![RolloutItem::EventMsg(EventMsg::UserMessage(
                UserMessageEvent {
                    message: "hello".to_string(),
                    images: None,
                    local_images: Vec::new(),
                    text_elements: Vec::new(),
                },
            ))],
        });

        let adapter = ThreadStoreAdapter::new(
            (),
            "fallback".to_string(),
            AbsolutePathBuf::from_absolute_path_checked("/fallback").expect("absolute path"),
        );
        let thread = adapter.thread_from_stored_thread(stored_thread, /*include_turns*/ true);

        assert_eq!(thread.turns.len(), 1);
    }

    fn stored_thread(created_at: DateTime<Utc>, updated_at: DateTime<Utc>) -> StoredThread {
        StoredThread {
            thread_id: ThreadId::from_string("00000000-0000-0000-0000-000000000123")
                .expect("valid thread"),
            rollout_path: Some(PathBuf::from("/tmp/thread.jsonl")),
            forked_from_id: None,
            preview: "preview".to_string(),
            name: None,
            model_provider: "openai".to_string(),
            model: None,
            reasoning_effort: None,
            created_at,
            updated_at,
            archived_at: None,
            cwd: PathBuf::from("/tmp"),
            cli_version: "0.0.0".to_string(),
            source: SessionSource::Cli,
            agent_nickname: None,
            agent_role: None,
            agent_path: None,
            git_info: None,
            approval_mode: AskForApproval::OnRequest,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            token_usage: None,
            first_user_message: Some("first user message".to_string()),
            history: None,
        }
    }
}

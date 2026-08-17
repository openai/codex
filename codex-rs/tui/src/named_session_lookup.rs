use std::path::Path;

use crate::app_server_session::AppServerSession;
use crate::legacy_core::config::Config;
use crate::resume_picker::SessionTarget;
use codex_app_server_protocol::Thread as AppServerThread;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadSortKey;
use codex_app_server_protocol::ThreadSourceKind;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionSource;

// Keep this aligned with `resume_source_kinds(/*include_non_interactive*/ false)`.
const RESUME_SESSION_SOURCES: [SessionSource; 2] = [SessionSource::Cli, SessionSource::VSCode];

/// Selects whether a local lookup can repair SQLite metadata from rollout files.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SessionNameLookupMode {
    StateDbOnly,
    ScanAndRepair,
}

/// Selects the rollout collection that a local candidate must belong to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SessionCollection {
    Active,
    Archived,
}

/// An exact-name match and the rollout metadata verified for local candidates.
pub(super) struct NamedSessionCandidate {
    pub(super) thread: AppServerThread,
    /// Remote candidates never inspect local rollout files and therefore have no session metadata.
    pub(super) session_meta: Option<SessionMeta>,
}

/// Lazily pages through exact-name matches and skips unusable local rollouts.
pub(super) struct NamedSessionCandidates<'a> {
    name: &'a str,
    codex_home: &'a Path,
    collection: SessionCollection,
    mode: SessionNameLookupMode,
    search_term: Option<&'a str>,
    source_kinds: Vec<ThreadSourceKind>,
    cursor: Option<String>,
    pending: std::vec::IntoIter<AppServerThread>,
    has_more: bool,
}

/// Resolves an active session by exact name.
///
/// Local lookups treat a usable SQLite match as authoritative and consult legacy rollout metadata
/// only to recover a missing or unusable row. Remote workspaces retain the backend's scan behavior.
pub(super) async fn lookup(
    app_server: &mut AppServerSession,
    config: &Config,
    name: &str,
) -> color_eyre::Result<Option<SessionTarget>> {
    if app_server.uses_remote_workspace() {
        return Ok(lookup_from_app_server(
            app_server,
            config,
            name,
            SessionNameLookupMode::ScanAndRepair,
        )
        .await?
        .and_then(super::session_target_from_app_server_thread));
    }

    if let Some(thread) =
        lookup_from_app_server(app_server, config, name, SessionNameLookupMode::StateDbOnly).await?
    {
        return Ok(super::session_target_from_app_server_thread(thread));
    }

    if let Some(target) = lookup_legacy_index_target(app_server, config, name).await? {
        return Ok(Some(target));
    }

    Ok(lookup_from_app_server(
        app_server,
        config,
        name,
        SessionNameLookupMode::ScanAndRepair,
    )
    .await?
    .and_then(super::session_target_from_app_server_thread))
}

/// Recovers a local target from legacy metadata without overriding an explicit SQLite name.
///
/// Rewriting the recovered name into SQLite is best effort and cannot invalidate a usable target.
async fn lookup_legacy_index_target(
    app_server: &mut AppServerSession,
    config: &Config,
    name: &str,
) -> color_eyre::Result<Option<SessionTarget>> {
    // Older rollouts omitted the provider and were historically assigned to the current provider
    // during scans, so only reject an explicit mismatch here.
    let candidates = codex_rollout::find_thread_meta_candidates_by_name_str(
        config.codex_home.as_path(),
        name,
        /*state_db_ctx*/ None,
        &RESUME_SESSION_SOURCES,
        std::slice::from_ref(&config.model_provider_id),
    )
    .await?;
    for (path, session_meta) in candidates {
        if !path.starts_with(config.codex_home.join(codex_rollout::SESSIONS_SUBDIR)) {
            continue;
        }

        // `thread/read` resolves the candidate's current SQLite title. A conflicting title means
        // the sidecar is stale, while a missing legacy title is the partial-write case it recovers.
        let thread = app_server
            .thread_read(session_meta.meta.id, /*include_turns*/ false)
            .await?;
        if !current_name_is_compatible(&thread, name) {
            continue;
        }
        if let Err(err) = app_server
            .thread_set_name(session_meta.meta.id, name.to_string())
            .await
        {
            tracing::warn!(
                thread_id = %session_meta.meta.id,
                %err,
                "failed to repair recovered thread name"
            );
        }
        return Ok(Some(SessionTarget {
            path: Some(path),
            thread_id: session_meta.meta.id,
            history_mode: Some(thread.history_mode),
        }));
    }
    Ok(None)
}

/// Allows a missing SQLite name only for legacy threads, whose name may exist only in the index.
pub(super) fn current_name_is_compatible(thread: &AppServerThread, name: &str) -> bool {
    match thread.history_mode {
        ThreadHistoryMode::Legacy => thread.name.as_deref().is_none_or(|current| current == name),
        ThreadHistoryMode::Paginated => thread.name.as_deref() == Some(name),
    }
}

/// Returns an eligible active thread for one SQLite-only or scan-and-repair lookup pass.
///
/// Local resume candidates also honor source/provider filters and reject stale scanned titles.
async fn lookup_from_app_server(
    app_server: &mut AppServerSession,
    config: &Config,
    name: &str,
    mode: SessionNameLookupMode,
) -> color_eyre::Result<Option<AppServerThread>> {
    let mut candidates = NamedSessionCandidates::new(
        name,
        config.codex_home.as_path(),
        SessionCollection::Active,
        mode,
        Some(name),
        super::resume_source_kinds(/*include_non_interactive*/ false),
    );
    while let Some(candidate) = candidates.next(app_server).await? {
        if let Some(session_meta) = candidate.session_meta
            && (!RESUME_SESSION_SOURCES.contains(&session_meta.source)
                || session_meta
                    .model_provider
                    .as_deref()
                    .is_some_and(|provider| provider != config.model_provider_id.as_str()))
        {
            continue;
        }
        if mode == SessionNameLookupMode::ScanAndRepair && !app_server.uses_remote_workspace() {
            // Scans can surface stale sidecar names, so verify the candidate's current title.
            let thread = app_server
                .thread_read(
                    ThreadId::from_string(&candidate.thread.id)?,
                    /*include_turns*/ false,
                )
                .await?;
            if !current_name_is_compatible(&thread, name) {
                continue;
            }
            return Ok(Some(thread));
        }
        return Ok(Some(candidate.thread));
    }
    Ok(None)
}

impl<'a> NamedSessionCandidates<'a> {
    pub(super) fn new(
        name: &'a str,
        codex_home: &'a Path,
        collection: SessionCollection,
        mode: SessionNameLookupMode,
        search_term: Option<&'a str>,
        source_kinds: Vec<ThreadSourceKind>,
    ) -> Self {
        Self {
            name,
            codex_home,
            collection,
            mode,
            search_term,
            source_kinds,
            cursor: None,
            pending: Vec::new().into_iter(),
            has_more: true,
        }
    }

    /// Returns the next exact-name match, advancing through server pages as needed.
    ///
    /// Local candidates must have a rollout in the requested collection with a matching session
    /// header; remote candidates preserve server-side lookup without touching local files.
    pub(super) async fn next(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> color_eyre::Result<Option<NamedSessionCandidate>> {
        loop {
            if let Some(thread) = self.pending.next() {
                if thread.name.as_deref() != Some(self.name) {
                    continue;
                }
                let session_meta = if app_server.uses_remote_workspace() {
                    None
                } else {
                    let (Ok(thread_id), Some(path)) =
                        (ThreadId::from_string(&thread.id), thread.path.as_deref())
                    else {
                        continue;
                    };
                    let Some(path) = codex_rollout::existing_rollout_path(path).await else {
                        continue;
                    };
                    let expected_root = self.codex_home.join(match self.collection {
                        SessionCollection::Active => codex_rollout::SESSIONS_SUBDIR,
                        SessionCollection::Archived => codex_rollout::ARCHIVED_SESSIONS_SUBDIR,
                    });
                    if !path.starts_with(expected_root) {
                        continue;
                    }
                    let Ok(session_meta) =
                        codex_rollout::read_session_meta_line(path.as_path()).await
                    else {
                        continue;
                    };
                    if session_meta.meta.id != thread_id {
                        continue;
                    }
                    Some(session_meta.meta)
                };
                return Ok(Some(NamedSessionCandidate {
                    thread,
                    session_meta,
                }));
            }

            if !self.has_more {
                return Ok(None);
            }
            // Only embedded SQLite supports recency sorting; scanners and older daemons use mtime.
            let sort_key = if self.mode == SessionNameLookupMode::StateDbOnly
                && app_server.uses_embedded_app_server()
            {
                ThreadSortKey::RecencyAt
            } else {
                ThreadSortKey::UpdatedAt
            };
            let response = app_server
                .thread_list(ThreadListParams {
                    cursor: self.cursor.clone(),
                    limit: Some(100),
                    sort_key: Some(sort_key),
                    sort_direction: None,
                    model_providers: None,
                    source_kinds: Some(self.source_kinds.clone()),
                    archived: Some(self.collection == SessionCollection::Archived),
                    section_id: None,
                    project_id: None,
                    parent_thread_id: None,
                    ancestor_thread_id: None,
                    cwd: None,
                    use_state_db_only: self.mode == SessionNameLookupMode::StateDbOnly,
                    search_term: self.search_term.map(str::to_string),
                })
                .await?;
            self.cursor = response.next_cursor;
            self.has_more = self.cursor.is_some();
            self.pending = response.data.into_iter();
        }
    }
}

#[cfg(test)]
#[path = "named_session_lookup_tests.rs"]
mod tests;

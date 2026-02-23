use async_trait::async_trait;
use std::cmp::Reverse;
use std::ffi::OsStr;
use std::io::{self};
use std::num::NonZero;
use std::ops::ControlFlow;
use std::path::Path;
use std::path::PathBuf;
use time::OffsetDateTime;
use time::PrimitiveDateTime;
use time::format_description::FormatItem;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use uuid::Uuid;

use super::ARCHIVED_SESSIONS_SUBDIR;
use super::SESSIONS_SUBDIR;
use crate::protocol::EventMsg;
use crate::state_db;
use codex_file_search as file_search;
use codex_protocol::ThreadId;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::USER_MESSAGE_BEGIN;

/// Returned page of thread (thread) summaries.
#[derive(Debug, Default, PartialEq)]
pub struct ThreadsPage {
    /// Thread summaries ordered newest first.
    pub items: Vec<ThreadItem>,
    /// Opaque pagination token to resume after the last item, or `None` if end.
    pub next_cursor: Option<Cursor>,
    /// Total number of files touched while scanning this request.
    pub num_scanned_files: usize,
    /// True if a hard scan cap was hit; consider resuming with `next_cursor`.
    pub reached_scan_cap: bool,
}

/// Summary information for a thread rollout file.
#[derive(Debug, PartialEq, Default)]
pub struct ThreadItem {
    /// Absolute path to the rollout file.
    pub path: PathBuf,
    /// Thread ID from session metadata.
    pub thread_id: Option<ThreadId>,
    /// First user message captured for this thread, if any.
    pub first_user_message: Option<String>,
    /// Working directory from session metadata.
    pub cwd: Option<PathBuf>,
    /// Git branch from session metadata.
    pub git_branch: Option<String>,
    /// Git commit SHA from session metadata.
    pub git_sha: Option<String>,
    /// Git origin URL from session metadata.
    pub git_origin_url: Option<String>,
    /// Session source from session metadata.
    pub source: Option<SessionSource>,
    /// Random unique nickname from session metadata for AgentControl-spawned sub-agents.
    pub agent_nickname: Option<String>,
    /// Role (agent_role) from session metadata for AgentControl-spawned sub-agents.
    pub agent_role: Option<String>,
    /// Model provider from session metadata.
    pub model_provider: Option<String>,
    /// CLI version from session metadata.
    pub cli_version: Option<String>,
    /// RFC3339 timestamp string for when the session was created, if available.
    /// created_at comes from the filename timestamp with second precision.
    pub created_at: Option<String>,
    /// RFC3339 timestamp string for the most recent update (from file mtime).
    /// updated_at is truncated to second precision to match created_at.
    pub updated_at: Option<String>,
}

#[allow(dead_code)]
#[deprecated(note = "use ThreadItem")]
pub type ConversationItem = ThreadItem;
#[allow(dead_code)]
#[deprecated(note = "use ThreadsPage")]
pub type ConversationsPage = ThreadsPage;

#[derive(Default)]
struct HeadTailSummary {
    saw_session_meta: bool,
    saw_user_event: bool,
    thread_id: Option<ThreadId>,
    first_user_message: Option<String>,
    cwd: Option<PathBuf>,
    git_branch: Option<String>,
    git_sha: Option<String>,
    git_origin_url: Option<String>,
    source: Option<SessionSource>,
    agent_nickname: Option<String>,
    agent_role: Option<String>,
    model_provider: Option<String>,
    cli_version: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

/// Hard cap to bound worst‑case work per request.
const MAX_SCAN_FILES: usize = 10000;
const HEAD_RECORD_LIMIT: usize = 10;
const USER_EVENT_SCAN_LIMIT: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadSortKey {
    CreatedAt,
    UpdatedAt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThreadListLayout {
    NestedByDate,
    Flat,
}

pub(crate) struct ThreadListConfig<'a> {
    pub(crate) allowed_sources: &'a [SessionSource],
    pub(crate) model_providers: Option<&'a [String]>,
    pub(crate) default_provider: &'a str,
    pub(crate) layout: ThreadListLayout,
}

/// Pagination cursor identifying a file by timestamp and UUID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    ts: OffsetDateTime,
    id: Uuid,
}

impl Cursor {
    fn new(ts: OffsetDateTime, id: Uuid) -> Self {
        Self { ts, id }
    }
}

/// Keeps track of where a paginated listing left off. As the file scan goes newest -> oldest,
/// it ignores everything until it reaches the last seen item from the previous page, then
/// starts returning results after that. This makes paging stable even if new files show up during
/// pagination.
struct AnchorState {
    ts: OffsetDateTime,
    id: Uuid,
    passed: bool,
}

impl AnchorState {
    fn new(anchor: Option<Cursor>) -> Self {
        match anchor {
            Some(cursor) => Self {
                ts: cursor.ts,
                id: cursor.id,
                passed: false,
            },
            None => Self {
                ts: OffsetDateTime::UNIX_EPOCH,
                id: Uuid::nil(),
                passed: true,
            },
        }
    }

    fn should_skip(&mut self, ts: OffsetDateTime, id: Uuid) -> bool {
        if self.passed {
            return false;
        }
        if ts < self.ts || (ts == self.ts && id < self.id) {
            self.passed = true;
            false
        } else {
            true
        }
    }
}

/// Visitor interface to customize behavior when visiting each rollout file
/// in `walk_rollout_files`.
///
/// We need to apply different logic if we're ultimately going to be returning
/// threads ordered by created_at or updated_at.
#[async_trait]
trait RolloutFileVisitor {
    async fn visit(
        &mut self,
        ts: OffsetDateTime,
        id: Uuid,
        path: PathBuf,
        scanned: usize,
    ) -> ControlFlow<()>;
}

/// Collects thread items during directory traversal in created_at order,
/// applying pagination and filters inline.
struct FilesByCreatedAtVisitor<'a> {
    items: &'a mut Vec<ThreadItem>,
    page_size: usize,
    anchor_state: AnchorState,
    more_matches_available: bool,
    allowed_sources: &'a [SessionSource],
    provider_matcher: Option<&'a ProviderMatcher<'a>>,
}

#[async_trait]
impl<'a> RolloutFileVisitor for FilesByCreatedAtVisitor<'a> {
    async fn visit(
        &mut self,
        ts: OffsetDateTime,
        id: Uuid,
        path: PathBuf,
        scanned: usize,
    ) -> ControlFlow<()> {
        if scanned >= MAX_SCAN_FILES && self.items.len() >= self.page_size {
            self.more_matches_available = true;
            return ControlFlow::Break(());
        }
        if self.anchor_state.should_skip(ts, id) {
            return ControlFlow::Continue(());
        }
        if self.items.len() == self.page_size {
            self.more_matches_available = true;
            return ControlFlow::Break(());
        }
        let updated_at = file_modified_time(&path)
            .await
            .unwrap_or(None)
            .and_then(format_rfc3339);
        if let Some(item) = build_thread_item(
            path,
            self.allowed_sources,
            self.provider_matcher,
            updated_at,
        )
        .await
        {
            self.items.push(item);
        }
        ControlFlow::Continue(())
    }
}

/// Collects lightweight file candidates (path + id + mtime).
/// Sorting after mtime happens after all files are collected.
struct FilesByUpdatedAtVisitor<'a> {
    candidates: &'a mut Vec<ThreadCandidate>,
}

#[async_trait]
impl<'a> RolloutFileVisitor for FilesByUpdatedAtVisitor<'a> {
    async fn visit(
        &mut self,
        _ts: OffsetDateTime,
        id: Uuid,
        path: PathBuf,
        _scanned: usize,
    ) -> ControlFlow<()> {
        let updated_at = file_modified_time(&path).await.unwrap_or(None);
        self.candidates.push(ThreadCandidate {
            path,
            id,
            updated_at,
        });
        ControlFlow::Continue(())
    }
}

impl serde::Serialize for Cursor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let ts_str = self
            .ts
            .format(&Rfc3339)
            .map_err(|e| serde::ser::Error::custom(format!("format error: {e}")))?;
        serializer.serialize_str(&format!("{ts_str}|{}", self.id))
    }
}

impl<'de> serde::Deserialize<'de> for Cursor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        parse_cursor(&s).ok_or_else(|| serde::de::Error::custom("invalid cursor"))
    }
}

impl From<codex_state::Anchor> for Cursor {
    fn from(anchor: codex_state::Anchor) -> Self {
        let ts = OffsetDateTime::from_unix_timestamp(anchor.ts.timestamp())
            .unwrap_or(OffsetDateTime::UNIX_EPOCH);
        Self::new(ts, anchor.id)
    }
}

/// Retrieve recorded thread file paths with token pagination. The returned `next_cursor`
/// can be supplied on the next call to resume after the last returned item, resilient to
/// concurrent new sessions being appended. Ordering is stable by the requested sort key
/// (timestamp desc, then UUID desc).
pub(crate) async fn get_threads(
    codex_home: &Path,
    page_size: usize,
    cursor: Option<&Cursor>,
    sort_key: ThreadSortKey,
    allowed_sources: &[SessionSource],
    model_providers: Option<&[String]>,
    default_provider: &str,
) -> io::Result<ThreadsPage> {
    let root = codex_home.join(SESSIONS_SUBDIR);
    get_threads_in_root(
        root,
        page_size,
        cursor,
        sort_key,
        ThreadListConfig {
            allowed_sources,
            model_providers,
            default_provider,
            layout: ThreadListLayout::NestedByDate,
        },
    )
    .await
}

pub(crate) async fn get_threads_in_root(
    root: PathBuf,
    page_size: usize,
    cursor: Option<&Cursor>,
    sort_key: ThreadSortKey,
    config: ThreadListConfig<'_>,
) -> io::Result<ThreadsPage> {
    if !root.exists() {
        return Ok(ThreadsPage {
            items: Vec::new(),
            next_cursor: None,
            num_scanned_files: 0,
            reached_scan_cap: false,
        });
    }

    let anchor = cursor.cloned();

    let provider_matcher = config
        .model_providers
        .and_then(|filters| ProviderMatcher::new(filters, config.default_provider));

    let result = match config.layout {
        ThreadListLayout::NestedByDate => {
            traverse_directories_for_paths(
                root.clone(),
                page_size,
                anchor,
                sort_key,
                config.allowed_sources,
                provider_matcher.as_ref(),
            )
            .await?
        }
        ThreadListLayout::Flat => {
            traverse_flat_paths(
                root.clone(),
                page_size,
                anchor,
                sort_key,
                config.allowed_sources,
                provider_matcher.as_ref(),
            )
            .await?
        }
    };
    Ok(result)
}

/// Load thread file paths from disk using directory traversal.
///
/// Directory layout: `~/.codex/sessions/YYYY/MM/DD/rollout-YYYY-MM-DDThh-mm-ss-<uuid>.jsonl`
/// Returned newest (based on sort key) first.
async fn traverse_directories_for_paths(
    root: PathBuf,
    page_size: usize,
    anchor: Option<Cursor>,
    sort_key: ThreadSortKey,
    allowed_sources: &[SessionSource],
    provider_matcher: Option<&ProviderMatcher<'_>>,
) -> io::Result<ThreadsPage> {
    match sort_key {
        ThreadSortKey::CreatedAt => {
            traverse_directories_for_paths_created(
                root,
                page_size,
                anchor,
                allowed_sources,
                provider_matcher,
            )
            .await
        }
        ThreadSortKey::UpdatedAt => {
            traverse_directories_for_paths_updated(
                root,
                page_size,
                anchor,
                allowed_sources,
                provider_matcher,
            )
            .await
        }
    }
}

async fn traverse_flat_paths(
    root: PathBuf,
    page_size: usize,
    anchor: Option<Cursor>,
    sort_key: ThreadSortKey,
    allowed_sources: &[SessionSource],
    provider_matcher: Option<&ProviderMatcher<'_>>,
) -> io::Result<ThreadsPage> {
    match sort_key {
        ThreadSortKey::CreatedAt => {
            traverse_flat_paths_created(root, page_size, anchor, allowed_sources, provider_matcher)
                .await
        }
        ThreadSortKey::UpdatedAt => {
            traverse_flat_paths_updated(root, page_size, anchor, allowed_sources, provider_matcher)
                .await
        }
    }
}

/// Walk the rollout directory tree in reverse chronological order and
/// collect items until the page fills or the scan cap is hit.
///
/// Ordering comes from directory/filename sorting, so created_at is derived
/// from the filename timestamp. Pagination is handled by the anchor cursor
/// so we resume strictly after the last returned `(ts, id)` pair.
async fn traverse_directories_for_paths_created(
    root: PathBuf,
    page_size: usize,
    anchor: Option<Cursor>,
    allowed_sources: &[SessionSource],
    provider_matcher: Option<&ProviderMatcher<'_>>,
) -> io::Result<ThreadsPage> {
    let mut items: Vec<ThreadItem> = Vec::with_capacity(page_size);
    let mut scanned_files = 0usize;
    let mut more_matches_available = false;
    let mut visitor = FilesByCreatedAtVisitor {
        items: &mut items,
        page_size,
        anchor_state: AnchorState::new(anchor),
        more_matches_available,
        allowed_sources,
        provider_matcher,
    };
    walk_rollout_files(&root, &mut scanned_files, &mut visitor).await?;
    more_matches_available = visitor.more_matches_available;

    let reached_scan_cap = scanned_files >= MAX_SCAN_FILES;
    if reached_scan_cap && !items.is_empty() {
        more_matches_available = true;
    }

    let next = if more_matches_available {
        build_next_cursor(&items, ThreadSortKey::CreatedAt)
    } else {
        None
    };
    Ok(ThreadsPage {
        items,
        next_cursor: next,
        num_scanned_files: scanned_files,
        reached_scan_cap,
    })
}

/// Walk the rollout directory tree to collect files by updated_at, then sort by
/// file mtime (updated_at) and apply pagination/filtering in that order.
///
/// Because updated_at is not encoded in filenames, this path must scan all
/// files up to the scan cap, then sort and filter by the anchor cursor.
///
/// NOTE: This can be optimized in the future if we store additional state on disk
/// to cache updated_at timestamps.
async fn traverse_directories_for_paths_updated(
    root: PathBuf,
    page_size: usize,
    anchor: Option<Cursor>,
    allowed_sources: &[SessionSource],
    provider_matcher: Option<&ProviderMatcher<'_>>,
) -> io::Result<ThreadsPage> {
    let mut items: Vec<ThreadItem> = Vec::with_capacity(page_size);
    let mut scanned_files = 0usize;
    let mut anchor_state = AnchorState::new(anchor);
    let mut more_matches_available = false;

    let candidates = collect_files_by_updated_at(&root, &mut scanned_files).await?;
    let mut candidates = candidates;
    candidates.sort_by_key(|candidate| {
        let ts = candidate.updated_at.unwrap_or(OffsetDateTime::UNIX_EPOCH);
        (Reverse(ts), Reverse(candidate.id))
    });

    for candidate in candidates.into_iter() {
        let ts = candidate.updated_at.unwrap_or(OffsetDateTime::UNIX_EPOCH);
        if anchor_state.should_skip(ts, candidate.id) {
            continue;
        }
        if items.len() == page_size {
            more_matches_available = true;
            break;
        }

        let updated_at_fallback = candidate.updated_at.and_then(format_rfc3339);
        if let Some(item) = build_thread_item(
            candidate.path,
            allowed_sources,
            provider_matcher,
            updated_at_fallback,
        )
        .await
        {
            items.push(item);
        }
    }

    let reached_scan_cap = scanned_files >= MAX_SCAN_FILES;
    if reached_scan_cap && !items.is_empty() {
        more_matches_available = true;
    }

    let next = if more_matches_available {
        build_next_cursor(&items, ThreadSortKey::UpdatedAt)
    } else {
        None
    };
    Ok(ThreadsPage {
        items,
        next_cursor: next,
        num_scanned_files: scanned_files,
        reached_scan_cap,
    })
}

async fn traverse_flat_paths_created(
    root: PathBuf,
    page_size: usize,
    anchor: Option<Cursor>,
    allowed_sources: &[SessionSource],
    provider_matcher: Option<&ProviderMatcher<'_>>,
) -> io::Result<ThreadsPage> {
    let mut items: Vec<ThreadItem> = Vec::with_capacity(page_size);
    let mut scanned_files = 0usize;
    let mut anchor_state = AnchorState::new(anchor);
    let mut more_matches_available = false;

    let files = collect_flat_rollout_files(&root, &mut scanned_files).await?;
    for (ts, id, path) in files.into_iter() {
        if anchor_state.should_skip(ts, id) {
            continue;
        }
        if items.len() == page_size {
            more_matches_available = true;
            break;
        }
        let updated_at = file_modified_time(&path)
            .await
            .unwrap_or(None)
            .and_then(format_rfc3339);
        if let Some(item) =
            build_thread_item(path, allowed_sources, provider_matcher, updated_at).await
        {
            items.push(item);
        }
    }

    let reached_scan_cap = scanned_files >= MAX_SCAN_FILES;
    if reached_scan_cap && !items.is_empty() {
        more_matches_available = true;
    }

    let next = if more_matches_available {
        build_next_cursor(&items, ThreadSortKey::CreatedAt)
    } else {
        None
    };
    Ok(ThreadsPage {
        items,
        next_cursor: next,
        num_scanned_files: scanned_files,
        reached_scan_cap,
    })
}

async fn traverse_flat_paths_updated(
    root: PathBuf,
    page_size: usize,
    anchor: Option<Cursor>,
    allowed_sources: &[SessionSource],
    provider_matcher: Option<&ProviderMatcher<'_>>,
) -> io::Result<ThreadsPage> {
    let mut items: Vec<ThreadItem> = Vec::with_capacity(page_size);
    let mut scanned_files = 0usize;
    let mut anchor_state = AnchorState::new(anchor);
    let mut more_matches_available = false;

    let candidates = collect_flat_files_by_updated_at(&root, &mut scanned_files).await?;
    let mut candidates = candidates;
    candidates.sort_by_key(|candidate| {
        let ts = candidate.updated_at.unwrap_or(OffsetDateTime::UNIX_EPOCH);
        (Reverse(ts), Reverse(candidate.id))
    });

    for candidate in candidates.into_iter() {
        let ts = candidate.updated_at.unwrap_or(OffsetDateTime::UNIX_EPOCH);
        if anchor_state.should_skip(ts, candidate.id) {
            continue;
        }
        if items.len() == page_size {
            more_matches_available = true;
            break;
        }

        let updated_at_fallback = candidate.updated_at.and_then(format_rfc3339);
        if let Some(item) = build_thread_item(
            candidate.path,
            allowed_sources,
            provider_matcher,
            updated_at_fallback,
        )
        .await
        {
            items.push(item);
        }
    }

    let reached_scan_cap = scanned_files >= MAX_SCAN_FILES;
    if reached_scan_cap && !items.is_empty() {
        more_matches_available = true;
    }

    let next = if more_matches_available {
        build_next_cursor(&items, ThreadSortKey::UpdatedAt)
    } else {
        None
    };
    Ok(ThreadsPage {
        items,
        next_cursor: next,
        num_scanned_files: scanned_files,
        reached_scan_cap,
    })
}

/// Pagination cursor token format: "<ts>|<uuid>" where `ts` uses
/// YYYY-MM-DDThh-mm-ss (UTC, second precision).
/// The cursor orders files by the requested sort key (timestamp desc, then UUID desc).
pub fn parse_cursor(token: &str) -> Option<Cursor> {
    let (file_ts, uuid_str) = token.split_once('|')?;

    let Ok(uuid) = Uuid::parse_str(uuid_str) else {
        return None;
    };

    let ts = OffsetDateTime::parse(file_ts, &Rfc3339).ok().or_else(|| {
        let format: &[FormatItem] =
            format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
        PrimitiveDateTime::parse(file_ts, format)
            .ok()
            .map(PrimitiveDateTime::assume_utc)
    })?;

    Some(Cursor::new(ts, uuid))
}

fn build_next_cursor(items: &[ThreadItem], sort_key: ThreadSortKey) -> Option<Cursor> {
    let last = items.last()?;
    let file_name = last.path.file_name()?.to_string_lossy();
    let (created_ts, id) = parse_timestamp_uuid_from_filename(&file_name)?;
    let ts = match sort_key {
        ThreadSortKey::CreatedAt => created_ts,
        ThreadSortKey::UpdatedAt => {
            let updated_at = last.updated_at.as_deref()?;
            OffsetDateTime::parse(updated_at, &Rfc3339).ok()?
        }
    };
    Some(Cursor::new(ts, id))
}

async fn build_thread_item(
    path: PathBuf,
    allowed_sources: &[SessionSource],
    provider_matcher: Option<&ProviderMatcher<'_>>,
    updated_at: Option<String>,
) -> Option<ThreadItem> {
    // Read head and detect message events; stop once meta + user are found.
    let summary = read_head_summary(&path, HEAD_RECORD_LIMIT)
        .await
        .unwrap_or_default();
    if !allowed_sources.is_empty()
        && !summary
            .source
            .as_ref()
            .is_some_and(|source| allowed_sources.contains(source))
    {
        return None;
    }
    if let Some(matcher) = provider_matcher
        && !matcher.matches(summary.model_provider.as_deref())
    {
        return None;
    }
    // Apply filters: must have session meta and at least one user message event
    if summary.saw_session_meta && summary.saw_user_event {
        let HeadTailSummary {
            thread_id,
            first_user_message,
            cwd,
            git_branch,
            git_sha,
            git_origin_url,
            source,
            agent_nickname,
            agent_role,
            model_provider,
            cli_version,
            created_at,
            updated_at: mut summary_updated_at,
            ..
        } = summary;
        if summary_updated_at.is_none() {
            summary_updated_at = updated_at.or_else(|| created_at.clone());
        }
        return Some(ThreadItem {
            path,
            thread_id,
            first_user_message,
            cwd,
            git_branch,
            git_sha,
            git_origin_url,
            source,
            agent_nickname,
            agent_role,
            model_provider,
            cli_version,
            created_at,
            updated_at: summary_updated_at,
        });
    }
    None
}

/// Collects immediate subdirectories of `parent`, parses their (string) names with `parse`,
/// and returns them sorted descending by the parsed key.
async fn collect_dirs_desc<T, F>(parent: &Path, parse: F) -> io::Result<Vec<(T, PathBuf)>>
where
    T: Ord + Copy,
    F: Fn(&str) -> Option<T>,
{
    let mut dir = tokio::fs::read_dir(parent).await?;
    let mut vec: Vec<(T, PathBuf)> = Vec::new();
    while let Some(entry) = dir.next_entry().await? {
        if entry
            .file_type()
            .await
            .map(|ft| ft.is_dir())
            .unwrap_or(false)
            && let Some(s) = entry.file_name().to_str()
            && let Some(v) = parse(s)
        {
            vec.push((v, entry.path()));
        }
    }
    vec.sort_by_key(|(v, _)| Reverse(*v));
    Ok(vec)
}

/// Collects files in a directory and parses them with `parse`.
async fn collect_files<T, F>(parent: &Path, parse: F) -> io::Result<Vec<T>>
where
    F: Fn(&str, &Path) -> Option<T>,
{
    let mut dir = tokio::fs::read_dir(parent).await?;
    let mut collected: Vec<T> = Vec::new();
    while let Some(entry) = dir.next_entry().await? {
        if entry
            .file_type()
            .await
            .map(|ft| ft.is_file())
            .unwrap_or(false)
            && let Some(s) = entry.file_name().to_str()
            && let Some(v) = parse(s, &entry.path())
        {
            collected.push(v);
        }
    }
    Ok(collected)
}

async fn collect_flat_rollout_files(
    root: &Path,
    scanned_files: &mut usize,
) -> io::Result<Vec<(OffsetDateTime, Uuid, PathBuf)>> {
    let mut dir = tokio::fs::read_dir(root).await?;
    let mut collected = Vec::new();
    while let Some(entry) = dir.next_entry().await? {
        if *scanned_files >= MAX_SCAN_FILES {
            break;
        }
        if !entry
            .file_type()
            .await
            .map(|ft| ft.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        let file_name = entry.file_name();
        let Some(name_str) = file_name.to_str() else {
            continue;
        };
        if !name_str.starts_with("rollout-") || !name_str.ends_with(".jsonl") {
            continue;
        }
        let Some((ts, id)) = parse_timestamp_uuid_from_filename(name_str) else {
            continue;
        };
        *scanned_files += 1;
        if *scanned_files > MAX_SCAN_FILES {
            break;
        }
        collected.push((ts, id, entry.path()));
    }
    collected.sort_by_key(|(ts, sid, _path)| (Reverse(*ts), Reverse(*sid)));
    Ok(collected)
}

async fn collect_rollout_day_files(
    day_path: &Path,
) -> io::Result<Vec<(OffsetDateTime, Uuid, PathBuf)>> {
    let mut day_files = collect_files(day_path, |name_str, path| {
        if !name_str.starts_with("rollout-") || !name_str.ends_with(".jsonl") {
            return None;
        }

        parse_timestamp_uuid_from_filename(name_str).map(|(ts, id)| (ts, id, path.to_path_buf()))
    })
    .await?;
    // Stable ordering within the same second: (timestamp desc, uuid desc)
    day_files.sort_by_key(|(ts, sid, _path)| (Reverse(*ts), Reverse(*sid)));
    Ok(day_files)
}

pub(crate) fn parse_timestamp_uuid_from_filename(name: &str) -> Option<(OffsetDateTime, Uuid)> {
    // Expected: rollout-YYYY-MM-DDThh-mm-ss-<uuid>.jsonl
    let core = name.strip_prefix("rollout-")?.strip_suffix(".jsonl")?;

    // Scan from the right for a '-' such that the suffix parses as a UUID.
    let (sep_idx, uuid) = core
        .match_indices('-')
        .rev()
        .find_map(|(i, _)| Uuid::parse_str(&core[i + 1..]).ok().map(|u| (i, u)))?;

    let ts_str = &core[..sep_idx];
    let format: &[FormatItem] =
        format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
    let ts = PrimitiveDateTime::parse(ts_str, format).ok()?.assume_utc();
    Some((ts, uuid))
}

struct ThreadCandidate {
    path: PathBuf,
    id: Uuid,
    updated_at: Option<OffsetDateTime>,
}

async fn collect_files_by_updated_at(
    root: &Path,
    scanned_files: &mut usize,
) -> io::Result<Vec<ThreadCandidate>> {
    let mut candidates = Vec::new();
    let mut visitor = FilesByUpdatedAtVisitor {
        candidates: &mut candidates,
    };
    walk_rollout_files(root, scanned_files, &mut visitor).await?;

    Ok(candidates)
}

async fn collect_flat_files_by_updated_at(
    root: &Path,
    scanned_files: &mut usize,
) -> io::Result<Vec<ThreadCandidate>> {
    let mut candidates = Vec::new();
    let mut dir = tokio::fs::read_dir(root).await?;
    while let Some(entry) = dir.next_entry().await? {
        if *scanned_files >= MAX_SCAN_FILES {
            break;
        }
        if !entry
            .file_type()
            .await
            .map(|ft| ft.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        let file_name = entry.file_name();
        let Some(name_str) = file_name.to_str() else {
            continue;
        };
        if !name_str.starts_with("rollout-") || !name_str.ends_with(".jsonl") {
            continue;
        }
        let Some((_ts, id)) = parse_timestamp_uuid_from_filename(name_str) else {
            continue;
        };
        *scanned_files += 1;
        if *scanned_files > MAX_SCAN_FILES {
            break;
        }
        let updated_at = file_modified_time(&entry.path()).await.unwrap_or(None);
        candidates.push(ThreadCandidate {
            path: entry.path(),
            id,
            updated_at,
        });
    }

    Ok(candidates)
}

async fn walk_rollout_files(
    root: &Path,
    scanned_files: &mut usize,
    visitor: &mut impl RolloutFileVisitor,
) -> io::Result<()> {
    let year_dirs = collect_dirs_desc(root, |s| s.parse::<u16>().ok()).await?;

    'outer: for (_year, year_path) in year_dirs.iter() {
        if *scanned_files >= MAX_SCAN_FILES {
            break;
        }
        let month_dirs = collect_dirs_desc(year_path, |s| s.parse::<u8>().ok()).await?;
        for (_month, month_path) in month_dirs.iter() {
            if *scanned_files >= MAX_SCAN_FILES {
                break 'outer;
            }
            let day_dirs = collect_dirs_desc(month_path, |s| s.parse::<u8>().ok()).await?;
            for (_day, day_path) in day_dirs.iter() {
                if *scanned_files >= MAX_SCAN_FILES {
                    break 'outer;
                }
                let day_files = collect_rollout_day_files(day_path).await?;
                for (ts, id, path) in day_files.into_iter() {
                    *scanned_files += 1;
                    if *scanned_files > MAX_SCAN_FILES {
                        break 'outer;
                    }
                    if let ControlFlow::Break(()) =
                        visitor.visit(ts, id, path, *scanned_files).await
                    {
                        break 'outer;
                    }
                }
            }
        }
    }

    Ok(())
}

struct ProviderMatcher<'a> {
    filters: &'a [String],
    matches_default_provider: bool,
}

impl<'a> ProviderMatcher<'a> {
    fn new(filters: &'a [String], default_provider: &'a str) -> Option<Self> {
        if filters.is_empty() {
            return None;
        }

        let matches_default_provider = filters.iter().any(|provider| provider == default_provider);
        Some(Self {
            filters,
            matches_default_provider,
        })
    }

    fn matches(&self, session_provider: Option<&str>) -> bool {
        match session_provider {
            Some(provider) => self.filters.iter().any(|candidate| candidate == provider),
            None => self.matches_default_provider,
        }
    }
}

async fn read_head_summary(path: &Path, head_limit: usize) -> io::Result<HeadTailSummary> {
    use tokio::io::AsyncBufReadExt;

    let file = tokio::fs::File::open(path).await?;
    let reader = tokio::io::BufReader::new(file);
    let mut lines = reader.lines();
    let mut summary = HeadTailSummary::default();
    let mut lines_scanned = 0usize;

    while lines_scanned < head_limit
        || (summary.saw_session_meta
            && !summary.saw_user_event
            && lines_scanned < head_limit + USER_EVENT_SCAN_LIMIT)
    {
        let line_opt = lines.next_line().await?;
        let Some(line) = line_opt else { break };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        lines_scanned += 1;

        let parsed: Result<RolloutLine, _> = serde_json::from_str(trimmed);
        let Ok(rollout_line) = parsed else { continue };

        match rollout_line.item {
            RolloutItem::SessionMeta(session_meta_line) => {
                if !summary.saw_session_meta {
                    summary.source = Some(session_meta_line.meta.source.clone());
                    summary.agent_nickname = session_meta_line.meta.agent_nickname.clone();
                    summary.agent_role = session_meta_line.meta.agent_role.clone();
                    summary.model_provider = session_meta_line.meta.model_provider.clone();
                    summary.thread_id = Some(session_meta_line.meta.id);
                    summary.cwd = Some(session_meta_line.meta.cwd.clone());
                    summary.git_branch = session_meta_line
                        .git
                        .as_ref()
                        .and_then(|git| git.branch.clone());
                    summary.git_sha = session_meta_line
                        .git
                        .as_ref()
                        .and_then(|git| git.commit_hash.clone());
                    summary.git_origin_url = session_meta_line
                        .git
                        .as_ref()
                        .and_then(|git| git.repository_url.clone());
                    summary.cli_version = Some(session_meta_line.meta.cli_version);
                    summary.created_at = Some(session_meta_line.meta.timestamp.clone());
                    summary.saw_session_meta = true;
                }
            }
            RolloutItem::ResponseItem(_) => {
                summary.created_at = summary
                    .created_at
                    .clone()
                    .or_else(|| Some(rollout_line.timestamp.clone()));
            }
            RolloutItem::TurnContext(_) => {
                // Not included in `head`; skip.
            }
            RolloutItem::Compacted(_) => {
                // Not included in `head`; skip.
            }
            RolloutItem::EventMsg(ev) => {
                if let EventMsg::UserMessage(user) = ev {
                    summary.saw_user_event = true;
                    if summary.first_user_message.is_none() {
                        let message = strip_user_message_prefix(user.message.as_str()).to_string();
                        if !message.is_empty() {
                            summary.first_user_message = Some(message);
                        }
                    }
                }
            }
        }

        if summary.saw_session_meta && summary.saw_user_event {
            break;
        }
    }

    Ok(summary)
}

/// Read up to `HEAD_RECORD_LIMIT` records from the start of the rollout file at `path`.
/// This should be enough to produce a summary including the session meta line.
pub async fn read_head_for_summary(path: &Path) -> io::Result<Vec<serde_json::Value>> {
    use tokio::io::AsyncBufReadExt;

    let file = tokio::fs::File::open(path).await?;
    let reader = tokio::io::BufReader::new(file);
    let mut lines = reader.lines();
    let mut head = Vec::new();

    while head.len() < HEAD_RECORD_LIMIT {
        let Some(line) = lines.next_line().await? else {
            break;
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(rollout_line) = serde_json::from_str::<RolloutLine>(trimmed) {
            match rollout_line.item {
                RolloutItem::SessionMeta(session_meta_line) => {
                    if let Ok(value) = serde_json::to_value(session_meta_line) {
                        head.push(value);
                    }
                }
                RolloutItem::ResponseItem(item) => {
                    if let Ok(value) = serde_json::to_value(item) {
                        head.push(value);
                    }
                }
                RolloutItem::Compacted(_)
                | RolloutItem::TurnContext(_)
                | RolloutItem::EventMsg(_) => {}
            }
        }
    }

    Ok(head)
}

fn strip_user_message_prefix(text: &str) -> &str {
    match text.find(USER_MESSAGE_BEGIN) {
        Some(idx) => text[idx + USER_MESSAGE_BEGIN.len()..].trim(),
        None => text.trim(),
    }
}

/// Read the SessionMetaLine from the head of a rollout file for reuse by
/// callers that need the session metadata (e.g. to derive a cwd for config).
pub async fn read_session_meta_line(path: &Path) -> io::Result<SessionMetaLine> {
    let head = read_head_for_summary(path).await?;
    let Some(first) = head.first() else {
        return Err(io::Error::other(format!(
            "rollout at {} is empty",
            path.display()
        )));
    };
    serde_json::from_value::<SessionMetaLine>(first.clone()).map_err(|_| {
        io::Error::other(format!(
            "rollout at {} does not start with session metadata",
            path.display()
        ))
    })
}

async fn file_modified_time(path: &Path) -> io::Result<Option<OffsetDateTime>> {
    let meta = tokio::fs::metadata(path).await?;
    let modified = meta.modified().ok();
    let Some(modified) = modified else {
        return Ok(None);
    };
    let dt = OffsetDateTime::from(modified);
    // Truncate to seconds so ordering and cursor comparisons align with the
    // cursor timestamp format (which exposes seconds), keeping pagination stable.
    Ok(truncate_to_seconds(dt))
}

fn format_rfc3339(dt: OffsetDateTime) -> Option<String> {
    dt.format(&Rfc3339).ok()
}

fn truncate_to_seconds(dt: OffsetDateTime) -> Option<OffsetDateTime> {
    dt.replace_nanosecond(0).ok()
}

async fn find_thread_path_by_id_str_in_subdir(
    codex_home: &Path,
    subdir: &str,
    id_str: &str,
) -> io::Result<Option<PathBuf>> {
    // Validate UUID format early.
    if Uuid::parse_str(id_str).is_err() {
        return Ok(None);
    }

    // Prefer DB lookup, then fall back to rollout file search.
    // TODO(jif): sqlite migration phase 1
    let archived_only = match subdir {
        SESSIONS_SUBDIR => Some(false),
        ARCHIVED_SESSIONS_SUBDIR => Some(true),
        _ => None,
    };
    let thread_id = ThreadId::from_string(id_str).ok();
    let state_db_ctx = state_db::open_if_present(codex_home, "").await;
    if let Some(state_db_ctx) = state_db_ctx.as_deref()
        && let Some(thread_id) = thread_id
        && let Some(db_path) = state_db::find_rollout_path_by_id(
            Some(state_db_ctx),
            thread_id,
            archived_only,
            "find_path_query",
        )
        .await
    {
        if tokio::fs::try_exists(&db_path).await.unwrap_or(false) {
            return Ok(Some(db_path));
        }
        tracing::error!(
            "state db returned stale rollout path for thread {id_str}: {}",
            db_path.display()
        );
        state_db::record_discrepancy("find_thread_path_by_id_str_in_subdir", "stale_db_path");
    }

    let mut root = codex_home.to_path_buf();
    root.push(subdir);
    if !root.exists() {
        return Ok(None);
    }
    // This is safe because we know the values are valid.
    #[allow(clippy::unwrap_used)]
    let limit = NonZero::new(1).unwrap();
    let options = file_search::FileSearchOptions {
        limit,
        compute_indices: false,
        respect_gitignore: false,
        ..Default::default()
    };

    let results = file_search::run(id_str, vec![root], options, None)
        .map_err(|e| io::Error::other(format!("file search failed: {e}")))?;

    let found = results.matches.into_iter().next().map(|m| m.full_path());
    if let Some(found_path) = found.as_ref() {
        tracing::debug!("state db missing rollout path for thread {id_str}");
        state_db::record_discrepancy("find_thread_path_by_id_str_in_subdir", "falling_back");
        state_db::read_repair_rollout_path(
            state_db_ctx.as_deref(),
            thread_id,
            archived_only,
            found_path.as_path(),
        )
        .await;
    }

    Ok(found)
}

/// Locate a recorded thread rollout file by its UUID string using the existing
/// paginated listing implementation. Returns `Ok(Some(path))` if found, `Ok(None)` if not present
/// or the id is invalid.
pub async fn find_thread_path_by_id_str(
    codex_home: &Path,
    id_str: &str,
) -> io::Result<Option<PathBuf>> {
    find_thread_path_by_id_str_in_subdir(codex_home, SESSIONS_SUBDIR, id_str).await
}

/// Locate an archived thread rollout file by its UUID string.
pub async fn find_archived_thread_path_by_id_str(
    codex_home: &Path,
    id_str: &str,
) -> io::Result<Option<PathBuf>> {
    find_thread_path_by_id_str_in_subdir(codex_home, ARCHIVED_SESSIONS_SUBDIR, id_str).await
}

/// Extract the `YYYY/MM/DD` directory components from a rollout filename.
pub fn rollout_date_parts(file_name: &OsStr) -> Option<(String, String, String)> {
    let name = file_name.to_string_lossy();
    let date = name.strip_prefix("rollout-")?.get(..10)?;
    let year = date.get(..4)?.to_string();
    let month = date.get(5..7)?.to_string();
    let day = date.get(8..10)?.to_string();
    Some((year, month, day))
}

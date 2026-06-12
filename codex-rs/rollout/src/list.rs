#![allow(warnings, clippy::all)]

use codex_utils_path as path_utils;
use std::cmp::Reverse;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::io;
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
use super::ROTATED_ROLLOUT_SEGMENTS_SUBDIR;
use super::SESSIONS_SUBDIR;
use super::compression;
use crate::protocol::EventMsg;
use crate::state_db;
use codex_file_search as file_search;
use codex_protocol::SegmentId;
use codex_protocol::ThreadId;
use codex_protocol::models::ResponseItem;
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
    /// Best available user-facing preview for discovery and list display.
    pub preview: Option<String>,
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
    /// Immediate control/spawn parent thread id from session metadata.
    pub parent_thread_id: Option<ThreadId>,
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
    saw_compacted_user_message: bool,
    thread_id: Option<ThreadId>,
    first_user_message: Option<String>,
    preview: Option<String>,
    cwd: Option<PathBuf>,
    git_branch: Option<String>,
    git_sha: Option<String>,
    git_origin_url: Option<String>,
    source: Option<SessionSource>,
    parent_thread_id: Option<ThreadId>,
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
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadListLayout {
    NestedByDate,
    Flat,
}

pub struct ThreadListConfig<'a> {
    pub allowed_sources: &'a [SessionSource],
    pub model_providers: Option<&'a [String]>,
    pub cwd_filters: Option<&'a [PathBuf]>,
    pub default_provider: &'a str,
    pub layout: ThreadListLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchivedThreadRolloutDisposition {
    CanonicalArchivedThread,
    LegacyRotatedSegment { live_rollout_path: Option<PathBuf> },
}

/// Pagination cursor identifying the timestamp of the last item in a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    ts: OffsetDateTime,
}

impl Cursor {
    fn new(ts: OffsetDateTime) -> Self {
        Self { ts }
    }

    pub(crate) fn timestamp(&self) -> OffsetDateTime {
        self.ts
    }
}

/// Keeps track of where a paginated listing left off. As the file scan goes newest -> oldest,
/// it ignores everything until it passes the last seen timestamp from the previous page, then
/// starts returning results after that. This makes paging stable even if new files show up during
/// pagination.
struct AnchorState {
    ts: OffsetDateTime,
    passed: bool,
}

impl AnchorState {
    fn new(anchor: Option<Cursor>) -> Self {
        match anchor {
            Some(cursor) => Self {
                ts: cursor.ts,
                passed: false,
            },
            None => Self {
                ts: OffsetDateTime::UNIX_EPOCH,
                passed: true,
            },
        }
    }

    fn should_skip(&mut self, ts: OffsetDateTime, _id: Uuid) -> bool {
        if self.passed {
            return false;
        }
        if ts < self.ts {
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
trait RolloutFileVisitor {
    fn visit(
        &mut self,
        ts: OffsetDateTime,
        id: Uuid,
        path: PathBuf,
        scanned: usize,
    ) -> impl std::future::Future<Output = ControlFlow<()>> + Send;
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
    cwd_filters: Option<&'a [PathBuf]>,
}

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
            self.cwd_filters,
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
        serializer.serialize_str(&ts_str)
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
        let ts = anchor
            .ts
            .timestamp_nanos_opt()
            .and_then(|nanos| OffsetDateTime::from_unix_timestamp_nanos(nanos as i128).ok())
            .unwrap_or(OffsetDateTime::UNIX_EPOCH);
        Self::new(ts)
    }
}

/// Retrieve recorded thread file paths with token pagination. The returned `next_cursor`
/// can be supplied on the next call to resume after the last returned item, resilient to
/// concurrent new sessions being appended. Ordering is stable by the requested sort key
/// (timestamp desc).
pub async fn get_threads(
    codex_home: &Path,
    page_size: usize,
    cursor: Option<&Cursor>,
    sort_key: ThreadSortKey,
    allowed_sources: &[SessionSource],
    model_providers: Option<&[String]>,
    cwd_filters: Option<&[PathBuf]>,
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
            cwd_filters,
            default_provider,
            layout: ThreadListLayout::NestedByDate,
        },
    )
    .await
}

pub async fn get_threads_in_root(
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
                config.cwd_filters,
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
                config.cwd_filters,
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
    cwd_filters: Option<&[PathBuf]>,
) -> io::Result<ThreadsPage> {
    match sort_key {
        ThreadSortKey::CreatedAt => {
            traverse_directories_for_paths_created(
                root,
                page_size,
                anchor,
                allowed_sources,
                provider_matcher,
                cwd_filters,
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
                cwd_filters,
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
    cwd_filters: Option<&[PathBuf]>,
) -> io::Result<ThreadsPage> {
    match sort_key {
        ThreadSortKey::CreatedAt => {
            traverse_flat_paths_created(
                root,
                page_size,
                anchor,
                allowed_sources,
                provider_matcher,
                cwd_filters,
            )
            .await
        }
        ThreadSortKey::UpdatedAt => {
            traverse_flat_paths_updated(
                root,
                page_size,
                anchor,
                allowed_sources,
                provider_matcher,
                cwd_filters,
            )
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
    cwd_filters: Option<&[PathBuf]>,
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
        cwd_filters,
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
    cwd_filters: Option<&[PathBuf]>,
) -> io::Result<ThreadsPage> {
    let mut items: Vec<ThreadItem> = Vec::with_capacity(page_size);
    let mut scanned_files = 0usize;
    let mut anchor_state = AnchorState::new(anchor);
    let mut more_matches_available = false;

    let mut candidates = collect_files_by_updated_at(&root, &mut scanned_files).await?;
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
            cwd_filters,
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
    cwd_filters: Option<&[PathBuf]>,
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
        if let Some(item) = build_thread_item(
            path,
            allowed_sources,
            provider_matcher,
            cwd_filters,
            updated_at,
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
    cwd_filters: Option<&[PathBuf]>,
) -> io::Result<ThreadsPage> {
    let mut items: Vec<ThreadItem> = Vec::with_capacity(page_size);
    let mut scanned_files = 0usize;
    let mut anchor_state = AnchorState::new(anchor);
    let mut more_matches_available = false;

    let mut candidates = collect_flat_files_by_updated_at(&root, &mut scanned_files).await?;
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
            cwd_filters,
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

/// Pagination cursor token format: an RFC3339 timestamp.
pub fn parse_cursor(token: &str) -> Option<Cursor> {
    if token.contains('|') {
        return None;
    }

    let ts = OffsetDateTime::parse(token, &Rfc3339).ok().or_else(|| {
        let format: &[FormatItem] =
            format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
        PrimitiveDateTime::parse(token, format)
            .ok()
            .map(PrimitiveDateTime::assume_utc)
    })?;

    Some(Cursor::new(ts))
}

fn build_next_cursor(items: &[ThreadItem], sort_key: ThreadSortKey) -> Option<Cursor> {
    let last = items.last()?;
    let file_name = last.path.file_name()?.to_string_lossy();
    let (created_ts, _id) = parse_timestamp_uuid_from_filename(&file_name)?;
    let ts = match sort_key {
        ThreadSortKey::CreatedAt => created_ts,
        ThreadSortKey::UpdatedAt => {
            let updated_at = last.updated_at.as_deref()?;
            OffsetDateTime::parse(updated_at, &Rfc3339).ok()?
        }
    };
    Some(Cursor::new(ts))
}

async fn build_thread_item(
    path: PathBuf,
    allowed_sources: &[SessionSource],
    provider_matcher: Option<&ProviderMatcher<'_>>,
    cwd_filters: Option<&[PathBuf]>,
    updated_at: Option<String>,
) -> Option<ThreadItem> {
    // Read head and detect preview-bearing events; goal previews can appear before
    // the first normal user message.
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
    if let Some(cwd_filters) = cwd_filters
        && !summary.cwd.as_ref().is_some_and(|cwd| {
            cwd_filters
                .iter()
                .any(|filter| path_utils::paths_match_after_normalization(cwd, filter))
        })
    {
        return None;
    }
    // Apply filters: must have session meta and either a discoverable preview or compacted user
    // history.
    if summary.saw_session_meta && (summary.preview.is_some() || summary.saw_compacted_user_message)
    {
        let HeadTailSummary {
            thread_id,
            first_user_message,
            preview,
            cwd,
            git_branch,
            git_sha,
            git_origin_url,
            source,
            parent_thread_id,
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
            preview,
            cwd,
            git_branch,
            git_sha,
            git_origin_url,
            source,
            parent_thread_id,
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

/// Read a single rollout file into the same summary item shape used by thread listing.
///
/// This is for callers that already resolved a rollout path and need the same
/// metadata/preview extraction as list operations without scanning the whole
/// sessions tree.
pub async fn read_thread_item_from_rollout(path: PathBuf) -> Option<ThreadItem> {
    build_thread_item(
        path,
        &[],
        /*provider_matcher*/ None,
        /*cwd_filters*/ None,
        /*updated_at*/ None,
    )
    .await
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
        let Some(rollout_file) = compression::RolloutFile::from_path(entry.path()) else {
            continue;
        };
        let Some((ts, id)) = parse_timestamp_uuid_from_filename(rollout_file.plain_file_name())
        else {
            continue;
        };
        *scanned_files += 1;
        if *scanned_files > MAX_SCAN_FILES {
            break;
        }
        collected.push((ts, id, rollout_file.into_path()));
    }
    collected.sort_by_key(|(ts, sid, _path)| (Reverse(*ts), Reverse(*sid)));
    Ok(collected)
}

async fn collect_rollout_day_files(
    day_path: &Path,
) -> io::Result<Vec<(OffsetDateTime, Uuid, PathBuf)>> {
    let mut day_files = collect_files(day_path, |_name_str, path| {
        let rollout_file = compression::RolloutFile::from_path(path.to_path_buf())?;
        parse_timestamp_uuid_from_filename(rollout_file.plain_file_name())
            .map(|(ts, id)| (ts, id, rollout_file.into_path()))
    })
    .await?;
    // Stable ordering within the same second: (timestamp desc, uuid desc)
    day_files.sort_by_key(|(ts, sid, _path)| (Reverse(*ts), Reverse(*sid)));
    Ok(day_files)
}

pub(crate) fn parse_timestamp_uuid_from_filename(name: &str) -> Option<(OffsetDateTime, Uuid)> {
    // Expected: rollout-YYYY-MM-DDThh-mm-ss-<uuid>.jsonl[.zst]
    let name = compression::parse_rollout_file_name(name)?;
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

fn parse_timestamp_string_uuid_from_filename(name: &str) -> Option<(&str, Uuid)> {
    // Expected: rollout-YYYY-MM-DDThh-mm-ss-<uuid>.jsonl
    let core = name.strip_prefix("rollout-")?.strip_suffix(".jsonl")?;
    core.match_indices('-')
        .rev()
        .find_map(|(index, _)| {
            Uuid::parse_str(&core[index + 1..])
                .ok()
                .map(|uuid| (index, uuid))
        })
        .map(|(index, uuid)| (&core[..index], uuid))
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
        let Some(rollout_file) = compression::RolloutFile::from_path(entry.path()) else {
            continue;
        };
        let Some((_ts, id)) = parse_timestamp_uuid_from_filename(rollout_file.plain_file_name())
        else {
            continue;
        };
        *scanned_files += 1;
        if *scanned_files > MAX_SCAN_FILES {
            break;
        }
        let updated_at = file_modified_time(rollout_file.path())
            .await
            .unwrap_or(None);
        candidates.push(ThreadCandidate {
            path: rollout_file.into_path(),
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
    const MAX_SUMMARY_REFERENCE_FILES: usize = 128;

    let (summary, mut reference) = read_head_summary_file(path, head_limit).await?;
    let mut summaries = vec![summary];
    let mut segment_edges = Vec::new();
    let mut visited_references = HashSet::new();
    loop {
        let Some(next_reference) = reference.take() else {
            break;
        };
        if next_reference.nth_user_message == Some(0) {
            break;
        }
        if visited_references.len() >= MAX_SUMMARY_REFERENCE_FILES {
            tracing::warn!(
                "stopped reading rollout reference metadata after {MAX_SUMMARY_REFERENCE_FILES} files"
            );
            break;
        }
        let is_segment_edge = next_reference.nth_user_message.is_none();
        let reference_path = if let Some(codex_home) = codex_home_from_rollout_path(path) {
            match Box::pin(resolve_rollout_reference_rollout_path(
                codex_home,
                &next_reference,
            ))
            .await
            {
                Ok(reference_path) => reference_path,
                Err(_) => break,
            }
        } else if let Some(reference_path) =
            compression::existing_rollout_path(next_reference.rollout_path.as_path()).await
        {
            reference_path
        } else {
            break;
        };
        let reference_identity = next_reference
            .segment_id
            .map(|segment_id| format!("segment:{segment_id}"))
            .unwrap_or_else(|| format!("path:{}", reference_path.display()));
        if !visited_references.insert(reference_identity) {
            break;
        }
        let (referenced_summary, next_reference) =
            read_head_summary_file(reference_path.as_path(), head_limit).await?;
        segment_edges.push(is_segment_edge);
        summaries.push(referenced_summary);
        reference = next_reference;
    }

    let mut resolved = summaries.pop().unwrap_or_default();
    while let Some(mut source) = summaries.pop() {
        let is_segment_edge = segment_edges.pop().unwrap_or(false);
        source.saw_compacted_user_message |= resolved.saw_compacted_user_message;
        if is_segment_edge {
            source.preview = resolved.preview.or(source.preview);
            source.first_user_message = resolved.first_user_message.or(source.first_user_message);
        } else {
            source.preview = source.preview.or(resolved.preview);
            source.first_user_message = source.first_user_message.or(resolved.first_user_message);
        }
        resolved = source;
    }
    Ok(resolved)
}

pub(crate) async fn read_preview_metadata(
    path: &Path,
) -> io::Result<(Option<String>, Option<String>)> {
    let summary = read_head_summary(path, HEAD_RECORD_LIMIT).await?;
    Ok((summary.first_user_message, summary.preview))
}

fn codex_home_from_rollout_path(path: &Path) -> Option<&Path> {
    path.ancestors().find_map(|ancestor| {
        matches!(
            ancestor.file_name().and_then(|name| name.to_str()),
            Some(SESSIONS_SUBDIR | ARCHIVED_SESSIONS_SUBDIR)
        )
        .then(|| ancestor.parent())
        .flatten()
    })
}

async fn read_head_summary_file(
    path: &Path,
    head_limit: usize,
) -> io::Result<(
    HeadTailSummary,
    Option<codex_protocol::protocol::RolloutReferenceItem>,
)> {
    let mut lines = compression::open_rollout_line_reader(path).await?;
    let mut summary = HeadTailSummary::default();
    let mut reference = None;
    let mut lines_scanned = 0usize;

    while lines_scanned < head_limit
        || (summary.saw_session_meta
            && (summary.preview.is_none() || summary.first_user_message.is_none())
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
                    summary.parent_thread_id = session_meta_line.meta.parent_thread_id;
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
                        .and_then(|git| git.commit_hash.as_ref().map(|sha| sha.0.clone()));
                    summary.git_origin_url = session_meta_line
                        .git
                        .as_ref()
                        .and_then(|git| git.repository_url.clone());
                    summary.cli_version = Some(session_meta_line.meta.cli_version);
                    summary.created_at = Some(session_meta_line.meta.timestamp.clone());
                    summary.saw_session_meta = true;
                }
            }
            RolloutItem::RolloutReference(item) => {
                if reference.is_none() {
                    reference = Some(item);
                }
            }
            RolloutItem::ResponseItem(_) | RolloutItem::InterAgentCommunication(_) => {
                summary
                    .created_at
                    .get_or_insert_with(|| rollout_line.timestamp.clone());
            }
            RolloutItem::TurnContext(_) => {
                // Not included in `head`; skip.
            }
            RolloutItem::Compacted(compacted) => {
                if compacted
                    .replacement_history
                    .as_deref()
                    .is_some_and(|history| history.iter().any(ResponseItem::is_user_message))
                {
                    summary.saw_compacted_user_message = true;
                }
            }
            RolloutItem::EventMsg(ev) => {
                if let Some(preview) = event_msg_preview(&ev) {
                    if summary.preview.is_none() {
                        summary.preview = Some(preview.clone());
                    }
                    if let EventMsg::UserMessage(_) = ev
                        && summary.first_user_message.is_none()
                    {
                        summary.first_user_message = Some(preview);
                    }
                }
            }
        }

        if summary.saw_session_meta
            && summary.preview.is_some()
            && summary.first_user_message.is_some()
        {
            break;
        }
    }

    Ok((summary, reference))
}

/// Read up to `HEAD_RECORD_LIMIT` records from the start of the rollout file at `path`.
/// This should be enough to produce a summary including the session meta line.
pub async fn read_head_for_summary(path: &Path) -> io::Result<Vec<serde_json::Value>> {
    let mut lines = compression::open_rollout_line_reader(path).await?;
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
                RolloutItem::InterAgentCommunication(communication) => {
                    if let Ok(value) = serde_json::to_value(communication.to_model_input_item()) {
                        head.push(value);
                    }
                }
                RolloutItem::RolloutReference(_)
                | RolloutItem::Compacted(_)
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

fn event_msg_preview(event: &EventMsg) -> Option<String> {
    match event {
        EventMsg::UserMessage(user) => {
            let message = strip_user_message_prefix(user.message.as_str());
            if !message.is_empty() {
                return Some(message.to_string());
            }
            if user
                .images
                .as_ref()
                .is_some_and(|images| !images.is_empty())
                || !user.local_images.is_empty()
            {
                return Some("[Image]".to_string());
            }
            None
        }
        EventMsg::ThreadGoalUpdated(event) => {
            let objective = event.goal.objective.trim();
            (!objective.is_empty()).then(|| objective.to_string())
        }
        _ => None,
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
    Ok(compression::file_modified_time(path)
        .await?
        .and_then(truncate_to_millis))
}

fn format_rfc3339(dt: OffsetDateTime) -> Option<String> {
    dt.format(&Rfc3339).ok()
}

fn truncate_to_millis(dt: OffsetDateTime) -> Option<OffsetDateTime> {
    let millis_nanos = (dt.nanosecond() / 1_000_000) * 1_000_000;
    dt.replace_nanosecond(millis_nanos).ok()
}

async fn find_thread_path_by_id_str_in_subdir(
    codex_home: &Path,
    subdir: &str,
    id_str: &str,
    state_db_ctx: Option<&codex_state::StateRuntime>,
) -> io::Result<Option<PathBuf>> {
    let found =
        find_thread_path_by_id_str_in_subdir_unvalidated(codex_home, subdir, id_str, state_db_ctx)
            .await?;
    if subdir != ARCHIVED_SESSIONS_SUBDIR {
        return Ok(found);
    }
    let Some(found_path) = found else {
        return Ok(None);
    };
    match classify_archived_thread_rollout(codex_home, found_path.as_path(), state_db_ctx).await? {
        ArchivedThreadRolloutDisposition::CanonicalArchivedThread => Ok(Some(found_path)),
        ArchivedThreadRolloutDisposition::LegacyRotatedSegment { live_rollout_path } => {
            if let Some(live_rollout_path) = live_rollout_path {
                state_db::read_repair_rollout_path(
                    state_db_ctx,
                    ThreadId::from_string(id_str).ok(),
                    Some(false),
                    live_rollout_path.as_path(),
                )
                .await;
            }
            Ok(None)
        }
    }
}

async fn find_thread_path_by_id_str_in_subdir_unvalidated(
    codex_home: &Path,
    subdir: &str,
    id_str: &str,
    state_db_ctx: Option<&codex_state::StateRuntime>,
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
    let mut unverified_db_path = None;
    let mut fallback_reason = state_db_ctx.is_none().then_some("db_unavailable");
    if let Some(state_db_ctx) = state_db_ctx
        && let Some(thread_id) = thread_id
    {
        match state_db_ctx
            .find_rollout_path_by_id(thread_id, archived_only)
            .await
        {
            Ok(Some(db_path)) => {
                if let Some(existing_db_path) =
                    compression::existing_rollout_path(db_path.as_path()).await
                {
                    match read_session_meta_line(&existing_db_path).await {
                        Ok(meta_line) if meta_line.meta.id == thread_id => {
                            return Ok(Some(existing_db_path));
                        }
                        Ok(meta_line) => {
                            tracing::error!(
                                "state db returned rollout path for thread {id_str} but file belongs to thread {}: {}",
                                meta_line.meta.id,
                                existing_db_path.display()
                            );
                            tracing::warn!(
                                "state db discrepancy during find_thread_path_by_id_str_in_subdir: mismatched_db_path"
                            );
                            codex_state::record_fallback(
                                "find_thread_path",
                                "mismatch",
                                /*telemetry_override*/ None,
                            );
                        }
                        Err(err) => {
                            tracing::debug!(
                                "state db returned rollout path for thread {id_str} that could not be verified: {}: {err}",
                                existing_db_path.display()
                            );
                            unverified_db_path = Some(existing_db_path);
                        }
                    }
                } else {
                    tracing::error!(
                        "state db returned stale rollout path for thread {id_str}: {}",
                        db_path.display()
                    );
                    tracing::warn!(
                        "state db discrepancy during find_thread_path_by_id_str_in_subdir: stale_db_path"
                    );
                    codex_state::record_fallback(
                        "find_thread_path",
                        "stale_path",
                        /*telemetry_override*/ None,
                    );
                }
            }
            Ok(None) => fallback_reason = Some("missing_row"),
            Err(err) => {
                tracing::warn!(
                    "state db find_rollout_path_by_id failed during find_path_query: {err}"
                );
                fallback_reason = Some("db_error");
            }
        }
    }

    let mut root = codex_home.to_path_buf();
    root.push(subdir);
    if !root.exists() {
        return Ok(unverified_db_path);
    }
    let (filename_match, filename_scan_error) = match find_rollout_path_by_id_from_filenames(
        root.as_path(),
        id_str,
    )
    .await
    {
        Ok(path) => (path, None),
        Err(err) => {
            tracing::warn!(
                "rollout filename lookup failed during find_thread_path_by_id_str_in_subdir: {err}"
            );
            (None, Some(err))
        }
    };

    let found = match filename_match {
        Some(path) => Some(path),
        None => {
            // This is safe because we know the values are valid.
            #[allow(clippy::unwrap_used)]
            let limit = NonZero::new(1).unwrap();
            let options = file_search::FileSearchOptions {
                limit,
                compute_indices: false,
                respect_gitignore: false,
                ..Default::default()
            };

            let results = file_search::run(
                id_str,
                vec![root.clone()],
                options,
                /*cancel_flag*/ None,
            )
            .map_err(|e| io::Error::other(format!("file search failed: {e}")))?;

            let found = results
                .matches
                .into_iter()
                .map(|m| m.full_path())
                .find_map(compression::RolloutFile::from_path)
                .map(compression::RolloutFile::into_path);

            if found.is_none()
                && let Some(err) = filename_scan_error
            {
                return Err(err);
            }
            found
        }
    };
    if let Some(found_path) = found.as_ref() {
        tracing::debug!("state db missing rollout path for thread {id_str}");
        tracing::warn!(
            "state db discrepancy during find_thread_path_by_id_str_in_subdir: falling_back"
        );
        if let Some(reason) = fallback_reason {
            codex_state::record_fallback(
                "find_thread_path",
                reason,
                /*telemetry_override*/ None,
            );
        }
        state_db::read_repair_rollout_path(
            state_db_ctx,
            thread_id,
            archived_only,
            found_path.as_path(),
        )
        .await;
    }

    Ok(found.or(unverified_db_path))
}

async fn find_rollout_path_by_id_from_filenames(
    root: &Path,
    id_str: &str,
) -> io::Result<Option<PathBuf>> {
    let Ok(target) = Uuid::parse_str(id_str) else {
        return Ok(None);
    };
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut read_dir = match tokio::fs::read_dir(dir.as_path()).await {
            Ok(read_dir) => read_dir,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };
        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            let file_type = entry.file_type().await?;
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let Some(rollout_file) = compression::RolloutFile::from_path(path) else {
                continue;
            };
            let Some((_ts, id)) =
                parse_timestamp_uuid_from_filename(rollout_file.plain_file_name())
            else {
                continue;
            };
            if id == target {
                return Ok(Some(rollout_file.into_path()));
            }
        }
    }
    Ok(None)
}

/// Locate a recorded thread rollout file by its UUID string using the existing
/// paginated listing implementation. Returns `Ok(Some(path))` if found, `Ok(None)` if not present
/// or the id is invalid.
pub async fn find_thread_path_by_id_str(
    codex_home: &Path,
    id_str: &str,
    state_db_ctx: Option<&codex_state::StateRuntime>,
) -> io::Result<Option<PathBuf>> {
    find_thread_path_by_id_str_in_subdir(codex_home, SESSIONS_SUBDIR, id_str, state_db_ctx).await
}

/// Locate an archived thread rollout file by its UUID string.
pub async fn find_archived_thread_path_by_id_str(
    codex_home: &Path,
    id_str: &str,
    state_db_ctx: Option<&codex_state::StateRuntime>,
) -> io::Result<Option<PathBuf>> {
    find_thread_path_by_id_str_in_subdir(codex_home, ARCHIVED_SESSIONS_SUBDIR, id_str, state_db_ctx)
        .await
}

pub async fn find_rollout_path_by_segment_id(
    codex_home: &Path,
    thread_id: ThreadId,
    segment_id: SegmentId,
) -> io::Result<Option<PathBuf>> {
    if let Some(path) = find_rollout_path_by_segment_id_in_subdir(
        codex_home,
        SESSIONS_SUBDIR,
        thread_id,
        segment_id,
    )
    .await?
    {
        return Ok(Some(path));
    }
    if let Some(path) = find_rollout_path_by_segment_id_in_subdir(
        codex_home,
        ROTATED_ROLLOUT_SEGMENTS_SUBDIR,
        thread_id,
        segment_id,
    )
    .await?
    {
        return Ok(Some(path));
    }
    find_rollout_path_by_segment_id_in_subdir(
        codex_home,
        ARCHIVED_SESSIONS_SUBDIR,
        thread_id,
        segment_id,
    )
    .await
}

pub async fn classify_archived_thread_rollout(
    codex_home: &Path,
    rollout_path: &Path,
    state_db_ctx: Option<&codex_state::StateRuntime>,
) -> io::Result<ArchivedThreadRolloutDisposition> {
    let archived_root = codex_home.join(ARCHIVED_SESSIONS_SUBDIR);
    let Ok(relative_path) = rollout_path.strip_prefix(archived_root.as_path()) else {
        return Ok(ArchivedThreadRolloutDisposition::CanonicalArchivedThread);
    };
    if relative_path.components().count() != 1 {
        return Ok(ArchivedThreadRolloutDisposition::CanonicalArchivedThread);
    }

    let live_rollout_path =
        find_live_rollout_path_for_archived_candidate(codex_home, rollout_path, state_db_ctx)
            .await?;
    if live_rollout_path.is_some() {
        return Ok(ArchivedThreadRolloutDisposition::LegacyRotatedSegment { live_rollout_path });
    }
    Ok(ArchivedThreadRolloutDisposition::CanonicalArchivedThread)
}

async fn find_live_rollout_path_for_archived_candidate(
    codex_home: &Path,
    rollout_path: &Path,
    state_db_ctx: Option<&codex_state::StateRuntime>,
) -> io::Result<Option<PathBuf>> {
    let meta_line = match read_session_meta_line(rollout_path).await {
        Ok(meta_line) => meta_line,
        Err(_) => return Ok(None),
    };
    let thread_id = meta_line.meta.id.to_string();
    find_thread_path_by_id_str_in_subdir_unvalidated(
        codex_home,
        SESSIONS_SUBDIR,
        thread_id.as_str(),
        state_db_ctx,
    )
    .await
}

async fn find_rollout_path_by_segment_id_in_subdir(
    codex_home: &Path,
    subdir: &str,
    thread_id: ThreadId,
    segment_id: SegmentId,
) -> io::Result<Option<PathBuf>> {
    let root = codex_home.join(subdir);
    if !tokio::fs::try_exists(&root).await.unwrap_or(false) {
        return Ok(None);
    }

    let target_thread_id = thread_id.to_string();
    let mut stack = vec![root];
    let mut scanned_files = 0usize;
    while let Some(dir) = stack.pop() {
        let mut read_dir = match tokio::fs::read_dir(&dir).await {
            Ok(read_dir) => read_dir,
            Err(err) => {
                tracing::warn!("failed to read rollout directory {}: {err}", dir.display());
                continue;
            }
        };
        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            let file_type = entry.file_type().await?;
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let Some(rollout_file) = compression::RolloutFile::from_path(path) else {
                continue;
            };
            let Some((_, uuid)) =
                parse_timestamp_uuid_from_filename(rollout_file.plain_file_name())
            else {
                continue;
            };
            if uuid.to_string() != target_thread_id {
                continue;
            }
            scanned_files = scanned_files.saturating_add(1);
            if scanned_files > MAX_SCAN_FILES {
                return Ok(None);
            }
            let Ok(meta_line) = read_session_meta_line(rollout_file.path()).await else {
                continue;
            };
            if meta_line.meta.id == thread_id && meta_line.meta.segment_id == Some(segment_id) {
                return Ok(Some(rollout_file.into_path()));
            }
        }
    }

    Ok(None)
}

pub async fn resolve_rollout_reference_rollout_path(
    codex_home: &Path,
    reference: &codex_protocol::protocol::RolloutReferenceItem,
) -> io::Result<PathBuf> {
    // A compacted live thread can keep the same rollout path and timestamp while moving the
    // referenced predecessor into archive storage, so segment identity must win before fallback
    // lookup by mutable live-path metadata.
    if let Some(segment_id) = reference.segment_id {
        if let Some(thread_id) = reference.thread_id
            && let Some(path) =
                find_rollout_path_by_segment_id(codex_home, thread_id, segment_id).await?
        {
            return Ok(path);
        }
        if let Some(path) = compression::existing_rollout_path(&reference.rollout_path).await
            && let Ok(meta) = read_session_meta_line(&path).await
            && meta.meta.segment_id == Some(segment_id)
            && reference
                .thread_id
                .is_none_or(|thread_id| meta.meta.id == thread_id)
        {
            return Ok(path);
        }
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("rollout segment {segment_id} could not be resolved"),
        ));
    }

    let rollout_path = reference.rollout_path.as_path();
    let plain_rollout_path = compression::plain_rollout_path(rollout_path);
    let parsed_path_identity = plain_rollout_path
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .and_then(parse_timestamp_string_uuid_from_filename);
    let identity_thread_id = reference.thread_id.or_else(|| {
        parsed_path_identity.and_then(|(_, uuid)| ThreadId::from_string(&uuid.to_string()).ok())
    });
    let identity_timestamp = reference
        .rollout_timestamp
        .as_deref()
        .or_else(|| parsed_path_identity.map(|(timestamp, _)| timestamp));

    // Legacy references do not carry an immutable segment id. If the canonical live path has
    // since been replaced, the flat archived path is the only candidate that can still name the
    // referenced predecessor, so check it before the mutable live path.
    if let (Some(thread_id), Some(rollout_timestamp)) = (identity_thread_id, identity_timestamp) {
        let file_name = format!("rollout-{rollout_timestamp}-{thread_id}.jsonl");
        let archived_path = codex_home.join(ARCHIVED_SESSIONS_SUBDIR).join(&file_name);
        if let Some(path) =
            validated_legacy_rollout_reference_path(codex_home, archived_path.as_path(), reference)
                .await?
        {
            return Ok(path);
        }
    }

    if let Some(path) =
        validated_legacy_rollout_reference_path(codex_home, rollout_path, reference).await?
    {
        return Ok(path);
    }

    if let (Some(thread_id), Some(rollout_timestamp)) = (identity_thread_id, identity_timestamp) {
        let file_name = format!("rollout-{rollout_timestamp}-{thread_id}.jsonl");
        if let Some(active_path) =
            rollout_path_for_timestamp_file(codex_home, rollout_timestamp, &file_name)
            && let Some(path) = validated_legacy_rollout_reference_path(
                codex_home,
                active_path.as_path(),
                reference,
            )
            .await?
        {
            return Ok(path);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "legacy rollout reference {} could not be resolved without changing segment identity",
            rollout_path.display()
        ),
    ))
}

async fn validated_legacy_rollout_reference_path(
    codex_home: &Path,
    path: &Path,
    reference: &codex_protocol::protocol::RolloutReferenceItem,
) -> io::Result<Option<PathBuf>> {
    let Some(path) = compression::existing_rollout_path(path).await else {
        return Ok(None);
    };
    let Ok(meta) = read_session_meta_line(path.as_path()).await else {
        return Ok(None);
    };
    if (meta.meta.segment_id.is_some() && path.starts_with(codex_home.join(SESSIONS_SUBDIR)))
        || reference
            .thread_id
            .is_some_and(|thread_id| meta.meta.id != thread_id)
    {
        return Ok(None);
    }
    if let Some(expected_timestamp) = reference.rollout_timestamp.as_deref() {
        let plain_path = compression::plain_rollout_path(path.as_path());
        let Some((actual_timestamp, _)) = plain_path
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .and_then(parse_timestamp_string_uuid_from_filename)
        else {
            return Ok(None);
        };
        if actual_timestamp != expected_timestamp {
            return Ok(None);
        }
    }
    Ok(Some(path))
}

fn rollout_path_for_timestamp_file(
    codex_home: &Path,
    rollout_timestamp: &str,
    file_name: &str,
) -> Option<PathBuf> {
    let year = rollout_timestamp.get(0..4)?;
    let month = rollout_timestamp.get(5..7)?;
    let day = rollout_timestamp.get(8..10)?;
    Some(
        codex_home
            .join(SESSIONS_SUBDIR)
            .join(year)
            .join(month)
            .join(day)
            .join(file_name),
    )
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

#![allow(warnings, clippy::all)]

use codex_utils_path as path_utils;
use std::cmp::Reverse;
use std::ffi::OsStr;
use std::io;
use std::num::NonZero;
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
use super::compression;
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
    /// Total number of directory entries examined while scanning this request.
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
    /// RFC3339 timestamp string used for product recency ordering.
    pub recency_at: Option<String>,
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
const MAX_SCAN_ENTRIES: usize = 10000;
const HEAD_RECORD_LIMIT: usize = 10;
const USER_EVENT_SCAN_LIMIT: usize = 200;
// Candidate reads are independent, but result processing remains in candidate order.
const LIST_READ_BATCH_SIZE: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadSortKey {
    CreatedAt,
    UpdatedAt,
    RecencyAt,
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

/// Pagination cursor identifying the last item in a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    ts: OffsetDateTime,
    id: Option<ThreadId>,
}

impl Cursor {
    pub(crate) fn new(ts: OffsetDateTime) -> Self {
        Self { ts, id: None }
    }

    pub(crate) fn with_thread_id(ts: OffsetDateTime, id: ThreadId) -> Self {
        Self { ts, id: Some(id) }
    }

    pub(crate) fn timestamp(&self) -> OffsetDateTime {
        self.ts
    }

    pub(crate) fn thread_id(&self) -> Option<ThreadId> {
        self.id
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

impl serde::Serialize for Cursor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let ts_str = self
            .ts
            .format(&Rfc3339)
            .map_err(|e| serde::ser::Error::custom(format!("format error: {e}")))?;
        match self.id {
            Some(id) => serializer.serialize_str(&format!("{ts_str}|{id}")),
            None => serializer.serialize_str(&ts_str),
        }
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
        Self { ts, id: anchor.id }
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
        ThreadSortKey::UpdatedAt | ThreadSortKey::RecencyAt => {
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
        ThreadSortKey::UpdatedAt | ThreadSortKey::RecencyAt => {
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
    let mut entries_scanned = 0usize;
    let mut reached_scan_cap = false;
    let mut anchor_state = AnchorState::new(anchor);
    let mut more_matches_available = false;
    let mut pending = Vec::with_capacity(LIST_READ_BATCH_SIZE);

    let year_dirs = collect_dirs_desc(root.as_path(), |s| s.parse::<u16>().ok()).await?;
    'outer: for (_year, year_path) in year_dirs {
        let month_dirs = collect_dirs_desc(year_path.as_path(), |s| s.parse::<u8>().ok()).await?;
        for (_month, month_path) in month_dirs {
            let day_dirs =
                collect_dirs_desc(month_path.as_path(), |s| s.parse::<u8>().ok()).await?;
            for (_day, day_path) in day_dirs {
                if entries_scanned >= MAX_SCAN_ENTRIES {
                    break 'outer;
                }
                let day_collection = collect_created_candidates_in_dir(
                    day_path.as_path(),
                    MAX_SCAN_ENTRIES - entries_scanned,
                )
                .await?;
                entries_scanned += day_collection.entries_scanned;
                reached_scan_cap |= day_collection.reached_scan_cap;
                for candidate in day_collection.candidates {
                    if anchor_state.should_skip(candidate.created_at, candidate.id) {
                        continue;
                    }
                    pending.push(candidate);
                    if pending.len() == thread_list_batch_size(items.len(), page_size) {
                        more_matches_available = append_created_candidate_batch(
                            pending.as_mut_slice(),
                            &mut items,
                            page_size,
                            allowed_sources,
                            provider_matcher,
                            cwd_filters,
                        )
                        .await?;
                        pending.clear();
                        if more_matches_available {
                            break 'outer;
                        }
                    }
                }
                if reached_scan_cap {
                    break 'outer;
                }
            }
        }
    }
    if !more_matches_available && !pending.is_empty() {
        more_matches_available = append_created_candidate_batch(
            pending.as_mut_slice(),
            &mut items,
            page_size,
            allowed_sources,
            provider_matcher,
            cwd_filters,
        )
        .await?;
    }

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
        num_scanned_files: entries_scanned,
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
    let mut anchor_state = AnchorState::new(anchor);
    let mut more_matches_available = false;

    let collection =
        collect_thread_candidates(root.as_path(), ThreadListLayout::NestedByDate).await?;
    let entries_scanned = collection.entries_scanned;
    let reached_scan_cap = collection.reached_scan_cap;
    let mut candidates = collection.candidates;
    candidates.sort_by_key(|candidate| {
        let ts = candidate.updated_at.unwrap_or(OffsetDateTime::UNIX_EPOCH);
        (Reverse(ts), Reverse(candidate.id))
    });

    let candidates = candidates
        .into_iter()
        .filter(|candidate| {
            let ts = candidate.updated_at.unwrap_or(OffsetDateTime::UNIX_EPOCH);
            !anchor_state.should_skip(ts, candidate.id)
        })
        .collect::<Vec<_>>();

    let mut candidate_index = 0usize;
    'batches: while candidate_index < candidates.len() {
        let batch_end = candidate_index
            .saturating_add(thread_list_batch_size(items.len(), page_size))
            .min(candidates.len());
        let candidates = &candidates[candidate_index..batch_end];
        let thread_items =
            build_thread_item_batch(candidates, allowed_sources, provider_matcher, cwd_filters)
                .await?;
        for item in thread_items {
            if items.len() == page_size {
                more_matches_available = true;
                break 'batches;
            }
            if let Some(item) = item {
                items.push(item);
            }
        }
        candidate_index = batch_end;
    }

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
        num_scanned_files: entries_scanned,
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
    let mut anchor_state = AnchorState::new(anchor);
    let mut more_matches_available = false;

    let collection = collect_created_candidates_in_dir(root.as_path(), MAX_SCAN_ENTRIES).await?;
    let entries_scanned = collection.entries_scanned;
    let reached_scan_cap = collection.reached_scan_cap;
    let mut candidates = collection.candidates;
    candidates.sort_by_key(|candidate| (Reverse(candidate.created_at), Reverse(candidate.id)));
    let mut pending = Vec::with_capacity(LIST_READ_BATCH_SIZE);
    for candidate in candidates {
        if anchor_state.should_skip(candidate.created_at, candidate.id) {
            continue;
        }
        pending.push(candidate);
        if pending.len() == thread_list_batch_size(items.len(), page_size) {
            more_matches_available = append_created_candidate_batch(
                pending.as_mut_slice(),
                &mut items,
                page_size,
                allowed_sources,
                provider_matcher,
                cwd_filters,
            )
            .await?;
            pending.clear();
            if more_matches_available {
                break;
            }
        }
    }
    if !more_matches_available && !pending.is_empty() {
        more_matches_available = append_created_candidate_batch(
            pending.as_mut_slice(),
            &mut items,
            page_size,
            allowed_sources,
            provider_matcher,
            cwd_filters,
        )
        .await?;
    }

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
        num_scanned_files: entries_scanned,
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
    let mut anchor_state = AnchorState::new(anchor);
    let mut more_matches_available = false;

    let collection = collect_thread_candidates(root.as_path(), ThreadListLayout::Flat).await?;
    let entries_scanned = collection.entries_scanned;
    let reached_scan_cap = collection.reached_scan_cap;
    let mut candidates = collection.candidates;
    candidates.sort_by_key(|candidate| {
        let ts = candidate.updated_at.unwrap_or(OffsetDateTime::UNIX_EPOCH);
        (Reverse(ts), Reverse(candidate.id))
    });

    let candidates = candidates
        .into_iter()
        .filter(|candidate| {
            let ts = candidate.updated_at.unwrap_or(OffsetDateTime::UNIX_EPOCH);
            !anchor_state.should_skip(ts, candidate.id)
        })
        .collect::<Vec<_>>();

    let mut candidate_index = 0usize;
    'batches: while candidate_index < candidates.len() {
        let batch_end = candidate_index
            .saturating_add(thread_list_batch_size(items.len(), page_size))
            .min(candidates.len());
        let candidates = &candidates[candidate_index..batch_end];
        let thread_items =
            build_thread_item_batch(candidates, allowed_sources, provider_matcher, cwd_filters)
                .await?;
        for item in thread_items {
            if items.len() == page_size {
                more_matches_available = true;
                break 'batches;
            }
            if let Some(item) = item {
                items.push(item);
            }
        }
        candidate_index = batch_end;
    }

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
        num_scanned_files: entries_scanned,
        reached_scan_cap,
    })
}

/// Pagination cursor token format: an RFC3339 timestamp with an optional thread ID tie-breaker.
pub fn parse_cursor(token: &str) -> Option<Cursor> {
    let (timestamp, id) = match token.rsplit_once('|') {
        Some((timestamp, id)) => (timestamp, Some(ThreadId::from_string(id).ok()?)),
        None => (token, None),
    };

    let ts = OffsetDateTime::parse(timestamp, &Rfc3339)
        .ok()
        .or_else(|| {
            let format: &[FormatItem] =
                format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
            PrimitiveDateTime::parse(timestamp, format)
                .ok()
                .map(PrimitiveDateTime::assume_utc)
        })?;

    Some(Cursor { ts, id })
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
        ThreadSortKey::RecencyAt => {
            let recency_at = last.recency_at.as_deref().or(last.updated_at.as_deref())?;
            OffsetDateTime::parse(recency_at, &Rfc3339).ok()?
        }
    };
    match sort_key {
        ThreadSortKey::RecencyAt => Some(Cursor::with_thread_id(
            ts,
            ThreadId::from_string(&id.to_string()).ok()?,
        )),
        ThreadSortKey::CreatedAt | ThreadSortKey::UpdatedAt => Some(Cursor::new(ts)),
    }
}

async fn build_thread_item(
    path: PathBuf,
    allowed_sources: &[SessionSource],
    provider_matcher: Option<&ProviderMatcher<'_>>,
    cwd_filters: Option<&[PathBuf]>,
    updated_at: Option<String>,
) -> Option<ThreadItem> {
    if allowed_sources.is_empty() && provider_matcher.is_none() && cwd_filters.is_none() {
        return build_thread_item_from_summary(
            path,
            allowed_sources,
            provider_matcher,
            cwd_filters,
            updated_at,
        )
        .await;
    }
    let session_meta_read = match read_first_session_meta(&path).await {
        Ok(session_meta_read) => session_meta_read,
        Err(_) => {
            return build_thread_item_from_summary(
                path,
                allowed_sources,
                provider_matcher,
                cwd_filters,
                updated_at,
            )
            .await;
        }
    };
    if !session_meta_matches_filters(
        &session_meta_read.session_meta,
        allowed_sources,
        provider_matcher,
        cwd_filters,
    ) {
        return None;
    }

    #[cfg(test)]
    crate::list_test_support::record_full_head_summary();
    let summary = read_head_summary_from_reader(
        session_meta_read.remaining_lines,
        HEAD_RECORD_LIMIT,
        Some(session_meta_read.session_meta),
    )
    .await
    .unwrap_or_default();
    build_thread_item_from_head_summary(
        path,
        allowed_sources,
        provider_matcher,
        cwd_filters,
        updated_at,
        summary,
    )
}

async fn build_thread_item_from_summary(
    path: PathBuf,
    allowed_sources: &[SessionSource],
    provider_matcher: Option<&ProviderMatcher<'_>>,
    cwd_filters: Option<&[PathBuf]>,
    updated_at: Option<String>,
) -> Option<ThreadItem> {
    #[cfg(test)]
    crate::list_test_support::record_full_head_summary();
    // Read head and detect preview-bearing events; goal previews can appear before
    // the first normal user message.
    let summary = read_head_summary(&path, HEAD_RECORD_LIMIT)
        .await
        .unwrap_or_default();
    build_thread_item_from_head_summary(
        path,
        allowed_sources,
        provider_matcher,
        cwd_filters,
        updated_at,
        summary,
    )
}

fn build_thread_item_from_head_summary(
    path: PathBuf,
    allowed_sources: &[SessionSource],
    provider_matcher: Option<&ProviderMatcher<'_>>,
    cwd_filters: Option<&[PathBuf]>,
    updated_at: Option<String>,
    summary: HeadTailSummary,
) -> Option<ThreadItem> {
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
    // Apply filters: must have session meta and a discoverable preview.
    if summary.saw_session_meta && summary.preview.is_some() {
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
            recency_at: summary_updated_at.clone(),
            updated_at: summary_updated_at,
        });
    }
    None
}

fn session_meta_matches_filters(
    session_meta: &SessionMetaLine,
    allowed_sources: &[SessionSource],
    provider_matcher: Option<&ProviderMatcher<'_>>,
    cwd_filters: Option<&[PathBuf]>,
) -> bool {
    if !allowed_sources.is_empty() && !allowed_sources.contains(&session_meta.meta.source) {
        return false;
    }
    if let Some(matcher) = provider_matcher
        && !matcher.matches(session_meta.meta.model_provider.as_deref())
    {
        return false;
    }
    if let Some(cwd_filters) = cwd_filters
        && !cwd_filters.iter().any(|filter| {
            path_utils::paths_match_after_normalization(&session_meta.meta.cwd, filter)
        })
    {
        return false;
    }
    true
}

/// An open rollout positioned immediately after its first session metadata record.
struct SessionMetaRead {
    /// Session metadata used for source, provider, and working-directory filters.
    session_meta: SessionMetaLine,
    /// Reader positioned after `session_meta`, ready to continue summary extraction.
    remaining_lines: compression::RolloutLineReader,
}

async fn read_first_session_meta(path: &Path) -> io::Result<SessionMetaRead> {
    let mut lines = compression::open_rollout_line_reader(path).await?;
    #[cfg(test)]
    crate::list_test_support::record_rollout_open();
    while let Some(line) = lines.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let rollout_line = serde_json::from_str::<RolloutLine>(trimmed).map_err(|err| {
            io::Error::other(format!(
                "rollout at {} has invalid first record: {err}",
                path.display()
            ))
        })?;
        let RolloutItem::SessionMeta(session_meta) = rollout_line.item else {
            return Err(io::Error::other(format!(
                "rollout at {} does not start with session metadata",
                path.display()
            )));
        };
        #[cfg(test)]
        crate::list_test_support::record_session_meta();
        return Ok(SessionMetaRead {
            session_meta,
            remaining_lines: lines,
        });
    }
    Err(io::Error::other(format!(
        "rollout at {} is empty",
        path.display()
    )))
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

struct ThreadCandidate {
    path: PathBuf,
    id: Uuid,
    created_at: OffsetDateTime,
    updated_at: Option<OffsetDateTime>,
}

/// Candidates found while examining a bounded number of directory entries.
struct ThreadCandidateCollection {
    candidates: Vec<ThreadCandidate>,
    entries_scanned: usize,
    reached_scan_cap: bool,
}

/// Rollout files found while examining a bounded prefix of `read_dir` entries.
///
/// `read_dir` order is unspecified, so `reached_scan_cap` means files beyond this set may sort
/// before files in this set. Callers must report the cap instead of presenting the set as complete.
struct RolloutFileDiscovery {
    files: Vec<(OffsetDateTime, Uuid, PathBuf)>,
    entries_scanned: usize,
    reached_scan_cap: bool,
}

fn thread_list_batch_size(items_len: usize, page_size: usize) -> usize {
    page_size
        .saturating_sub(items_len)
        .saturating_add(1)
        .clamp(1, LIST_READ_BATCH_SIZE)
}

async fn append_created_candidate_batch(
    candidates: &mut [ThreadCandidate],
    items: &mut Vec<ThreadItem>,
    page_size: usize,
    allowed_sources: &[SessionSource],
    provider_matcher: Option<&ProviderMatcher<'_>>,
    cwd_filters: Option<&[PathBuf]>,
) -> io::Result<bool> {
    read_candidate_modified_times_batch(candidates).await?;
    let thread_items =
        build_thread_item_batch(candidates, allowed_sources, provider_matcher, cwd_filters).await?;
    for item in thread_items {
        if items.len() == page_size {
            return Ok(true);
        }
        if let Some(item) = item {
            items.push(item);
        }
    }
    Ok(false)
}

async fn read_candidate_modified_times_batch(candidates: &mut [ThreadCandidate]) -> io::Result<()> {
    let mut tasks = tokio::task::JoinSet::new();
    for (index, candidate) in candidates.iter().enumerate() {
        let path = candidate.path.clone();
        tasks.spawn(async move { (index, compression::file_modified_time(path.as_path()).await) });
    }

    let mut modified_times = Vec::with_capacity(candidates.len());
    while let Some(result) = tasks.join_next().await {
        modified_times.push(result.map_err(io::Error::other)?);
    }
    for (index, result) in modified_times {
        candidates[index].updated_at = result.unwrap_or(None).and_then(truncate_to_millis);
    }
    Ok(())
}

/// Builds candidate summaries concurrently and restores candidate order before returning.
async fn build_thread_item_batch(
    candidates: &[ThreadCandidate],
    allowed_sources: &[SessionSource],
    provider_matcher: Option<&ProviderMatcher<'_>>,
    cwd_filters: Option<&[PathBuf]>,
) -> io::Result<Vec<Option<ThreadItem>>> {
    let mut tasks = tokio::task::JoinSet::new();
    for (index, candidate) in candidates.iter().enumerate() {
        #[cfg(test)]
        let work_scope = crate::list_test_support::capture_thread_list_work();
        let path = candidate.path.clone();
        let updated_at = candidate.updated_at.and_then(format_rfc3339);
        let allowed_sources = allowed_sources.to_vec();
        let provider_filters = provider_matcher.map(|matcher| matcher.filters.to_vec());
        let matches_default_provider =
            provider_matcher.is_some_and(|matcher| matcher.matches_default_provider);
        let cwd_filters = cwd_filters.map(<[PathBuf]>::to_vec);
        let build = async move {
            let provider_matcher = provider_filters.as_deref().map(|filters| ProviderMatcher {
                filters,
                matches_default_provider,
            });
            let item = build_thread_item(
                path,
                allowed_sources.as_slice(),
                provider_matcher.as_ref(),
                cwd_filters.as_deref(),
                updated_at,
            )
            .await;
            (index, item)
        };
        #[cfg(test)]
        let build = work_scope.scope(build);
        tasks.spawn(build);
    }

    let mut items = Vec::with_capacity(candidates.len());
    while let Some(result) = tasks.join_next().await {
        items.push(result.map_err(io::Error::other)?);
    }
    items.sort_by_key(|(index, _item)| *index);
    Ok(items.into_iter().map(|(_index, item)| item).collect())
}

async fn collect_thread_candidates(
    root: &Path,
    layout: ThreadListLayout,
) -> io::Result<ThreadCandidateCollection> {
    let root = root.to_path_buf();
    tokio::task::spawn_blocking(move || collect_thread_candidates_blocking(root.as_path(), layout))
        .await
        .map_err(io::Error::other)?
}

async fn collect_dirs_desc<T, F>(parent: &Path, parse: F) -> io::Result<Vec<(T, PathBuf)>>
where
    T: Ord + Copy,
    F: Fn(&str) -> Option<T>,
{
    let mut dir = tokio::fs::read_dir(parent).await?;
    let mut dirs = Vec::new();
    while let Some(entry) = dir.next_entry().await? {
        if !entry
            .file_type()
            .await
            .is_ok_and(|file_type| file_type.is_dir())
        {
            continue;
        }
        let Some(value) = entry.file_name().to_str().and_then(&parse) else {
            continue;
        };
        dirs.push((value, entry.path()));
    }
    dirs.sort_by_key(|(value, _path)| Reverse(*value));
    Ok(dirs)
}

async fn collect_created_candidates_in_dir(
    dir: &Path,
    entry_limit: usize,
) -> io::Result<ThreadCandidateCollection> {
    let dir = dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let discovery = discover_rollout_files_in_dir(dir.as_path(), entry_limit)?;
        let mut candidates = discovery
            .files
            .into_iter()
            .map(|(created_at, id, path)| ThreadCandidate {
                path,
                id,
                created_at,
                updated_at: None,
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|candidate| (Reverse(candidate.created_at), Reverse(candidate.id)));
        Ok(ThreadCandidateCollection {
            candidates,
            entries_scanned: discovery.entries_scanned,
            reached_scan_cap: discovery.reached_scan_cap,
        })
    })
    .await
    .map_err(io::Error::other)?
}

/// Enumerates candidates and reads their mtimes in one blocking task.
///
/// `std::fs` keeps the directory walk and metadata syscalls on one blocking worker instead of
/// scheduling one Tokio blocking-pool job per file. Mtime lookup resolves the logical rollout path
/// so a concurrent plain-to-compressed transition does not make a session appear stale.
fn collect_thread_candidates_blocking(
    root: &Path,
    layout: ThreadListLayout,
) -> io::Result<ThreadCandidateCollection> {
    collect_thread_candidates_blocking_with_limit(root, layout, MAX_SCAN_ENTRIES)
}

fn collect_thread_candidates_blocking_with_limit(
    root: &Path,
    layout: ThreadListLayout,
    entry_limit: usize,
) -> io::Result<ThreadCandidateCollection> {
    let mut candidates = Vec::new();
    let mut entries_scanned = 0usize;
    let mut reached_scan_cap = false;
    match layout {
        ThreadListLayout::NestedByDate => {
            let year_dirs = collect_dirs_desc_blocking(root, |s| s.parse::<u16>().ok())?;
            'outer: for (_year, year_path) in year_dirs {
                let month_dirs =
                    collect_dirs_desc_blocking(year_path.as_path(), |s| s.parse::<u8>().ok())?;
                for (_month, month_path) in month_dirs {
                    let day_dirs =
                        collect_dirs_desc_blocking(month_path.as_path(), |s| s.parse::<u8>().ok())?;
                    for (_day, day_path) in day_dirs {
                        if entries_scanned >= entry_limit {
                            break 'outer;
                        }
                        let mut day_collection = collect_updated_candidates_in_dir(
                            day_path.as_path(),
                            entry_limit - entries_scanned,
                        )?;
                        entries_scanned += day_collection.entries_scanned;
                        reached_scan_cap |= day_collection.reached_scan_cap;
                        day_collection.candidates.sort_by_key(|candidate| {
                            let created_at = candidate.created_at;
                            (Reverse(created_at), Reverse(candidate.id))
                        });
                        for candidate in day_collection.candidates {
                            candidates.push(candidate);
                        }
                        if reached_scan_cap {
                            break 'outer;
                        }
                    }
                }
            }
        }
        ThreadListLayout::Flat => {
            let collection = collect_updated_candidates_in_dir(root, entry_limit)?;
            candidates = collection.candidates;
            entries_scanned = collection.entries_scanned;
            reached_scan_cap = collection.reached_scan_cap;
        }
    }
    Ok(ThreadCandidateCollection {
        candidates,
        entries_scanned,
        reached_scan_cap,
    })
}

#[cfg(test)]
pub(crate) fn collect_thread_candidate_stats_for_test(
    root: &Path,
    layout: ThreadListLayout,
    entry_limit: usize,
) -> io::Result<(usize, usize, bool)> {
    let collection = collect_thread_candidates_blocking_with_limit(root, layout, entry_limit)?;
    Ok((
        collection.candidates.len(),
        collection.entries_scanned,
        collection.reached_scan_cap,
    ))
}

fn collect_dirs_desc_blocking<T, F>(parent: &Path, parse: F) -> io::Result<Vec<(T, PathBuf)>>
where
    T: Ord + Copy,
    F: Fn(&str) -> Option<T>,
{
    let mut dirs = Vec::new();
    for entry in std::fs::read_dir(parent)? {
        let entry = entry?;
        if !entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
            continue;
        }
        let Some(value) = entry.file_name().to_str().and_then(&parse) else {
            continue;
        };
        dirs.push((value, entry.path()));
    }
    dirs.sort_by_key(|(value, _path)| Reverse(*value));
    Ok(dirs)
}

fn collect_updated_candidates_in_dir(
    dir: &Path,
    entry_limit: usize,
) -> io::Result<ThreadCandidateCollection> {
    let discovery = discover_rollout_files_in_dir(dir, entry_limit)?;
    let mut candidates = Vec::new();
    for (created_at, id, path) in discovery.files {
        let updated_at = compression::file_modified_time_blocking(path.as_path())
            .unwrap_or(None)
            .and_then(truncate_to_millis);
        candidates.push(ThreadCandidate {
            path,
            id,
            created_at,
            updated_at,
        });
    }
    Ok(ThreadCandidateCollection {
        candidates,
        entries_scanned: discovery.entries_scanned,
        reached_scan_cap: discovery.reached_scan_cap,
    })
}

fn discover_rollout_files_in_dir(
    dir: &Path,
    entry_limit: usize,
) -> io::Result<RolloutFileDiscovery> {
    let mut files = Vec::new();
    let mut entries_scanned = 0usize;
    for entry in std::fs::read_dir(dir)? {
        if entries_scanned >= entry_limit {
            break;
        }
        entries_scanned += 1;
        let entry = entry?;
        if !entry.file_type().is_ok_and(|file_type| file_type.is_file()) {
            continue;
        }
        let Some(rollout_file) = compression::RolloutFile::from_path(entry.path()) else {
            continue;
        };
        let Some((created_at, id)) =
            parse_timestamp_uuid_from_filename(rollout_file.plain_file_name())
        else {
            continue;
        };
        files.push((created_at, id, rollout_file.into_path()));
    }
    Ok(RolloutFileDiscovery {
        files,
        entries_scanned,
        reached_scan_cap: entries_scanned == entry_limit,
    })
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
    let lines = compression::open_rollout_line_reader(path).await?;
    #[cfg(test)]
    crate::list_test_support::record_rollout_open();
    read_head_summary_from_reader(lines, head_limit, /*initial_session_meta*/ None).await
}

async fn read_head_summary_from_reader(
    mut lines: compression::RolloutLineReader,
    head_limit: usize,
    initial_session_meta: Option<SessionMetaLine>,
) -> io::Result<HeadTailSummary> {
    let mut summary = HeadTailSummary::default();
    let mut lines_scanned = 0usize;
    if let Some(session_meta) = initial_session_meta {
        record_session_meta_in_summary(&mut summary, &session_meta);
        lines_scanned = 1;
    }

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
                record_session_meta_in_summary(&mut summary, &session_meta_line);
            }
            RolloutItem::ResponseItem(_) | RolloutItem::InterAgentCommunication(_) => {
                summary
                    .created_at
                    .get_or_insert_with(|| rollout_line.timestamp.clone());
            }
            RolloutItem::InterAgentCommunicationMetadata { .. } => {}
            RolloutItem::TurnContext(_) => {
                // Not included in `head`; skip.
            }
            RolloutItem::Compacted(_) => {
                // Not included in `head`; skip.
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

    Ok(summary)
}

fn record_session_meta_in_summary(summary: &mut HeadTailSummary, session_meta: &SessionMetaLine) {
    if summary.saw_session_meta {
        return;
    }
    summary.source = Some(session_meta.meta.source.clone());
    summary.parent_thread_id = session_meta.meta.parent_thread_id;
    summary.agent_nickname = session_meta.meta.agent_nickname.clone();
    summary.agent_role = session_meta.meta.agent_role.clone();
    summary.model_provider = session_meta.meta.model_provider.clone();
    summary.thread_id = Some(session_meta.meta.id);
    summary.cwd = Some(session_meta.meta.cwd.clone());
    summary.git_branch = session_meta.git.as_ref().and_then(|git| git.branch.clone());
    summary.git_sha = session_meta
        .git
        .as_ref()
        .and_then(|git| git.commit_hash.as_ref().map(|sha| sha.0.clone()));
    summary.git_origin_url = session_meta
        .git
        .as_ref()
        .and_then(|git| git.repository_url.clone());
    summary.cli_version = Some(session_meta.meta.cli_version.clone());
    summary.created_at = Some(session_meta.meta.timestamp.clone());
    summary.saw_session_meta = true;
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
                RolloutItem::InterAgentCommunicationMetadata { .. }
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

/// Extract the `YYYY/MM/DD` directory components from a rollout filename.
pub fn rollout_date_parts(file_name: &OsStr) -> Option<(String, String, String)> {
    let name = file_name.to_string_lossy();
    let date = name.strip_prefix("rollout-")?.get(..10)?;
    let year = date.get(..4)?.to_string();
    let month = date.get(5..7)?.to_string();
    let day = date.get(8..10)?.to_string();
    Some((year, month, day))
}

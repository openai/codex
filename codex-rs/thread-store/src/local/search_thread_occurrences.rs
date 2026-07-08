use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

use chrono::DateTime;
use codex_app_server_protocol::ThreadHistoryBuilder;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::UserInput;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::USER_MESSAGE_BEGIN;
use pulldown_cmark::Event;
use pulldown_cmark::Parser;
use pulldown_cmark::TagEnd;
use serde::Deserialize;
use serde::Serialize;

use super::LocalThreadStore;
use crate::SearchTextRange;
use crate::SearchThreadOccurrencesParams;
use crate::StoredThreadOccurrence;
use crate::ThreadOccurrenceSearchPage;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

const SNAPSHOT_TTL: Duration = Duration::from_secs(10 * 60);
const COMPLETED_SNAPSHOT_TTL: Duration = Duration::from_secs(30);
const MAX_ACTIVE_SNAPSHOTS: usize = 256;
const MAX_CACHED_PROJECTIONS: usize = 8;
const SNIPPET_CONTEXT_BEFORE_CHARS: usize = 48;
const SNIPPET_CONTEXT_AFTER_CHARS: usize = 96;

#[cfg(test)]
#[path = "search_thread_occurrences_tests.rs"]
mod tests;

#[derive(Default)]
pub(super) struct ThreadOccurrenceSearchCache {
    next_snapshot_id: u64,
    snapshots: HashMap<u64, ThreadOccurrenceSearchSnapshot>,
    projections: HashMap<codex_protocol::ThreadId, CachedSearchProjection>,
}

#[derive(Clone)]
struct ThreadOccurrenceSearchSnapshot {
    thread_id: codex_protocol::ThreadId,
    search_term: String,
    items: Vec<StoredThreadOccurrence>,
    is_capped: bool,
    expires_at: Instant,
    completed: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct RolloutFingerprint {
    path: PathBuf,
    len: u64,
    modified_at: Option<SystemTime>,
}

#[derive(Clone)]
struct SearchProjection {
    fingerprint: RolloutFingerprint,
    items: Vec<SearchableThreadItem>,
}

struct CachedSearchProjection {
    projection: Arc<SearchProjection>,
    expires_at: Instant,
}

#[derive(Clone)]
struct SearchableThreadItem {
    turn_id: String,
    item_id: String,
    text: String,
    turn_started_at: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadOccurrenceSearchCursor {
    snapshot_id: u64,
    offset: usize,
}

pub(super) async fn search_thread_occurrences(
    store: &LocalThreadStore,
    params: SearchThreadOccurrencesParams,
) -> ThreadStoreResult<ThreadOccurrenceSearchPage> {
    if params.search_term.is_empty() {
        return Err(ThreadStoreError::InvalidRequest {
            message: "search_thread_occurrences requires search_term".to_string(),
        });
    }
    if params.max_results == 0 {
        return Err(ThreadStoreError::InvalidRequest {
            message: "search_thread_occurrences requires max_results greater than zero".to_string(),
        });
    }
    let page_size = params.page_size.clamp(1, params.max_results);

    if let Some(cursor) = params.cursor.as_deref() {
        return page_from_cached_snapshot(store, &params, cursor, page_size).await;
    }

    let snapshot = build_snapshot(store, &params).await?;
    let is_capped = snapshot.is_capped;
    let page_end = page_size.min(snapshot.items.len());
    let items = snapshot.items[..page_end].to_vec();
    let next_cursor = if page_end < snapshot.items.len() {
        let snapshot_id = store
            .occurrence_search_cache
            .lock()
            .await
            .insert_snapshot(snapshot)?;
        Some(serialize_cursor(ThreadOccurrenceSearchCursor {
            snapshot_id,
            offset: page_end,
        })?)
    } else {
        None
    };

    Ok(ThreadOccurrenceSearchPage {
        items,
        next_cursor,
        is_capped,
    })
}

async fn page_from_cached_snapshot(
    store: &LocalThreadStore,
    params: &SearchThreadOccurrencesParams,
    cursor: &str,
    page_size: usize,
) -> ThreadStoreResult<ThreadOccurrenceSearchPage> {
    let cursor = parse_cursor(cursor)?;
    let mut cache = store.occurrence_search_cache.lock().await;
    cache.remove_expired();
    let snapshot = cache.snapshots.get(&cursor.snapshot_id).ok_or_else(|| {
        ThreadStoreError::InvalidRequest {
            message: "invalid cursor: search snapshot is no longer available".to_string(),
        }
    })?;
    if snapshot.thread_id != params.thread_id || snapshot.search_term != params.search_term {
        return Err(ThreadStoreError::InvalidRequest {
            message: "invalid cursor: thread or search term does not match snapshot".to_string(),
        });
    }
    if cursor.offset >= snapshot.items.len() {
        return Err(ThreadStoreError::InvalidRequest {
            message: "invalid cursor: search offset is out of range".to_string(),
        });
    }

    let page_end = cursor
        .offset
        .saturating_add(page_size)
        .min(snapshot.items.len());
    let items = snapshot.items[cursor.offset..page_end].to_vec();
    let has_next_page = page_end < snapshot.items.len();
    let next_cursor = has_next_page
        .then(|| {
            serialize_cursor(ThreadOccurrenceSearchCursor {
                snapshot_id: cursor.snapshot_id,
                offset: page_end,
            })
        })
        .transpose()?;

    let page = ThreadOccurrenceSearchPage {
        items,
        next_cursor,
        is_capped: snapshot.is_capped,
    };
    if !has_next_page && let Some(snapshot) = cache.snapshots.get_mut(&cursor.snapshot_id) {
        snapshot.completed = true;
        snapshot.expires_at = Instant::now() + COMPLETED_SNAPSHOT_TTL;
    }
    Ok(page)
}

async fn build_snapshot(
    store: &LocalThreadStore,
    params: &SearchThreadOccurrencesParams,
) -> ThreadStoreResult<ThreadOccurrenceSearchSnapshot> {
    let projection = load_search_projection(store, params.thread_id).await?;
    let matcher = LiteralMatcher::new(params.search_term.as_str());
    let mut items = Vec::with_capacity(params.max_results.saturating_add(1));
    for item in &projection.items {
        let remaining = params
            .max_results
            .saturating_add(1)
            .saturating_sub(items.len());
        items.extend(occurrences_in_item(
            params.thread_id,
            item.turn_id.clone(),
            item.item_id.clone(),
            item.text.as_str(),
            item.turn_started_at,
            &matcher,
            remaining,
        ));
        if items.len() > params.max_results {
            break;
        }
    }
    let is_capped = items.len() > params.max_results;
    items.truncate(params.max_results);

    Ok(ThreadOccurrenceSearchSnapshot {
        thread_id: params.thread_id,
        search_term: params.search_term.clone(),
        items,
        is_capped,
        expires_at: Instant::now() + SNAPSHOT_TTL,
        completed: false,
    })
}

async fn load_search_projection(
    store: &LocalThreadStore,
    thread_id: codex_protocol::ThreadId,
) -> ThreadStoreResult<Arc<SearchProjection>> {
    let path =
        super::read_thread::resolve_rollout_path(store, thread_id, /*include_archived*/ true)
            .await?
            .ok_or(ThreadStoreError::ThreadNotFound { thread_id })?;
    let mut lines = match codex_rollout::open_rollout_line_reader(path.as_path()).await {
        Ok(lines) => lines,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(ThreadStoreError::ThreadNotFound { thread_id });
        }
        Err(err) => {
            return Err(ThreadStoreError::Internal {
                message: format!("failed to open rollout for search: {err}"),
            });
        }
    };
    let physical_path = lines.source_path().to_path_buf();
    let fingerprint = RolloutFingerprint {
        path: physical_path.clone(),
        len: lines.source_len(),
        modified_at: lines.source_modified_at(),
    };
    {
        let mut cache = store.occurrence_search_cache.lock().await;
        cache.remove_expired();
        if let Some(cached) = cache.projections.get_mut(&thread_id)
            && cached.projection.fingerprint == fingerprint
        {
            cached.expires_at = Instant::now() + SNAPSHOT_TTL;
            return Ok(Arc::clone(&cached.projection));
        }
    }

    let plain_path = codex_rollout::plain_rollout_path(physical_path.as_path());
    let snapshot_len = if physical_path == plain_path {
        Some(fingerprint.len)
    } else {
        // Compressed rollouts are immutable, so the decompressed byte count does not need a bound.
        None
    };
    let mut builder = ThreadHistoryBuilder::new();
    let mut turn_timestamps = HashMap::<String, i64>::new();
    let mut bytes_read = 0_u64;
    let mut parse_errors = 0_usize;

    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to read rollout during search: {err}"),
        })?
    {
        if snapshot_len.is_some_and(|limit| bytes_read.saturating_add(line.len() as u64) > limit) {
            break;
        }
        bytes_read = bytes_read.saturating_add(line.len() as u64 + 1);
        let rollout_line = match serde_json::from_str::<RolloutLine>(line.trim()) {
            Ok(rollout_line) => rollout_line,
            Err(_) => {
                parse_errors = parse_errors.saturating_add(1);
                continue;
            }
        };
        let changes = builder.handle_rollout_item_with_changes(&rollout_line.item);
        let line_timestamp = if changes
            .changed_turns
            .iter()
            .any(|turn| turn.started_at.is_none())
        {
            DateTime::parse_from_rfc3339(&rollout_line.timestamp)
                .map(|timestamp| timestamp.timestamp())
                .unwrap_or_default()
        } else {
            0
        };

        for removed_turn_id in changes.removed_turn_ids {
            turn_timestamps.remove(&removed_turn_id);
        }
        for turn in changes.changed_turns {
            if let Some(started_at) = turn.started_at {
                turn_timestamps.insert(turn.turn_id, started_at);
            } else {
                turn_timestamps
                    .entry(turn.turn_id)
                    .or_insert(line_timestamp);
            }
        }
    }
    if parse_errors > 0 {
        tracing::warn!(%thread_id, parse_errors, "skipped malformed rollout records during thread search");
    }

    let mut items = Vec::new();
    for turn in builder.finish() {
        let turn_started_at = turn
            .started_at
            .or_else(|| turn_timestamps.get(&turn.id).copied())
            .unwrap_or_default();
        let last_assistant_id = turn.items.iter().rev().find_map(|item| match item {
            ThreadItem::AgentMessage { id, .. } => Some(id.clone()),
            _ => None,
        });
        for item in turn.items {
            if matches!(&item, ThreadItem::AgentMessage { id, .. } if Some(id) != last_assistant_id.as_ref())
            {
                continue;
            }
            let Some(text) = searchable_text(&item) else {
                continue;
            };
            items.push(SearchableThreadItem {
                turn_id: turn.id.clone(),
                item_id: item.id().to_string(),
                text: text.into_owned(),
                turn_started_at,
            });
        }
    }
    let projection = Arc::new(SearchProjection { fingerprint, items });
    store
        .occurrence_search_cache
        .lock()
        .await
        .insert_projection(
            thread_id,
            CachedSearchProjection {
                projection: Arc::clone(&projection),
                expires_at: Instant::now() + SNAPSHOT_TTL,
            },
        );
    Ok(projection)
}

fn searchable_text(item: &ThreadItem) -> Option<Cow<'_, str>> {
    match item {
        ThreadItem::UserMessage { content, .. } => {
            let mut text_parts = content
                .iter()
                .filter_map(|input| match input {
                    UserInput::Text { text, .. } => Some(strip_user_message_prefix(text)),
                    UserInput::Image { .. }
                    | UserInput::LocalImage { .. }
                    | UserInput::Skill { .. }
                    | UserInput::Mention { .. } => None,
                })
                .filter(|text| !text.is_empty())
                .peekable();
            let first = text_parts.next()?;
            match text_parts.next() {
                None => Some(Cow::Borrowed(first)),
                Some(second) => {
                    let mut parts = vec![first, second];
                    parts.extend(text_parts);
                    Some(Cow::Owned(parts.join("\n")))
                }
            }
        }
        ThreadItem::AgentMessage { text, .. } => {
            let text = markdown_to_search_text(text);
            (!text.is_empty()).then_some(Cow::Owned(text))
        }
        _ => None,
    }
}

fn strip_user_message_prefix(text: &str) -> &str {
    match text.find(USER_MESSAGE_BEGIN) {
        Some(index) => text[index + USER_MESSAGE_BEGIN.len()..].trim(),
        None => text.trim(),
    }
}

fn markdown_to_search_text(markdown: &str) -> String {
    let mut text = String::new();
    for event in Parser::new(markdown.trim()) {
        match event {
            Event::Text(value)
            | Event::Code(value)
            | Event::Html(value)
            | Event::InlineHtml(value) => text.push_str(&value),
            Event::SoftBreak | Event::Rule => text.push(' '),
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::BlockQuote
                | TagEnd::CodeBlock
                | TagEnd::List(_)
                | TagEnd::Item
                | TagEnd::Table
                | TagEnd::TableHead
                | TagEnd::TableRow
                | TagEnd::TableCell,
            ) => text.push(' '),
            Event::Start(_)
            | Event::End(
                TagEnd::Emphasis
                | TagEnd::Strong
                | TagEnd::Strikethrough
                | TagEnd::Link
                | TagEnd::HtmlBlock
                | TagEnd::FootnoteDefinition
                | TagEnd::Image
                | TagEnd::MetadataBlock(_),
            )
            | Event::HardBreak
            | Event::FootnoteReference(_)
            | Event::TaskListMarker(_) => {}
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

struct LiteralMatcher {
    lowercase_needle: String,
}

impl LiteralMatcher {
    fn new(needle: &str) -> Self {
        Self {
            lowercase_needle: needle.to_lowercase(),
        }
    }

    fn find_ranges(&self, text: &str, limit: usize) -> Vec<std::ops::Range<usize>> {
        let lowercase_text = text.to_lowercase();
        let mut spans = Vec::with_capacity(text.chars().count());
        let mut lowercase_start = 0;
        for (original_start, character) in text.char_indices() {
            let lowercase_end =
                lowercase_start + character.to_lowercase().map(char::len_utf8).sum::<usize>();
            spans.push((
                lowercase_start..lowercase_end,
                original_start..original_start + character.len_utf8(),
            ));
            lowercase_start = lowercase_end;
        }

        lowercase_text
            .match_indices(self.lowercase_needle.as_str())
            .take(limit)
            .filter_map(|(start, matched)| {
                let end = start.saturating_add(matched.len());
                let original_start = spans
                    .iter()
                    .find(|(lowercase, _)| lowercase.contains(&start))?
                    .1
                    .start;
                let original_end = spans
                    .iter()
                    .find(|(lowercase, _)| lowercase.contains(&end.saturating_sub(1)))?
                    .1
                    .end;
                Some(original_start..original_end)
            })
            .collect()
    }
}

fn occurrences_in_item(
    thread_id: codex_protocol::ThreadId,
    turn_id: String,
    item_id: String,
    text: &str,
    turn_started_at: i64,
    matcher: &LiteralMatcher,
    limit: usize,
) -> Vec<StoredThreadOccurrence> {
    let mut previous_match_end = 0;
    let mut utf16_offset = 0_u32;
    matcher
        .find_ranges(text, limit)
        .into_iter()
        .enumerate()
        .map(|(occurrence_index, matched)| {
            utf16_offset =
                utf16_offset.saturating_add(utf16_len(&text[previous_match_end..matched.start]));
            let match_start = utf16_offset;
            let match_end = match_start.saturating_add(utf16_len(&text[matched.clone()]));
            previous_match_end = matched.end;
            utf16_offset = match_end;
            let snippet_start =
                char_start_before(text, matched.start, SNIPPET_CONTEXT_BEFORE_CHARS);
            let snippet_end = char_end_after(text, matched.end, SNIPPET_CONTEXT_AFTER_CHARS);
            let leading_ellipsis = snippet_start > 0;
            let trailing_ellipsis = snippet_end < text.len();
            let mut snippet = String::new();
            if leading_ellipsis {
                snippet.push_str("... ");
            }
            snippet.push_str(&text[snippet_start..snippet_end]);
            if trailing_ellipsis {
                snippet.push_str(" ...");
            }
            let snippet_match_start = if leading_ellipsis { 4 } else { 0 }
                + utf16_len(&text[snippet_start..matched.start]);
            let match_len = utf16_len(&text[matched]);
            let occurrence_index = u32::try_from(occurrence_index).unwrap_or(u32::MAX);

            StoredThreadOccurrence {
                thread_id,
                occurrence_id: format!("{turn_id}:{item_id}:{match_start}"),
                turn_id: turn_id.clone(),
                item_id: item_id.clone(),
                occurrence_index,
                snippet,
                match_range: SearchTextRange {
                    start: match_start,
                    end: match_end,
                },
                snippet_match_range: SearchTextRange {
                    start: snippet_match_start,
                    end: snippet_match_start.saturating_add(match_len),
                },
                turn_started_at,
            }
        })
        .collect()
}

fn utf16_len(text: &str) -> u32 {
    u32::try_from(text.encode_utf16().count()).unwrap_or(u32::MAX)
}

fn char_start_before(text: &str, byte_index: usize, chars_before: usize) -> usize {
    text[..byte_index]
        .char_indices()
        .rev()
        .nth(chars_before)
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn char_end_after(text: &str, byte_index: usize, chars_after: usize) -> usize {
    text[byte_index..]
        .char_indices()
        .nth(chars_after)
        .map(|(offset, _)| byte_index.saturating_add(offset))
        .unwrap_or(text.len())
}

fn serialize_cursor(cursor: ThreadOccurrenceSearchCursor) -> ThreadStoreResult<String> {
    serde_json::to_string(&cursor).map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to serialize search cursor: {err}"),
    })
}

fn parse_cursor(cursor: &str) -> ThreadStoreResult<ThreadOccurrenceSearchCursor> {
    serde_json::from_str(cursor).map_err(|_| ThreadStoreError::InvalidRequest {
        message: format!("invalid cursor: {cursor}"),
    })
}

impl ThreadOccurrenceSearchCache {
    fn insert_snapshot(
        &mut self,
        snapshot: ThreadOccurrenceSearchSnapshot,
    ) -> ThreadStoreResult<u64> {
        self.remove_expired();
        if self.snapshots.len() >= MAX_ACTIVE_SNAPSHOTS
            && let Some(completed_snapshot_id) = self
                .snapshots
                .iter()
                .filter(|(_, snapshot)| snapshot.completed)
                .min_by_key(|(_, snapshot)| snapshot.expires_at)
                .map(|(snapshot_id, _)| *snapshot_id)
        {
            self.snapshots.remove(&completed_snapshot_id);
        }
        if self.snapshots.len() >= MAX_ACTIVE_SNAPSHOTS {
            return Err(ThreadStoreError::Internal {
                message: "too many active thread search snapshots".to_string(),
            });
        }
        self.next_snapshot_id = self.next_snapshot_id.wrapping_add(1);
        let snapshot_id = self.next_snapshot_id;
        self.snapshots.insert(snapshot_id, snapshot);
        Ok(snapshot_id)
    }

    fn insert_projection(
        &mut self,
        thread_id: codex_protocol::ThreadId,
        projection: CachedSearchProjection,
    ) {
        self.remove_expired();
        if self.projections.len() >= MAX_CACHED_PROJECTIONS
            && !self.projections.contains_key(&thread_id)
            && let Some(oldest_thread_id) = self
                .projections
                .iter()
                .min_by_key(|(_, projection)| projection.expires_at)
                .map(|(thread_id, _)| *thread_id)
        {
            self.projections.remove(&oldest_thread_id);
        }
        self.projections.insert(thread_id, projection);
    }

    fn remove_expired(&mut self) {
        let now = Instant::now();
        self.snapshots
            .retain(|_, snapshot| snapshot.expires_at > now);
        self.projections
            .retain(|_, projection| projection.expires_at > now);
    }
}

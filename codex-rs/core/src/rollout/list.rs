use std::cmp::Reverse;
use std::io::{self};
use std::path::Path;
use std::path::PathBuf;

use codex_file_search as file_search;
use std::num::NonZero;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use time::OffsetDateTime;
use time::PrimitiveDateTime;
use time::format_description::FormatItem;
use time::macros::format_description;
use uuid::Uuid;

use super::SESSIONS_SUBDIR;
use crate::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionSource;

/// Returned page of conversation summaries.
#[derive(Debug, Default, PartialEq)]
pub struct ConversationsPage {
    /// Conversation summaries ordered newest first.
    pub items: Vec<ConversationItem>,
    /// Opaque pagination token to resume after the last item, or `None` if end.
    pub next_cursor: Option<Cursor>,
    /// Total number of files touched while scanning this request.
    pub num_scanned_files: usize,
    /// True if a hard scan cap was hit; consider resuming with `next_cursor`.
    pub reached_scan_cap: bool,
}

/// Summary information for a conversation rollout file.
#[derive(Debug, PartialEq)]
pub struct ConversationItem {
    /// Absolute path to the rollout file.
    pub path: PathBuf,
    /// First up to `HEAD_RECORD_LIMIT` JSONL records parsed as JSON (includes meta line).
    pub head: Vec<serde_json::Value>,
    /// Last up to `TAIL_RECORD_LIMIT` JSONL response records parsed as JSON.
    pub tail: Vec<serde_json::Value>,
    /// RFC3339 timestamp string for when the session was created, if available.
    pub created_at: Option<String>,
    /// RFC3339 timestamp string for the most recent response in the tail, if available.
    pub updated_at: Option<String>,
}

#[derive(Default)]
struct HeadTailSummary {
    head: Vec<serde_json::Value>,
    tail: Vec<serde_json::Value>,
    saw_session_meta: bool,
    saw_user_event: bool,
    source: Option<SessionSource>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

/// Hard cap to bound worst‑case work per request.
const MAX_SCAN_FILES: usize = 10000;
const HEAD_RECORD_LIMIT: usize = 10;
const TAIL_RECORD_LIMIT: usize = 10;

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

impl serde::Serialize for Cursor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let ts_str = self
            .ts
            .format(&format_description!(
                "[year]-[month]-[day]T[hour]-[minute]-[second]"
            ))
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

/// Retrieve recorded conversation file paths with token pagination. The returned `next_cursor`
/// can be supplied on the next call to resume after the last returned item, resilient to
/// concurrent new sessions being appended. Ordering is stable by timestamp desc, then UUID desc.
pub(crate) async fn get_conversations(
    codex_home: &Path,
    page_size: usize,
    cursor: Option<&Cursor>,
    allowed_sources: &[SessionSource],
) -> io::Result<ConversationsPage> {
    let mut root = codex_home.to_path_buf();
    root.push(SESSIONS_SUBDIR);

    if !root.exists() {
        return Ok(ConversationsPage {
            items: Vec::new(),
            next_cursor: None,
            num_scanned_files: 0,
            reached_scan_cap: false,
        });
    }

    let anchor = cursor.cloned();

    let result =
        traverse_directories_for_paths(root.clone(), page_size, anchor, allowed_sources).await?;
    Ok(result)
}

/// Load the full contents of a single conversation session file at `path`.
/// Returns the entire file contents as a String.
#[allow(dead_code)]
pub(crate) async fn get_conversation(path: &Path) -> io::Result<String> {
    tokio::fs::read_to_string(path).await
}

/// Load conversation file paths from disk using directory traversal.
///
/// Directory layout: `~/.codex/sessions/YYYY/MM/DD/rollout-YYYY-MM-DDThh-mm-ss-<uuid>.jsonl`
/// Returned newest (latest) first.
async fn traverse_directories_for_paths(
    root: PathBuf,
    page_size: usize,
    anchor: Option<Cursor>,
    allowed_sources: &[SessionSource],
) -> io::Result<ConversationsPage> {
    let mut items: Vec<ConversationItem> = Vec::with_capacity(page_size);
    let mut scanned_files = 0usize;
    let mut anchor_passed = anchor.is_none();
    let (anchor_ts, anchor_id) = match anchor {
        Some(c) => (c.ts, c.id),
        None => (OffsetDateTime::UNIX_EPOCH, Uuid::nil()),
    };

    let year_dirs = collect_dirs_desc(&root, |s| s.parse::<u16>().ok()).await?;

    'outer: for (_year, year_path) in year_dirs.iter() {
        if scanned_files >= MAX_SCAN_FILES {
            break;
        }
        let month_dirs = collect_dirs_desc(year_path, |s| s.parse::<u8>().ok()).await?;
        for (_month, month_path) in month_dirs.iter() {
            if scanned_files >= MAX_SCAN_FILES {
                break 'outer;
            }
            let day_dirs = collect_dirs_desc(month_path, |s| s.parse::<u8>().ok()).await?;
            for (_day, day_path) in day_dirs.iter() {
                if scanned_files >= MAX_SCAN_FILES {
                    break 'outer;
                }
                let mut day_files = collect_files(day_path, |name_str, path| {
                    if !name_str.starts_with("rollout-") || !name_str.ends_with(".jsonl") {
                        return None;
                    }

                    parse_timestamp_uuid_from_filename(name_str)
                        .map(|(ts, id)| (ts, id, name_str.to_string(), path.to_path_buf()))
                })
                .await?;
                // Stable ordering within the same second: (timestamp desc, uuid desc)
                day_files.sort_by_key(|(ts, sid, _name_str, _path)| (Reverse(*ts), Reverse(*sid)));
                for (ts, sid, _name_str, path) in day_files.into_iter() {
                    scanned_files += 1;
                    if scanned_files >= MAX_SCAN_FILES && items.len() >= page_size {
                        break 'outer;
                    }
                    if !anchor_passed {
                        if ts < anchor_ts || (ts == anchor_ts && sid < anchor_id) {
                            anchor_passed = true;
                        } else {
                            continue;
                        }
                    }
                    if items.len() == page_size {
                        break 'outer;
                    }
                    // Read head and simultaneously detect message events within the same
                    // first N JSONL records to avoid a second file read.
                    let summary = read_head_and_tail(&path, HEAD_RECORD_LIMIT, TAIL_RECORD_LIMIT)
                        .await
                        .unwrap_or_default();
                    if !allowed_sources.is_empty()
                        && !summary
                            .source
                            .is_some_and(|source| allowed_sources.iter().any(|s| s == &source))
                    {
                        continue;
                    }
                    // Apply filters: must have session meta and at least one user message event
                    if summary.saw_session_meta && summary.saw_user_event {
                        let HeadTailSummary {
                            head,
                            tail,
                            created_at,
                            mut updated_at,
                            ..
                        } = summary;
                        updated_at = updated_at.or_else(|| created_at.clone());
                        items.push(ConversationItem {
                            path,
                            head,
                            tail,
                            created_at,
                            updated_at,
                        });
                    }
                }
            }
        }
    }

    let next = build_next_cursor(&items);
    Ok(ConversationsPage {
        items,
        next_cursor: next,
        num_scanned_files: scanned_files,
        reached_scan_cap: scanned_files >= MAX_SCAN_FILES,
    })
}

/// Pagination cursor token format: "<file_ts>|<uuid>" where `file_ts` matches the
/// filename timestamp portion (YYYY-MM-DDThh-mm-ss) used in rollout filenames.
/// The cursor orders files by timestamp desc, then UUID desc.
fn parse_cursor(token: &str) -> Option<Cursor> {
    let (file_ts, uuid_str) = token.split_once('|')?;

    let Ok(uuid) = Uuid::parse_str(uuid_str) else {
        return None;
    };

    let format: &[FormatItem] =
        format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
    let ts = PrimitiveDateTime::parse(file_ts, format).ok()?.assume_utc();

    Some(Cursor::new(ts, uuid))
}

fn build_next_cursor(items: &[ConversationItem]) -> Option<Cursor> {
    let last = items.last()?;
    let file_name = last.path.file_name()?.to_string_lossy();
    let (ts, id) = parse_timestamp_uuid_from_filename(&file_name)?;
    Some(Cursor::new(ts, id))
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

fn parse_timestamp_uuid_from_filename(name: &str) -> Option<(OffsetDateTime, Uuid)> {
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

async fn read_head_and_tail(
    path: &Path,
    head_limit: usize,
    tail_limit: usize,
) -> io::Result<HeadTailSummary> {
    use tokio::io::AsyncBufReadExt;

    let file = tokio::fs::File::open(path).await?;
    let reader = tokio::io::BufReader::new(file);
    let mut lines = reader.lines();
    let mut summary = HeadTailSummary::default();

    while summary.head.len() < head_limit {
        let line_opt = lines.next_line().await?;
        let Some(line) = line_opt else { break };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parsed: Result<RolloutLine, _> = serde_json::from_str(trimmed);
        let Ok(rollout_line) = parsed else { continue };

        match rollout_line.item {
            RolloutItem::SessionMeta(session_meta_line) => {
                summary.source = Some(session_meta_line.meta.source);
                summary.created_at = summary
                    .created_at
                    .clone()
                    .or_else(|| Some(rollout_line.timestamp.clone()));
                if let Ok(val) = serde_json::to_value(session_meta_line) {
                    summary.head.push(val);
                    summary.saw_session_meta = true;
                }
            }
            RolloutItem::ResponseItem(item) => {
                summary.created_at = summary
                    .created_at
                    .clone()
                    .or_else(|| Some(rollout_line.timestamp.clone()));
                if let Ok(val) = serde_json::to_value(item) {
                    summary.head.push(val);
                }
            }
            RolloutItem::TurnContext(_) => {
                // Not included in `head`; skip.
            }
            RolloutItem::Compacted(_) => {
                // Not included in `head`; skip.
            }
            RolloutItem::EventMsg(ev) => {
                if matches!(ev, EventMsg::UserMessage(_)) {
                    summary.saw_user_event = true;
                }
            }
        }
    }

    if tail_limit != 0 {
        let (tail, updated_at) = read_tail_records(path, tail_limit).await?;
        summary.tail = tail;
        summary.updated_at = updated_at;
    }
    Ok(summary)
}

async fn read_tail_records(
    path: &Path,
    max_records: usize,
) -> io::Result<(Vec<serde_json::Value>, Option<String>)> {
    use std::io::SeekFrom;
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncSeekExt;

    if max_records == 0 {
        return Ok((Vec::new(), None));
    }

    const CHUNK_SIZE: usize = 8192;

    let mut file = tokio::fs::File::open(path).await?;
    let mut pos = file.seek(SeekFrom::End(0)).await?;
    if pos == 0 {
        return Ok((Vec::new(), None));
    }

    let mut buffer: Vec<u8> = Vec::new();
    let mut latest_timestamp: Option<String> = None;

    loop {
        let slice_start = match (pos > 0, buffer.iter().position(|&b| b == b'\n')) {
            (true, Some(idx)) => idx + 1,
            _ => 0,
        };
        let (tail, newest_ts) = collect_last_response_values(&buffer[slice_start..], max_records);
        if latest_timestamp.is_none() {
            latest_timestamp = newest_ts.clone();
        }
        if tail.len() >= max_records || pos == 0 {
            return Ok((tail, latest_timestamp.or(newest_ts)));
        }

        let read_size = CHUNK_SIZE.min(pos as usize);
        if read_size == 0 {
            return Ok((tail, latest_timestamp.or(newest_ts)));
        }
        pos -= read_size as u64;
        file.seek(SeekFrom::Start(pos)).await?;
        let mut chunk = vec![0; read_size];
        file.read_exact(&mut chunk).await?;
        chunk.extend_from_slice(&buffer);
        buffer = chunk;
    }
}

fn collect_last_response_values(
    buffer: &[u8],
    max_records: usize,
) -> (Vec<serde_json::Value>, Option<String>) {
    use std::borrow::Cow;

    if buffer.is_empty() || max_records == 0 {
        return (Vec::new(), None);
    }

    let text: Cow<'_, str> = String::from_utf8_lossy(buffer);
    let mut collected_rev: Vec<serde_json::Value> = Vec::new();
    let mut latest_timestamp: Option<String> = None;
    for line in text.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: serde_json::Result<RolloutLine> = serde_json::from_str(trimmed);
        let Ok(rollout_line) = parsed else { continue };
        let RolloutLine { timestamp, item } = rollout_line;
        if let RolloutItem::ResponseItem(item) = item
            && let Ok(val) = serde_json::to_value(&item)
        {
            if latest_timestamp.is_none() {
                latest_timestamp = Some(timestamp.clone());
            }
            collected_rev.push(val);
            if collected_rev.len() == max_records {
                break;
            }
        }
    }
    collected_rev.reverse();
    (collected_rev, latest_timestamp)
}

/// Locate a recorded conversation rollout file by its UUID string using the existing
/// paginated listing implementation. Returns `Ok(Some(path))` if found, `Ok(None)` if not present
/// or the id is invalid.
pub async fn find_conversation_path_by_id_str(
    codex_home: &Path,
    id_str: &str,
) -> io::Result<Option<PathBuf>> {
    // Validate UUID format early.
    if Uuid::parse_str(id_str).is_err() {
        return Ok(None);
    }

    let mut root = codex_home.to_path_buf();
    root.push(SESSIONS_SUBDIR);
    if !root.exists() {
        return Ok(None);
    }
    // This is safe because we know the values are valid.
    #[allow(clippy::unwrap_used)]
    let limit = NonZero::new(1).unwrap();
    // This is safe because we know the values are valid.
    #[allow(clippy::unwrap_used)]
    let threads = NonZero::new(2).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let exclude: Vec<String> = Vec::new();
    let compute_indices = false;

    let results = file_search::run(
        id_str,
        limit,
        &root,
        exclude,
        threads,
        cancel,
        compute_indices,
    )
    .map_err(|e| io::Error::other(format!("file search failed: {e}")))?;

    Ok(results
        .matches
        .into_iter()
        .next()
        .map(|m| root.join(m.path)))
}

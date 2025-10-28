use std::cmp::Reverse;
use std::io::{self};
use std::path::Path;
use std::path::PathBuf;

use code_file_search as file_search;
use std::num::NonZero;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use time::OffsetDateTime;
use time::PrimitiveDateTime;
use time::format_description::FormatItem;
use time::macros::format_description;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use super::SESSIONS_SUBDIR;
use crate::config::resolve_code_path_for_read;
use crate::protocol::event_msg_from_protocol;
use crate::protocol::EventMsg;
use code_protocol::protocol::RolloutItem;
use code_protocol::protocol::RolloutLine;
use code_protocol::protocol::SessionSource;

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
    /// RFC3339 timestamp string for the file's last modification time, if available.
    pub modified_at: Option<String>,
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
    code_home: &Path,
    page_size: usize,
    cursor: Option<&Cursor>,
    allowed_sources: &[SessionSource],
) -> io::Result<ConversationsPage> {
    let root = resolve_code_path_for_read(code_home, Path::new(SESSIONS_SUBDIR));

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
    let mut candidates: Vec<(OffsetDateTime, OffsetDateTime, Uuid, ConversationItem)> = Vec::new();
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
                let day_files = collect_files(day_path, |name_str, path| {
                    if !name_str.starts_with("rollout-") || !name_str.ends_with(".jsonl") {
                        return None;
                    }

                    parse_timestamp_uuid_from_filename(name_str)
                        .map(|(ts, id)| (ts, id, name_str.to_string(), path.to_path_buf()))
                })
                .await?;
                let mut day_entries = Vec::with_capacity(day_files.len());
                for (ts, sid, _name_str, path) in day_files.into_iter() {
                    let modified = tokio::fs::metadata(&path)
                        .await
                        .ok()
                        .and_then(|meta| meta.modified().ok())
                        .map(OffsetDateTime::from)
                        .unwrap_or(OffsetDateTime::UNIX_EPOCH);
                    day_entries.push((modified, ts, sid, path));
                }
                day_entries.sort_by_key(|(modified, ts, sid, _)| {
                    (Reverse(*modified), Reverse(*ts), Reverse(*sid))
                });
                for (modified, ts, sid, path) in day_entries.into_iter() {
                    scanned_files += 1;
                    if scanned_files >= MAX_SCAN_FILES {
                        break 'outer;
                    }
                    if !anchor_passed {
                        if ts < anchor_ts || (ts == anchor_ts && sid < anchor_id) {
                            anchor_passed = true;
                        } else {
                            continue;
                        }
                    }
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
                    if summary.saw_session_meta && summary.saw_user_event {
                        let HeadTailSummary {
                            head,
                            tail,
                            created_at,
                            mut updated_at,
                            ..
                        } = summary;
                        updated_at = updated_at.or_else(|| created_at.clone());
                        let modified_at = if modified == OffsetDateTime::UNIX_EPOCH {
                            updated_at.clone().or_else(|| created_at.clone())
                        } else {
                            modified.format(&Rfc3339).ok()
                        };
                        candidates.push((
                            modified,
                            ts,
                            sid,
                            ConversationItem {
                                path,
                                head,
                                tail,
                                created_at,
                                updated_at,
                                modified_at,
                            },
                        ));
                    }
                }
            }
        }
    }

    candidates.sort_by_key(|(modified, ts, sid, _)| {
        (Reverse(*modified), Reverse(*ts), Reverse(*sid))
    });

    let mut items: Vec<ConversationItem> = Vec::with_capacity(page_size.min(candidates.len()));
    for (_, _, _, item) in candidates.into_iter().take(page_size) {
        items.push(item);
    }

    let next = build_next_cursor(&items);
    Ok(ConversationsPage {
        items,
        next_cursor: next,
        num_scanned_files: scanned_files,
        reached_scan_cap: scanned_files >= MAX_SCAN_FILES,
    })
}

fn build_next_cursor(items: &[ConversationItem]) -> Option<Cursor> {
    let last = items.last()?;
    let file_name = last.path.file_name()?.to_string_lossy();
    let (ts, id) = parse_timestamp_uuid_from_filename(&file_name)?;
    Some(Cursor::new(ts, id))
}

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
    let core = name.strip_prefix("rollout-")?.strip_suffix(".jsonl")?;

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

        match &rollout_line.item {
            RolloutItem::SessionMeta(session_meta_line) => {
                summary.source = Some(session_meta_line.meta.source);
                summary.created_at = summary
                    .created_at
                    .clone()
                    .or_else(|| Some(rollout_line.timestamp.clone()));
                summary.updated_at = Some(rollout_line.timestamp.clone());
                summary.saw_session_meta = true;
            }
            RolloutItem::Event(event) => {
                if let Some(msg) = event_msg_from_protocol(&event.msg) {
                    if matches!(msg, EventMsg::UserMessage(_) | EventMsg::AgentMessage(_)) {
                        summary.saw_user_event = true;
                        summary.updated_at = Some(rollout_line.timestamp.clone());
                    }
                }
            }
            RolloutItem::ResponseItem(_) | RolloutItem::Compacted(_) | RolloutItem::TurnContext(_) => {}
        }

        summary
            .head
            .push(serde_json::to_value(&rollout_line.item).unwrap_or_default());
    }

    // Collect tail by reading entire file (bounded by TAIL_RECORD_LIMIT).
    let tail_lines = tokio::fs::read_to_string(path).await?;
    let mut tail_events = Vec::new();
    for line in tail_lines.lines().rev() {
        if tail_events.len() >= tail_limit {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: Result<RolloutLine, _> = serde_json::from_str(trimmed);
        if parsed.is_ok() {
            tail_events.push(serde_json::from_str(trimmed).unwrap_or_default());
        }
    }
    tail_events.reverse();
    summary.tail = tail_events;

    Ok(summary)
}

/// Locate a recorded conversation rollout file by its UUID string using the existing
/// paginated listing implementation. Returns `Ok(Some(path))` if found, `Ok(None)` if not present
/// or the id is invalid.
pub async fn find_conversation_path_by_id_str(
    code_home: &Path,
    id_str: &str,
) -> io::Result<Option<PathBuf>> {
    // Validate UUID format early.
    if Uuid::parse_str(id_str).is_err() {
        return Ok(None);
    }

    let mut root = code_home.to_path_buf();
    root.push(SESSIONS_SUBDIR);
    if !root.exists() {
        return Ok(None);
    }
    // Retrieve enough matches to prefer rollout `.jsonl` files when multiple results are returned
    // for the same UUID (e.g., accompanying `.snapshot.json` snapshots).
    #[allow(clippy::unwrap_used)]
    let limit = NonZero::new(8).unwrap();
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

    let mut fallback: Option<PathBuf> = None;
    for file_match in results.matches {
        let candidate = root.join(Path::new(&file_match.path));

        if candidate
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("jsonl"))
        {
            return Ok(Some(candidate));
        }

        if let Some(jsonl_candidate) = snapshot_to_rollout_path(&candidate) {
            if jsonl_candidate.exists() {
                return Ok(Some(jsonl_candidate));
            }
            fallback = fallback.or(Some(candidate));
            continue;
        }

        if fallback.is_none() {
            fallback = Some(candidate);
        }
    }

    Ok(fallback)
}

fn snapshot_to_rollout_path(path: &Path) -> Option<PathBuf> {
    let file_name = path.file_name()?.to_str()?;
    let base = file_name.strip_suffix(".snapshot.json")?;
    let mut candidate = path.to_path_buf();
    candidate.set_file_name(format!("{base}.jsonl"));
    Some(candidate)
}

#[cfg(test)]
mod tests {
    use super::find_conversation_path_by_id_str;
    use super::snapshot_to_rollout_path;
    use super::SESSIONS_SUBDIR;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use uuid::Uuid;

    #[tokio::test]
    async fn prefers_rollout_jsonl_when_snapshot_present() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let code_home = temp.path();
        let uuid = Uuid::new_v4();
        let uuid_str = uuid.to_string();

        let sessions_dir = code_home
            .join(SESSIONS_SUBDIR)
            .join("2025")
            .join("10")
            .join("06");
        fs::create_dir_all(&sessions_dir)?;

        let base = format!("rollout-2025-10-06T12-00-00-{uuid_str}");
        let rollout_path = sessions_dir.join(format!("{base}.jsonl"));
        let snapshot_path = sessions_dir.join(format!("{base}.snapshot.json"));

        fs::write(&rollout_path, "{\"item\":\"dummy\"}\n")?;
        fs::write(&snapshot_path, "{}")?;

        let resolved = find_conversation_path_by_id_str(code_home, &uuid_str).await?;
        assert_eq!(resolved, Some(rollout_path));

        Ok(())
    }

    #[test]
    fn snapshot_conversion_rewrites_extension() {
        let snapshot = PathBuf::from(
            "/tmp/rollout-2025-10-06T12-00-00-12345678-1234-1234-1234-123456789abc.snapshot.json",
        );
        let expected = PathBuf::from(
            "/tmp/rollout-2025-10-06T12-00-00-12345678-1234-1234-1234-123456789abc.jsonl",
        );
        let converted = snapshot_to_rollout_path(&snapshot);
        assert_eq!(converted, Some(expected));
    }
}

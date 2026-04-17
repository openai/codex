use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex as StdMutex;
use std::time::SystemTime;

use chrono::DateTime;
use chrono::SecondsFormat;
use chrono::Utc;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use tokio::sync::Mutex as AsyncMutex;

use super::storage::WindowState;
use super::storage::ensure_sidecar_dirs;
use super::storage::read_state;
use super::storage::write_atomic;
use super::transcript;
use crate::RolloutRecorder;

const NOTE_VERSIONS_FILE: &str = "note_versions.json";
const SHARED_NOTE_VERSIONS_FILE: &str = ".versions.json";
const WINDOW_ID_WIDTH: usize = 5;
pub(crate) const MAX_NOTE_CHARS: usize = 65_536;
const DEFAULT_LIST_WINDOW: usize = 50;
const MAX_LIST_ITEMS: usize = 200;
const DEFAULT_LOG_READ_WINDOW: usize = 50;
const MAX_LOG_READ_ENTRIES: usize = 200;
const DEFAULT_NOTE_READ_WINDOW: usize = 1000;
const MAX_NOTE_READ_LINES: usize = 1000;
const DEFAULT_SEARCH_WINDOW: usize = 20;
const MAX_SEARCH_RESULTS: usize = 100;

static NOTE_STORE_WRITE_LOCKS: LazyLock<StdMutex<HashMap<PathBuf, Arc<AsyncMutex<()>>>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RangeInfo {
    pub(crate) start: usize,
    pub(crate) stop: usize,
    pub(crate) total: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ListResponse {
    pub(crate) collection: String,
    pub(crate) items: Vec<Value>,
    pub(crate) range: RangeInfo,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ReadLogResponse {
    pub(crate) kind: &'static str,
    pub(crate) id: String,
    pub(crate) entries: Vec<transcript::LogEntry>,
    pub(crate) range: RangeInfo,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ReadNoteResponse {
    pub(crate) kind: &'static str,
    pub(crate) id: String,
    pub(crate) content: String,
    pub(crate) content_chars: usize,
    pub(crate) range: RangeInfo,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SearchResponse {
    pub(crate) scope: String,
    pub(crate) query: String,
    pub(crate) results: Vec<Value>,
    pub(crate) range: RangeInfo,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WriteNoteResponse {
    pub(crate) kind: &'static str,
    pub(crate) id: String,
    pub(crate) operation: String,
    pub(crate) content_chars: usize,
    pub(crate) total_content_chars: usize,
    pub(crate) line_count: usize,
    pub(crate) version: u64,
    pub(crate) ok: bool,
}

#[derive(Debug)]
pub(crate) enum StorageToolError {
    Io(std::io::Error),
    Invalid(String),
}

impl std::fmt::Display for StorageToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "Reflections storage failed: {err}"),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}

impl From<std::io::Error> for StorageToolError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

type StorageToolResult<T> = Result<T, StorageToolError>;

#[derive(Debug, Clone, Copy)]
struct Selection {
    start: usize,
    stop: usize,
}

pub(crate) async fn list_logs(
    sidecar_path: &Path,
    rollout_path: &Path,
    start: Option<usize>,
    stop: Option<usize>,
) -> StorageToolResult<ListResponse> {
    let selection = selection(start, stop, DEFAULT_LIST_WINDOW, MAX_LIST_ITEMS)?;
    let state = read_state(sidecar_path, rollout_path).await?;
    let rollout_items = load_rollout_items(rollout_path).await?;
    let mut items = Vec::new();
    for window in state.windows.iter().rev() {
        let entries = entries_for_window(window, &state.windows, &rollout_items)?;
        items.push(json!({
            "kind": "log",
            "id": window.window,
            "trigger": window.trigger,
            "created_at": window.created_at,
            "context_window_size": window.context_window_size,
            "entry_count": entries.len(),
        }));
    }

    let total = items.len();
    let items = slice_items(items, selection);
    Ok(ListResponse {
        collection: "logs".to_string(),
        items,
        range: range_info(selection, total),
    })
}

pub(crate) async fn list_notes(
    sidecar_path: &Path,
    start: Option<usize>,
    stop: Option<usize>,
) -> StorageToolResult<ListResponse> {
    ensure_sidecar_dirs(sidecar_path).await?;
    list_notes_in_store(&sidecar_path.join("notes"), "note", "notes", start, stop).await
}

pub(crate) async fn list_shared_notes(
    shared_notes_path: &Path,
    start: Option<usize>,
    stop: Option<usize>,
) -> StorageToolResult<ListResponse> {
    list_notes_in_store(
        shared_notes_path,
        "shared_note",
        "shared_notes",
        start,
        stop,
    )
    .await
}

pub(crate) async fn read_log(
    sidecar_path: &Path,
    rollout_path: &Path,
    id: &str,
    start: Option<usize>,
    stop: Option<usize>,
) -> StorageToolResult<ReadLogResponse> {
    reject_latest(id)?;
    validate_window_id(id)?;
    let selection = selection(start, stop, DEFAULT_LOG_READ_WINDOW, MAX_LOG_READ_ENTRIES)?;
    let state = read_state(sidecar_path, rollout_path).await?;
    let window = state
        .windows
        .iter()
        .find(|window| window.window == id)
        .ok_or_else(|| {
            StorageToolError::Invalid(format!("Reflections log `{id}` was not found"))
        })?;
    let rollout_items = load_rollout_items(rollout_path).await?;
    let entries = entries_for_window(window, &state.windows, &rollout_items)?;
    let total = entries.len();
    let entries = slice_items(entries, selection);
    Ok(ReadLogResponse {
        kind: "log",
        id: id.to_string(),
        entries,
        range: range_info(selection, total),
    })
}

pub(crate) async fn read_note(
    sidecar_path: &Path,
    id: &str,
    start: Option<usize>,
    stop: Option<usize>,
) -> StorageToolResult<ReadNoteResponse> {
    read_note_in_store(&sidecar_path.join("notes"), "note", id, start, stop).await
}

pub(crate) async fn read_shared_note(
    shared_notes_path: &Path,
    id: &str,
    start: Option<usize>,
    stop: Option<usize>,
) -> StorageToolResult<ReadNoteResponse> {
    read_note_in_store(shared_notes_path, "shared_note", id, start, stop).await
}

pub(crate) async fn search(
    sidecar_path: &Path,
    rollout_path: &Path,
    scope: &str,
    query: &str,
    log_id: Option<&str>,
    start: Option<usize>,
    stop: Option<usize>,
) -> StorageToolResult<SearchResponse> {
    if query.is_empty() {
        return Err(StorageToolError::Invalid(
            "`query` is required and must not be empty".to_string(),
        ));
    }
    if log_id.is_some() && scope != "logs" {
        return Err(StorageToolError::Invalid(
            "`log_id` is only valid when `scope` is `logs`".to_string(),
        ));
    }
    if let Some(log_id) = log_id {
        reject_latest(log_id)?;
        validate_window_id(log_id)?;
    }
    let selection = selection(start, stop, DEFAULT_SEARCH_WINDOW, MAX_SEARCH_RESULTS)?;
    let needle = query.to_ascii_lowercase();
    let mut results = Vec::new();

    if matches!(scope, "all" | "logs") {
        let state = read_state(sidecar_path, rollout_path).await?;
        let rollout_items = load_rollout_items(rollout_path).await?;
        for window in &state.windows {
            if log_id.is_some_and(|id| id != window.window) {
                continue;
            }
            let entries = entries_for_window(window, &state.windows, &rollout_items)?;
            for (entry_index, entry) in entries.iter().enumerate() {
                if entry.content.to_ascii_lowercase().contains(&needle) {
                    let entry_number = entry_index + 1;
                    results.push(json!({
                        "kind": "log",
                        "log_id": window.window,
                        "entry_id": entry.entry_id,
                        "snippet": snippet(&entry.content, query),
                        "read": {
                            "kind": "log",
                            "id": window.window,
                            "start": entry_number,
                            "stop": entry_number,
                        },
                    }));
                }
            }
        }
    }

    if matches!(scope, "all" | "notes") {
        ensure_sidecar_dirs(sidecar_path).await?;
        results.extend(
            search_note_hits_in_store(
                &sidecar_path.join("notes"),
                "note",
                query,
                "reflections_read",
            )
            .await?,
        );
    }

    let total = results.len();
    let results = slice_items(results, selection);
    Ok(SearchResponse {
        scope: scope.to_string(),
        query: query.to_string(),
        results,
        range: range_info(selection, total),
    })
}

pub(crate) async fn search_shared_notes(
    shared_notes_path: &Path,
    query: &str,
    start: Option<usize>,
    stop: Option<usize>,
) -> StorageToolResult<SearchResponse> {
    search_notes_in_store(
        shared_notes_path,
        "shared_note",
        query,
        start,
        stop,
        "reflections_read_shared_note",
    )
    .await
}

pub(crate) async fn write_note(
    sidecar_path: &Path,
    id: &str,
    operation: &str,
    content: &str,
) -> StorageToolResult<WriteNoteResponse> {
    ensure_sidecar_dirs(sidecar_path).await?;
    write_note_in_store(
        &sidecar_path.join("notes"),
        &sidecar_path.join(NOTE_VERSIONS_FILE),
        "note",
        id,
        operation,
        content,
    )
    .await
}

pub(crate) async fn write_shared_note(
    shared_notes_path: &Path,
    id: &str,
    operation: &str,
    content: &str,
) -> StorageToolResult<WriteNoteResponse> {
    write_note_in_store(
        shared_notes_path,
        &shared_notes_path.join(SHARED_NOTE_VERSIONS_FILE),
        "shared_note",
        id,
        operation,
        content,
    )
    .await
}

async fn list_notes_in_store(
    notes_dir: &Path,
    kind: &'static str,
    collection: &str,
    start: Option<usize>,
    stop: Option<usize>,
) -> StorageToolResult<ListResponse> {
    let selection = selection(start, stop, DEFAULT_LIST_WINDOW, MAX_LIST_ITEMS)?;
    tokio::fs::create_dir_all(notes_dir).await?;
    let mut entries = tokio::fs::read_dir(notes_dir).await?;
    let mut items = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let file_type = entry.file_type().await?;
        if !file_type.is_file() {
            continue;
        }
        let note_id = entry.file_name().to_string_lossy().to_string();
        if note_id.starts_with('.') || !valid_note_id(&note_id) {
            continue;
        }
        let metadata = entry.metadata().await?;
        let content = tokio::fs::read_to_string(entry.path())
            .await
            .unwrap_or_default();
        items.push((
            metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            note_id.clone(),
            json!({
                "kind": kind,
                "id": note_id,
                "updated_at": system_time_rfc3339(metadata.modified().ok()),
                "content_chars": content.chars().count(),
                "line_count": line_count(&content),
            }),
        ));
    }
    items.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    let items = items
        .into_iter()
        .map(|(_, _, value)| value)
        .collect::<Vec<_>>();
    let total = items.len();
    let items = slice_items(items, selection);
    Ok(ListResponse {
        collection: collection.to_string(),
        items,
        range: range_info(selection, total),
    })
}

async fn read_note_in_store(
    notes_dir: &Path,
    kind: &'static str,
    id: &str,
    start: Option<usize>,
    stop: Option<usize>,
) -> StorageToolResult<ReadNoteResponse> {
    reject_latest(id)?;
    validate_note_id(id)?;
    let selection = selection(start, stop, DEFAULT_NOTE_READ_WINDOW, MAX_NOTE_READ_LINES)?;
    let path = notes_dir.join(id);
    let content = tokio::fs::read_to_string(&path).await.map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            StorageToolError::Invalid(format!("Reflections note `{id}` was not found"))
        } else {
            StorageToolError::Io(err)
        }
    })?;
    let lines = content.lines().collect::<Vec<_>>();
    let total = lines.len();
    let selected_content = if selection.start > total {
        String::new()
    } else {
        let end = selection.stop.min(total);
        let mut selected = lines[selection.start - 1..end].join("\n");
        if !selected.is_empty() && content.ends_with('\n') {
            selected.push('\n');
        }
        selected
    };
    Ok(ReadNoteResponse {
        kind,
        id: id.to_string(),
        content: selected_content.clone(),
        content_chars: selected_content.chars().count(),
        range: range_info(selection, total),
    })
}

async fn search_notes_in_store(
    notes_dir: &Path,
    kind: &'static str,
    query: &str,
    start: Option<usize>,
    stop: Option<usize>,
    read_tool_name: &'static str,
) -> StorageToolResult<SearchResponse> {
    if query.is_empty() {
        return Err(StorageToolError::Invalid(
            "`query` is required and must not be empty".to_string(),
        ));
    }
    let selection = selection(start, stop, DEFAULT_SEARCH_WINDOW, MAX_SEARCH_RESULTS)?;
    let mut results = search_note_hits_in_store(notes_dir, kind, query, read_tool_name).await?;
    let total = results.len();
    results = slice_items(results, selection);
    Ok(SearchResponse {
        scope: if kind == "shared_note" {
            "shared_notes"
        } else {
            "notes"
        }
        .to_string(),
        query: query.to_string(),
        results,
        range: range_info(selection, total),
    })
}

async fn search_note_hits_in_store(
    notes_dir: &Path,
    kind: &'static str,
    query: &str,
    read_tool_name: &'static str,
) -> StorageToolResult<Vec<Value>> {
    tokio::fs::create_dir_all(notes_dir).await?;
    let needle = query.to_ascii_lowercase();
    let mut entries = tokio::fs::read_dir(notes_dir).await?;
    let mut results = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let file_type = entry.file_type().await?;
        if !file_type.is_file() {
            continue;
        }
        let note_id = entry.file_name().to_string_lossy().to_string();
        if note_id.starts_with('.') || !valid_note_id(&note_id) {
            continue;
        }
        let content = tokio::fs::read_to_string(entry.path())
            .await
            .unwrap_or_default();
        for (line_index, line) in content.lines().enumerate() {
            if line.to_ascii_lowercase().contains(&needle) {
                let line_number = line_index + 1;
                let read = if kind == "shared_note" {
                    json!({
                        "tool": read_tool_name,
                        "note_id": &note_id,
                        "start": line_number,
                        "stop": line_number,
                    })
                } else {
                    json!({
                        "kind": "note",
                        "id": &note_id,
                        "start": line_number,
                        "stop": line_number,
                    })
                };
                results.push(json!({
                    "kind": kind,
                    "note_id": &note_id,
                    "line": line_number,
                    "snippet": snippet(line, query),
                    "read": read,
                }));
            }
        }
    }
    Ok(results)
}

async fn write_note_in_store(
    notes_dir: &Path,
    version_file: &Path,
    kind: &'static str,
    id: &str,
    operation: &str,
    content: &str,
) -> StorageToolResult<WriteNoteResponse> {
    validate_note_id(id)?;
    if content.chars().count() > MAX_NOTE_CHARS {
        return Err(StorageToolError::Invalid(format!(
            "Reflections note writes are limited to {MAX_NOTE_CHARS} characters"
        )));
    }
    tokio::fs::create_dir_all(notes_dir).await?;
    let write_lock = note_store_write_lock(notes_dir);
    let _guard = write_lock.lock().await;
    let path = notes_dir.join(id);
    let existing = tokio::fs::read_to_string(&path).await;
    let existing = match existing {
        Ok(existing) => Some(existing),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(StorageToolError::Io(err)),
    };
    let new_content = match operation {
        "create_only" => {
            if existing.is_some() {
                return Err(StorageToolError::Invalid(format!(
                    "Reflections note `{id}` already exists"
                )));
            }
            content.to_string()
        }
        "append" => {
            let mut combined = existing.unwrap_or_default();
            combined.push_str(content);
            if combined.chars().count() > MAX_NOTE_CHARS {
                return Err(StorageToolError::Invalid(format!(
                    "Appending would make Reflections note `{id}` exceed {MAX_NOTE_CHARS} characters; replace the note with a shorter version instead"
                )));
            }
            combined
        }
        "replace" => content.to_string(),
        _ => {
            return Err(StorageToolError::Invalid(format!(
                "unsupported Reflections note operation `{operation}`"
            )));
        }
    };
    if new_content.chars().count() > MAX_NOTE_CHARS {
        return Err(StorageToolError::Invalid(format!(
            "Reflections note `{id}` would exceed {MAX_NOTE_CHARS} characters"
        )));
    }
    write_atomic(&path, new_content.as_bytes()).await?;
    let version = increment_note_version(version_file, id).await?;
    Ok(WriteNoteResponse {
        kind,
        id: id.to_string(),
        operation: operation.to_string(),
        content_chars: content.chars().count(),
        total_content_chars: new_content.chars().count(),
        line_count: line_count(&new_content),
        version,
        ok: true,
    })
}

async fn load_rollout_items(
    rollout_path: &Path,
) -> StorageToolResult<Vec<codex_protocol::protocol::RolloutItem>> {
    let (items, _, _) = RolloutRecorder::load_rollout_items(rollout_path).await?;
    Ok(items)
}

fn entries_for_window(
    window: &WindowState,
    windows: &[WindowState],
    rollout_items: &[codex_protocol::protocol::RolloutItem],
) -> StorageToolResult<Vec<transcript::LogEntry>> {
    let range = rollout_range_for_window(window, windows, rollout_items)?;
    Ok(transcript::log_entries_from_items(
        &window.window,
        &rollout_items[range],
    ))
}

fn rollout_range_for_window(
    window: &WindowState,
    windows: &[WindowState],
    rollout_items: &[codex_protocol::protocol::RolloutItem],
) -> StorageToolResult<std::ops::Range<usize>> {
    if let (Some(start_line), Some(end_line)) = (window.rollout_start_line, window.rollout_end_line)
    {
        if start_line == 0 || end_line < start_line {
            return Ok(0..0);
        }
        let start = start_line.saturating_sub(1).min(rollout_items.len());
        let end = end_line.min(rollout_items.len());
        return Ok(start..end);
    }

    let window_index = windows
        .iter()
        .position(|candidate| candidate.window == window.window)
        .ok_or_else(|| {
            StorageToolError::Invalid(format!(
                "Reflections log `{}` is missing from state",
                window.window
            ))
        })?;
    Ok(rollout_range_for_legacy_window(window_index, rollout_items))
}

fn rollout_range_for_legacy_window(
    window_index: usize,
    rollout_items: &[codex_protocol::protocol::RolloutItem],
) -> std::ops::Range<usize> {
    let compacted_indices = rollout_items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            matches!(item, codex_protocol::protocol::RolloutItem::Compacted(_)).then_some(index)
        })
        .collect::<Vec<_>>();
    let start = if window_index == 0 {
        0
    } else {
        compacted_indices
            .get(window_index - 1)
            .map_or(rollout_items.len(), |index| index + 1)
    };
    let end = compacted_indices
        .get(window_index)
        .copied()
        .unwrap_or(rollout_items.len());
    start.min(rollout_items.len())..end.min(rollout_items.len())
}

fn selection(
    start: Option<usize>,
    stop: Option<usize>,
    default_window: usize,
    max_window: usize,
) -> StorageToolResult<Selection> {
    let start = start.unwrap_or(1);
    if start == 0 {
        return Err(StorageToolError::Invalid(
            "`start` must be 1 or greater".to_string(),
        ));
    }
    let requested_stop = stop.unwrap_or_else(|| start.saturating_add(default_window - 1));
    if requested_stop < start {
        return Err(StorageToolError::Invalid(
            "`stop` must be greater than or equal to `start`".to_string(),
        ));
    }
    let max_stop = start.saturating_add(max_window - 1);
    Ok(Selection {
        start,
        stop: requested_stop.min(max_stop),
    })
}

fn slice_items<T>(items: Vec<T>, selection: Selection) -> Vec<T> {
    if selection.start > items.len() {
        return Vec::new();
    }
    let start_index = selection.start - 1;
    let end_index = selection.stop.min(items.len());
    items
        .into_iter()
        .skip(start_index)
        .take(end_index.saturating_sub(start_index))
        .collect()
}

fn range_info(selection: Selection, total: usize) -> RangeInfo {
    RangeInfo {
        start: selection.start,
        stop: selection.stop,
        total,
    }
}

fn reject_latest(id: &str) -> StorageToolResult<()> {
    if id == "latest" {
        return Err(StorageToolError::Invalid(
            "`latest` is not supported; list logs or notes and choose an explicit ID".to_string(),
        ));
    }
    Ok(())
}

fn validate_window_id(id: &str) -> StorageToolResult<()> {
    if id.len() == WINDOW_ID_WIDTH + 2
        && id.starts_with("cw")
        && id[2..].chars().all(|ch| ch.is_ascii_digit())
    {
        Ok(())
    } else {
        Err(StorageToolError::Invalid(format!(
            "Reflections log ID `{id}` is invalid; expected cwNNNNN"
        )))
    }
}

fn validate_note_id(id: &str) -> StorageToolResult<()> {
    if valid_note_id(id) {
        Ok(())
    } else {
        Err(StorageToolError::Invalid(format!(
            "Reflections note ID `{id}` is invalid; use ^[A-Za-z0-9][A-Za-z0-9_.-]{{0,127}}$ with no slashes, `..`, or absolute paths"
        )))
    }
}

fn valid_note_id(id: &str) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() || id.len() > 128 || id.contains("..") {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-'))
}

fn note_store_write_lock(notes_dir: &Path) -> Arc<AsyncMutex<()>> {
    let mut locks = NOTE_STORE_WRITE_LOCKS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    locks
        .entry(notes_dir.to_path_buf())
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

fn line_count(content: &str) -> usize {
    content.lines().count()
}

fn system_time_rfc3339(time: Option<SystemTime>) -> Option<String> {
    time.map(|time| {
        let datetime: DateTime<Utc> = time.into();
        datetime.to_rfc3339_opts(SecondsFormat::Secs, true)
    })
}

async fn increment_note_version(path: &Path, id: &str) -> StorageToolResult<u64> {
    let mut versions = match tokio::fs::read_to_string(&path).await {
        Ok(contents) => {
            serde_json::from_str::<BTreeMap<String, u64>>(&contents).unwrap_or_default()
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(err) => return Err(StorageToolError::Io(err)),
    };
    let version = versions.get(id).copied().unwrap_or(0).saturating_add(1);
    versions.insert(id.to_string(), version);
    let bytes = serde_json::to_vec_pretty(&versions).map_err(|err| {
        StorageToolError::Invalid(format!("failed to serialize note versions: {err}"))
    })?;
    write_atomic(path, &bytes).await?;
    Ok(version)
}

fn snippet(content: &str, query: &str) -> String {
    let lower = content.to_ascii_lowercase();
    let needle = query.to_ascii_lowercase();
    let mut match_start = lower.find(&needle).unwrap_or(0).min(content.len());
    while !content.is_char_boundary(match_start) {
        match_start = match_start.saturating_sub(1);
    }
    let start = content[..match_start]
        .char_indices()
        .rev()
        .nth(60)
        .map_or(0, |(index, _)| index);
    let mut end = match_start;
    let mut chars = content[match_start..].char_indices();
    for _ in 0..180 {
        match chars.next() {
            Some((offset, ch)) => end = match_start + offset + ch.len_utf8(),
            None => {
                end = content.len();
                break;
            }
        }
    }
    let mut snippet = String::new();
    if start > 0 {
        snippet.push_str("...");
    }
    snippet.push_str(content[start..end].trim());
    if end < content.len() {
        snippet.push_str("...");
    }
    snippet
}

#[cfg(test)]
mod tests {
    use codex_analytics::CompactionTrigger;
    use codex_protocol::models::FunctionCallOutputPayload;
    use codex_protocol::models::ResponseItem;
    use codex_protocol::protocol::AgentMessageEvent;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::RolloutItem;
    use codex_protocol::protocol::RolloutLine;
    use codex_protocol::protocol::UserMessageEvent;
    use pretty_assertions::assert_eq;

    use super::MAX_NOTE_CHARS;
    use super::list_logs;
    use super::list_notes;
    use super::list_shared_notes;
    use super::read_log;
    use super::read_note;
    use super::read_shared_note;
    use super::search;
    use super::search_shared_notes;
    use super::write_note;
    use super::write_shared_note;
    use crate::reflections::storage::sidecar_path_for_rollout;
    use crate::reflections::storage::write_window;

    #[tokio::test]
    async fn write_note_validates_ids_and_size() -> std::io::Result<()> {
        let temp = tempfile::tempdir()?;
        let rollout = temp.path().join("rollout-2026-04-16T00-00-00-thread.jsonl");
        let sidecar = sidecar_path_for_rollout(&rollout);

        let invalid = write_note(&sidecar, "../bad", "append", "content").await;
        assert!(invalid.is_err());

        let too_large = "x".repeat(MAX_NOTE_CHARS + 1);
        let too_large = write_note(&sidecar, "handoff", "append", &too_large).await;
        assert!(too_large.is_err());

        let first = write_note(&sidecar, "handoff", "create_only", "Current task\n")
            .await
            .expect("create note");
        assert_eq!(first.version, 1);
        assert_eq!(first.total_content_chars, "Current task\n".chars().count());

        let append = write_note(&sidecar, "handoff", "append", "Next step\n")
            .await
            .expect("append note");
        assert_eq!(append.version, 2);
        assert_eq!(append.line_count, 2);

        let note = read_note(&sidecar, "handoff", Some(2), Some(2))
            .await
            .expect("read note");
        assert_eq!(note.content, "Next step\n");
        Ok(())
    }

    #[tokio::test]
    async fn append_fails_when_note_would_exceed_limit() -> std::io::Result<()> {
        let temp = tempfile::tempdir()?;
        let rollout = temp.path().join("rollout-2026-04-16T00-00-00-thread.jsonl");
        let sidecar = sidecar_path_for_rollout(&rollout);

        write_note(
            &sidecar,
            "handoff",
            "create_only",
            &"x".repeat(MAX_NOTE_CHARS - 1),
        )
        .await
        .expect("create near-limit note");
        let result = write_note(&sidecar, "handoff", "append", "xx").await;
        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn shared_notes_use_shared_labels_limits_and_read_locators() -> std::io::Result<()> {
        let temp = tempfile::tempdir()?;
        let shared_notes = temp.path().join("root.reflections").join("shared_notes");

        let invalid = write_shared_note(&shared_notes, "../bad", "append", "content").await;
        assert!(invalid.is_err());

        write_shared_note(
            &shared_notes,
            "handoff",
            "create_only",
            "Current task: parser tests\n",
        )
        .await
        .expect("create shared note");
        let too_large_append = write_shared_note(
            &shared_notes,
            "handoff",
            "append",
            &"x".repeat(MAX_NOTE_CHARS),
        )
        .await;
        assert!(too_large_append.is_err());

        let listed = list_shared_notes(&shared_notes, Some(1), Some(50))
            .await
            .expect("list shared notes");
        assert_eq!(listed.collection, "shared_notes");
        assert_eq!(listed.items[0]["kind"], "shared_note");
        assert_eq!(listed.items[0]["id"], "handoff");
        assert!(listed.items[0].get("path").is_none());

        let read = read_shared_note(&shared_notes, "handoff", Some(1), Some(1))
            .await
            .expect("read shared note");
        assert_eq!(read.kind, "shared_note");
        assert_eq!(read.content, "Current task: parser tests\n");

        let latest = read_shared_note(&shared_notes, "latest", Some(1), Some(1)).await;
        assert!(latest.is_err());

        let search_result = search_shared_notes(&shared_notes, "parser tests", Some(1), Some(20))
            .await
            .expect("search shared notes");
        assert_eq!(search_result.scope, "shared_notes");
        assert_eq!(search_result.results[0]["kind"], "shared_note");
        assert_eq!(search_result.results[0]["note_id"], "handoff");
        assert_eq!(
            search_result.results[0]["read"]["tool"],
            "reflections_read_shared_note"
        );
        assert_eq!(search_result.results[0]["read"]["note_id"], "handoff");
        assert_eq!(search_result.results[0]["read"]["start"], 1);

        Ok(())
    }

    #[tokio::test]
    async fn list_read_and_search_use_rollout_jsonl_not_markdown() -> std::io::Result<()> {
        let temp = tempfile::tempdir()?;
        let rollout = temp.path().join("rollout-2026-04-16T00-00-00-thread.jsonl");
        let sidecar = sidecar_path_for_rollout(&rollout);
        write_rollout(
            &rollout,
            &[
                RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
                    message: "Need parser tests".to_string(),
                    images: None,
                    local_images: Vec::new(),
                    text_elements: Vec::new(),
                })),
                RolloutItem::EventMsg(EventMsg::AgentMessage(AgentMessageEvent {
                    message: "I will inspect parser tests next.".to_string(),
                    phase: None,
                    memory_citation: None,
                })),
            ],
        )
        .await?;

        write_window(
            &sidecar,
            &rollout,
            CompactionTrigger::Auto,
            Some(9500),
            1,
            2,
            "markdown-only parser marker should not be parsed".to_string(),
        )
        .await?;
        write_note(
            &sidecar,
            "handoff",
            "replace",
            "Current task: parser tests\n",
        )
        .await
        .expect("write note");

        let logs = list_logs(&sidecar, &rollout, Some(1), Some(50))
            .await
            .expect("list logs");
        assert_eq!(logs.items[0]["id"], "cw00000");
        assert_eq!(logs.items[0]["entry_count"], 2);

        let notes = list_notes(&sidecar, Some(1), Some(50))
            .await
            .expect("list notes");
        assert_eq!(notes.items[0]["id"], "handoff");

        let read = read_log(&sidecar, &rollout, "cw00000", Some(1), Some(50))
            .await
            .expect("read log");
        assert_eq!(read.entries[0].entry_id, "cw00000:msg-000001");
        assert_eq!(read.entries[0].kind, "user_message");
        assert_eq!(read.entries[0].role.as_deref(), Some("user"));
        assert!(read.entries[0].content.contains("Need parser tests"));

        let latest = read_log(&sidecar, &rollout, "latest", Some(1), Some(1)).await;
        assert!(latest.is_err());

        let search_result = search(
            &sidecar,
            &rollout,
            "logs",
            "parser tests",
            Some("cw00000"),
            Some(1),
            Some(20),
        )
        .await
        .expect("search logs");
        assert_eq!(search_result.results[0]["log_id"], "cw00000");
        assert_eq!(search_result.results[0]["entry_id"], "cw00000:msg-000001");
        assert_eq!(search_result.results[0]["read"]["start"], 1);

        let markdown_only = search(
            &sidecar,
            &rollout,
            "logs",
            "markdown-only",
            Some("cw00000"),
            Some(1),
            Some(20),
        )
        .await
        .expect("search logs");
        assert!(markdown_only.results.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn reflections_storage_tool_entries_are_metadata_only() -> std::io::Result<()> {
        let temp = tempfile::tempdir()?;
        let rollout = temp.path().join("rollout-2026-04-16T00-00-00-thread.jsonl");
        let sidecar = sidecar_path_for_rollout(&rollout);
        write_rollout(
            &rollout,
            &[
                RolloutItem::ResponseItem(ResponseItem::FunctionCall {
                    id: None,
                    name: "reflections_write_note".to_string(),
                    namespace: None,
                    arguments: serde_json::json!({
                        "note_id": "handoff",
                        "operation": "append",
                        "content": "SECRET NOTE BODY"
                    })
                    .to_string(),
                    call_id: "call-1".to_string(),
                }),
                RolloutItem::ResponseItem(ResponseItem::FunctionCallOutput {
                    call_id: "call-1".to_string(),
                    output: FunctionCallOutputPayload::from_text(
                        serde_json::json!({
                            "kind": "note",
                            "id": "handoff",
                            "operation": "append",
                            "content_chars": 16,
                            "total_content_chars": 16,
                            "line_count": 1,
                            "version": 1,
                            "ok": true
                        })
                        .to_string(),
                    ),
                }),
                RolloutItem::ResponseItem(ResponseItem::FunctionCall {
                    id: None,
                    name: "reflections_write_shared_note".to_string(),
                    namespace: None,
                    arguments: serde_json::json!({
                        "note_id": "team",
                        "operation": "append",
                        "content": "SECRET SHARED BODY"
                    })
                    .to_string(),
                    call_id: "call-2".to_string(),
                }),
                RolloutItem::ResponseItem(ResponseItem::FunctionCallOutput {
                    call_id: "call-2".to_string(),
                    output: FunctionCallOutputPayload::from_text(
                        serde_json::json!({
                            "kind": "shared_note",
                            "id": "team",
                            "operation": "append",
                            "content_chars": 18,
                            "total_content_chars": 18,
                            "line_count": 1,
                            "version": 1,
                            "ok": true
                        })
                        .to_string(),
                    ),
                }),
                RolloutItem::ResponseItem(ResponseItem::FunctionCall {
                    id: None,
                    name: "reflections_read_shared_note".to_string(),
                    namespace: None,
                    arguments: serde_json::json!({
                        "note_id": "team",
                        "start": 1,
                        "stop": 1
                    })
                    .to_string(),
                    call_id: "call-3".to_string(),
                }),
                RolloutItem::ResponseItem(ResponseItem::FunctionCallOutput {
                    call_id: "call-3".to_string(),
                    output: FunctionCallOutputPayload::from_text(
                        serde_json::json!({
                            "kind": "shared_note",
                            "id": "team",
                            "content": "SECRET READ BODY",
                            "content_chars": 16,
                            "range": {
                                "start": 1,
                                "stop": 1,
                                "total": 1
                            }
                        })
                        .to_string(),
                    ),
                }),
            ],
        )
        .await?;
        write_window(
            &sidecar,
            &rollout,
            CompactionTrigger::Manual,
            Some(9500),
            1,
            6,
            String::new(),
        )
        .await?;

        let read = read_log(&sidecar, &rollout, "cw00000", Some(1), Some(50))
            .await
            .expect("read log");
        assert_eq!(read.entries.len(), 6);
        assert!(read.entries[0].content.contains("\"content_chars\": 16"));
        assert!(!read.entries[0].content.contains("SECRET NOTE BODY"));
        assert!(!read.entries[1].content.contains("SECRET NOTE BODY"));
        assert!(!read.entries[2].content.contains("SECRET SHARED BODY"));
        assert!(!read.entries[3].content.contains("SECRET SHARED BODY"));
        assert!(!read.entries[5].content.contains("SECRET READ BODY"));
        Ok(())
    }

    async fn write_rollout(path: &std::path::Path, items: &[RolloutItem]) -> std::io::Result<()> {
        let mut contents = String::new();
        for item in items {
            let line = RolloutLine {
                timestamp: "2026-04-16T00:00:00.000Z".to_string(),
                item: item.clone(),
            };
            contents.push_str(&serde_json::to_string(&line)?);
            contents.push('\n');
        }
        tokio::fs::write(path, contents).await
    }
}

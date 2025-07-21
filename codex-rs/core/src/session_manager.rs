use std::cmp::Reverse;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fs;
use std::io::BufRead;
use std::io::{self};
use std::path::Path;
use std::path::PathBuf;

use serde_json::Value;
use time::OffsetDateTime;
use time::PrimitiveDateTime;
use time::format_description::FormatItem;
use time::macros::format_description;
use uuid::Uuid;

use crate::config::Config;

pub(crate) const SESSIONS_SUBDIR: &str = "sessions";

/// Mode for session listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionsMode {
    Full,
    Lite,
}

/// Returned page of sessions.
#[derive(Debug)]
pub struct SessionsPage {
    pub sessions: Vec<Value>,
    pub next_page_token: Option<String>,
    pub scanned_files: usize,
    pub reached_scan_cap: bool,
}

const MAX_SCAN_FILES: usize = 50_000; // Hard cap to bound worst‑case work per request.

/// Pagination token format: "<file_ts>|<uuid>" where `file_ts` matches the
/// filename timestamp portion (YYYY-MM-DDThh-mm-ss) used in rollout filenames.
fn parse_page_token(token: &str) -> Option<(OffsetDateTime, Uuid)> {
    let (file_ts, uuid_str) = token.split_once('|')?;
    let Ok(uuid) = Uuid::parse_str(uuid_str) else {
        return None;
    };
    let format: &[FormatItem] =
        format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
    let ts = PrimitiveDateTime::parse(file_ts, format).ok()?.assume_utc();
    Some((ts, uuid))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DescendDecision {
    /// Current element is newer than upper bound; continue scanning siblings.
    SkipNewer,
    /// Current element is older than lower bound; remaining (older) elements can be skipped entirely.
    StopOlder,
    Include,
}

#[derive(Debug, Clone, Copy)]
struct Interval {
    start: Option<OffsetDateTime>,
    end: Option<OffsetDateTime>,
}
impl Interval {
    fn new(start: Option<OffsetDateTime>, end: Option<OffsetDateTime>) -> Self {
        Self { start, end }
    }

    fn year(&self, year: i32) -> DescendDecision {
        if let Some(end) = self.end {
            if year > end.year() {
                return DescendDecision::SkipNewer;
            }
        }
        if let Some(start) = self.start {
            if year < start.year() {
                return DescendDecision::StopOlder;
            }
        }
        DescendDecision::Include
    }
    fn month(&self, year: i32, month: u8) -> DescendDecision {
        if let Some(end) = self.end {
            if year == end.year() && month > u8::from(end.month()) {
                return DescendDecision::SkipNewer;
            }
        }
        if let Some(start) = self.start {
            if year == start.year() && month < u8::from(start.month()) {
                return DescendDecision::StopOlder;
            }
        }
        DescendDecision::Include
    }
    fn day(&self, year: i32, month: u8, day: u8) -> DescendDecision {
        if let Some(end) = self.end {
            if year == end.year() && month == u8::from(end.month()) && day > end.day() {
                return DescendDecision::SkipNewer;
            }
        }
        if let Some(start) = self.start {
            if year == start.year() && month == u8::from(start.month()) && day < start.day() {
                return DescendDecision::StopOlder;
            }
        }
        DescendDecision::Include
    }
    fn timestamp(&self, ts: OffsetDateTime) -> DescendDecision {
        if let Some(end) = self.end {
            if ts > end {
                return DescendDecision::SkipNewer;
            }
        }
        if let Some(start) = self.start {
            if ts < start {
                return DescendDecision::StopOlder;
            }
        }
        DescendDecision::Include
    }
}

/// Retrieve recorded sessions with filtering + token pagination. The returned `next_page_token`
/// can be supplied on the next call to resume after the last returned session, resilient to
/// concurrent new sessions being appended.
pub async fn get_sessions(
    config: &Config,
    mode: SessionsMode,
    page_size: usize,
    page_token: Option<&str>,
    start: Option<OffsetDateTime>,
    end: Option<OffsetDateTime>,
    filter_ids: Option<&[Uuid]>,
) -> io::Result<SessionsPage> {
    if page_size == 0 {
        return Ok(SessionsPage {
            sessions: Vec::new(),
            next_page_token: None,
            scanned_files: 0,
            reached_scan_cap: false,
        });
    }

    let ids_set: Option<HashSet<Uuid>> = filter_ids.map(|ids| ids.iter().cloned().collect());
    let mut root = config.codex_home.clone();
    root.push(SESSIONS_SUBDIR);
    if !root.exists() {
        return Ok(SessionsPage {
            sessions: Vec::new(),
            next_page_token: None,
            scanned_files: 0,
            reached_scan_cap: false,
        });
    }

    let anchor = page_token.and_then(parse_page_token);

    let result = tokio::task::spawn_blocking({
        let root = root.clone();
        move || traverse_directories(root, mode, page_size, anchor, start, end, ids_set)
    })
    .await
    .map_err(|e| io::Error::other(format!("join error: {e}")))??;
    Ok(result)
}

/// Load sessions from disk using directory traversal.
///
/// Directory layout: `~/.codex/sessions/YYYY/MM/DD/rollout-YYYY-MM-DDThh-mm-ss-<uuid>.jsonl`
/// The first JSONL line is a `SessionMeta` object; subsequent lines are response/state records.
///
/// Returned structure (latest (newest) first):
fn traverse_directories(
    root: PathBuf,
    mode: SessionsMode,
    page_size: usize,
    anchor: Option<(OffsetDateTime, Uuid)>,
    start: Option<OffsetDateTime>,
    end: Option<OffsetDateTime>,
    ids_set: Option<HashSet<Uuid>>,
) -> io::Result<SessionsPage> {
    let mut sessions = Vec::with_capacity(page_size);
    let mut scanned = 0usize;
    let mut after_anchor = anchor.is_none();
    let (anchor_ts, anchor_id) =
        anchor.unwrap_or_else(|| (OffsetDateTime::UNIX_EPOCH, Uuid::nil()));
    let interval = Interval::new(start, end);

    let year_dirs = collect_dirs_desc(&root, |s| s.parse::<i32>().ok())?;

    'outer: for (year, year_path) in year_dirs.iter() {
        if scanned >= MAX_SCAN_FILES {
            break;
        }
        match interval.year(*year) {
            DescendDecision::SkipNewer => continue,
            DescendDecision::StopOlder => break,
            DescendDecision::Include => {}
        }
        let month_dirs = collect_dirs_desc(year_path, |s| s.parse::<u8>().ok())?;
        for (month, month_path) in month_dirs.iter() {
            if scanned >= MAX_SCAN_FILES {
                break 'outer;
            }
            match interval.month(*year, *month) {
                DescendDecision::SkipNewer => continue,
                DescendDecision::StopOlder => break, // older months exhausted for this year
                DescendDecision::Include => {}
            }
            let day_dirs = collect_dirs_desc(month_path, |s| s.parse::<u8>().ok())?;
            for (day, day_path) in day_dirs.iter() {
                if scanned >= MAX_SCAN_FILES {
                    break 'outer;
                }
                match interval.day(*year, *month, *day) {
                    DescendDecision::SkipNewer => continue,
                    DescendDecision::StopOlder => break, // older days exhausted for this month
                    DescendDecision::Include => {}
                }
                let mut files = collect_files(day_path, |name_str, path| {
                    if !name_str.starts_with("rollout-") || !name_str.ends_with(".jsonl") {
                        return None;
                    }
                    parse_timestamp_uuid_from_filename(name_str)
                        .map(|(ts, id)| (ts, id, name_str.to_string(), path.to_path_buf()))
                })?;
                files.sort_by_key(|(ts, _, _, _)| Reverse(*ts));
                for (ts, sid, _name_str, path) in files.into_iter() {
                    scanned += 1;
                    if scanned >= MAX_SCAN_FILES && sessions.len() >= page_size {
                        break 'outer;
                    }
                    if !after_anchor {
                        if ts < anchor_ts || (ts == anchor_ts && sid < anchor_id) {
                            after_anchor = true;
                        } else {
                            continue;
                        }
                    }
                    match interval.timestamp(ts) {
                        DescendDecision::SkipNewer => continue,
                        DescendDecision::StopOlder => break 'outer,
                        DescendDecision::Include => {}
                    }
                    if let Some(ref ids) = ids_set {
                        if !ids.contains(&sid) {
                            continue;
                        }
                    }
                    if sessions.len() == page_size {
                        break 'outer;
                    }
                    match load_single_session(&path, mode) {
                        Ok(value) => sessions.push(value),
                        Err(_) => continue,
                    }
                }
            }
        }
    }
    // Compute next page token if we returned exactly `page_size` sessions –
    // in that case there *may* be more sessions after the last one we just
    // returned. We encode the token as "<timestamp>|<uuid>", matching the
    // `parse_page_token` format used for the incoming anchor argument.
    let next = if sessions.len() == page_size {
        sessions.last().and_then(|v| {
            if let Value::Array(arr) = v {
                if arr.len() >= 2 {
                    let ts = arr[0].as_str()?;
                    let id = arr[1].as_str()?;
                    Some(format!("{ts}|{id}"))
                } else {
                    None
                }
            } else {
                None
            }
        })
    } else {
        None
    };
    Ok(SessionsPage {
        sessions,
        next_page_token: next,
        scanned_files: scanned,
        reached_scan_cap: scanned >= MAX_SCAN_FILES,
    })
}

// Helper: collect immediate subdirectories of `parent`, parse their (string) names with `parse`,
// and return them sorted descending by the parsed key.
fn collect_dirs_desc<T, F>(parent: &Path, parse: F) -> io::Result<Vec<(T, PathBuf)>>
where
    T: Ord + Copy,
    F: Fn(&str) -> Option<T>,
{
    let mut vec: Vec<(T, PathBuf)> = fs::read_dir(parent)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name();
            let s = name.to_str()?;
            parse(s).map(|v| (v, e.path()))
        })
        .collect();
    vec.sort_by_key(|(v, _)| Reverse(*v));
    Ok(vec)
}

// Helper: collect files in `parent`, parse with `parse(name_str, path)` into arbitrary value.
fn collect_files<T, F>(parent: &Path, parse: F) -> io::Result<Vec<T>>
where
    F: Fn(&str, &Path) -> Option<T>,
{
    let vec: Vec<T> = fs::read_dir(parent)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name();
            let s = name.to_str()?;
            parse(s, &e.path())
        })
        .collect();
    Ok(vec)
}

fn parse_timestamp_uuid_from_filename(name: &str) -> Option<(OffsetDateTime, Uuid)> {
    // Format: rollout-YYYY-MM-DDThh-mm-ss-<uuid>.jsonl
    let core = name.strip_prefix("rollout-")?.strip_suffix(".jsonl")?;
    if core.len() < 37 {
        return None;
    } // need at least dt + '-' + 36
    let uuid_part = &core[core.len() - 36..];
    let dt_part = &core[..core.len() - 37]; // strip trailing '-' before uuid
    let Ok(uuid) = Uuid::parse_str(uuid_part) else {
        return None;
    };
    let format: &[FormatItem] =
        format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
    let ts = PrimitiveDateTime::parse(dt_part, format).ok()?.assume_utc();
    Some((ts, uuid))
}

fn load_single_session(path: &Path, mode: SessionsMode) -> io::Result<Value> {
    let file = fs::File::open(path)?;
    let mut reader = io::BufReader::new(file);
    let mut first_line = String::new();
    reader.read_line(&mut first_line)?;
    if first_line.trim().is_empty() {
        return Err(io::Error::other("empty session file"));
    }
    let meta: serde_json::Value = serde_json::from_str(&first_line)
        .map_err(|e| io::Error::other(format!("failed to parse session meta: {e}")))?;
    let timestamp = meta
        .get("timestamp")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let session_id = meta
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    match mode {
        SessionsMode::Full => {
            let mut records: Vec<Value> = Vec::new();
            // Read remaining lines one by one to avoid loading entire file into a single string.
            for line_res in reader.lines() {
                let line = match line_res {
                    Ok(l) => l,
                    Err(_) => continue,
                };
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
                    records.push(v);
                }
            }
            Ok(Value::Array(vec![
                Value::String(timestamp),
                Value::String(session_id),
                Value::Array(records),
            ]))
        }
        SessionsMode::Lite => {
            const HEAD: usize = 5;
            const TAIL: usize = 5;
            let mut head: Vec<Value> = Vec::with_capacity(HEAD);
            let mut tail: VecDeque<Value> = VecDeque::with_capacity(TAIL);
            let mut last_state: Option<Value> = None;
            for line_res in reader.lines() {
                let line = match line_res {
                    Ok(l) => l,
                    Err(_) => continue,
                };
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let Ok(v) = serde_json::from_str::<Value>(trimmed) else {
                    continue;
                };
                // Track only the most recent state line.
                if v.get("record_type").and_then(|t| t.as_str()) == Some("state") {
                    last_state = Some(v);
                    continue;
                }
                if head.len() < HEAD {
                    head.push(v);
                } else {
                    if tail.len() == TAIL {
                        tail.pop_front();
                    }
                    tail.push_back(v);
                }
            }
            let mut records: Vec<Value> = head;
            for v in tail {
                records.push(v);
            }
            if let Some(state) = last_state {
                records.push(state);
            }
            Ok(Value::Array(vec![
                Value::String(timestamp),
                Value::String(session_id),
                Value::Array(records),
            ]))
        }
    }
}

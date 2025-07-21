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
/// Returned structure (earliest first):
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

    let mut year_dirs: Vec<_> = fs::read_dir(&root)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .filter_map(|e| {
            e.file_name()
                .to_str()
                .and_then(|s| s.parse::<i32>().ok())
                .map(|y| (y, e.path()))
        })
        .collect();
    year_dirs.sort_by_key(|(y, _)| *y);

    'outer: for (year, year_path) in year_dirs {
        if scanned >= MAX_SCAN_FILES {
            break;
        }
        if let Some(start_ts) = start {
            if year < start_ts.year() {
                continue;
            }
        }
        if let Some(end_ts) = end {
            if year > end_ts.year() {
                break;
            }
        }
        let mut month_dirs: Vec<_> = fs::read_dir(&year_path)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
            .filter_map(|e| {
                e.file_name()
                    .to_str()
                    .and_then(|s| s.parse::<u8>().ok())
                    .map(|m| (m, e.path()))
            })
            .collect();
        month_dirs.sort_by_key(|(m, _)| *m);
        for (month, month_path) in month_dirs {
            if scanned >= MAX_SCAN_FILES {
                break 'outer;
            }
            if let Some(start_ts) = start {
                if year == start_ts.year() && month < u8::from(start_ts.month()) {
                    continue;
                }
            }
            if let Some(end_ts) = end {
                if year == end_ts.year() && month > u8::from(end_ts.month()) {
                    break 'outer;
                }
            }
            let mut day_dirs: Vec<_> = fs::read_dir(&month_path)?
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
                .filter_map(|e| {
                    e.file_name()
                        .to_str()
                        .and_then(|s| s.parse::<u8>().ok())
                        .map(|d| (d, e.path()))
                })
                .collect();
            day_dirs.sort_by_key(|(d, _)| *d);
            for (day, day_path) in day_dirs {
                if scanned >= MAX_SCAN_FILES {
                    break 'outer;
                }
                if let Some(start_ts) = start {
                    if year == start_ts.year()
                        && month == u8::from(start_ts.month())
                        && day < start_ts.day()
                    {
                        continue;
                    }
                }
                if let Some(end_ts) = end {
                    if year == end_ts.year()
                        && month == u8::from(end_ts.month())
                        && day > end_ts.day()
                    {
                        break 'outer;
                    }
                }
                let mut files: Vec<_> = fs::read_dir(&day_path)?
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
                    .filter_map(|e| {
                        let name = e.file_name();
                        let name_str = name.to_str()?;
                        if !name_str.starts_with("rollout-") || !name_str.ends_with(".jsonl") {
                            return None;
                        }
                        parse_timestamp_uuid_from_filename(name_str)
                            .map(|(ts, id)| (ts, id, name_str.to_string(), e.path()))
                    })
                    .collect();
                files.sort_by_key(|(ts, _, _, _)| *ts);
                for (ts, sid, _name_str, path) in files {
                    scanned += 1;
                    if scanned >= MAX_SCAN_FILES && sessions.len() >= page_size {
                        break 'outer;
                    }
                    // Anchor logic: skip until strictly after (anchor_ts, anchor_id)
                    if !after_anchor {
                        if ts > anchor_ts || (ts == anchor_ts && sid > anchor_id) {
                            after_anchor = true;
                        } else {
                            continue;
                        }
                    }
                    if let Some(start_ts) = start {
                        if ts < start_ts {
                            continue;
                        }
                    }
                    if let Some(end_ts) = end {
                        if ts > end_ts {
                            break 'outer;
                        }
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

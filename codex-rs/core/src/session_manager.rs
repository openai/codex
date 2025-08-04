use std::cmp::Reverse;
use std::fs;
use std::io::{self};
use std::path::Path;
use std::path::PathBuf;

use time::OffsetDateTime;
use time::PrimitiveDateTime;
use time::format_description::FormatItem;
use time::macros::format_description;
use uuid::Uuid;

use crate::config::Config;

pub(crate) const SESSIONS_SUBDIR: &str = "sessions";

/// Returned page of conversation file paths.
#[derive(Debug)]
pub struct ConversationsPage {
    /// Absolute paths to rollout files, ordered newest first.
    pub paths: Vec<PathBuf>,
    /// Opaque pagination token to resume after the last item, or `None` if end.
    pub next_cursor: Option<String>,
    /// Total number of files touched while scanning this request.
    pub scanned_files: usize,
    /// True if a hard scan cap was hit; consider resuming with `next_cursor`.
    pub reached_scan_cap: bool,
}

const MAX_SCAN_FILES: usize = 50_000; // Hard cap to bound worst‑case work per request.

/// Retrieve recorded conversation file paths with token pagination. The returned `next_cursor`
/// can be supplied on the next call to resume after the last returned item, resilient to
/// concurrent new sessions being appended. Ordering is stable by timestamp desc, then UUID desc.
pub async fn get_conversations(
    config: &Config,
    page_size: usize,
    cursor: Option<&str>,
) -> io::Result<ConversationsPage> {
    let mut root = config.codex_home.clone();
    root.push(SESSIONS_SUBDIR);
    if !root.exists() {
        return Ok(ConversationsPage {
            paths: Vec::new(),
            next_cursor: None,
            scanned_files: 0,
            reached_scan_cap: false,
        });
    }

    let anchor = cursor.and_then(parse_cursor);

    let result = tokio::task::spawn_blocking({
        let root = root.clone();
        move || traverse_directories_for_paths(root, page_size, anchor)
    })
    .await
    .map_err(|e| io::Error::other(format!("join error: {e}")))??;
    Ok(result)
}

/// Load the full contents of a single conversation session file at `path`.
/// Returns the entire file contents as a String.
pub async fn get_conversation(path: &Path) -> io::Result<String> {
    tokio::fs::read_to_string(path).await
}

/// Load conversation file paths from disk using directory traversal.
///
/// Directory layout: `~/.codex/sessions/YYYY/MM/DD/rollout-YYYY-MM-DDThh-mm-ss-<uuid>.jsonl`
/// Returned newest (latest) first.
fn traverse_directories_for_paths(
    root: PathBuf,
    page_size: usize,
    anchor: Option<(OffsetDateTime, Uuid)>,
) -> io::Result<ConversationsPage> {
    let mut paths = Vec::with_capacity(page_size);
    let mut scanned_files = 0usize;
    let mut anchor_passed = anchor.is_none();
    let (anchor_ts, anchor_id) =
        anchor.unwrap_or_else(|| (OffsetDateTime::UNIX_EPOCH, Uuid::nil()));

    let year_dirs = collect_dirs_desc(&root, |s| s.parse::<u16>().ok())?;

    'outer: for (_year, year_path) in year_dirs.iter() {
        if scanned_files >= MAX_SCAN_FILES {
            break;
        }
        let month_dirs = collect_dirs_desc(year_path, |s| s.parse::<u8>().ok())?;
        for (_month, month_path) in month_dirs.iter() {
            if scanned_files >= MAX_SCAN_FILES {
                break 'outer;
            }
            let day_dirs = collect_dirs_desc(month_path, |s| s.parse::<u8>().ok())?;
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
                })?;
                // Stable ordering within the same second: (timestamp desc, uuid desc)
                day_files.sort_by_key(|(ts, sid, _name_str, _path)| (Reverse(*ts), Reverse(*sid)));
                for (ts, sid, _name_str, path) in day_files.into_iter() {
                    scanned_files += 1;
                    if scanned_files >= MAX_SCAN_FILES && paths.len() >= page_size {
                        break 'outer;
                    }
                    if !anchor_passed {
                        if ts < anchor_ts || (ts == anchor_ts && sid < anchor_id) {
                            anchor_passed = true;
                        } else {
                            continue;
                        }
                    }
                    if paths.len() == page_size {
                        break 'outer;
                    }
                    paths.push(path);
                }
            }
        }
    }

    let next = build_next_cursor(&paths);
    Ok(ConversationsPage {
        paths,
        next_cursor: next,
        scanned_files,
        reached_scan_cap: scanned_files >= MAX_SCAN_FILES,
    })
}

/// Pagination cursor token format: "<file_ts>|<uuid>" where `file_ts` matches the
/// filename timestamp portion (YYYY-MM-DDThh-mm-ss) used in rollout filenames.
/// The cursor orders files by timestamp desc, then UUID desc.
fn parse_cursor(token: &str) -> Option<(OffsetDateTime, Uuid)> {
    let (file_ts, uuid_str) = token.split_once('|')?;
    let Ok(uuid) = Uuid::parse_str(uuid_str) else {
        return None;
    };
    let format: &[FormatItem] =
        format_description!("[year]-[month]-[day]T[hour]-[minute]-[second]");
    let ts = PrimitiveDateTime::parse(file_ts, format).ok()?.assume_utc();
    Some((ts, uuid))
}

fn build_next_cursor(paths: &[PathBuf]) -> Option<String> {
    let last = paths.last()?;
    let file_name = last.file_name()?.to_string_lossy();
    let (ts, id) = parse_timestamp_uuid_from_filename(&file_name)?;
    Some(format!(
        "{}|{}",
        ts.format(&format_description!(
            "[year]-[month]-[day]T[hour]-[minute]-[second]"
        ))
        .ok()?,
        id
    ))
}

/// Collects immediate subdirectories of `parent`, parses their (string) names with `parse`,
/// and returns them sorted descending by the parsed key.
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

// Collects files in a directory and parses them with `parse`.
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

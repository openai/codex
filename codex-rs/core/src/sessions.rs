use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use time::OffsetDateTime;
use time::UtcOffset;

use serde_json::Value;
use uuid::Uuid;

use crate::config::Config;

#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub id: Uuid,
    pub created: String,
    pub last_active: Option<String>,
    pub title: Option<String>,
    pub path: PathBuf,
}

/// Return the root directory where sessions are stored ("~/.codex/sessions").
pub fn sessions_root(config: &Config) -> PathBuf {
    let mut p = config.codex_home.clone();
    p.push("sessions");
    p
}

/// List saved sessions discovered under the configured Codex home.
///
/// Scans recursively for files named like `rollout-*.jsonl` and extracts:
/// - id and created timestamp from the first JSON line
/// - a human-friendly title from the first user message if present
/// - last active from file modification time
pub fn list_sessions(config: &Config) -> std::io::Result<Vec<SessionEntry>> {
    let root = sessions_root(config);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut entries: Vec<SessionEntry> = Vec::new();
    collect_sessions_recursively(&root, &mut entries)?;
    // Sort newest-first by last_active, then by created.
    entries.sort_by(|a, b| {
        b.last_active
            .cmp(&a.last_active)
            .then(b.created.cmp(&a.created))
    });
    Ok(entries)
}

fn collect_sessions_recursively(dir: &Path, out: &mut Vec<SessionEntry>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let ft = entry.file_type()?;
        if ft.is_dir() {
            collect_sessions_recursively(&path, out)?;
            continue;
        }
        if ft.is_file()
            && path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("rollout-") && n.ends_with(".jsonl"))
                .unwrap_or(false)
            && let Ok(e) = parse_session_file(&path)
        {
            out.push(e);
        }
    }
    Ok(())
}

fn parse_session_file(path: &Path) -> std::io::Result<SessionEntry> {
    let file = fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut first_line = String::new();
    reader.read_line(&mut first_line)?;
    if first_line.trim().is_empty() {
        return Err(std::io::Error::other("empty session file"));
    }
    let v: Value = serde_json::from_str(&first_line)
        .map_err(|e| std::io::Error::other(format!("failed to parse meta: {e}")))?;
    let id = v
        .get("id")
        .and_then(|x| x.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| std::io::Error::other("missing id in session meta"))?;
    let created = v
        .get("timestamp")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();

    // Derive title from the first user message text, if present.
    let title = derive_first_user_message_title(reader).ok();

    // File mtime as last_active.
    let last_active = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| human_time(t).ok());

    Ok(SessionEntry {
        id,
        created,
        last_active,
        title,
        path: path.to_path_buf(),
    })
}

fn derive_first_user_message_title<R: BufRead>(mut reader: R) -> Result<String, ()> {
    let mut line = String::new();
    // Scan a limited number of lines to avoid heavy I/O on very long sessions.
    let mut scanned = 0usize;
    while scanned < 256 {
        line.clear();
        if reader.read_line(&mut line).map_err(|_| ())? == 0 {
            break;
        }
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let role = v.get("role").and_then(|x| x.as_str()).unwrap_or("");
        if role == "user" {
            // Find text from the first OutputText content item.
            if let Some(text) = v.get("content").and_then(|c| c.as_array()).and_then(|arr| {
                arr.iter()
                    .find_map(|item| item.get("text").and_then(|t| t.as_str()))
            }) {
                let trimmed = text.trim();
                let short: String = trimmed.chars().take(80).collect();
                let short = if trimmed.chars().count() > 80 {
                    format!("{short}…")
                } else {
                    short
                };
                return Ok(short);
            }
        }
        scanned += 1;
    }
    Err(())
}

fn human_time(t: SystemTime) -> std::io::Result<String> {
    let dur = t
        .duration_since(UNIX_EPOCH)
        .map_err(|e| std::io::Error::other(format!("{e}")))?;
    let dt = OffsetDateTime::from_unix_timestamp(dur.as_secs() as i64)
        .map_err(|e| std::io::Error::other(format!("{e}")))?;
    let local = match UtcOffset::current_local_offset() {
        Ok(off) => dt.to_offset(off),
        Err(_) => dt,
    };
    local
        .format(&time::macros::format_description!(
            "[year]-[month]-[day] [hour]:[minute]"
        ))
        .map_err(|e| std::io::Error::other(format!("{e}")))
}

/// Find the session entry by UUID.
pub fn find_by_id(config: &Config, id: Uuid) -> std::io::Result<Option<SessionEntry>> {
    let sessions = list_sessions(config)?;
    Ok(sessions.into_iter().find(|e| e.id == id))
}

/// Return the most recent session, if any.
pub fn latest(config: &Config) -> std::io::Result<Option<SessionEntry>> {
    let mut sessions = list_sessions(config)?;
    sessions.sort_by(|a, b| {
        b.last_active
            .cmp(&a.last_active)
            .then(b.created.cmp(&a.created))
    });
    Ok(sessions.into_iter().next())
}

use crate::ExternalAgentSessionMigration;
use crate::ledger::load_import_ledger;
use crate::ledger::save_import_ledger;
use crate::now_unix_seconds;
use crate::records::summarize_session_with_cwd;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

const SESSION_IMPORT_MAX_COUNT: usize = 50;
const SESSION_IMPORT_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

pub fn detect_recent_cu_sessions(
    external_agent_home: &Path,
    codex_home: &Path,
) -> io::Result<Vec<ExternalAgentSessionMigration>> {
    let projects_root = external_agent_home.join("projects");
    if !projects_root.is_dir() {
        return Ok(Vec::new());
    }

    let now = now_unix_seconds();
    let mut ledger = load_import_ledger(codex_home)?;
    let source_states = ledger.source_states();
    let mut candidates = BinaryHeap::with_capacity(SESSION_IMPORT_MAX_COUNT + 1);
    for project_entry in fs::read_dir(projects_root)? {
        let Ok(project_entry) = project_entry else {
            continue;
        };
        let project_storage = project_entry.path();
        if !project_storage.is_dir() {
            continue;
        }
        let Some(cwd) = cursor_project_cwd(&project_storage) else {
            continue;
        };
        for path in cursor_transcript_files(&project_storage.join("agent-transcripts")) {
            let Ok(metadata) = fs::metadata(&path) else {
                continue;
            };
            let Ok(modified_at) = metadata.modified() else {
                continue;
            };
            let Ok(modified_at) = modified_at.duration_since(std::time::UNIX_EPOCH) else {
                continue;
            };
            if (modified_at.as_secs() as i64)
                < now.saturating_sub(SESSION_IMPORT_MAX_AGE.as_secs() as i64)
            {
                continue;
            }
            let Ok(modified_at_nanos) = i64::try_from(modified_at.as_nanos()) else {
                continue;
            };
            let Ok(source_path) = fs::canonicalize(&path) else {
                continue;
            };
            if let Some(state) = source_states.get(source_path.as_path())
                && (state.source_modified_at == Some(modified_at_nanos)
                    || state.source_modified_at.is_none()
                        && modified_at.as_secs() as i64 <= state.imported_at)
            {
                continue;
            }
            candidates.push((Reverse(modified_at_nanos), path, cwd.clone()));
            if candidates.len() > SESSION_IMPORT_MAX_COUNT {
                candidates.pop();
            }
        }
    }

    drop(source_states);
    let mut migrations = Vec::new();
    let mut ledger_changed = false;
    for (modified_at, path, cwd) in candidates.into_sorted_vec() {
        match ledger.refresh_current_source(&path, modified_at.0) {
            Ok(false) => {}
            Ok(true) => {
                ledger_changed = true;
                continue;
            }
            Err(_) => continue,
        }
        let Ok(Some(summary)) = summarize_session_with_cwd(&path, Some(&cwd)) else {
            continue;
        };
        migrations.push(summary.migration);
    }
    if ledger_changed {
        save_import_ledger(codex_home, &ledger)?;
    }
    Ok(migrations)
}

fn cursor_transcript_files(transcripts_root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![transcripts_root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                if entry.file_name() != "subagents" {
                    pending.push(path);
                }
            } else if file_type.is_file()
                && path.extension().and_then(|extension| extension.to_str()) == Some("jsonl")
            {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn cursor_project_cwd(project_storage: &Path) -> Option<PathBuf> {
    let encoded = project_storage.file_name()?.to_str()?;
    decode_cursor_project_path(encoded)
}

#[cfg(not(windows))]
fn decode_cursor_project_path(encoded: &str) -> Option<PathBuf> {
    decode_cursor_project_path_from(encoded, Path::new("/"), /*depth*/ 0).or_else(|| {
        encoded.strip_prefix('-').and_then(|encoded| {
            decode_cursor_project_path_from(encoded, Path::new("/"), /*depth*/ 0)
        })
    })
}

#[cfg(windows)]
fn decode_cursor_project_path(encoded: &str) -> Option<PathBuf> {
    let (drive, remaining) = encoded.split_once('-')?;
    if drive.len() != 1 || !drive.as_bytes()[0].is_ascii_alphabetic() {
        return None;
    }
    let base = PathBuf::from(format!("{drive}:\\"));
    decode_cursor_project_path_from(remaining, &base, /*depth*/ 0).or_else(|| {
        remaining.strip_prefix('-').and_then(|remaining| {
            decode_cursor_project_path_from(remaining, &base, /*depth*/ 0)
        })
    })
}

fn decode_cursor_project_path_from(encoded: &str, base: &Path, depth: usize) -> Option<PathBuf> {
    if encoded.is_empty() || depth > 32 {
        return None;
    }
    let whole = base.join(encoded);
    if whole.is_dir() {
        return Some(whole);
    }
    for (index, _) in encoded.match_indices('-') {
        let segment = &encoded[..index];
        if segment.is_empty() {
            continue;
        }
        let candidate = base.join(segment);
        if !candidate.is_dir() {
            continue;
        }
        if let Some(path) =
            decode_cursor_project_path_from(&encoded[index + 1..], &candidate, depth + 1)
        {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
#[path = "detect_cu_tests.rs"]
mod tests;

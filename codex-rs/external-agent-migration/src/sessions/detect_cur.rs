use super::ExternalAgentSessionMigration;
use super::ledger::load_import_ledger;
use super::ledger::save_import_ledger;
use super::now_unix_seconds;
use super::records::summarize_session_with_cwd;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

const SESSION_IMPORT_MAX_COUNT: usize = 50;
const SESSION_IMPORT_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

pub fn detect_recent_cur_sessions(
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
        let fallback_cwd = cur_project_cwd(&project_storage);
        for path in cur_transcript_files(&project_storage.join("agent-transcripts")) {
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
            candidates.push((Reverse(modified_at_nanos), path, fallback_cwd.clone()));
            if candidates.len() > SESSION_IMPORT_MAX_COUNT {
                candidates.pop();
            }
        }
    }

    drop(source_states);
    let mut migrations = Vec::new();
    let mut ledger_changed = false;
    for (modified_at, path, fallback_cwd) in candidates.into_sorted_vec() {
        match ledger.refresh_current_source(&path, modified_at.0) {
            Ok(false) => {}
            Ok(true) => {
                ledger_changed = true;
                continue;
            }
            Err(_) => continue,
        }
        let Ok(Some(summary)) = summarize_session_with_cwd(&path, fallback_cwd.as_deref()) else {
            continue;
        };
        migrations.push(summary.migration);
    }
    if ledger_changed {
        save_import_ledger(codex_home, &ledger)?;
    }
    Ok(migrations)
}

fn cur_transcript_files(transcripts_root: &Path) -> Vec<PathBuf> {
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

fn cur_project_cwd(project_storage: &Path) -> Option<PathBuf> {
    let encoded = project_storage.file_name()?.to_str()?;
    decode_cur_project_path(encoded)
}

#[cfg(not(windows))]
fn decode_cur_project_path(encoded: &str) -> Option<PathBuf> {
    let root = Path::new("/");
    let mut matches = Vec::new();
    collect_cur_project_paths(encoded, root, root, /*depth*/ 0, &mut matches);
    if let Some(encoded) = encoded.strip_prefix('-') {
        collect_cur_project_paths(encoded, root, root, /*depth*/ 0, &mut matches);
    }
    unique_path(matches)
}

#[cfg(windows)]
fn decode_cur_project_path(encoded: &str) -> Option<PathBuf> {
    let drive = encoded.as_bytes().first().copied()?;
    if !drive.is_ascii_alphabetic() || encoded.as_bytes().get(1) != Some(&b'-') {
        return None;
    }
    let encoded = encoded.get(2..)?;
    let base = PathBuf::from(format!("{}:\\", char::from(drive)));
    let mut matches = Vec::new();
    collect_cur_project_paths(encoded, &base, &base, /*depth*/ 0, &mut matches);
    unique_path(matches)
}

fn collect_cur_project_paths(
    encoded: &str,
    base: &Path,
    root: &Path,
    depth: usize,
    matches: &mut Vec<PathBuf>,
) {
    if encoded.is_empty() || depth > 32 || matches.len() > 1 {
        return;
    }
    let Ok(entries) = fs::read_dir(base) else {
        return;
    };
    for entry in entries.flatten() {
        if matches.len() > 1 {
            break;
        }
        let candidate = entry.path();
        if !candidate.is_dir() {
            continue;
        }
        let Ok(candidate_from_root) = candidate.strip_prefix(root) else {
            continue;
        };
        let candidate_slug = cur_project_path_slug(candidate_from_root);
        if candidate_slug == encoded {
            if !matches.contains(&candidate) {
                matches.push(candidate);
            }
        } else if encoded
            .strip_prefix(&candidate_slug)
            .is_some_and(|remaining| remaining.starts_with('-'))
        {
            collect_cur_project_paths(encoded, &candidate, root, depth + 1, matches);
        }
    }
}

fn cur_project_path_slug(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches(['/', '\\'])
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn unique_path(mut matches: Vec<PathBuf>) -> Option<PathBuf> {
    (matches.len() == 1).then(|| matches.swap_remove(0))
}

#[cfg(test)]
#[path = "detect_cur_tests.rs"]
mod tests;

#[cfg(test)]
use super::common::SESSION_IMPORT_MAX_COUNT;
use super::common::SessionFileCandidate;
use super::common::detect_recent_sessions;
use crate::sessions::ExternalAgentSessionMigration;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

pub fn detect_recent_cur_sessions(
    external_agent_home: &Path,
    codex_home: &Path,
) -> io::Result<Vec<ExternalAgentSessionMigration>> {
    let projects_root = external_agent_home.join("projects");
    if !projects_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut candidates = Vec::new();
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
            candidates.push(SessionFileCandidate {
                path,
                fallback_cwd: fallback_cwd.clone(),
            });
        }
    }
    detect_recent_sessions(codex_home, candidates, /*require_existing_cwd*/ false)
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
#[path = "cur_tests.rs"]
mod tests;

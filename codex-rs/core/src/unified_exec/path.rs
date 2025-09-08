use std::env;
use std::path::Path;
use std::path::PathBuf;

use super::errors::UnifiedExecError;

pub(crate) fn resolve_command_path(command: &str) -> Result<String, UnifiedExecError> {
    if command.is_empty() {
        return Err(UnifiedExecError::MissingCommandLine);
    }

    if is_explicit_path(command) {
        return ensure_executable(Path::new(command))
            .then_some(command.to_string())
            .ok_or_else(|| UnifiedExecError::CommandNotFound {
                command: command.to_string(),
            });
    }

    if let Some(resolved) = find_in_path(command) {
        return Ok(resolved.to_string_lossy().to_string());
    }

    Err(UnifiedExecError::CommandNotFound {
        command: command.to_string(),
    })
}

fn is_explicit_path(command: &str) -> bool {
    let path = Path::new(command);
    path.is_absolute() || path.components().count() > 1
}

fn find_in_path(command: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    env::split_paths(&path_var)
        .flat_map(|dir| candidate_paths(dir, command))
        .find(|candidate| ensure_executable(candidate))
}

fn candidate_paths(dir: PathBuf, command: &str) -> Vec<PathBuf> {
    build_platform_candidates(dir.join(command))
}

#[cfg(unix)]
fn build_platform_candidates(candidate: PathBuf) -> Vec<PathBuf> {
    vec![candidate]
}

#[cfg(windows)]
fn build_platform_candidates(candidate: PathBuf) -> Vec<PathBuf> {
    if candidate.extension().is_some() {
        return vec![candidate];
    }

    let pathext = env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
    let mut candidates = Vec::new();
    for ext in pathext.split(';') {
        if ext.is_empty() {
            continue;
        }
        let mut path_with_ext = candidate.clone();
        let new_ext = ext.trim_start_matches('.');
        path_with_ext.set_extension(new_ext);
        candidates.push(path_with_ext);
    }
    if candidates.is_empty() {
        candidates.push(candidate);
    }
    candidates
}

fn ensure_executable(path: &Path) -> bool {
    match path.metadata() {
        Ok(metadata) => metadata.is_file() && is_executable(&metadata),
        Err(_) => false,
    }
}

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    metadata.is_file()
}

use std::path::Path;
use std::path::PathBuf;

const SYSTEM_BWRAP_PROGRAM: &str = "bwrap";

pub fn system_bwrap_warning() -> Option<String> {
    system_bwrap_warning_for_lookup(find_system_bwrap_in_path())
}

fn system_bwrap_warning_for_lookup(system_bwrap_path: Option<PathBuf>) -> Option<String> {
    match system_bwrap_path {
        Some(_) => None,
        None => Some(
            "Codex could not find system bubblewrap on PATH. Please install bubblewrap with your package manager. Codex will use the vendored bubblewrap in the meantime."
                .to_string(),
        ),
    }
}

pub fn find_system_bwrap_in_path() -> Option<PathBuf> {
    let search_path = std::env::var_os("PATH")?;
    let cwd = std::env::current_dir().ok()?;
    find_system_bwrap_in_search_paths(std::env::split_paths(&search_path), &cwd)
}

fn find_system_bwrap_in_search_paths(
    search_paths: impl IntoIterator<Item = PathBuf>,
    cwd: &Path,
) -> Option<PathBuf> {
    let search_path = std::env::join_paths(search_paths).ok()?;
    let cwd = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    which::which_in_all(SYSTEM_BWRAP_PROGRAM, Some(search_path), &cwd)
        .ok()?
        .find_map(|path| {
            let path = std::fs::canonicalize(path).ok()?;
            if path.starts_with(&cwd) {
                None
            } else {
                Some(path)
            }
        })
}

#[cfg(test)]
#[path = "bwrap_tests.rs"]
mod tests;

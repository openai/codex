//! Utility functions for TUI rendering.

use std::path::Path;

/// Convert an absolute path to a relative path within the workspace.
/// Returns the original path if it's not within the workspace.
pub fn relative_path(path: &str, workspace: &Path) -> String {
    let workspace_str = workspace.to_str().unwrap_or("");
    path.strip_prefix(workspace_str)
        .unwrap_or(path)
        .trim_start_matches('/')
        .to_string()
}

/// Convert a Path to a relative path within the workspace.
pub fn relative_path_buf(path: &Path, workspace: &Path) -> String {
    path.strip_prefix(workspace)
        .unwrap_or(path)
        .display()
        .to_string()
}

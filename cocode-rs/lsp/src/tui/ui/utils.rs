//! Utility functions for TUI rendering.

use std::path::Path;

/// Convert a path to a relative path within the workspace.
///
/// Accepts any type that can be converted to a Path (string slices, Path references).
/// Returns the original path if it's not within the workspace.
pub fn relative_path<P: AsRef<Path>>(path: P, workspace: &Path) -> String {
    let path = path.as_ref();
    path.strip_prefix(workspace)
        .unwrap_or(path)
        .display()
        .to_string()
}

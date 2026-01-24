//! Plan forking utilities for spawn tasks.
//!
//! Provides functions to read parent agent's plan content for inheritance.

use std::path::Path;

/// Read plan content from a file path.
///
/// Returns `Some(content)` if the file exists, can be read, and is non-empty.
/// Returns `None` if file doesn't exist, can't be read, or contains only whitespace.
///
/// This creates a content snapshot - the returned string is independent
/// of any future modifications to the source file.
pub fn read_plan_content(plan_path: &Path) -> Option<String> {
    if plan_path.exists() {
        std::fs::read_to_string(plan_path)
            .ok()
            .filter(|c| !c.trim().is_empty())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_read_plan_content_exists() {
        let dir = tempdir().unwrap();
        let plan_path = dir.path().join("plan.md");
        let content = "# My Plan\n\n1. Step one\n2. Step two";
        fs::write(&plan_path, content).unwrap();

        let result = read_plan_content(&plan_path);
        assert_eq!(result, Some(content.to_string()));
    }

    #[test]
    fn test_read_plan_content_not_exists() {
        let dir = tempdir().unwrap();
        let plan_path = dir.path().join("nonexistent.md");

        let result = read_plan_content(&plan_path);
        assert!(result.is_none());
    }

    #[test]
    fn test_read_plan_content_empty_file() {
        let dir = tempdir().unwrap();
        let plan_path = dir.path().join("empty.md");
        fs::write(&plan_path, "").unwrap();

        let result = read_plan_content(&plan_path);
        assert!(result.is_none()); // Empty files return None
    }

    #[test]
    fn test_read_plan_content_whitespace_only() {
        let dir = tempdir().unwrap();
        let plan_path = dir.path().join("whitespace.md");
        fs::write(&plan_path, "   \n\t\n  ").unwrap();

        let result = read_plan_content(&plan_path);
        assert!(result.is_none()); // Whitespace-only files return None
    }
}

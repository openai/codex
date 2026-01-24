//! Default ignore patterns for file operations.
//!
//! Provides common patterns for consistent file filtering across tools.

/// Common ignore patterns for version control and dependency directories.
///
/// These directories are typically ignored in development projects.
pub const COMMON_IGNORE_PATTERNS: &[&str] = &[
    "**/node_modules/**",
    "**/.git/**",
    "**/bower_components/**",
    "**/.svn/**",
    "**/.hg/**",
];

/// Binary file extension patterns.
///
/// Only truly binary/compiled files that cannot be meaningfully displayed.
/// NOTE: Office files (.doc, .docx, etc.) are NOT excluded - users may want to reference them.
pub const BINARY_FILE_PATTERNS: &[&str] = &[
    "**/*.bin",
    "**/*.exe",
    "**/*.dll",
    "**/*.so",
    "**/*.dylib",
    "**/*.class",
    "**/*.jar",
    "**/*.war",
    "**/*.zip",
    "**/*.tar",
    "**/*.gz",
    "**/*.bz2",
    "**/*.rar",
    "**/*.7z",
];

/// Common directory patterns typically ignored in development.
///
/// Build outputs, IDE configs, and cache directories.
pub const COMMON_DIRECTORY_EXCLUDES: &[&str] = &[
    "**/.vscode/**",
    "**/.idea/**",
    "**/dist/**",
    "**/build/**",
    "**/coverage/**",
    "**/__pycache__/**",
];

/// System file patterns.
///
/// NOTE: .env is NOT excluded - users should see env files in listings.
/// Content protection is handled separately via .ignore.
pub const SYSTEM_FILE_EXCLUDES: &[&str] = &["**/.DS_Store"];

/// Get all default exclude patterns combined.
///
/// Returns a combined vector of all pattern categories.
pub fn get_all_default_excludes() -> Vec<&'static str> {
    let mut patterns = Vec::with_capacity(
        COMMON_IGNORE_PATTERNS.len()
            + BINARY_FILE_PATTERNS.len()
            + COMMON_DIRECTORY_EXCLUDES.len()
            + SYSTEM_FILE_EXCLUDES.len(),
    );
    patterns.extend(COMMON_IGNORE_PATTERNS);
    patterns.extend(BINARY_FILE_PATTERNS);
    patterns.extend(COMMON_DIRECTORY_EXCLUDES);
    patterns.extend(SYSTEM_FILE_EXCLUDES);
    patterns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_common_patterns_not_empty() {
        assert!(!COMMON_IGNORE_PATTERNS.is_empty());
    }

    #[test]
    fn test_binary_patterns_not_empty() {
        assert!(!BINARY_FILE_PATTERNS.is_empty());
    }

    #[test]
    fn test_get_all_default_excludes() {
        let all = get_all_default_excludes();
        let expected_len = COMMON_IGNORE_PATTERNS.len()
            + BINARY_FILE_PATTERNS.len()
            + COMMON_DIRECTORY_EXCLUDES.len()
            + SYSTEM_FILE_EXCLUDES.len();
        assert_eq!(all.len(), expected_len);
    }
}

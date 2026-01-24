//! Pattern matcher for gitignore-style glob patterns.
//!
//! Provides efficient path matching using compiled glob patterns.

use crate::patterns::BINARY_FILE_PATTERNS;
use crate::patterns::COMMON_DIRECTORY_EXCLUDES;
use crate::patterns::COMMON_IGNORE_PATTERNS;
use crate::patterns::SYSTEM_FILE_EXCLUDES;
use globset::Glob;
use globset::GlobSet;
use globset::GlobSetBuilder;

/// A compiled pattern matcher for efficient path filtering.
///
/// Pre-compiles glob patterns into a single matcher for optimal performance
/// when checking many paths against the same set of patterns.
#[derive(Debug)]
pub struct PatternMatcher {
    glob_set: GlobSet,
}

impl PatternMatcher {
    /// Create a new pattern matcher from a slice of gitignore-style patterns.
    ///
    /// Patterns support:
    /// - `*` - matches any sequence of characters except `/`
    /// - `**` - matches any sequence including `/`
    /// - `?` - matches any single character except `/`
    /// - `[abc]` - matches any character in the set
    /// - `{a,b}` - matches either `a` or `b`
    ///
    /// # Errors
    ///
    /// Returns error if any pattern is invalid.
    pub fn new(patterns: &[&str]) -> Result<Self, globset::Error> {
        let mut builder = GlobSetBuilder::new();
        for pattern in patterns {
            builder.add(Glob::new(pattern)?);
        }
        Ok(Self {
            glob_set: builder.build()?,
        })
    }

    /// Check if the path matches any of the patterns.
    pub fn is_match(&self, path: &str) -> bool {
        self.glob_set.is_match(path)
    }

    /// Create a matcher from all default exclude patterns.
    ///
    /// Combines COMMON_IGNORE_PATTERNS, BINARY_FILE_PATTERNS,
    /// COMMON_DIRECTORY_EXCLUDES, and SYSTEM_FILE_EXCLUDES.
    pub fn default_excludes() -> Result<Self, globset::Error> {
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
        Self::new(&patterns)
    }
}

impl Default for PatternMatcher {
    fn default() -> Self {
        Self {
            glob_set: GlobSet::empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_pattern() {
        let matcher = PatternMatcher::new(&["**/*.exe"]).unwrap();
        assert!(matcher.is_match("/project/target/debug/main.exe"));
        assert!(matcher.is_match("main.exe"));
        assert!(!matcher.is_match("/project/src/main.rs"));
    }

    #[test]
    fn test_directory_pattern() {
        let matcher = PatternMatcher::new(&["**/node_modules/**"]).unwrap();
        assert!(matcher.is_match("/project/node_modules/pkg/index.js"));
        assert!(matcher.is_match("node_modules/pkg/index.js"));
        assert!(!matcher.is_match("/project/src/index.js"));
    }

    #[test]
    fn test_filename_pattern() {
        let matcher = PatternMatcher::new(&["**/.DS_Store"]).unwrap();
        assert!(matcher.is_match("/project/.DS_Store"));
        assert!(matcher.is_match("/project/src/.DS_Store"));
        assert!(!matcher.is_match("/project/.DS_Store_backup"));
    }

    #[test]
    fn test_git_directory() {
        let matcher = PatternMatcher::new(&["**/.git/**"]).unwrap();
        assert!(matcher.is_match("/project/.git/config"));
        assert!(matcher.is_match(".git/HEAD"));
        // .gitignore should NOT match
        assert!(!matcher.is_match("/project/.gitignore"));
    }

    #[test]
    fn test_multiple_patterns() {
        let matcher = PatternMatcher::new(&["**/*.exe", "**/*.dll", "**/node_modules/**"]).unwrap();
        assert!(matcher.is_match("/project/main.exe"));
        assert!(matcher.is_match("/project/lib.dll"));
        assert!(matcher.is_match("/project/node_modules/pkg/index.js"));
        assert!(!matcher.is_match("/project/src/main.rs"));
    }

    #[test]
    fn test_default_excludes() {
        let matcher = PatternMatcher::default_excludes().unwrap();

        // node_modules
        assert!(matcher.is_match("/project/node_modules/pkg/index.js"));

        // .git
        assert!(matcher.is_match("/project/.git/config"));

        // binary extensions
        assert!(matcher.is_match("/project/main.exe"));
        assert!(matcher.is_match("/project/lib.so"));
        assert!(matcher.is_match("/project/lib.dll"));

        // build directories
        assert!(matcher.is_match("/project/dist/bundle.js"));
        assert!(matcher.is_match("/project/build/output.js"));
        assert!(matcher.is_match("/project/coverage/lcov.info"));

        // IDE directories
        assert!(matcher.is_match("/project/.vscode/settings.json"));
        assert!(matcher.is_match("/project/.idea/workspace.xml"));

        // system files
        assert!(matcher.is_match("/project/.DS_Store"));

        // Should NOT match
        assert!(!matcher.is_match("/project/src/main.rs"));
        assert!(!matcher.is_match("/project/.gitignore"));
        assert!(!matcher.is_match("/project/package.json"));
    }

    #[test]
    fn test_empty_matcher() {
        let matcher = PatternMatcher::default();
        assert!(!matcher.is_match("/any/path.txt"));
    }

    #[test]
    fn test_edge_case_distribute_vs_dist() {
        let matcher = PatternMatcher::new(&["**/dist/**"]).unwrap();
        // Should match dist directory
        assert!(matcher.is_match("/project/dist/bundle.js"));
        // Should NOT match distribute directory (this was a bug in the old implementation)
        assert!(!matcher.is_match("/project/distribute/file.js"));
    }

    #[test]
    fn test_archive_patterns() {
        let matcher = PatternMatcher::default_excludes().unwrap();
        assert!(matcher.is_match("/project/archive.zip"));
        assert!(matcher.is_match("/project/data.tar"));
        assert!(matcher.is_match("/project/compressed.gz"));
        assert!(matcher.is_match("/project/backup.rar"));
        assert!(matcher.is_match("/project/files.7z"));
    }

    #[test]
    fn test_python_cache() {
        let matcher = PatternMatcher::default_excludes().unwrap();
        assert!(matcher.is_match("/project/__pycache__/module.cpython-311.pyc"));
    }
}

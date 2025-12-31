//! File ignore service for consistent file filtering.

use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use std::path::Path;
use std::path::PathBuf;
use walkdir::WalkDir;

use crate::config::IgnoreConfig;

/// Ignore file names (ripgrep native support)
pub const IGNORE_FILES: &[&str] = &[".ignore"];
use crate::patterns::BINARY_FILE_PATTERNS;
use crate::patterns::COMMON_DIRECTORY_EXCLUDES;
use crate::patterns::COMMON_IGNORE_PATTERNS;
use crate::patterns::SYSTEM_FILE_EXCLUDES;

/// Service for handling file ignore patterns.
///
/// Provides consistent file filtering behavior across all file-related
/// operations (glob, list_dir, grep, file_search, etc.).
///
/// # Example
///
/// ```rust,no_run
/// use codex_file_ignore::{IgnoreService, IgnoreConfig};
/// use std::path::Path;
///
/// let service = IgnoreService::with_defaults();
/// let walker = service.create_walk_builder(Path::new("."));
///
/// for entry in walker.build() {
///     match entry {
///         Ok(e) => println!("{}", e.path().display()),
///         Err(e) => eprintln!("Error: {}", e),
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct IgnoreService {
    config: IgnoreConfig,
}

impl IgnoreService {
    /// Create a new service with the given configuration.
    pub fn new(config: IgnoreConfig) -> Self {
        Self { config }
    }

    /// Create a new service with default configuration.
    ///
    /// Defaults:
    /// - Respects `.gitignore` files
    /// - Respects `.ignore` files (ripgrep native support)
    /// - Excludes hidden files
    /// - Does not follow symlinks
    pub fn with_defaults() -> Self {
        Self::new(IgnoreConfig::default())
    }

    /// Create a WalkBuilder with all ignore rules applied.
    ///
    /// The returned WalkBuilder is configured to:
    /// - Respect `.gitignore` if enabled
    /// - Respect `.ignore` if enabled (ripgrep native support)
    /// - Handle hidden files according to config
    /// - Apply custom exclude patterns
    ///
    /// # Arguments
    ///
    /// * `root` - The root directory to start walking from
    ///
    /// # Returns
    ///
    /// A configured `WalkBuilder` ready for traversal.
    pub fn create_walk_builder(&self, root: &Path) -> WalkBuilder {
        let mut builder = WalkBuilder::new(root);

        // Configure gitignore handling
        if self.config.respect_gitignore {
            builder.git_ignore(true).git_global(true).git_exclude(true);
        } else {
            builder
                .git_ignore(false)
                .git_global(false)
                .git_exclude(false);
        }

        // Configure .ignore files (ripgrep native support)
        if self.config.respect_ignore {
            builder.add_custom_ignore_filename(".ignore");
        }

        // Configure hidden files and symlinks
        builder
            .hidden(!self.config.include_hidden)
            .follow_links(self.config.follow_links)
            .require_git(false); // Don't require git repo

        // Apply custom exclude patterns
        if !self.config.custom_excludes.is_empty() {
            if let Ok(overrides) = self.build_overrides(root) {
                builder.overrides(overrides);
            }
        }

        builder
    }

    /// Build override matcher for custom exclude patterns.
    fn build_overrides(&self, root: &Path) -> Result<ignore::overrides::Override, ignore::Error> {
        let mut override_builder = OverrideBuilder::new(root);
        for pattern in &self.config.custom_excludes {
            // Prefix with ! for exclusion in override syntax
            override_builder.add(&format!("!{pattern}"))?;
        }
        override_builder.build()
    }

    /// Get common ignore patterns for basic operations.
    ///
    /// Returns patterns like `**/node_modules/**`, `**/.git/**`, etc.
    pub fn get_core_patterns() -> &'static [&'static str] {
        COMMON_IGNORE_PATTERNS
    }

    /// Get all default exclude patterns combined.
    ///
    /// Includes:
    /// - Common ignore patterns (node_modules, .git, etc.)
    /// - Binary file patterns (*.exe, *.dll, etc.)
    /// - Common directory excludes (dist, build, etc.)
    /// - System file excludes (.DS_Store, etc.)
    pub fn get_default_excludes() -> Vec<&'static str> {
        let mut patterns = Vec::new();
        patterns.extend(COMMON_IGNORE_PATTERNS);
        patterns.extend(BINARY_FILE_PATTERNS);
        patterns.extend(COMMON_DIRECTORY_EXCLUDES);
        patterns.extend(SYSTEM_FILE_EXCLUDES);
        patterns
    }

    /// Get the current configuration.
    pub fn config(&self) -> &IgnoreConfig {
        &self.config
    }
}

impl Default for IgnoreService {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Find all .ignore files for a given root path.
///
/// This is useful for external tools that need explicit file paths
/// rather than built-in ignore handling.
///
/// Note: ripgrep natively supports .ignore files, so this function
/// is typically not needed when using rg directly.
///
/// Searches:
/// 1. UP - from root through parent directories (for project-level ignores)
/// 2. DOWN - into subdirectories (for nested ignores like src/.ignore)
///
/// # Arguments
///
/// * `root` - The root directory to search from
///
/// # Returns
///
/// A vector of paths to found ignore files.
pub fn find_ignore_files(root: &Path) -> Vec<PathBuf> {
    let mut ignore_files = Vec::new();

    // 1. Walk UP to parent directories (for project-level ignores)
    // Stop at git root or max depth to avoid walking all the way to filesystem root
    const MAX_PARENT_DEPTH: usize = 20;
    let mut current = Some(root.to_path_buf());
    let mut depth = 0;
    while let Some(dir) = current {
        for name in IGNORE_FILES {
            let path = dir.join(name);
            if path.exists() {
                ignore_files.push(path);
            }
        }
        depth += 1;
        // Stop at git root or max depth
        if depth >= MAX_PARENT_DEPTH || dir.join(".git").exists() {
            break;
        }
        current = dir.parent().map(|p| p.to_path_buf());
    }

    // 2. Walk DOWN into subdirectories (for nested ignores)
    if root.is_dir() {
        for entry in WalkDir::new(root)
            .max_depth(10) // Limit depth to avoid performance issues
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let name = entry.file_name().to_string_lossy();
                if IGNORE_FILES.iter().any(|&n| n == name) {
                    let path = entry.path().to_path_buf();
                    // Avoid duplicates (root was already added in step 1)
                    if !ignore_files.contains(&path) {
                        ignore_files.push(path);
                    }
                }
            }
        }
    }

    ignore_files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_with_defaults() {
        let service = IgnoreService::with_defaults();
        assert!(service.config().respect_gitignore);
        assert!(service.config().respect_ignore);
    }

    #[test]
    fn test_create_walk_builder() {
        let temp = tempdir().expect("create temp dir");
        let service = IgnoreService::with_defaults();
        let _builder = service.create_walk_builder(temp.path());
        // Verify it doesn't panic
    }

    #[test]
    fn test_respects_gitignore() {
        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();

        // Create test files
        fs::write(dir.join("keep.rs"), "code").expect("write");
        fs::write(dir.join("ignored.log"), "log").expect("write");
        fs::write(dir.join(".gitignore"), "*.log").expect("write");

        let service = IgnoreService::with_defaults();
        let walker = service.create_walk_builder(dir);

        let files: Vec<_> = walker
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();

        assert!(files.contains(&"keep.rs".to_string()));
        // .gitignore itself is a dotfile, and we exclude hidden files by default
        assert!(!files.contains(&"ignored.log".to_string()));
    }

    #[test]
    fn test_respects_ignore() {
        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();

        // Create test files
        fs::write(dir.join("keep.rs"), "code").expect("write");
        fs::write(dir.join("secret.env"), "secrets").expect("write");
        fs::write(dir.join(".ignore"), "*.env").expect("write");

        let service = IgnoreService::with_defaults();
        let walker = service.create_walk_builder(dir);

        let files: Vec<_> = walker
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();

        assert!(files.contains(&"keep.rs".to_string()));
        assert!(!files.contains(&"secret.env".to_string()));
    }

    #[test]
    fn test_custom_excludes() {
        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();

        fs::write(dir.join("keep.rs"), "code").expect("write");
        fs::write(dir.join("temp.tmp"), "temp").expect("write");

        let config = IgnoreConfig::default().with_excludes(vec!["*.tmp".to_string()]);
        let service = IgnoreService::new(config);
        let walker = service.create_walk_builder(dir);

        let files: Vec<_> = walker
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();

        assert!(files.contains(&"keep.rs".to_string()));
        assert!(!files.contains(&"temp.tmp".to_string()));
    }

    #[test]
    fn test_get_core_patterns() {
        let patterns = IgnoreService::get_core_patterns();
        assert!(patterns.contains(&"**/node_modules/**"));
        assert!(patterns.contains(&"**/.git/**"));
    }

    #[test]
    fn test_get_default_excludes() {
        let excludes = IgnoreService::get_default_excludes();
        assert!(excludes.len() > 10);
        assert!(excludes.contains(&"**/*.exe"));
        assert!(excludes.contains(&"**/.DS_Store"));
    }

    #[test]
    fn test_include_hidden_files() {
        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();

        fs::write(dir.join("visible.rs"), "code").expect("write");
        fs::write(dir.join(".hidden"), "hidden").expect("write");

        // Default: exclude hidden
        let service = IgnoreService::with_defaults();
        let walker = service.create_walk_builder(dir);
        let files: Vec<_> = walker
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(files.contains(&"visible.rs".to_string()));
        assert!(!files.contains(&".hidden".to_string()));

        // With hidden included
        let config = IgnoreConfig::default().with_hidden(true);
        let service = IgnoreService::new(config);
        let walker = service.create_walk_builder(dir);
        let files: Vec<_> = walker
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(files.contains(&"visible.rs".to_string()));
        assert!(files.contains(&".hidden".to_string()));
    }

    #[test]
    fn test_find_ignore_files_single() {
        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();

        // Create .ignore file
        fs::write(dir.join(".ignore"), "*.log").expect("write file");

        let files = find_ignore_files(dir);
        assert!(!files.is_empty());
        assert!(files.iter().any(|f| f.ends_with(".ignore")));
    }

    #[test]
    fn test_find_ignore_files_nested() {
        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();

        // Create nested directory structure
        fs::create_dir_all(dir.join("src/nested")).expect("create dirs");

        // Root-level .ignore
        fs::write(dir.join(".ignore"), "*.log").expect("write root ignore");

        // Nested .ignore in src/
        fs::write(dir.join("src/.ignore"), "*.tmp").expect("write src ignore");

        // Deeply nested .ignore
        fs::write(dir.join("src/nested/.ignore"), "*.bak").expect("write nested ignore");

        let files = find_ignore_files(dir);

        // Should find all 3 ignore files
        assert!(files.len() >= 3);
        assert!(
            files.iter().any(|f| {
                f.ends_with(".ignore") && f.parent().map(|p| p == dir).unwrap_or(false)
            })
        );
        assert!(files.iter().any(|f| {
            f.ends_with(".ignore")
                && f.parent()
                    .and_then(|p| p.file_name())
                    .map(|n| n == "src")
                    .unwrap_or(false)
        }));
        assert!(files.iter().any(|f| {
            f.ends_with(".ignore")
                && f.parent()
                    .and_then(|p| p.file_name())
                    .map(|n| n == "nested")
                    .unwrap_or(false)
        }));
    }

    #[test]
    fn test_find_ignore_files_no_duplicates() {
        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();

        // Create .ignore file at root
        fs::write(dir.join(".ignore"), "*.log").expect("write file");

        let files = find_ignore_files(dir);

        // Count occurrences of root .ignore
        let root_count = files
            .iter()
            .filter(|f| f.parent().map(|p| p == dir).unwrap_or(false) && f.ends_with(".ignore"))
            .count();

        assert_eq!(root_count, 1, "Should not have duplicate root ignore file");
    }
}

//! File walker using codex-file-ignore.

use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use codex_file_ignore::IgnoreConfig;
use codex_file_ignore::IgnoreService;

use crate::error::Result;

/// File walker for traversing directories.
///
/// Uses codex-file-ignore for .gitignore and .ignore support.
/// Handles symlinks with loop detection.
pub struct FileWalker {
    ignore_service: IgnoreService,
    max_file_size: u64,
    follow_symlinks: bool,
}

impl FileWalker {
    /// Create a new file walker.
    pub fn new(max_file_size_mb: i32) -> Self {
        let config = IgnoreConfig::respecting_all();
        Self {
            ignore_service: IgnoreService::new(config),
            max_file_size: (max_file_size_mb as u64) * 1024 * 1024,
            follow_symlinks: true,
        }
    }

    /// Create a file walker with custom symlink behavior.
    pub fn with_symlink_follow(max_file_size_mb: i32, follow: bool) -> Self {
        let config = IgnoreConfig::respecting_all();
        Self {
            ignore_service: IgnoreService::new(config),
            max_file_size: (max_file_size_mb as u64) * 1024 * 1024,
            follow_symlinks: follow,
        }
    }

    /// Walk a directory and return file paths.
    ///
    /// Handles symlinks with loop detection to avoid infinite recursion.
    /// Symlinks are resolved to their canonical paths.
    pub fn walk(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let mut builder = self.ignore_service.create_walk_builder(root);

        // Configure symlink following
        builder.follow_links(self.follow_symlinks);

        let mut files = Vec::new();
        let mut seen_paths: HashSet<PathBuf> = HashSet::new();

        for entry in builder.build() {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();

            // Skip directories
            if path.is_dir() {
                continue;
            }

            // Handle symlinks
            let resolved_path = if path.is_symlink() {
                match self.resolve_symlink(path) {
                    Some(p) => p,
                    None => continue, // Skip broken symlinks
                }
            } else {
                path.to_path_buf()
            };

            // Skip if we've already seen this file (handles symlink loops)
            if !seen_paths.insert(resolved_path.clone()) {
                continue;
            }

            // Skip files that are too large
            if let Ok(metadata) = resolved_path.metadata() {
                if metadata.len() > self.max_file_size {
                    continue;
                }
            }

            // Skip non-text files by extension
            if !is_text_file(&resolved_path) {
                continue;
            }

            // Return original path for symlinks (not resolved) for consistency
            files.push(path.to_path_buf());
        }

        Ok(files)
    }

    /// Resolve a symlink to its canonical path.
    ///
    /// Returns None if the symlink is broken or points outside the filesystem.
    fn resolve_symlink(&self, path: &Path) -> Option<PathBuf> {
        // Try to get canonical path (resolves all symlinks)
        match path.canonicalize() {
            Ok(canonical) => {
                // Verify the target exists and is a file
                if canonical.is_file() {
                    Some(canonical)
                } else {
                    None
                }
            }
            Err(_) => None, // Broken symlink
        }
    }
}

/// Check if a file is likely a text file based on extension.
fn is_text_file(path: &Path) -> bool {
    let text_extensions = [
        // Programming languages
        "rs",
        "go",
        "py",
        "java",
        "js",
        "jsx",
        "ts",
        "tsx",
        "c",
        "cpp",
        "cc",
        "cxx",
        "h",
        "hpp",
        "cs",
        "rb",
        "php",
        "swift",
        "kt",
        "kts",
        "scala",
        "lua",
        "sh",
        "bash",
        "zsh",
        "fish",
        "pl",
        "pm",
        "r",
        "m",
        "mm",
        "hs",
        "ex",
        "exs",
        "erl",
        "hrl",
        "clj",
        "cljs",
        "elm",
        "fs",
        "fsx",
        "ml",
        "mli",
        "nim",
        "zig",
        "v",
        "vala",
        "d",
        "dart",
        "groovy",
        "gradle",
        // Web
        "html",
        "htm",
        "css",
        "scss",
        "sass",
        "less",
        "vue",
        "svelte",
        // Data/Config
        "json",
        "yaml",
        "yml",
        "toml",
        "xml",
        "ini",
        "cfg",
        "conf",
        "properties",
        // Documentation
        "md",
        "rst",
        "txt",
        "adoc",
        // SQL
        "sql",
        // Build
        "mk",
        "cmake",
        "makefile",
        "dockerfile",
        // Other
        "proto",
        "thrift",
        "graphql",
        "gql",
    ];

    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| text_extensions.contains(&ext.to_lowercase().as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_text_file() {
        assert!(is_text_file(Path::new("main.rs")));
        assert!(is_text_file(Path::new("package.json")));
        assert!(is_text_file(Path::new("README.md")));
        assert!(!is_text_file(Path::new("image.png")));
        assert!(!is_text_file(Path::new("binary.exe")));
    }

    #[test]
    fn test_with_symlink_follow() {
        let walker = FileWalker::with_symlink_follow(10, false);
        assert!(!walker.follow_symlinks);

        let walker = FileWalker::with_symlink_follow(10, true);
        assert!(walker.follow_symlinks);
    }

    #[test]
    fn test_walker_default_follows_symlinks() {
        let walker = FileWalker::new(10);
        assert!(walker.follow_symlinks);
    }

    #[cfg(unix)]
    #[test]
    fn test_symlink_handling() {
        use std::os::unix::fs::symlink;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Create a real file
        let real_file = root.join("real.rs");
        std::fs::write(&real_file, "fn main() {}").unwrap();

        // Create a symlink to the file
        let link_file = root.join("link.rs");
        symlink(&real_file, &link_file).unwrap();

        // Create a broken symlink
        let broken_link = root.join("broken.rs");
        symlink(root.join("nonexistent.rs"), &broken_link).unwrap();

        let walker = FileWalker::new(10);
        let files = walker.walk(root).unwrap();

        // Should find real file and valid symlink, but skip broken symlink
        // Due to deduplication, if both point to same canonical path, only one is counted
        assert!(files.len() >= 1);
        assert!(files.len() <= 2);

        // Verify broken symlink is not in results
        for file in &files {
            assert!(!file.ends_with("broken.rs"));
        }
    }
}

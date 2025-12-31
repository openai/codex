//! Enhanced List Directory Handler
//!
//! This module provides the EnhancedListDirHandler which lists directory entries
//! while respecting .gitignore and .ignore files.

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use codex_file_ignore::IgnoreConfig;
use codex_file_ignore::IgnoreService;
use serde::Deserialize;
use std::cmp::Ordering;
use std::path::Path;
use std::path::PathBuf;

const INDENTATION_SPACES: usize = 2;

fn default_offset() -> usize {
    1
}

fn default_limit() -> usize {
    25
}

fn default_depth() -> usize {
    2
}

#[derive(Deserialize)]
struct ListDirArgs {
    dir_path: String,
    #[serde(default = "default_offset")]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default = "default_depth")]
    depth: usize,
}

/// Directory entry with metadata
#[derive(Clone)]
struct DirEntry {
    /// Relative path for sorting
    name: String,
    /// Display name (filename only)
    display_name: String,
    /// Depth level for indentation
    depth: usize,
    /// Entry kind
    kind: DirEntryKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DirEntryKind {
    Directory,
    File,
    Symlink,
    Other,
}

/// Enhanced list_dir handler with ignore support
pub struct EnhancedListDirHandler;

#[async_trait]
impl ToolHandler for EnhancedListDirHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation { payload, .. } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "list_dir handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: ListDirArgs = serde_json::from_str(&arguments).map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "failed to parse function arguments: {err:?}"
            ))
        })?;

        let ListDirArgs {
            dir_path,
            offset,
            limit,
            depth,
        } = args;

        // Validate parameters
        if offset == 0 {
            return Err(FunctionCallError::RespondToModel(
                "offset must be a 1-indexed entry number".to_string(),
            ));
        }

        if limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "limit must be greater than zero".to_string(),
            ));
        }

        if depth == 0 {
            return Err(FunctionCallError::RespondToModel(
                "depth must be greater than zero".to_string(),
            ));
        }

        let path = PathBuf::from(&dir_path);
        if !path.is_absolute() {
            return Err(FunctionCallError::RespondToModel(
                "dir_path must be an absolute path".to_string(),
            ));
        }

        if !path.exists() {
            return Err(FunctionCallError::RespondToModel(format!(
                "Path does not exist: {}",
                path.display()
            )));
        }

        if !path.is_dir() {
            return Err(FunctionCallError::RespondToModel(format!(
                "Path is not a directory: {}",
                path.display()
            )));
        }

        // Create ignore service with hidden files visible
        // Only ignore rules filter files out
        let ignore_config = IgnoreConfig {
            respect_gitignore: true,
            respect_ignore: true,
            include_hidden: true, // Show dotfiles (only ignore rules filter)
            follow_links: false,
            custom_excludes: Vec::new(),
        };
        let ignore_service = IgnoreService::new(ignore_config);

        // Collect entries using walk builder with ignore support
        let entries = collect_entries_with_ignore(&path, depth, &ignore_service)?;

        if entries.is_empty() {
            let output = format!("Absolute path: {}\n[Empty directory]", path.display());
            return Ok(ToolOutput::Function {
                content: output,
                content_items: None,
                success: Some(true),
            });
        }

        // Apply offset/limit pagination
        let start_index = offset - 1;
        if start_index >= entries.len() {
            return Err(FunctionCallError::RespondToModel(
                "offset exceeds directory entry count".to_string(),
            ));
        }

        let remaining_entries = entries.len() - start_index;
        let capped_limit = limit.min(remaining_entries);
        let end_index = start_index + capped_limit;
        let selected_entries = &entries[start_index..end_index];

        // Format output
        let mut output = Vec::with_capacity(selected_entries.len() + 2);
        output.push(format!("Absolute path: {}", path.display()));
        output.push(format!(
            "[{} of {} entries shown]",
            selected_entries.len(),
            entries.len()
        ));

        for entry in selected_entries {
            output.push(format_entry_line(entry));
        }

        if end_index < entries.len() {
            output.push(format!("More than {} entries found", capped_limit));
        }

        Ok(ToolOutput::Function {
            content: output.join("\n"),
            content_items: None,
            success: Some(true),
        })
    }
}

/// Collect directory entries with ignore support
fn collect_entries_with_ignore(
    root: &Path,
    max_depth: usize,
    ignore_service: &IgnoreService,
) -> Result<Vec<DirEntry>, FunctionCallError> {
    let mut walker = ignore_service.create_walk_builder(root);
    // Set max_depth on walker for efficiency
    // WalkBuilder max_depth: 0=root only, 1=root+children, 2=root+children+grandchildren
    // Our max_depth: 1=children, 2=children+grandchildren
    // So walker.max_depth(N) matches our API max_depth=N
    walker.max_depth(Some(max_depth));

    let mut entries = Vec::new();

    for entry_result in walker.build() {
        let entry = match entry_result {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Skip the root directory itself
        if entry.path() == root {
            continue;
        }

        // Calculate depth relative to root
        let rel_path = match entry.path().strip_prefix(root) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let entry_depth = rel_path.components().count();

        // Get file type
        let file_type = match entry.file_type() {
            Some(ft) => ft,
            None => continue,
        };

        let kind = if file_type.is_symlink() {
            DirEntryKind::Symlink
        } else if file_type.is_dir() {
            DirEntryKind::Directory
        } else if file_type.is_file() {
            DirEntryKind::File
        } else {
            DirEntryKind::Other
        };

        let display_name = entry
            .path()
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let sort_key = format_sort_key(rel_path);

        entries.push(DirEntry {
            name: sort_key,
            display_name,           // No truncation - trust file system limits
            depth: entry_depth - 1, // Adjust for indentation (0-indexed)
            kind,
        });
    }

    // Sort: directories first at each level, then alphabetically
    entries.sort_by(|a, b| {
        // First compare by name prefix (path hierarchy)
        let a_parts: Vec<&str> = a.name.split('/').collect();
        let b_parts: Vec<&str> = b.name.split('/').collect();

        // Compare common path components
        let min_len = a_parts.len().min(b_parts.len());
        for i in 0..min_len {
            let a_is_last = i == a_parts.len() - 1;
            let b_is_last = i == b_parts.len() - 1;

            // At each level, directories come first
            if a_parts[i] != b_parts[i] {
                // If one is a directory and the other is not at this position
                let a_is_dir_at_level = !a_is_last || a.kind == DirEntryKind::Directory;
                let b_is_dir_at_level = !b_is_last || b.kind == DirEntryKind::Directory;

                if a_is_dir_at_level && !b_is_dir_at_level {
                    return Ordering::Less;
                }
                if !a_is_dir_at_level && b_is_dir_at_level {
                    return Ordering::Greater;
                }

                return a_parts[i].cmp(b_parts[i]);
            }
        }

        // If paths are equal up to min_len, shorter path (directory) comes first
        a_parts.len().cmp(&b_parts.len())
    });

    Ok(entries)
}

/// Format sort key from relative path
fn format_sort_key(path: &Path) -> String {
    // Trust file system limits (Linux: 255 bytes, macOS: 255 chars)
    // No truncation needed - consistent with glob_files and ripgrep handlers
    path.to_string_lossy().replace('\\', "/")
}

/// Format entry line with indentation and type suffix
fn format_entry_line(entry: &DirEntry) -> String {
    let indent = " ".repeat(entry.depth * INDENTATION_SPACES);
    let mut name = entry.display_name.clone();
    match entry.kind {
        DirEntryKind::Directory => name.push('/'),
        DirEntryKind::Symlink => name.push('@'),
        DirEntryKind::Other => name.push('?'),
        DirEntryKind::File => {}
    }
    format!("{indent}{name}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_format_entry_line_directory() {
        let entry = DirEntry {
            name: "src".to_string(),
            display_name: "src".to_string(),
            depth: 0,
            kind: DirEntryKind::Directory,
        };
        assert_eq!(format_entry_line(&entry), "src/");
    }

    #[test]
    fn test_format_entry_line_file_with_depth() {
        let entry = DirEntry {
            name: "src/main.rs".to_string(),
            display_name: "main.rs".to_string(),
            depth: 1,
            kind: DirEntryKind::File,
        };
        assert_eq!(format_entry_line(&entry), "  main.rs");
    }

    #[test]
    fn test_format_entry_line_symlink() {
        let entry = DirEntry {
            name: "link".to_string(),
            display_name: "link".to_string(),
            depth: 0,
            kind: DirEntryKind::Symlink,
        };
        assert_eq!(format_entry_line(&entry), "link@");
    }

    #[tokio::test]
    async fn test_respects_gitignore() {
        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();

        // Create test files
        fs::write(dir.join("keep.rs"), "rust code").expect("write keep.rs");
        fs::write(dir.join("ignored.log"), "log file").expect("write ignored.log");

        // Create .gitignore
        fs::write(dir.join(".gitignore"), "*.log").expect("write .gitignore");

        let ignore_config = IgnoreConfig {
            respect_gitignore: true,
            respect_ignore: true,
            include_hidden: true,
            ..Default::default()
        };
        let ignore_service = IgnoreService::new(ignore_config);

        let entries = collect_entries_with_ignore(dir, 2, &ignore_service).expect("collect");

        let names: Vec<&str> = entries.iter().map(|e| e.display_name.as_str()).collect();
        assert!(names.contains(&"keep.rs"));
        assert!(!names.contains(&"ignored.log"));
    }

    #[tokio::test]
    async fn test_respects_ignore() {
        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();

        // Create test files
        fs::write(dir.join("keep.rs"), "rust code").expect("write keep.rs");
        fs::write(dir.join("secret.env"), "secrets").expect("write secret.env");

        // Create .ignore
        fs::write(dir.join(".ignore"), "*.env").expect("write .ignore");

        let ignore_config = IgnoreConfig {
            respect_gitignore: true,
            respect_ignore: true,
            include_hidden: true,
            ..Default::default()
        };
        let ignore_service = IgnoreService::new(ignore_config);

        let entries = collect_entries_with_ignore(dir, 2, &ignore_service).expect("collect");

        let names: Vec<&str> = entries.iter().map(|e| e.display_name.as_str()).collect();
        assert!(names.contains(&"keep.rs"));
        assert!(!names.contains(&"secret.env"));
    }

    #[tokio::test]
    async fn test_shows_dotfiles_not_ignored() {
        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();

        // Create dotfiles
        fs::write(dir.join(".eslintrc"), "config").expect("write .eslintrc");
        fs::write(dir.join(".prettierrc"), "config").expect("write .prettierrc");
        fs::write(dir.join("main.rs"), "code").expect("write main.rs");

        let ignore_config = IgnoreConfig {
            respect_gitignore: true,
            respect_ignore: true,
            include_hidden: true, // Key: include hidden files
            ..Default::default()
        };
        let ignore_service = IgnoreService::new(ignore_config);

        let entries = collect_entries_with_ignore(dir, 2, &ignore_service).expect("collect");

        let names: Vec<&str> = entries.iter().map(|e| e.display_name.as_str()).collect();
        assert!(names.contains(&".eslintrc"));
        assert!(names.contains(&".prettierrc"));
        assert!(names.contains(&"main.rs"));
    }

    #[tokio::test]
    async fn test_depth_parameter() {
        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();

        // Create nested structure
        fs::create_dir_all(dir.join("a/b/c")).expect("create dirs");
        fs::write(dir.join("root.txt"), "root").expect("write root");
        fs::write(dir.join("a/level1.txt"), "level1").expect("write level1");
        fs::write(dir.join("a/b/level2.txt"), "level2").expect("write level2");
        fs::write(dir.join("a/b/c/level3.txt"), "level3").expect("write level3");

        let ignore_config = IgnoreConfig {
            respect_gitignore: true,
            respect_ignore: true,
            include_hidden: true,
            ..Default::default()
        };
        let ignore_service = IgnoreService::new(ignore_config);

        // Test depth 1
        let entries = collect_entries_with_ignore(dir, 1, &ignore_service).expect("collect");
        let names: Vec<&str> = entries.iter().map(|e| e.display_name.as_str()).collect();
        assert!(names.contains(&"root.txt"));
        assert!(names.contains(&"a"));
        assert!(!names.contains(&"level1.txt"));

        // Test depth 2
        let entries = collect_entries_with_ignore(dir, 2, &ignore_service).expect("collect");
        let names: Vec<&str> = entries.iter().map(|e| e.display_name.as_str()).collect();
        assert!(names.contains(&"root.txt"));
        assert!(names.contains(&"a"));
        assert!(names.contains(&"level1.txt"));
        assert!(names.contains(&"b"));
        assert!(!names.contains(&"level2.txt"));
    }

    #[tokio::test]
    async fn test_dirs_first_sorting() {
        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();

        // Create files and directories
        fs::create_dir(dir.join("zebra_dir")).expect("create zebra_dir");
        fs::write(dir.join("alpha.txt"), "alpha").expect("write alpha");
        fs::create_dir(dir.join("alpha_dir")).expect("create alpha_dir");
        fs::write(dir.join("zebra.txt"), "zebra").expect("write zebra");

        let ignore_config = IgnoreConfig {
            respect_gitignore: true,
            respect_ignore: true,
            include_hidden: true,
            ..Default::default()
        };
        let ignore_service = IgnoreService::new(ignore_config);

        let entries = collect_entries_with_ignore(dir, 1, &ignore_service).expect("collect");

        // First entries should be directories
        let mut found_file = false;
        for entry in &entries {
            if entry.kind == DirEntryKind::File {
                found_file = true;
            } else if entry.kind == DirEntryKind::Directory && found_file {
                panic!("Directory found after file - sorting is wrong");
            }
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_symlink_identification() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();

        // Create file and symlink
        fs::write(dir.join("target.txt"), "target").expect("write target");
        symlink(dir.join("target.txt"), dir.join("link")).expect("create symlink");

        let ignore_config = IgnoreConfig {
            respect_gitignore: true,
            respect_ignore: true,
            include_hidden: true,
            ..Default::default()
        };
        let ignore_service = IgnoreService::new(ignore_config);

        let entries = collect_entries_with_ignore(dir, 1, &ignore_service).expect("collect");

        let link_entry = entries.iter().find(|e| e.display_name == "link");
        assert!(link_entry.is_some());
        assert_eq!(link_entry.unwrap().kind, DirEntryKind::Symlink);
    }

    // Additional tests for error paths and pagination

    #[tokio::test]
    async fn test_pagination_offset_limit() {
        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();

        // Create 10 files
        for i in 0..10 {
            fs::write(dir.join(format!("file_{:02}.txt", i)), "content").expect("write file");
        }

        let ignore_config = IgnoreConfig {
            respect_gitignore: true,
            respect_ignore: true,
            include_hidden: true,
            ..Default::default()
        };
        let ignore_service = IgnoreService::new(ignore_config);

        let entries = collect_entries_with_ignore(dir, 1, &ignore_service).expect("collect");
        assert_eq!(entries.len(), 10);

        // Simulate pagination: offset=3 (1-indexed), limit=4
        // Should get entries 3,4,5,6 (0-indexed: 2,3,4,5)
        let start_index = 3 - 1; // offset is 1-indexed
        let limit = 4;
        let remaining = entries.len() - start_index;
        let capped_limit = limit.min(remaining);
        let end_index = start_index + capped_limit;
        let selected = &entries[start_index..end_index];

        assert_eq!(selected.len(), 4);
    }

    #[tokio::test]
    async fn test_empty_directory() {
        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();

        // Empty directory (no files created)
        let ignore_config = IgnoreConfig {
            respect_gitignore: true,
            respect_ignore: true,
            include_hidden: true,
            ..Default::default()
        };
        let ignore_service = IgnoreService::new(ignore_config);

        let entries = collect_entries_with_ignore(dir, 2, &ignore_service).expect("collect");
        assert!(entries.is_empty());
    }
}

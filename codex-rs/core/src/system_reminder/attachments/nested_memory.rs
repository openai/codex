//! Nested memory generator.
//!
//! Automatically discovers and injects AGENTS.md files and rules when files are read.
//! Matches Claude Code's nested_memory attachment system (qH5, ZY2 in chunks.107.mjs).

use crate::config::system_reminder::NestedMemoryConfig;
use crate::config::system_reminder::SystemReminderConfig;
use crate::error::Result;
use crate::system_reminder::generator::AttachmentGenerator;
use crate::system_reminder::generator::GeneratorContext;
use crate::system_reminder::throttle::ThrottleConfig;
use crate::system_reminder::types::AttachmentType;
use crate::system_reminder::types::SystemReminder;
use async_trait::async_trait;
use glob::Pattern;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

// ============================================
// Constants
// ============================================

/// Memory file name (project-specific, user preference: AGENTS.md).
const MEMORY_FILE_NAME: &str = "AGENTS.md";

/// Local memory file name (gitignored).
const LOCAL_MEMORY_FILE_NAME: &str = "AGENTS.local.md";

/// Project config directory.
const PROJECT_CONFIG_DIR: &str = ".codex";

/// User rules directory (relative to home).
const USER_RULES_DIR: &str = ".codex/rules";

// ============================================
// Types
// ============================================

/// Type of discovered memory file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveredFileType {
    /// User rules (~/.codex/rules/).
    User,
    /// Project AGENTS.md or .codex/AGENTS.md.
    Project,
    /// Local AGENTS.local.md (gitignored).
    Local,
}

/// A discovered memory file.
#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    /// Absolute path to the file.
    pub path: PathBuf,
    /// File content (truncated if needed).
    pub content: String,
    /// Type of file (reserved for future use: logging, filtering).
    #[allow(dead_code)]
    pub file_type: DiscoveredFileType,
    /// Glob patterns for conditional rules (reserved for future use: output metadata).
    #[allow(dead_code)]
    pub globs: Option<Vec<String>>,
}

/// Directory hierarchy for file discovery.
///
/// Matches Claude Code's CH5 function (chunks.107.mjs:1947-1963).
#[derive(Debug)]
struct DirectoryHierarchy {
    /// Directories from file's parent up to (not including) cwd.
    /// Ordered: closest to cwd first, closest to file last.
    nested_dirs: Vec<PathBuf>,
    /// Directories from cwd up to filesystem root.
    /// Ordered: root first, cwd last.
    cwd_level_dirs: Vec<PathBuf>,
}

// ============================================
// Nested Memory Generator
// ============================================

/// Nested memory generator.
///
/// Discovers and injects AGENTS.md files and rules when files are read.
#[derive(Debug)]
pub struct NestedMemoryGenerator {
    config: NestedMemoryConfig,
}

impl NestedMemoryGenerator {
    /// Create a new nested memory generator.
    pub fn new(config: NestedMemoryConfig) -> Self {
        Self { config }
    }

    /// Discover all memory files for triggered paths.
    fn discover_files(
        &self,
        triggers: Vec<PathBuf>,
        cwd: &Path,
        processed: &mut HashSet<PathBuf>,
    ) -> Vec<DiscoveredFile> {
        let mut files = Vec::new();

        for trigger_path in triggers {
            // Skip if already processed
            if processed.contains(&trigger_path) {
                continue;
            }

            // Calculate directory hierarchy
            let hierarchy = calculate_hierarchy(&trigger_path, cwd);

            // 1. User settings (~/.codex/rules/)
            if self.config.user_rules {
                if let Some(home) = dirs::home_dir() {
                    let user_rules_dir = home.join(USER_RULES_DIR);
                    files.extend(self.read_rules_directory(
                        &user_rules_dir,
                        DiscoveredFileType::User,
                        processed,
                        &trigger_path,
                        false, // include non-conditional
                        0,
                    ));
                }
            }

            // 2. Project files in nested directories (file → cwd)
            for dir in &hierarchy.nested_dirs {
                files.extend(self.read_project_files(dir, processed, &trigger_path));
            }

            // 3. CWD-level rules (cwd → root), conditional only
            for dir in &hierarchy.cwd_level_dirs {
                let rules_dir = dir.join(PROJECT_CONFIG_DIR).join("rules");
                files.extend(self.read_rules_directory(
                    &rules_dir,
                    DiscoveredFileType::Project,
                    processed,
                    &trigger_path,
                    true, // conditional only
                    0,
                ));
            }
        }

        files
    }

    /// Read project files in a directory.
    fn read_project_files(
        &self,
        dir: &Path,
        processed: &mut HashSet<PathBuf>,
        trigger_path: &Path,
    ) -> Vec<DiscoveredFile> {
        let mut files = Vec::new();

        // AGENTS.md
        if self.config.project_settings {
            let agents_md = dir.join(MEMORY_FILE_NAME);
            files.extend(self.read_file_with_imports(
                &agents_md,
                DiscoveredFileType::Project,
                processed,
                0,
            ));

            // .codex/AGENTS.md
            let codex_agents_md = dir.join(PROJECT_CONFIG_DIR).join(MEMORY_FILE_NAME);
            files.extend(self.read_file_with_imports(
                &codex_agents_md,
                DiscoveredFileType::Project,
                processed,
                0,
            ));
        }

        // AGENTS.local.md
        if self.config.local_settings {
            let local_md = dir.join(LOCAL_MEMORY_FILE_NAME);
            files.extend(self.read_file_with_imports(
                &local_md,
                DiscoveredFileType::Local,
                processed,
                0,
            ));
        }

        // .codex/rules/ (both conditional and non-conditional)
        let rules_dir = dir.join(PROJECT_CONFIG_DIR).join("rules");
        files.extend(self.read_rules_directory(
            &rules_dir,
            DiscoveredFileType::Project,
            processed,
            trigger_path,
            false, // include all
            0,
        ));

        files
    }

    /// Read rules directory recursively.
    fn read_rules_directory(
        &self,
        rules_dir: &Path,
        file_type: DiscoveredFileType,
        processed: &mut HashSet<PathBuf>,
        trigger_path: &Path,
        conditional_only: bool,
        depth: i32,
    ) -> Vec<DiscoveredFile> {
        // Prevent infinite recursion
        if depth > self.config.max_import_depth {
            return Vec::new();
        }

        if !rules_dir.is_dir() {
            return Vec::new();
        }

        let mut files = Vec::new();

        // Read all .md files in directory
        let entries = match std::fs::read_dir(rules_dir) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        for entry in entries.flatten() {
            let path = entry.path();

            // Skip directories (no recursive subdirectory traversal)
            if path.is_dir() {
                continue;
            }

            // Only process .md files
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }

            // Skip if already processed
            if processed.contains(&path) {
                continue;
            }

            // Read file content
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::debug!(
                        path = %path.display(),
                        error = %e,
                        "Failed to read rules file"
                    );
                    continue;
                }
            };

            // Parse globs from frontmatter
            let globs = extract_globs(&content);
            let is_conditional = globs.is_some();

            // Skip non-conditional if only conditional requested
            if conditional_only && !is_conditional {
                continue;
            }

            // For conditional rules, check if trigger matches globs
            if let Some(ref glob_patterns) = globs {
                if !matches_globs(trigger_path, glob_patterns, rules_dir) {
                    continue;
                }
            }

            // Truncate content if needed
            let truncated = truncate_content(
                &content,
                self.config.max_content_size,
                self.config.max_lines,
            );

            processed.insert(path.clone());
            files.push(DiscoveredFile {
                path,
                content: truncated,
                file_type,
                globs,
            });
        }

        files
    }

    /// Read a single file with @import support.
    fn read_file_with_imports(
        &self,
        path: &Path,
        file_type: DiscoveredFileType,
        processed: &mut HashSet<PathBuf>,
        depth: i32,
    ) -> Vec<DiscoveredFile> {
        // Check depth limit
        if depth > self.config.max_import_depth {
            return Vec::new();
        }

        // Skip if already processed
        if processed.contains(path) {
            return Vec::new();
        }

        // Check if file exists
        if !path.is_file() {
            return Vec::new();
        }

        // Resolve symlinks
        let resolved_path = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };

        // Skip if resolved path already processed
        if processed.contains(&resolved_path) {
            return Vec::new();
        }

        // Read content
        let content = match std::fs::read_to_string(&resolved_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!(
                    path = %resolved_path.display(),
                    error = %e,
                    "Failed to read memory file"
                );
                return Vec::new();
            }
        };

        processed.insert(resolved_path.clone());

        let mut files = Vec::new();

        // Extract and process @import directives
        let base_dir = resolved_path.parent().unwrap_or(Path::new("/"));
        let imports = extract_imports(&content, base_dir);

        for import_path in imports {
            files.extend(self.read_file_with_imports(
                &import_path,
                file_type,
                processed,
                depth + 1,
            ));
        }

        // Truncate content
        let truncated = truncate_content(
            &content,
            self.config.max_content_size,
            self.config.max_lines,
        );

        // Parse globs
        let globs = extract_globs(&content);

        files.push(DiscoveredFile {
            path: resolved_path,
            content: truncated,
            file_type,
            globs,
        });

        files
    }

    /// Build reminder content from discovered files.
    fn build_content(&self, files: Vec<DiscoveredFile>) -> Option<String> {
        if files.is_empty() {
            return None;
        }

        let mut content = String::new();

        for file in files {
            content.push_str(&format!(
                "Contents of {}:\n\n{}\n\n",
                file.path.display(),
                file.content
            ));
        }

        Some(content.trim_end().to_string())
    }
}

#[async_trait]
impl AttachmentGenerator for NestedMemoryGenerator {
    fn name(&self) -> &str {
        "nested_memory"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::NestedMemory
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Get triggers from file tracker
        let triggers = ctx.file_tracker.get_nested_memory_triggers();

        if triggers.is_empty() {
            return Ok(None);
        }

        // Track processed files to avoid duplicates
        let mut processed = HashSet::new();

        // Discover all memory files
        let files = self.discover_files(triggers, ctx.cwd, &mut processed);

        // Build content
        let content = match self.build_content(files) {
            Some(c) => c,
            None => return Ok(None),
        };

        Ok(Some(SystemReminder::new(
            AttachmentType::NestedMemory,
            content,
        )))
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.enabled && config.attachments.nested_memory && config.nested_memory.enabled
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // No throttling - always inject when triggers exist
        ThrottleConfig::default()
    }
}

impl Default for NestedMemoryGenerator {
    fn default() -> Self {
        Self::new(NestedMemoryConfig::default())
    }
}

// ============================================
// Helper Functions
// ============================================

/// Calculate directory hierarchy from file path to cwd.
///
/// Matches Claude Code's CH5 function.
fn calculate_hierarchy(file_path: &Path, cwd: &Path) -> DirectoryHierarchy {
    let mut nested_dirs = Vec::new();

    // Get file's parent directory
    let file_parent = file_path.parent().unwrap_or(Path::new("/"));

    // Calculate nested directories: from file's parent up to (not including) cwd
    // Ordered: closest to cwd first, closest to file last
    let cwd_abs = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let file_parent_abs = file_parent
        .canonicalize()
        .unwrap_or_else(|_| file_parent.to_path_buf());

    // Check if file is within cwd
    if file_parent_abs.starts_with(&cwd_abs) && file_parent_abs != cwd_abs {
        // Walk from cwd down to file's parent
        let relative = file_parent_abs
            .strip_prefix(&cwd_abs)
            .unwrap_or(Path::new(""));
        let mut current = cwd_abs.clone();

        for component in relative.components() {
            current = current.join(component);
            nested_dirs.push(current.clone());
        }
    }

    // Calculate cwd-level directories: from root up to cwd
    // Ordered: root first, cwd last
    let mut cwd_level_dirs = Vec::new();
    let mut current = cwd_abs.clone();
    loop {
        cwd_level_dirs.push(current.clone());
        match current.parent() {
            Some(p) if p != current => current = p.to_path_buf(),
            _ => break,
        }
    }
    cwd_level_dirs.reverse(); // Now root first, cwd last

    DirectoryHierarchy {
        nested_dirs,
        cwd_level_dirs,
    }
}

/// Extract @import directives from content.
///
/// Format: @import path/to/file.md
fn extract_imports(content: &str, base_dir: &Path) -> Vec<PathBuf> {
    let mut imports = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(path_str) = trimmed.strip_prefix("@import ") {
            let path_str = path_str.trim();
            // Skip empty paths
            if path_str.is_empty() {
                continue;
            }
            // Resolve relative to base_dir
            let import_path = if Path::new(path_str).is_absolute() {
                PathBuf::from(path_str)
            } else {
                base_dir.join(path_str)
            };
            imports.push(import_path);
        }
    }

    imports
}

/// Extract glob patterns from YAML frontmatter.
///
/// Format:
/// ---
/// globs: ["*.test.ts", "*.spec.ts"]
/// ---
fn extract_globs(content: &str) -> Option<Vec<String>> {
    // Check for YAML frontmatter
    if !content.starts_with("---") {
        return None;
    }

    // Find end of frontmatter
    let rest = &content[3..];
    let end_idx = rest.find("---")?;
    let frontmatter = &rest[..end_idx];

    // Simple YAML parsing for globs field
    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("globs:") {
            let value = value.trim();

            // Parse array format: ["pattern1", "pattern2"]
            if value.starts_with('[') && value.ends_with(']') {
                let inner = &value[1..value.len() - 1];
                let patterns: Vec<String> = inner
                    .split(',')
                    .filter_map(|s| {
                        let s = s.trim();
                        // Remove quotes
                        if (s.starts_with('"') && s.ends_with('"'))
                            || (s.starts_with('\'') && s.ends_with('\''))
                        {
                            Some(s[1..s.len() - 1].to_string())
                        } else if !s.is_empty() {
                            Some(s.to_string())
                        } else {
                            None
                        }
                    })
                    .collect();

                if !patterns.is_empty() {
                    return Some(patterns);
                }
            }
        }
    }

    None
}

/// Check if a file path matches any of the glob patterns.
fn matches_globs(file_path: &Path, globs: &[String], base_dir: &Path) -> bool {
    let file_str = file_path.to_string_lossy();

    for glob_str in globs {
        // Try to compile the pattern
        if let Ok(pattern) = Pattern::new(glob_str) {
            // Check against filename
            if let Some(file_name) = file_path.file_name() {
                if pattern.matches(&file_name.to_string_lossy()) {
                    return true;
                }
            }

            // Check against relative path from base_dir
            if let Ok(relative) = file_path.strip_prefix(base_dir) {
                if pattern.matches(&relative.to_string_lossy()) {
                    return true;
                }
            }

            // Check against full path
            if pattern.matches(&file_str) {
                return true;
            }
        }
    }

    false
}

/// Truncate content to size and line limits.
fn truncate_content(content: &str, max_size: i32, max_lines: i32) -> String {
    let max_size = max_size as usize;
    let max_lines = max_lines as usize;

    // First truncate by lines
    let lines: Vec<&str> = content.lines().take(max_lines).collect();
    let line_truncated = lines.join("\n");

    // Then truncate by size
    if line_truncated.len() > max_size {
        // Find last valid UTF-8 boundary before max_size
        let mut end = max_size;
        while end > 0 && !line_truncated.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...[truncated]", &line_truncated[..end])
    } else {
        line_truncated
    }
}

// ============================================
// Tests
// ============================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_calculate_hierarchy_basic() {
        let temp = TempDir::new().unwrap();
        let cwd = temp.path();
        let src_dir = cwd.join("src");
        fs::create_dir(&src_dir).unwrap();
        let file = src_dir.join("main.rs");
        fs::write(&file, "").unwrap();

        let hierarchy = calculate_hierarchy(&file, cwd);

        // nested_dirs should contain src
        assert_eq!(hierarchy.nested_dirs.len(), 1);
        assert!(hierarchy.nested_dirs[0].ends_with("src"));

        // cwd_level_dirs should contain path from root to cwd
        assert!(!hierarchy.cwd_level_dirs.is_empty());
        assert_eq!(
            hierarchy.cwd_level_dirs.last().unwrap(),
            &cwd.canonicalize().unwrap()
        );
    }

    #[test]
    fn test_calculate_hierarchy_file_at_cwd() {
        let temp = TempDir::new().unwrap();
        let cwd = temp.path();
        let file = cwd.join("main.rs");
        fs::write(&file, "").unwrap();

        let hierarchy = calculate_hierarchy(&file, cwd);

        // No nested dirs when file is at cwd
        assert!(hierarchy.nested_dirs.is_empty());
    }

    #[test]
    fn test_extract_imports_single() {
        let content = "@import common.md\n\n# Rules\nSome content";
        let base = Path::new("/project");
        let imports = extract_imports(content, base);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0], PathBuf::from("/project/common.md"));
    }

    #[test]
    fn test_extract_imports_multiple() {
        let content = "@import rules/common.md\n@import rules/testing.md\n";
        let base = Path::new("/project");
        let imports = extract_imports(content, base);

        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0], PathBuf::from("/project/rules/common.md"));
        assert_eq!(imports[1], PathBuf::from("/project/rules/testing.md"));
    }

    #[test]
    fn test_extract_imports_absolute_path() {
        let content = "@import /absolute/path/file.md";
        let base = Path::new("/project");
        let imports = extract_imports(content, base);

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0], PathBuf::from("/absolute/path/file.md"));
    }

    #[test]
    fn test_extract_globs_single() {
        let content = r#"---
globs: ["*.test.ts"]
---
# Testing rules"#;
        let globs = extract_globs(content);

        assert!(globs.is_some());
        let globs = globs.unwrap();
        assert_eq!(globs.len(), 1);
        assert_eq!(globs[0], "*.test.ts");
    }

    #[test]
    fn test_extract_globs_multiple() {
        let content = r#"---
globs: ["*.test.ts", "*.spec.ts"]
---
# Testing rules"#;
        let globs = extract_globs(content);

        assert!(globs.is_some());
        let globs = globs.unwrap();
        assert_eq!(globs.len(), 2);
        assert_eq!(globs[0], "*.test.ts");
        assert_eq!(globs[1], "*.spec.ts");
    }

    #[test]
    fn test_extract_globs_no_frontmatter() {
        let content = "# Just a header\nNo frontmatter here";
        let globs = extract_globs(content);
        assert!(globs.is_none());
    }

    #[test]
    fn test_matches_globs_filename() {
        let file = Path::new("/project/src/Button.test.ts");
        let globs = vec!["*.test.ts".to_string()];
        let base = Path::new("/project");

        assert!(matches_globs(file, &globs, base));
    }

    #[test]
    fn test_matches_globs_no_match() {
        let file = Path::new("/project/src/Button.ts");
        let globs = vec!["*.test.ts".to_string()];
        let base = Path::new("/project");

        assert!(!matches_globs(file, &globs, base));
    }

    #[test]
    fn test_truncate_content_by_size() {
        let content = "a".repeat(100);
        let truncated = truncate_content(&content, 50, 1000);

        // 50 chars + "...[truncated]" (14 chars) = 64 max
        assert!(truncated.len() <= 65);
        assert!(truncated.contains("...[truncated]"));
    }

    #[test]
    fn test_truncate_content_by_lines() {
        let content = "line1\nline2\nline3\nline4\nline5";
        let truncated = truncate_content(&content, 10000, 3);

        assert_eq!(truncated, "line1\nline2\nline3");
    }

    #[test]
    fn test_truncate_content_no_truncation() {
        let content = "short content";
        let truncated = truncate_content(content, 10000, 1000);

        assert_eq!(truncated, content);
    }

    #[test]
    fn test_discovered_file_type() {
        assert_eq!(DiscoveredFileType::User, DiscoveredFileType::User);
        assert_eq!(DiscoveredFileType::Project, DiscoveredFileType::Project);
        assert_eq!(DiscoveredFileType::Local, DiscoveredFileType::Local);
    }

    #[tokio::test]
    async fn test_generator_with_no_triggers() {
        use crate::system_reminder::file_tracker::FileTracker;
        use crate::system_reminder::generator::BackgroundTaskInfo;
        use crate::system_reminder::generator::PlanState;
        use crate::system_reminder::generator::PlanStep;
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let file_tracker = FileTracker::new();
        let plan_state = PlanState {
            is_empty: true,
            last_update_count: 0,
            steps: Vec::<PlanStep>::new(),
        };

        let ctx = GeneratorContext {
            turn_number: 1,
            is_main_agent: true,
            has_user_input: true,
            user_prompt: None,
            cwd: temp.path(),
            agent_id: "test",
            file_tracker: &file_tracker,
            is_plan_mode: false,
            plan_file_path: None,
            is_plan_reentry: false,
            plan_state: &plan_state,
            background_tasks: &[] as &[BackgroundTaskInfo],
            critical_instruction: None,
            diagnostics_store: None,
            lsp_diagnostics_min_severity:
                crate::config::system_reminder::LspDiagnosticsMinSeverity::Error,
            output_style: None,
            approved_plan: None,
        };

        let generator = NestedMemoryGenerator::default();
        let result = generator.generate(&ctx).await.unwrap();

        // No triggers, should return None
        assert!(result.is_none());
    }

    #[test]
    fn test_generator_is_enabled() {
        let generator = NestedMemoryGenerator::default();
        let config = SystemReminderConfig::default();

        assert!(generator.is_enabled(&config));
    }

    #[test]
    fn test_generator_is_disabled_by_global() {
        let generator = NestedMemoryGenerator::default();
        let mut config = SystemReminderConfig::default();
        config.enabled = false;

        assert!(!generator.is_enabled(&config));
    }

    #[test]
    fn test_generator_is_disabled_by_attachment() {
        let generator = NestedMemoryGenerator::default();
        let mut config = SystemReminderConfig::default();
        config.attachments.nested_memory = false;

        assert!(!generator.is_enabled(&config));
    }

    #[test]
    fn test_generator_is_disabled_by_nested_memory_config() {
        let mut nested_config = NestedMemoryConfig::default();
        nested_config.enabled = false;
        let generator = NestedMemoryGenerator::new(nested_config.clone());

        let mut config = SystemReminderConfig::default();
        config.nested_memory = nested_config;

        assert!(!generator.is_enabled(&config));
    }
}

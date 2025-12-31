//! Changed files generator.
//!
//! Notify when previously-read files change (P1).
//! Matches wH5() in Claude Code chunks.107.mjs:2102-2150.

use crate::config::system_reminder::SystemReminderConfig;
use crate::error::Result;
use crate::system_reminder::file_tracker::FileTracker;
use crate::system_reminder::generator::AttachmentGenerator;
use crate::system_reminder::generator::GeneratorContext;
use crate::system_reminder::throttle::ThrottleConfig;
use crate::system_reminder::types::AttachmentType;
use crate::system_reminder::types::SystemReminder;
use async_trait::async_trait;
use codex_file_ignore::PatternMatcher;
use once_cell::sync::Lazy;
use similar::ChangeTag;
use similar::TextDiff;
use std::path::PathBuf;
use std::time::SystemTime;

/// Pre-compiled pattern matcher for skip checks.
static SKIP_PATTERNS: Lazy<PatternMatcher> = Lazy::new(|| {
    PatternMatcher::default_excludes().expect("default exclude patterns should be valid")
});

/// Changed files generator.
///
/// Detects when previously-read files have been modified.
#[derive(Debug)]
pub struct ChangedFilesGenerator;

impl ChangedFilesGenerator {
    /// Create a new changed files generator.
    pub fn new() -> Self {
        Self
    }

    /// Detect file changes and collect changed file paths.
    fn detect_changes(&self, tracker: &FileTracker) -> Vec<FileChange> {
        let mut changes = Vec::new();

        for (path, state) in tracker.get_tracked_files() {
            // Skip partial reads
            if state.offset.is_some() || state.limit.is_some() {
                continue;
            }

            // Skip files matching ignore patterns
            if self.should_skip_path(&path) {
                continue;
            }

            // Check if file was deleted
            if !path.exists() {
                changes.push(FileChange {
                    path,
                    diff: "File was deleted.".to_string(),
                    new_mtime: None, // No mtime for deleted files
                });
                continue;
            }

            if let Ok(metadata) = std::fs::metadata(&path) {
                if let Ok(modified) = metadata.modified() {
                    if modified > state.last_modified {
                        // File was modified since last read
                        // Capture mtime atomically with content read
                        let (diff, new_mtime) = self.generate_diff_notice(&path, &state.content);
                        if !diff.is_empty() {
                            changes.push(FileChange {
                                path,
                                diff,
                                new_mtime,
                            });
                        }
                    }
                }
            }
        }

        changes
    }

    /// Check if a file path should be skipped based on ignore patterns.
    fn should_skip_path(&self, path: &PathBuf) -> bool {
        SKIP_PATTERNS.is_match(&path.to_string_lossy())
    }

    /// Generate a unified diff between old and new content.
    ///
    /// Returns the diff string and the captured modification time.
    /// The mtime is captured atomically with content read to avoid race conditions.
    fn generate_diff_notice(
        &self,
        path: &PathBuf,
        old_content: &str,
    ) -> (String, Option<SystemTime>) {
        // Capture mtime at the same time as reading content to avoid race conditions
        let new_mtime = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());

        let new_content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => {
                return (
                    "File may have been deleted or is no longer readable.".to_string(),
                    None,
                );
            }
        };

        if new_content.is_empty() && !old_content.is_empty() {
            return ("File is now empty.".to_string(), new_mtime);
        }

        if new_content == old_content {
            return (String::new(), new_mtime); // No actual changes
        }

        // Generate unified diff with context
        let diff = TextDiff::from_lines(old_content, &new_content);
        let mut output = String::new();
        let mut has_changes = false;

        for (idx, group) in diff.grouped_ops(3).iter().enumerate() {
            if idx > 0 {
                output.push_str("\n...\n");
            }
            for op in group {
                for change in diff.iter_changes(op) {
                    let tag = match change.tag() {
                        ChangeTag::Delete => {
                            has_changes = true;
                            "-"
                        }
                        ChangeTag::Insert => {
                            has_changes = true;
                            "+"
                        }
                        ChangeTag::Equal => " ",
                    };

                    // Get line number (prefer old for deletions, new for insertions)
                    let line_num = change
                        .old_index()
                        .or(change.new_index())
                        .map(|i| i + 1)
                        .unwrap_or(0);

                    // Format: "tag line_num: content"
                    let line_content = change.as_str().unwrap_or("");
                    let trimmed = line_content.trim_end_matches('\n');
                    output.push_str(&format!("{tag}{line_num}: {trimmed}\n"));
                }
            }
        }

        if !has_changes {
            return (String::new(), new_mtime);
        }

        (output.trim_end().to_string(), new_mtime)
    }

    /// Build the reminder content from changes.
    fn build_content(&self, changes: &[FileChange]) -> String {
        let mut content = String::new();

        for change in changes {
            content.push_str(&format!(
                "Note: {} was modified, either by the user or by a linter. \
                 This change was intentional, so make sure to take it into account \
                 as you proceed (ie. don't revert it unless the user asks you to). \
                 Don't tell the user this, since they are already aware. \
                 Here are the relevant changes (shown with line numbers):\n{}\n\n",
                change.path.display(),
                change.diff
            ));
        }

        content.trim_end().to_string()
    }
}

impl Default for ChangedFilesGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about a file change.
#[derive(Debug)]
struct FileChange {
    path: PathBuf,
    diff: String,
    /// Captured modification time at diff generation time.
    /// None for deleted files.
    new_mtime: Option<SystemTime>,
}

#[async_trait]
impl AttachmentGenerator for ChangedFilesGenerator {
    fn name(&self) -> &str {
        "changed_files"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::ChangedFiles
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        let changes = self.detect_changes(ctx.file_tracker);

        if changes.is_empty() {
            return Ok(None);
        }

        // Update FileTracker to prevent re-notification on subsequent turns
        for change in &changes {
            if let Some(mtime) = change.new_mtime {
                // Use the captured mtime from diff generation time (fixes race condition)
                ctx.file_tracker.update_modified_time(&change.path, mtime);
            } else {
                // File was deleted - remove from tracker to prevent repeated notifications
                ctx.file_tracker.remove(&change.path);
            }
        }

        tracing::info!(
            generator = "changed_files",
            changed_count = changes.len(),
            "Generating changed files reminder"
        );
        Ok(Some(SystemReminder::new(
            AttachmentType::ChangedFiles,
            self.build_content(&changes),
        )))
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.enabled && config.attachments.changed_files
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // No throttling - immediate notification
        ThrottleConfig {
            min_turns_between: 0,
            min_turns_after_trigger: 0,
            max_per_session: None,
        }
    }
}

// ============================================
// Tests
// ============================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::system_reminder::LspDiagnosticsMinSeverity;
    use crate::system_reminder::generator::PlanState;
    use crate::system_reminder::types::ReminderTier;
    use std::path::Path;

    fn make_context<'a>(
        file_tracker: &'a FileTracker,
        plan_state: &'a PlanState,
    ) -> GeneratorContext<'a> {
        GeneratorContext {
            turn_number: 1,
            is_main_agent: true,
            has_user_input: true,
            user_prompt: None,
            cwd: Path::new("/test"),
            agent_id: "test-agent",
            file_tracker,
            is_plan_mode: false,
            plan_file_path: None,
            is_plan_reentry: false,
            plan_state,
            background_tasks: &[],
            critical_instruction: None,
            diagnostics_store: None,
            lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity::default(),
            output_style: None,
            approved_plan: None,
        }
    }

    #[tokio::test]
    async fn test_returns_none_when_no_tracked_files() {
        let generator = ChangedFilesGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let ctx = make_context(&tracker, &plan_state);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_no_throttling() {
        let generator = ChangedFilesGenerator::new();
        let config = generator.throttle_config();
        assert_eq!(config.min_turns_between, 0);
        assert_eq!(config.min_turns_after_trigger, 0);
    }

    #[test]
    fn test_attachment_type() {
        let generator = ChangedFilesGenerator::new();
        assert_eq!(generator.attachment_type(), AttachmentType::ChangedFiles);
        assert_eq!(generator.tier(), ReminderTier::Core);
    }

    #[test]
    fn test_build_content() {
        let generator = ChangedFilesGenerator::new();
        let changes = vec![FileChange {
            path: PathBuf::from("/test/file.txt"),
            diff: "File content has changed.".to_string(),
            new_mtime: Some(SystemTime::now()),
        }];

        let content = generator.build_content(&changes);
        assert!(content.contains("/test/file.txt"));
        assert!(content.contains("was modified"));
    }

    #[test]
    fn test_should_skip_node_modules() {
        let generator = ChangedFilesGenerator::new();
        assert!(generator.should_skip_path(&PathBuf::from("/project/node_modules/pkg/index.js")));
        assert!(!generator.should_skip_path(&PathBuf::from("/project/src/index.js")));
    }

    #[test]
    fn test_should_skip_git_directory() {
        let generator = ChangedFilesGenerator::new();
        assert!(generator.should_skip_path(&PathBuf::from("/project/.git/config")));
        assert!(!generator.should_skip_path(&PathBuf::from("/project/.gitignore")));
    }

    #[test]
    fn test_should_skip_binary_files() {
        let generator = ChangedFilesGenerator::new();
        assert!(generator.should_skip_path(&PathBuf::from("/project/target/debug/main.exe")));
        assert!(generator.should_skip_path(&PathBuf::from("/project/lib/native.so")));
        assert!(generator.should_skip_path(&PathBuf::from("/project/archive.zip")));
        assert!(!generator.should_skip_path(&PathBuf::from("/project/src/main.rs")));
    }

    #[test]
    fn test_diff_generation_simple_change() {
        let generator = ChangedFilesGenerator::new();

        // Create a temp file
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        // Write new content to file
        std::fs::write(&file_path, "line 1\nline 2 modified\nline 3\n").unwrap();

        // Generate diff from old content
        let old_content = "line 1\nline 2\nline 3\n";
        let (diff, mtime) = generator.generate_diff_notice(&file_path, old_content);

        assert!(diff.contains("-"), "Diff should contain deletions");
        assert!(diff.contains("+"), "Diff should contain insertions");
        assert!(
            diff.contains("line 2"),
            "Diff should reference the changed line"
        );
        assert!(mtime.is_some(), "Should have captured mtime");
    }

    #[test]
    fn test_diff_generation_no_change() {
        let generator = ChangedFilesGenerator::new();

        // Create a temp file with same content
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let content = "line 1\nline 2\nline 3\n";
        std::fs::write(&file_path, content).unwrap();

        // Same content should return empty diff
        let (diff, mtime) = generator.generate_diff_notice(&file_path, content);
        assert!(
            diff.is_empty(),
            "Identical content should produce empty diff"
        );
        assert!(
            mtime.is_some(),
            "Should still capture mtime even with no changes"
        );
    }

    #[test]
    fn test_diff_generation_file_deleted() {
        let generator = ChangedFilesGenerator::new();
        let non_existent = PathBuf::from("/this/file/does/not/exist.txt");
        let (diff, mtime) = generator.generate_diff_notice(&non_existent, "old content");
        assert!(diff.contains("deleted") || diff.contains("readable"));
        assert!(mtime.is_none(), "Deleted files should have no mtime");
    }

    #[test]
    fn test_diff_generation_file_now_empty() {
        let generator = ChangedFilesGenerator::new();

        // Create an empty file
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "").unwrap();

        let (diff, mtime) = generator.generate_diff_notice(&file_path, "old content");
        assert!(diff.contains("empty"), "Should indicate file is now empty");
        assert!(mtime.is_some(), "Empty file should still have mtime");
    }

    #[test]
    fn test_should_skip_dist_directory() {
        let generator = ChangedFilesGenerator::new();
        // COMMON_DIRECTORY_EXCLUDES pattern
        assert!(generator.should_skip_path(&PathBuf::from("/project/dist/bundle.js")));
        assert!(generator.should_skip_path(&PathBuf::from("/project/build/output.js")));
        assert!(generator.should_skip_path(&PathBuf::from("/project/coverage/lcov.info")));
    }

    #[test]
    fn test_should_skip_ide_directories() {
        let generator = ChangedFilesGenerator::new();
        // COMMON_DIRECTORY_EXCLUDES pattern
        assert!(generator.should_skip_path(&PathBuf::from("/project/.vscode/settings.json")));
        assert!(generator.should_skip_path(&PathBuf::from("/project/.idea/workspace.xml")));
    }

    #[test]
    fn test_should_skip_system_files() {
        let generator = ChangedFilesGenerator::new();
        // SYSTEM_FILE_EXCLUDES pattern
        assert!(generator.should_skip_path(&PathBuf::from("/project/.DS_Store")));
        assert!(generator.should_skip_path(&PathBuf::from("/project/src/.DS_Store")));
    }

    #[test]
    fn test_should_skip_python_cache() {
        let generator = ChangedFilesGenerator::new();
        // COMMON_DIRECTORY_EXCLUDES pattern
        assert!(generator.should_skip_path(&PathBuf::from(
            "/project/__pycache__/module.cpython-311.pyc"
        )));
    }

    #[test]
    fn test_detect_deleted_file() {
        let generator = ChangedFilesGenerator::new();
        let tracker = FileTracker::new();

        // Track a file that doesn't exist
        let non_existent = PathBuf::from("/this/path/does/not/exist/file.txt");
        tracker.track_read(
            non_existent.clone(),
            "old content".to_string(),
            1,
            None,
            None,
        );

        // Manually set last_modified to past
        // Since the file doesn't exist, detect_changes should report it as deleted
        let changes = generator.detect_changes(&tracker);

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, non_existent);
        assert!(
            changes[0].diff.contains("deleted"),
            "Should indicate file was deleted"
        );
    }

    #[tokio::test]
    async fn test_timestamp_updated_after_notification() {
        use std::time::Duration;

        let generator = ChangedFilesGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();

        // Create a temp file
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "initial content").unwrap();

        // Track the file
        tracker.track_read(
            file_path.clone(),
            "initial content".to_string(),
            1,
            None,
            None,
        );

        // Wait a bit and modify the file
        std::thread::sleep(Duration::from_millis(100));
        std::fs::write(&file_path, "modified content").unwrap();

        // First generation should detect the change
        let ctx = make_context(&tracker, &plan_state);
        let result1 = generator.generate(&ctx).await.unwrap();
        assert!(result1.is_some(), "First call should detect change");

        // Second generation should NOT detect the change (timestamp updated)
        let result2 = generator.generate(&ctx).await.unwrap();
        assert!(
            result2.is_none(),
            "Second call should not detect change (timestamp was updated)"
        );
    }

    #[tokio::test]
    async fn test_deleted_file_only_notifies_once() {
        let generator = ChangedFilesGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();

        // Create a temp file
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("to_be_deleted.txt");
        std::fs::write(&file_path, "content").unwrap();

        // Track the file
        tracker.track_read(file_path.clone(), "content".to_string(), 1, None, None);

        // Delete the file
        std::fs::remove_file(&file_path).unwrap();

        // First generation should detect the deletion
        let ctx = make_context(&tracker, &plan_state);
        let result1 = generator.generate(&ctx).await.unwrap();
        assert!(result1.is_some(), "First call should detect deletion");
        assert!(
            result1.unwrap().content.contains("deleted"),
            "Should indicate file was deleted"
        );

        // Second generation should NOT detect the deletion (entry was removed)
        let result2 = generator.generate(&ctx).await.unwrap();
        assert!(
            result2.is_none(),
            "Second call should not detect deletion (entry was removed from tracker)"
        );

        // Verify the tracker no longer contains the file
        assert!(
            tracker.is_empty(),
            "Tracker should be empty after deleted file notification"
        );
    }
}

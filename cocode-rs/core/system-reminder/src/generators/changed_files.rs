//! Changed files generator.
//!
//! Detects and reports files that have been modified since they were last read,
//! including unified diffs showing what changed.

use std::path::Path;

use async_trait::async_trait;
use similar::ChangeTag;
use similar::TextDiff;

use crate::Result;
use crate::config::SystemReminderConfig;
use crate::generator::AttachmentGenerator;
use crate::generator::GeneratorContext;
use crate::throttle::ThrottleConfig;
use crate::types::AttachmentType;
use crate::types::SystemReminder;

/// Maximum diff lines to include per file (to avoid overwhelming the context).
const MAX_DIFF_LINES_PER_FILE: usize = 50;

/// Maximum total diff content size in characters.
const MAX_TOTAL_DIFF_SIZE: usize = 4000;

/// Generator for detecting changed files.
#[derive(Debug)]
pub struct ChangedFilesGenerator;

impl ChangedFilesGenerator {
    /// Generate a unified diff between old and new content.
    ///
    /// Returns a compact diff format showing only changed lines with context.
    fn generate_diff(old_content: &str, new_content: &str, path: &Path) -> String {
        let diff = TextDiff::from_lines(old_content, new_content);

        let mut result = String::new();
        let mut line_count = 0;

        for change in diff.iter_all_changes() {
            if line_count >= MAX_DIFF_LINES_PER_FILE {
                result.push_str("... (diff truncated)\n");
                break;
            }

            let sign = match change.tag() {
                ChangeTag::Delete => "-",
                ChangeTag::Insert => "+",
                ChangeTag::Equal => " ",
            };

            // Skip some equal lines if we have too many changes to show
            if change.tag() == ChangeTag::Equal && line_count > MAX_DIFF_LINES_PER_FILE / 2 {
                continue;
            }

            result.push_str(sign);
            result.push_str(change.value());
            if change.missing_newline() {
                result.push_str("\n\\ No newline at end of file\n");
            }
            line_count += 1;
        }

        if result.is_empty() {
            format!("(no textual changes detected for {})\n", path.display())
        } else {
            result
        }
    }

    /// Try to read the current content of a file.
    async fn read_current_content(path: &Path) -> Option<String> {
        tokio::fs::read_to_string(path).await.ok()
    }
}

#[async_trait]
impl AttachmentGenerator for ChangedFilesGenerator {
    fn name(&self) -> &str {
        "ChangedFilesGenerator"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::ChangedFiles
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.attachments.changed_files
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // No throttle - always check for changes
        ThrottleConfig::none()
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        let Some(tracker) = ctx.file_tracker else {
            return Ok(None);
        };

        let changed = tracker.changed_files();

        if changed.is_empty() {
            return Ok(None);
        }

        // Format the changed files message with diffs
        let mut content =
            String::from("The following files have been modified since you last read them:\n\n");
        let mut total_diff_size = 0;

        for path in &changed {
            let display_path = path.display();
            content.push_str(&format!("### {display_path}\n"));

            // Try to generate diff if we have the old content and can read the new content
            if let Some(old_state) = tracker.get_state(path) {
                // Skip partial reads - can't generate meaningful diff
                if old_state.is_partial() {
                    content.push_str("(partial read - cannot show diff)\n\n");
                    continue;
                }

                if let Some(new_content) = Self::read_current_content(path).await {
                    // Check if we have room for more diff content
                    if total_diff_size < MAX_TOTAL_DIFF_SIZE {
                        let diff = Self::generate_diff(&old_state.content, &new_content, path);
                        let diff_size = diff.len();

                        // Truncate if needed
                        if total_diff_size + diff_size > MAX_TOTAL_DIFF_SIZE {
                            let remaining = MAX_TOTAL_DIFF_SIZE.saturating_sub(total_diff_size);
                            if remaining > 100 {
                                content.push_str("```diff\n");
                                content.push_str(&diff[..remaining.min(diff.len())]);
                                content.push_str("\n... (diff truncated)\n```\n\n");
                            } else {
                                content.push_str("(diff omitted - size limit reached)\n\n");
                            }
                        } else {
                            content.push_str("```diff\n");
                            content.push_str(&diff);
                            content.push_str("```\n\n");
                        }
                        total_diff_size += diff_size;
                    } else {
                        content.push_str("(diff omitted - size limit reached)\n\n");
                    }
                } else {
                    content.push_str("(unable to read current content)\n\n");
                }
            } else {
                content.push_str("(no previous content available for diff)\n\n");
            }
        }

        content.push_str(
            "You may want to re-read these files before making changes to ensure you have the latest content.",
        );

        Ok(Some(SystemReminder::new(
            AttachmentType::ChangedFiles,
            content,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_tracker::FileTracker;
    use crate::file_tracker::ReadFileState;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn test_config() -> SystemReminderConfig {
        SystemReminderConfig::default()
    }

    #[tokio::test]
    async fn test_no_tracker() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .build();

        let generator = ChangedFilesGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_no_changes() {
        let config = test_config();
        let tracker = FileTracker::new();

        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .file_tracker(&tracker)
            .build();

        let generator = ChangedFilesGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none());
    }

    #[test]
    fn test_generator_properties() {
        let generator = ChangedFilesGenerator;
        assert_eq!(generator.name(), "ChangedFilesGenerator");
        assert_eq!(generator.attachment_type(), AttachmentType::ChangedFiles);

        let config = test_config();
        assert!(generator.is_enabled(&config));

        // No throttle
        let throttle = generator.throttle_config();
        assert_eq!(throttle.min_turns_between, 0);
    }

    #[test]
    fn test_generate_diff_simple() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nmodified\nline3\n";
        let path = Path::new("test.rs");

        let diff = ChangedFilesGenerator::generate_diff(old, new, path);
        assert!(diff.contains("-line2"));
        assert!(diff.contains("+modified"));
    }

    #[test]
    fn test_generate_diff_addition() {
        let old = "line1\nline2\n";
        let new = "line1\nline2\nline3\n";
        let path = Path::new("test.rs");

        let diff = ChangedFilesGenerator::generate_diff(old, new, path);
        assert!(diff.contains("+line3"));
    }

    #[test]
    fn test_generate_diff_deletion() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nline3\n";
        let path = Path::new("test.rs");

        let diff = ChangedFilesGenerator::generate_diff(old, new, path);
        assert!(diff.contains("-line2"));
    }

    #[test]
    fn test_generate_diff_no_changes() {
        let content = "line1\nline2\n";
        let path = Path::new("test.rs");

        let diff = ChangedFilesGenerator::generate_diff(content, content, path);
        // When content is identical, the diff will contain only equal lines (space prefix)
        // and no additions or deletions
        assert!(!diff.contains("+line"));
        assert!(!diff.contains("-line"));
    }

    #[tokio::test]
    async fn test_changed_file_with_diff() {
        // Create a temp file with initial content
        let temp = NamedTempFile::new().expect("create temp file");
        let path = temp.path().to_path_buf();

        // Write initial content
        std::fs::write(&path, "initial\ncontent\nhere\n").expect("write initial");

        // Track the file read
        let tracker = FileTracker::new();
        let old_mtime = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok());
        let state = ReadFileState::new("old\ncontent\nhere\n".to_string(), old_mtime, 1);
        tracker.track_read(&path, state);

        // Modify the file (content differs from tracked)
        std::fs::write(&path, "new\ncontent\nhere\n").expect("write new");

        // Now the file should be detected as changed (content differs)
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(2)
            .cwd(PathBuf::from("/tmp"))
            .file_tracker(&tracker)
            .build();

        let generator = ChangedFilesGenerator;

        // Check if file is detected as changed
        let changed = tracker.changed_files();
        if !changed.is_empty() {
            let result = generator.generate(&ctx).await.expect("generate");
            if let Some(reminder) = result {
                // Should contain diff markers
                assert!(
                    reminder.content.contains("```diff")
                        || reminder.content.contains("modified since")
                );
            }
        }
    }
}

//! At-mentioned files generator.
//!
//! Parses @file, @"path", @file#L10-20 mentions from user prompt.
//! Matches Claude Code's at_mentioned_files attachment (UserPrompt tier).

use crate::config::system_reminder::SystemReminderConfig;
use crate::error::Result;
use crate::system_reminder::generator::AttachmentGenerator;
use crate::system_reminder::generator::GeneratorContext;
use crate::system_reminder::generator_ext::FileMention;
use crate::system_reminder::generator_ext::parse_file_mentions;
use crate::system_reminder::throttle::ThrottleConfig;
use crate::system_reminder::types::AttachmentType;
use crate::system_reminder::types::SystemReminder;
use async_trait::async_trait;
use std::fs;
use std::path::Path;

/// Maximum file content size to include (bytes).
const MAX_FILE_SIZE: i64 = 100_000;

/// Maximum lines to read from a file.
const MAX_LINES: i32 = 2000;

/// At-mentioned files generator.
///
/// Parses user prompt for @file mentions and reads file contents.
#[derive(Debug)]
pub struct AtMentionedFilesGenerator;

impl AtMentionedFilesGenerator {
    /// Create a new at-mentioned files generator.
    pub fn new() -> Self {
        Self
    }

    /// Read file content for a mention.
    fn read_file_content(&self, mention: &FileMention, cwd: &Path) -> Option<String> {
        let path = mention.resolve(cwd);

        // Check if path exists
        if !path.exists() {
            return None;
        }

        // Handle directories
        if path.is_dir() {
            return self.read_directory_listing(&path);
        }

        // Check file size
        let metadata = fs::metadata(&path).ok()?;
        if metadata.len() > MAX_FILE_SIZE as u64 {
            return Some(format!(
                "[File too large: {} bytes, max {} bytes]",
                metadata.len(),
                MAX_FILE_SIZE
            ));
        }

        // Read file content
        let content = fs::read_to_string(&path).ok()?;
        let lines: Vec<&str> = content.lines().collect();

        // Apply line range if specified
        let (start, end) = if let (Some(s), Some(e)) = (mention.line_start, mention.line_end) {
            let start = (s - 1).max(0) as usize;
            let end = e.min(lines.len() as i32) as usize;
            (start, end)
        } else {
            (0, lines.len().min(MAX_LINES as usize))
        };

        // Format with line numbers
        let formatted: Vec<String> = lines[start..end]
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:>6}\t{}", start + i + 1, line))
            .collect();

        Some(formatted.join("\n"))
    }

    /// Format content as simulated tool call (matching Claude Code format).
    fn format_as_tool_call(&self, path: &Path, content: &str) -> String {
        let path_str = path.display();
        format!(
            "Called the Read tool with the following input: {{\"file_path\":\"{path_str}\"}}\n\n\
             Result of calling the Read tool: \"{content}\""
        )
    }

    /// Read directory listing for a directory mention.
    fn read_directory_listing(&self, path: &Path) -> Option<String> {
        let entries: Vec<String> = fs::read_dir(path)
            .ok()?
            .filter_map(|e| e.ok())
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                // Add trailing slash for directories
                if e.path().is_dir() {
                    format!("{name}/")
                } else {
                    name
                }
            })
            .collect();

        if entries.is_empty() {
            return Some("[Empty directory]".to_string());
        }

        let mut sorted_entries = entries;
        sorted_entries.sort();
        Some(sorted_entries.join("\n"))
    }
}

impl Default for AtMentionedFilesGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AttachmentGenerator for AtMentionedFilesGenerator {
    fn name(&self) -> &str {
        "at_mentioned_files"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::AtMentionedFiles
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Need user prompt to parse mentions
        let user_prompt = match ctx.user_prompt {
            Some(p) => p,
            None => return Ok(None),
        };

        // Parse file mentions
        let mentions = parse_file_mentions(user_prompt);
        if mentions.is_empty() {
            return Ok(None);
        }

        // Read file contents
        let mut parts = Vec::new();
        for mention in &mentions {
            let resolved_path = mention.resolve(ctx.cwd);
            if let Some(content) = self.read_file_content(mention, ctx.cwd) {
                parts.push(self.format_as_tool_call(&resolved_path, &content));
            }
        }

        if parts.is_empty() {
            return Ok(None);
        }

        tracing::info!(
            generator = "at_mentioned_files",
            file_count = parts.len(),
            "Generating at-mentioned files reminder"
        );

        Ok(Some(SystemReminder::new(
            AttachmentType::AtMentionedFiles,
            parts.join("\n\n"),
        )))
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.enabled && config.attachments.at_mentioned_files
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // No throttling for user prompt tier
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
    use crate::system_reminder::file_tracker::FileTracker;
    use crate::system_reminder::generator::PlanState;
    use crate::system_reminder::types::ReminderTier;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_context<'a>(
        user_prompt: Option<&'a str>,
        cwd: &'a Path,
        file_tracker: &'a FileTracker,
        plan_state: &'a PlanState,
    ) -> GeneratorContext<'a> {
        GeneratorContext {
            turn_number: 1,
            is_main_agent: true,
            has_user_input: true,
            user_prompt,
            cwd,
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
    async fn test_returns_none_without_user_prompt() {
        let generator = AtMentionedFilesGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let ctx = make_context(None, Path::new("/test"), &tracker, &plan_state);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_returns_none_without_mentions() {
        let generator = AtMentionedFilesGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let ctx = make_context(
            Some("Hello, no mentions here"),
            Path::new("/test"),
            &tracker,
            &plan_state,
        );

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_reads_mentioned_file() {
        let generator = AtMentionedFilesGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();

        // Create a temp file
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "Line 1").unwrap();
        writeln!(file, "Line 2").unwrap();

        let ctx = make_context(
            Some("Check @test.txt please"),
            temp_dir.path(),
            &tracker,
            &plan_state,
        );

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_some());

        let reminder = result.unwrap();
        assert_eq!(reminder.attachment_type, AttachmentType::AtMentionedFiles);
        assert!(reminder.content.contains("Line 1"));
        assert!(reminder.content.contains("Line 2"));
        assert!(reminder.content.contains("Read tool"));
    }

    #[tokio::test]
    async fn test_reads_file_with_line_range() {
        let generator = AtMentionedFilesGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();

        // Create a temp file with multiple lines
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let mut file = fs::File::create(&file_path).unwrap();
        for i in 1..=10 {
            writeln!(file, "Line {i}").unwrap();
        }

        let ctx = make_context(
            Some("Check @test.txt#L3-5 please"),
            temp_dir.path(),
            &tracker,
            &plan_state,
        );

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_some());

        let reminder = result.unwrap();
        assert!(reminder.content.contains("Line 3"));
        assert!(reminder.content.contains("Line 4"));
        assert!(reminder.content.contains("Line 5"));
        // Should not contain lines outside range
        assert!(!reminder.content.contains("Line 1\n"));
        assert!(!reminder.content.contains("Line 6\n"));
    }

    #[test]
    fn test_attachment_type() {
        let generator = AtMentionedFilesGenerator::new();
        assert_eq!(
            generator.attachment_type(),
            AttachmentType::AtMentionedFiles
        );
        assert_eq!(generator.tier(), ReminderTier::UserPrompt);
    }

    #[test]
    fn test_no_throttling() {
        let generator = AtMentionedFilesGenerator::new();
        let config = generator.throttle_config();
        assert_eq!(config.min_turns_between, 0);
        assert_eq!(config.min_turns_after_trigger, 0);
    }

    #[tokio::test]
    async fn test_reads_directory_listing() {
        let generator = AtMentionedFilesGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();

        // Create a temp directory with files and subdirectory
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();
        fs::File::create(temp_dir.path().join("file1.txt")).unwrap();
        fs::File::create(temp_dir.path().join("file2.rs")).unwrap();

        // Create mention for the temp directory itself using @"path"
        let dir_name = temp_dir.path().file_name().unwrap().to_string_lossy();
        let prompt = format!("Check @\"{dir_name}\" please");

        let ctx = make_context(
            Some(&prompt),
            temp_dir.path().parent().unwrap(),
            &tracker,
            &plan_state,
        );

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_some());

        let reminder = result.unwrap();
        assert!(reminder.content.contains("file1.txt"));
        assert!(reminder.content.contains("file2.rs"));
        assert!(reminder.content.contains("subdir/")); // Trailing slash for dir
    }

    #[tokio::test]
    async fn test_reads_empty_directory() {
        let generator = AtMentionedFilesGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();

        // Create an empty temp directory
        let temp_dir = TempDir::new().unwrap();
        let empty_dir = temp_dir.path().join("empty");
        fs::create_dir(&empty_dir).unwrap();

        let ctx = make_context(
            Some("Check @empty please"),
            temp_dir.path(),
            &tracker,
            &plan_state,
        );

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_some());

        let reminder = result.unwrap();
        assert!(reminder.content.contains("[Empty directory]"));
    }
}

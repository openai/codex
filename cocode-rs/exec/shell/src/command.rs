//! Command input and result types for shell execution.

use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

/// Extracted file paths from command output.
///
/// When a fast model is configured, the shell executor can analyze command
/// output to extract file paths that the command read or modified. This enables
/// fast model pre-reading for improved context.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExtractedPaths {
    /// File paths extracted from command output.
    pub paths: Vec<PathBuf>,
    /// Whether extraction was attempted.
    pub extraction_attempted: bool,
    /// Duration of extraction in milliseconds.
    pub extraction_ms: i64,
}

impl ExtractedPaths {
    /// Creates a new ExtractedPaths with the given paths.
    pub fn new(paths: Vec<PathBuf>, extraction_ms: i64) -> Self {
        Self {
            paths,
            extraction_attempted: true,
            extraction_ms,
        }
    }

    /// Creates an ExtractedPaths indicating extraction was not attempted.
    pub fn not_attempted() -> Self {
        Self {
            paths: Vec::new(),
            extraction_attempted: false,
            extraction_ms: 0,
        }
    }

    /// Returns true if any paths were extracted.
    pub fn has_paths(&self) -> bool {
        !self.paths.is_empty()
    }
}

/// Result of a shell command execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandResult {
    /// Process exit code (0 = success).
    pub exit_code: i32,
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
    /// Execution duration in milliseconds.
    pub duration_ms: i64,
    /// Whether the output was truncated due to size limits.
    pub truncated: bool,
    /// New working directory after command execution (if changed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_cwd: Option<PathBuf>,
    /// File paths extracted from command output (when fast model configured).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extracted_paths: Option<ExtractedPaths>,
}

impl CommandResult {
    /// Returns true if the command exited successfully (exit code 0).
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }
}

/// Input parameters for a shell command execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandInput {
    /// The shell command to execute.
    pub command: String,
    /// Optional timeout in milliseconds. Defaults to executor's default if None.
    #[serde(default)]
    pub timeout_ms: Option<i64>,
    /// Optional working directory override.
    #[serde(default)]
    pub working_dir: Option<PathBuf>,
    /// Optional human-readable description of what the command does.
    #[serde(default)]
    pub description: Option<String>,
    /// Whether to run the command in the background.
    #[serde(default)]
    pub run_in_background: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_result_success() {
        let result = CommandResult {
            exit_code: 0,
            stdout: "hello".to_string(),
            stderr: String::new(),
            duration_ms: 100,
            truncated: false,
            new_cwd: None,
            extracted_paths: None,
        };
        assert!(result.success());
    }

    #[test]
    fn test_command_result_failure() {
        let result = CommandResult {
            exit_code: 1,
            stdout: String::new(),
            stderr: "error".to_string(),
            duration_ms: 50,
            truncated: false,
            new_cwd: None,
            extracted_paths: None,
        };
        assert!(!result.success());
    }

    #[test]
    fn test_command_result_truncated() {
        let result = CommandResult {
            exit_code: 0,
            stdout: "partial...".to_string(),
            stderr: String::new(),
            duration_ms: 200,
            truncated: true,
            new_cwd: None,
            extracted_paths: None,
        };
        assert!(result.truncated);
        assert!(result.success());
    }

    #[test]
    fn test_command_input_defaults() {
        let input: CommandInput = serde_json::from_str(r#"{"command":"ls"}"#).expect("parse");
        assert_eq!(input.command, "ls");
        assert!(input.timeout_ms.is_none());
        assert!(input.working_dir.is_none());
        assert!(input.description.is_none());
        assert!(!input.run_in_background);
    }

    #[test]
    fn test_command_input_full() {
        let input = CommandInput {
            command: "cargo build".to_string(),
            timeout_ms: Some(30000),
            working_dir: Some(PathBuf::from("/tmp")),
            description: Some("Build the project".to_string()),
            run_in_background: true,
        };

        let json = serde_json::to_string(&input).expect("serialize");
        let parsed: CommandInput = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.command, "cargo build");
        assert_eq!(parsed.timeout_ms, Some(30000));
        assert_eq!(parsed.working_dir, Some(PathBuf::from("/tmp")));
        assert_eq!(parsed.description.as_deref(), Some("Build the project"));
        assert!(parsed.run_in_background);
    }

    #[test]
    fn test_command_result_serde_roundtrip() {
        let result = CommandResult {
            exit_code: 0,
            stdout: "output".to_string(),
            stderr: "warn".to_string(),
            duration_ms: 1234,
            truncated: false,
            new_cwd: Some(PathBuf::from("/home/user")),
            extracted_paths: None,
        };

        let json = serde_json::to_string(&result).expect("serialize");
        let parsed: CommandResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.exit_code, result.exit_code);
        assert_eq!(parsed.stdout, result.stdout);
        assert_eq!(parsed.stderr, result.stderr);
        assert_eq!(parsed.duration_ms, result.duration_ms);
        assert_eq!(parsed.truncated, result.truncated);
        assert_eq!(parsed.new_cwd, result.new_cwd);
        assert_eq!(
            parsed.extracted_paths.is_none(),
            result.extracted_paths.is_none()
        );
    }

    #[test]
    fn test_command_result_new_cwd_skipped_when_none() {
        let result = CommandResult {
            exit_code: 0,
            stdout: "ok".to_string(),
            stderr: String::new(),
            duration_ms: 10,
            truncated: false,
            new_cwd: None,
            extracted_paths: None,
        };

        let json = serde_json::to_string(&result).expect("serialize");
        // new_cwd should not appear in JSON when None
        assert!(!json.contains("new_cwd"));
        // extracted_paths should not appear in JSON when None
        assert!(!json.contains("extracted_paths"));
    }

    #[test]
    fn test_extracted_paths_new() {
        let paths = vec![PathBuf::from("/file1.txt"), PathBuf::from("/file2.txt")];
        let extracted = ExtractedPaths::new(paths.clone(), 50);

        assert_eq!(extracted.paths, paths);
        assert!(extracted.extraction_attempted);
        assert_eq!(extracted.extraction_ms, 50);
        assert!(extracted.has_paths());
    }

    #[test]
    fn test_extracted_paths_not_attempted() {
        let extracted = ExtractedPaths::not_attempted();

        assert!(extracted.paths.is_empty());
        assert!(!extracted.extraction_attempted);
        assert_eq!(extracted.extraction_ms, 0);
        assert!(!extracted.has_paths());
    }

    #[test]
    fn test_extracted_paths_default() {
        let extracted = ExtractedPaths::default();

        assert!(extracted.paths.is_empty());
        assert!(!extracted.extraction_attempted);
        assert_eq!(extracted.extraction_ms, 0);
    }

    #[test]
    fn test_command_result_with_extracted_paths() {
        let extracted = ExtractedPaths::new(vec![PathBuf::from("/test.rs")], 25);
        let result = CommandResult {
            exit_code: 0,
            stdout: "output".to_string(),
            stderr: String::new(),
            duration_ms: 100,
            truncated: false,
            new_cwd: None,
            extracted_paths: Some(extracted),
        };

        let json = serde_json::to_string(&result).expect("serialize");
        assert!(json.contains("extracted_paths"));
        assert!(json.contains("/test.rs"));

        let parsed: CommandResult = serde_json::from_str(&json).expect("deserialize");
        assert!(parsed.extracted_paths.is_some());
        let paths = parsed.extracted_paths.expect("extracted_paths");
        assert_eq!(paths.paths.len(), 1);
        assert!(paths.extraction_attempted);
    }
}

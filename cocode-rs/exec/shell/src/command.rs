//! Command input and result types for shell execution.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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
        };

        let json = serde_json::to_string(&result).expect("serialize");
        let parsed: CommandResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.exit_code, result.exit_code);
        assert_eq!(parsed.stdout, result.stdout);
        assert_eq!(parsed.stderr, result.stderr);
        assert_eq!(parsed.duration_ms, result.duration_ms);
        assert_eq!(parsed.truncated, result.truncated);
    }
}

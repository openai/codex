//! Rich Grep Handler - Search file contents with ripgrep
//!
//! This module provides the RipGrepHandler which searches file contents
//! using ripgrep in JSON mode, returning matching lines with file paths
//! and line numbers.

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use indexmap::IndexMap;
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// Internal safety limit (not exposed to LLM)
const INTERNAL_LIMIT: usize = 2000;

/// Command timeout
const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);

/// Rich Grep tool arguments
#[derive(Debug, Clone, Deserialize)]
struct RipGrepArgs {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    include: Option<String>,
    #[serde(default)]
    case_sensitive: Option<bool>,
    #[serde(default)]
    fixed_strings: Option<bool>,
    #[serde(default)]
    context: Option<i32>,
    #[serde(default)]
    after: Option<i32>,
    #[serde(default)]
    before: Option<i32>,
}

/// Match result from ripgrep JSON output
#[derive(Debug)]
struct GrepMatch {
    file_path: String,
    line_number: i32,
    line_content: String,
    is_context: bool,
}

/// Rich Grep Handler using ripgrep JSON mode
pub struct RipGrepHandler;

#[async_trait]
impl ToolHandler for RipGrepHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        // 1. Parse arguments
        let arguments = match &invocation.payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "Invalid payload type for grep_files".to_string(),
                ));
            }
        };

        let args: RipGrepArgs = serde_json::from_str(arguments)
            .map_err(|e| FunctionCallError::RespondToModel(format!("Invalid arguments: {e}")))?;

        // Validate pattern
        if args.pattern.trim().is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "Pattern must not be empty".to_string(),
            ));
        }

        // 2. Resolve search path
        let search_path = invocation.turn.resolve_path(args.path.clone());

        // Verify path exists
        if !search_path.exists() {
            return Err(FunctionCallError::RespondToModel(format!(
                "Path does not exist: {}",
                search_path.display()
            )));
        }

        // 3. Build rg command
        let mut cmd = Command::new("rg");
        cmd.current_dir(&invocation.turn.cwd);

        // JSON output for structured parsing
        cmd.arg("--json");

        // Case sensitivity (default: insensitive)
        if !args.case_sensitive.unwrap_or(false) {
            cmd.arg("--ignore-case");
        }

        // Fixed strings mode
        if args.fixed_strings.unwrap_or(false) {
            cmd.arg("--fixed-strings");
        }

        // Context lines
        if let Some(c) = args.context {
            if c > 0 {
                cmd.arg("--context").arg(c.to_string());
            }
        }
        if let Some(a) = args.after {
            if a > 0 {
                cmd.arg("--after-context").arg(a.to_string());
            }
        }
        if let Some(b) = args.before {
            if b > 0 {
                cmd.arg("--before-context").arg(b.to_string());
            }
        }

        // ripgrep natively supports .ignore files, no need for --ignore-file

        // Glob filter
        if let Some(glob) = &args.include {
            cmd.arg("--glob").arg(glob);
        }

        // Sort by mtime (unique feature)
        cmd.arg("--sortr=modified");

        // Suppress error messages for binary files, permission errors, etc.
        cmd.arg("--no-messages");

        // Pattern and search path
        cmd.arg("--").arg(&args.pattern).arg(&search_path);

        // 4. Execute with timeout
        let output = timeout(COMMAND_TIMEOUT, cmd.output())
            .await
            .map_err(|_| FunctionCallError::RespondToModel("Grep command timed out".to_string()))?
            .map_err(|e| {
                FunctionCallError::RespondToModel(format!("Failed to execute ripgrep: {e}"))
            })?;

        // 5. Parse JSON output
        let matches = parse_json_output(&output.stdout, INTERNAL_LIMIT);

        // 6. Format output
        let content = format_output(&matches, &args.pattern, &search_path);

        Ok(ToolOutput::Function {
            content,
            content_items: None,
            success: Some(!matches.is_empty()),
        })
    }
}

/// Parse ripgrep JSON output into structured matches
fn parse_json_output(stdout: &[u8], limit: usize) -> Vec<GrepMatch> {
    let mut matches = Vec::new();

    for line in stdout.split(|b| *b == b'\n') {
        if matches.len() >= limit {
            break;
        }
        if line.is_empty() {
            continue;
        }

        // Parse JSON line
        let json: serde_json::Value = match serde_json::from_slice(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Handle different message types
        let msg_type = json.get("type").and_then(|t| t.as_str());

        match msg_type {
            Some("match") => {
                if let Some(m) = parse_match_message(&json) {
                    matches.push(m);
                }
            }
            Some("context") => {
                if let Some(m) = parse_context_message(&json) {
                    matches.push(m);
                }
            }
            _ => {}
        }
    }

    matches
}

/// Parse a "match" type message from ripgrep JSON
fn parse_match_message(json: &serde_json::Value) -> Option<GrepMatch> {
    let data = json.get("data")?;

    let path = data
        .get("path")
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())?;

    let line_num = data.get("line_number").and_then(|n| n.as_i64())?;

    let text = data
        .get("lines")
        .and_then(|l| l.get("text"))
        .and_then(|t| t.as_str())?;

    Some(GrepMatch {
        file_path: path.to_string(),
        line_number: line_num as i32,
        line_content: text.trim_end().to_string(),
        is_context: false,
    })
}

/// Parse a "context" type message from ripgrep JSON
fn parse_context_message(json: &serde_json::Value) -> Option<GrepMatch> {
    let data = json.get("data")?;

    let path = data
        .get("path")
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())?;

    let line_num = data.get("line_number").and_then(|n| n.as_i64())?;

    let text = data
        .get("lines")
        .and_then(|l| l.get("text"))
        .and_then(|t| t.as_str())?;

    Some(GrepMatch {
        file_path: path.to_string(),
        line_number: line_num as i32,
        line_content: text.trim_end().to_string(),
        is_context: true,
    })
}

/// Format matches into readable output
fn format_output(matches: &[GrepMatch], pattern: &str, path: &Path) -> String {
    if matches.is_empty() {
        return format!("No matches found for pattern \"{}\"", pattern);
    }

    // Count actual matches (not context lines)
    let match_count = matches.iter().filter(|m| !m.is_context).count();

    // Group by file - use IndexMap to preserve mtime order from ripgrep
    let mut by_file: IndexMap<&str, Vec<&GrepMatch>> = IndexMap::new();
    for m in matches {
        by_file.entry(&m.file_path).or_default().push(m);
    }

    let match_word = if match_count == 1 { "match" } else { "matches" };
    let mut output = format!(
        "Found {} {} for pattern \"{}\" in path \"{}\":\n---\n",
        match_count,
        match_word,
        pattern,
        path.display()
    );

    for (file, file_matches) in by_file {
        output.push_str(&format!("File: {}\n", file));
        for m in file_matches {
            let prefix = if m.is_context { " " } else { ">" };
            output.push_str(&format!(
                "{}L{}: {}\n",
                prefix, m.line_number, m.line_content
            ));
        }
        output.push_str("---\n");
    }

    // Add truncation notice if limit was hit
    if matches.len() >= INTERNAL_LIMIT {
        output.push_str(&format!(
            "\n(Output truncated at {} lines. Refine your search pattern for more specific results.)\n",
            INTERNAL_LIMIT
        ));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_match() {
        let json = r#"{"type":"match","data":{"path":{"text":"src/main.rs"},"line_number":10,"lines":{"text":"fn main() {\n"}}}"#;
        let matches = parse_json_output(json.as_bytes(), 100);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].file_path, "src/main.rs");
        assert_eq!(matches[0].line_number, 10);
        assert_eq!(matches[0].line_content, "fn main() {");
        assert!(!matches[0].is_context);
    }

    #[test]
    fn test_parse_json_context() {
        let json = r#"{"type":"context","data":{"path":{"text":"src/lib.rs"},"line_number":5,"lines":{"text":"// context line\n"}}}"#;
        let matches = parse_json_output(json.as_bytes(), 100);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].file_path, "src/lib.rs");
        assert!(matches[0].is_context);
    }

    #[test]
    fn test_parse_json_mixed() {
        let json = concat!(
            r#"{"type":"context","data":{"path":{"text":"a.rs"},"line_number":1,"lines":{"text":"// before\n"}}}"#,
            "\n",
            r#"{"type":"match","data":{"path":{"text":"a.rs"},"line_number":2,"lines":{"text":"fn foo()\n"}}}"#,
            "\n",
            r#"{"type":"context","data":{"path":{"text":"a.rs"},"line_number":3,"lines":{"text":"// after\n"}}}"#,
        );
        let matches = parse_json_output(json.as_bytes(), 100);
        assert_eq!(matches.len(), 3);
        assert!(matches[0].is_context);
        assert!(!matches[1].is_context);
        assert!(matches[2].is_context);
    }

    #[test]
    fn test_parse_json_limit() {
        let json = concat!(
            r#"{"type":"match","data":{"path":{"text":"a.rs"},"line_number":1,"lines":{"text":"line1\n"}}}"#,
            "\n",
            r#"{"type":"match","data":{"path":{"text":"a.rs"},"line_number":2,"lines":{"text":"line2\n"}}}"#,
            "\n",
            r#"{"type":"match","data":{"path":{"text":"a.rs"},"line_number":3,"lines":{"text":"line3\n"}}}"#,
        );
        let matches = parse_json_output(json.as_bytes(), 2);
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_format_output_empty() {
        let output = format_output(&[], "test", Path::new("."));
        assert!(output.contains("No matches found"));
    }

    #[test]
    fn test_format_output_with_matches() {
        let matches = vec![
            GrepMatch {
                file_path: "src/main.rs".to_string(),
                line_number: 10,
                line_content: "fn main() {".to_string(),
                is_context: false,
            },
            GrepMatch {
                file_path: "src/main.rs".to_string(),
                line_number: 11,
                line_content: "    println!(\"hello\");".to_string(),
                is_context: true,
            },
        ];
        let output = format_output(&matches, "main", Path::new("."));
        assert!(output.contains("Found 1 match"));
        assert!(output.contains("File: src/main.rs"));
        assert!(output.contains(">L10: fn main()"));
        assert!(output.contains(" L11:"));
    }

    #[test]
    fn test_ripgrep_args_defaults() {
        let json = r#"{"pattern": "test"}"#;
        let args: RipGrepArgs = serde_json::from_str(json).expect("parse args");
        assert_eq!(args.pattern, "test");
        assert!(args.path.is_none());
        assert!(args.include.is_none());
        assert!(args.case_sensitive.is_none());
        assert!(args.fixed_strings.is_none());
        assert!(args.context.is_none());
        assert!(args.after.is_none());
        assert!(args.before.is_none());
    }

    #[test]
    fn test_ripgrep_args_full() {
        let json = r#"{
            "pattern": "test",
            "path": "/src",
            "include": "*.rs",
            "case_sensitive": true,
            "fixed_strings": true,
            "context": 2,
            "after": 3,
            "before": 1
        }"#;
        let args: RipGrepArgs = serde_json::from_str(json).expect("parse args");
        assert_eq!(args.pattern, "test");
        assert_eq!(args.path.as_deref(), Some("/src"));
        assert_eq!(args.include.as_deref(), Some("*.rs"));
        assert_eq!(args.case_sensitive, Some(true));
        assert_eq!(args.fixed_strings, Some(true));
        assert_eq!(args.context, Some(2));
        assert_eq!(args.after, Some(3));
        assert_eq!(args.before, Some(1));
    }

    #[tokio::test]
    async fn test_ripgrep_integration() {
        use std::fs;
        use tempfile::tempdir;

        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();

        // Create test files with searchable content
        fs::write(
            dir.join("main.rs"),
            "fn main() {\n    println!(\"hello\");\n}",
        )
        .expect("write");
        fs::write(
            dir.join("lib.rs"),
            "pub fn helper() {\n    // helper function\n}",
        )
        .expect("write");
        fs::write(dir.join("ignored.log"), "fn should_be_ignored() {}").expect("write");

        // Create .ignore file (ripgrep natively supports this)
        fs::write(dir.join(".ignore"), "*.log").expect("write ignore");

        // Build and execute rg command directly
        // ripgrep will automatically respect .ignore files
        let output = Command::new("rg")
            .arg("--json")
            .arg("--ignore-case")
            .arg("--")
            .arg("fn")
            .arg(dir)
            .output()
            .await
            .expect("execute rg");

        let matches = parse_json_output(&output.stdout, 100);

        // Should find matches in .rs files but not in .log (filtered by .ignore)
        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.file_path.ends_with("main.rs")));
        assert!(matches.iter().any(|m| m.file_path.ends_with("lib.rs")));
        assert!(!matches.iter().any(|m| m.file_path.ends_with(".log")));
    }
}

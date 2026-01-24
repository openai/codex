//! BashOutput Tool Specification
//!
//! Retrieves output from background shell commands with automatic tweakcc read support.

use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::tools::spec::JsonSchema;
use std::collections::BTreeMap;

/// Create BashOutput tool specification.
///
/// BashOutput retrieves stdout/stderr from background shell commands
/// started with `run_in_background: true`.
///
/// Automatically returns tweakcc output - each call returns new output
/// since the last read, so the agent doesn't need to manage offsets.
pub fn create_bash_output_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();

    properties.insert(
        "shell_id".to_string(),
        JsonSchema::String {
            description: Some("The shell ID to retrieve output for".to_string()),
        },
    );

    properties.insert(
        "block".to_string(),
        JsonSchema::Boolean {
            description: Some(
                "Whether to wait for completion (default: true). Set to false for non-blocking check."
                    .to_string(),
            ),
        },
    );

    properties.insert(
        "timeout".to_string(),
        JsonSchema::Number {
            description: Some(
                "Max wait time in milliseconds (default: 30000, max: 600000)".to_string(),
            ),
        },
    );

    properties.insert(
        "filter".to_string(),
        JsonSchema::String {
            description: Some("Optional regex pattern to filter output lines".to_string()),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "BashOutput".to_string(),
        description: r#"Retrieve output from a running or completed background shell command.

Usage:
- Use shell_id from Bash tool response when run_in_background: true
- Use block=true (default) to wait for shell completion
- Use block=false for non-blocking check of current status
- Use filter to filter output lines by regex pattern
- Each call returns new output since last call (tweakcc)

Response includes:
- shellId, command, status ("running"|"completed"|"failed"|"killed")
- stdout, stderr (separate streams)
- stdoutLines, stderrLines (line counts)
- exitCode, timestamp, hasMore

Example:
{
  "shellId": "shell-abc123",
  "command": "npm test",
  "status": "running",
  "stdout": "Running tests...\n",
  "stderr": "",
  "stdoutLines": 1,
  "stderrLines": 0,
  "exitCode": null,
  "timestamp": "2024-01-15T10:30:00Z",
  "hasMore": true
}"#
        .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["shell_id".to_string()]),
            additional_properties: Some(false.into()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_bash_output_tool_spec() {
        let spec = create_bash_output_tool();

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        assert_eq!(tool.name, "BashOutput");
        assert!(!tool.strict);
        assert!(tool.description.contains("background shell"));
        assert!(tool.description.contains("tweakcc"));

        let JsonSchema::Object {
            properties,
            required,
            ..
        } = tool.parameters
        else {
            panic!("Expected object parameters");
        };

        let required = required.expect("Should have required fields");
        assert!(required.contains(&"shell_id".to_string()));
        assert!(properties.contains_key("shell_id"));
        assert!(properties.contains_key("block"));
        assert!(properties.contains_key("timeout"));
        // offset and limit are internal, not exposed to agent
        assert!(!properties.contains_key("offset"));
        assert!(!properties.contains_key("limit"));
    }
}

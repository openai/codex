//! KillShell Tool Specification
//!
//! Terminates running background shell commands.

use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::tools::spec::JsonSchema;
use std::collections::BTreeMap;

/// Create KillShell tool specification.
///
/// KillShell terminates a running background shell command.
pub fn create_kill_shell_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();

    properties.insert(
        "shell_id".to_string(),
        JsonSchema::String {
            description: Some("The shell ID to terminate".to_string()),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "KillShell".to_string(),
        description: r#"Terminate a running background shell command.

Usage:
- Use shell_id from Bash tool response when run_in_background: true
- Can only kill shells that are still running
- Returns success/failure message

Note: Use this when you need to stop a long-running command that's no longer needed,
or when a command appears to be stuck."#
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
    fn test_create_kill_shell_tool_spec() {
        let spec = create_kill_shell_tool();

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        assert_eq!(tool.name, "KillShell");
        assert!(!tool.strict);
        assert!(tool.description.contains("Terminate"));

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
    }
}

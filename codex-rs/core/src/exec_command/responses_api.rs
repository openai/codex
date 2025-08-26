use std::collections::BTreeMap;

use crate::openai_tools::JsonSchema;
use crate::openai_tools::ResponsesApiTool;

pub const EXEC_COMMAND_TOOL_NAME: &str = "exec_command";
pub const WRITE_STDIN_TOOL_NAME: &str = "write_stdin";

pub fn create_exec_command_tool_for_responses_api() -> ResponsesApiTool {
    let mut properties = BTreeMap::<String, JsonSchema>::new();
    properties.insert(
        "cmd".to_string(),
        JsonSchema::String {
            description: Some("The shell command to execute.".to_string()),
        },
    );
    properties.insert(
        "yield_time_ms".to_string(),
        JsonSchema::Number {
            description: Some("The maximum time in milliseconds to wait for output.".to_string()),
        },
    );
    properties.insert(
        "max_output_tokens".to_string(),
        JsonSchema::Number {
            description: Some("The maximum number of tokens to output.".to_string()),
        },
    );
    properties.insert(
        "shell".to_string(),
        JsonSchema::String {
            description: Some("The shell to use. Defaults to \"/bin/bash\".".to_string()),
        },
    );
    properties.insert(
        "login".to_string(),
        JsonSchema::Boolean {
            description: Some(
                "Whether to run the command as a login shell. Defaults to true.".to_string(),
            ),
        },
    );

    ResponsesApiTool {
        name: EXEC_COMMAND_TOOL_NAME.to_owned(),
        description: r#"Execute shell commands on the local machine with streaming output."#
            .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["cmd".to_string()]),
            additional_properties: Some(false),
        },
    }
}

pub fn create_write_stdin_tool_for_responses_api() -> ResponsesApiTool {
    let mut properties = BTreeMap::<String, JsonSchema>::new();
    properties.insert(
        "session_id".to_string(),
        JsonSchema::Number {
            description: Some("The ID of the exec_command session.".to_string()),
        },
    );
    properties.insert(
        "chars".to_string(),
        JsonSchema::String {
            description: Some("The characters to write to stdin.".to_string()),
        },
    );
    properties.insert(
        "yield_time_ms".to_string(),
        JsonSchema::Number {
            description: Some(
                "The maximum time in milliseconds to wait for output after writing.".to_string(),
            ),
        },
    );
    properties.insert(
        "max_output_tokens".to_string(),
        JsonSchema::Number {
            description: Some("The maximum number of tokens to output.".to_string()),
        },
    );

    ResponsesApiTool {
        name: WRITE_STDIN_TOOL_NAME.to_owned(),
        description: r#"Write characters to an exec session's stdin. Returns all stdout+stderr received within yield_time_ms.
Can write control characters (\u0003 for Ctrl-C), or an empty string to just poll stdout+stderr."#
            .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["session_id".to_string(), "chars".to_string()]),
            additional_properties: Some(false),
        },
    }
}

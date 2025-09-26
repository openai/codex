use std::collections::BTreeMap;

use crate::openai_tools::JsonSchema;
use crate::openai_tools::ResponsesApiTool;

pub const EXEC_COMMAND_TOOL_NAME: &str = "exec_command";
pub const WRITE_STDIN_TOOL_NAME: &str = "write_stdin";
pub const EXEC_CONTROL_TOOL_NAME: &str = "exec_control";
pub const LIST_EXEC_SESSIONS_TOOL_NAME: &str = "list_exec_sessions";

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

pub fn create_exec_control_tool_for_responses_api() -> ResponsesApiTool {
    let mut action_properties = BTreeMap::<String, JsonSchema>::new();
    action_properties.insert(
        "type".to_string(),
        JsonSchema::String {
            description: Some(
                "Action to perform. Allowed values: keepalive, send_ctrl_c, terminate, force_kill, set_idle_timeout.".to_string(),
            ),
        },
    );
    action_properties.insert(
        "extend_timeout_ms".to_string(),
        JsonSchema::Number {
            description: Some(
                "Optional: when type=keepalive, reset idle timer to now and optionally extend the idle timeout.".to_string(),
            ),
        },
    );
    action_properties.insert(
        "timeout_ms".to_string(),
        JsonSchema::Number {
            description: Some(
                "Optional: when type=set_idle_timeout, new idle timeout in milliseconds."
                    .to_string(),
            ),
        },
    );

    let mut properties = BTreeMap::<String, JsonSchema>::new();
    properties.insert(
        "session_id".to_string(),
        JsonSchema::Number {
            description: Some("The target exec session identifier.".to_string()),
        },
    );
    properties.insert(
        "action".to_string(),
        JsonSchema::Object {
            properties: action_properties,
            required: Some(vec!["type".to_string()]),
            additional_properties: Some(false),
        },
    );

    ResponsesApiTool {
        name: EXEC_CONTROL_TOOL_NAME.to_owned(),
        description:
            "Send control signals to a running exec session (keepalive, interrupt, terminate)."
                .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["session_id".to_string(), "action".to_string()]),
            additional_properties: Some(false),
        },
    }
}

pub fn create_list_exec_sessions_tool_for_responses_api() -> ResponsesApiTool {
    ResponsesApiTool {
        name: LIST_EXEC_SESSIONS_TOOL_NAME.to_owned(),
        description:
            "Summarize currently known exec sessions (running, graceful, or recently terminated)."
                .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties: BTreeMap::new(),
            required: Some(Vec::new()),
            additional_properties: Some(false),
        },
    }
}

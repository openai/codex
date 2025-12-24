use serde_json::json;
use std::path::Path;

pub fn create_shell_command_sse_response(
    command: Vec<String>,
    workdir: Option<&Path>,
    timeout_ms: Option<u64>,
    call_id: &str,
) -> anyhow::Result<String> {
    // The `arguments` for the `shell_command` tool is a serialized JSON object.
    let command_str = shlex::try_join(command.iter().map(String::as_str))?;
    let tool_call_arguments = serde_json::to_string(&json!({
        "command": command_str,
        "workdir": workdir.map(|w| w.to_string_lossy()),
        "timeout_ms": timeout_ms
    }))?;
    let tool_call = json!({
        "choices": [
            {
                "delta": {
                    "tool_calls": [
                        {
                            "id": call_id,
                            "function": {
                                "name": "shell_command",
                                "arguments": tool_call_arguments
                            }
                        }
                    ]
                },
                "finish_reason": "tool_calls"
            }
        ]
    });

    let sse = format!(
        "data: {}\n\ndata: DONE\n\n",
        serde_json::to_string(&tool_call)?
    );
    Ok(sse)
}

pub fn create_final_assistant_message_sse_response(message: &str) -> anyhow::Result<String> {
    let assistant_message = json!({
        "choices": [
            {
                "delta": {
                    "content": message
                },
                "finish_reason": "stop"
            }
        ]
    });

    let sse = format!(
        "data: {}\n\ndata: DONE\n\n",
        serde_json::to_string(&assistant_message)?
    );
    Ok(sse)
}

pub fn create_apply_patch_sse_response(
    patch_content: &str,
    call_id: &str,
) -> anyhow::Result<String> {
    // Use shell_command to call apply_patch with heredoc format
    let command = format!("apply_patch <<'EOF'\n{patch_content}\nEOF");
    let tool_call_arguments = serde_json::to_string(&json!({
        "command": command
    }))?;

    let tool_call = json!({
        "choices": [
            {
                "delta": {
                    "tool_calls": [
                        {
                            "id": call_id,
                            "function": {
                                "name": "shell_command",
                                "arguments": tool_call_arguments
                            }
                        }
                    ]
                },
                "finish_reason": "tool_calls"
            }
        ]
    });

    let sse = format!(
        "data: {}\n\ndata: DONE\n\n",
        serde_json::to_string(&tool_call)?
    );
    Ok(sse)
}

pub fn create_exec_command_sse_response(call_id: &str) -> anyhow::Result<String> {
    let cmd = if cfg!(windows) {
        // Keep this as a plain string: our exec tool ultimately runs the string
        // via a shell anyway, and joining args naively loses quoting.
        "cmd.exe /d /c echo hi".to_string()
    } else {
        // Keep the command simple and shell-native so it's stable under Buck2's
        // sandboxing / symlink trees.
        "echo hi".to_string()
    };
    let tool_call_arguments = serde_json::to_string(&json!({
        "cmd": cmd,
        // Force a non-login shell for determinism: under Buck2 test runners,
        // login shells can pick up environment/profile differences that make
        // this simple command flaky.
        "login": false,
        "yield_time_ms": 500
    }))?;
    let tool_call = json!({
        "choices": [
            {
                "delta": {
                    "tool_calls": [
                        {
                            "id": call_id,
                            "function": {
                                "name": "exec_command",
                                "arguments": tool_call_arguments
                            }
                        }
                    ]
                },
                "finish_reason": "tool_calls"
            }
        ]
    });

    let sse = format!(
        "data: {}\n\ndata: DONE\n\n",
        serde_json::to_string(&tool_call)?
    );
    Ok(sse)
}

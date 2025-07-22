use serde_json::json;
use std::path::Path;

pub fn create_shell_sse_response(
    command: Vec<String>,
    workdir: Option<&Path>,
    timeout_ms: Option<u64>,
    call_id: &str,
) -> anyhow::Result<String> {
    // The `arguments`` for the `shell` tool is a serialized JSON object.
    let tool_call_arguments = serde_json::to_string(&json!({
        "command": command,
        "workdir": workdir.map(|w| w.to_string_lossy()),
        "timeout": timeout_ms
    }))?;
    let tool_call = json!({
        "choices": [
            {
                "delta": {
                    "tool_calls": [
                        {
                            "id": call_id,
                            "function": {
                                "name": "shell",
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
    // Use shell command to call apply_patch with heredoc format
    let shell_command = format!("apply_patch <<'EOF'\n{patch_content}\nEOF");
    let tool_call_arguments = serde_json::to_string(&json!({
        "command": ["bash", "-lc", shell_command]
    }))?;

    let tool_call = json!({
        "choices": [
            {
                "delta": {
                    "tool_calls": [
                        {
                            "id": call_id,
                            "function": {
                                "name": "shell",
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

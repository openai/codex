use core_test_support::responses;
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
    create_shell_command_sse_response_from_command(
        &command_str,
        workdir,
        timeout_ms,
        call_id,
        /*login*/ None,
    )
}

pub fn create_shell_command_sse_response_from_command(
    command: &str,
    workdir: Option<&Path>,
    timeout_ms: Option<u64>,
    call_id: &str,
    login: Option<bool>,
) -> anyhow::Result<String> {
    // Use this when a test already has a shell command string. It fixes string
    // quoting for those callers by avoiding a rebuild from argv with POSIX
    // rules, which can change how Windows PowerShell parses the command; for
    // sleep-based tests, nested parsing can add another PowerShell startup
    // before the requested sleep even begins.
    let mut tool_call_arguments = json!({
        "command": command,
        "workdir": workdir.map(|w| w.to_string_lossy()),
        "timeout_ms": timeout_ms
    });
    if let Some(login) = login {
        tool_call_arguments["login"] = json!(login);
    }

    let tool_call_arguments = serde_json::to_string(&tool_call_arguments)?;
    Ok(responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_function_call(call_id, "shell_command", &tool_call_arguments),
        responses::ev_completed("resp-1"),
    ]))
}

pub fn create_final_assistant_message_sse_response(message: &str) -> anyhow::Result<String> {
    Ok(responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", message),
        responses::ev_completed("resp-1"),
    ]))
}

pub fn create_apply_patch_sse_response(
    patch_content: &str,
    call_id: &str,
) -> anyhow::Result<String> {
    Ok(responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_apply_patch_shell_command_call_via_heredoc(call_id, patch_content),
        responses::ev_completed("resp-1"),
    ]))
}

pub fn create_exec_command_sse_response(call_id: &str) -> anyhow::Result<String> {
    let (cmd, args) = if cfg!(windows) {
        ("cmd.exe", vec!["/d", "/c", "echo hi"])
    } else {
        ("/bin/sh", vec!["-c", "echo hi"])
    };
    let command = std::iter::once(cmd.to_string())
        .chain(args.into_iter().map(str::to_string))
        .collect::<Vec<_>>();
    let tool_call_arguments = serde_json::to_string(&json!({
        "cmd": command.join(" "),
        "yield_time_ms": 500
    }))?;
    Ok(responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_function_call(call_id, "exec_command", &tool_call_arguments),
        responses::ev_completed("resp-1"),
    ]))
}

pub fn create_request_user_input_sse_response(call_id: &str) -> anyhow::Result<String> {
    let tool_call_arguments = serde_json::to_string(&json!({
        "questions": [{
            "id": "confirm_path",
            "header": "Confirm",
            "question": "Proceed with the plan?",
            "options": [{
                "label": "Yes (Recommended)",
                "description": "Continue the current plan."
            }, {
                "label": "No",
                "description": "Stop and revisit the approach."
            }]
        }]
    }))?;

    Ok(responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_function_call(call_id, "request_user_input", &tool_call_arguments),
        responses::ev_completed("resp-1"),
    ]))
}

pub fn create_request_permissions_sse_response(call_id: &str) -> anyhow::Result<String> {
    let tool_call_arguments = serde_json::to_string(&json!({
        "reason": "Select a workspace root",
        "permissions": {
            "file_system": {
                "write": [
                    ".",
                    "../shared"
                ]
            }
        }
    }))?;

    Ok(responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_function_call(call_id, "request_permissions", &tool_call_arguments),
        responses::ev_completed("resp-1"),
    ]))
}

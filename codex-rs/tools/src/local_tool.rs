use crate::JsonSchema;
use crate::ResponsesApiTool;
use crate::ToolSpec;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandToolOptions {
    pub allow_login_shell: bool,
    pub exec_permission_approvals_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellToolOptions {
    pub exec_permission_approvals_enabled: bool,
}

pub fn create_exec_command_tool(options: CommandToolOptions) -> ToolSpec {
    let mut properties = BTreeMap::from([
        (
            "cmd".to_string(),
            JsonSchema::string(Some("Shell command to execute.".to_string())),
        ),
        (
            "workdir".to_string(),
            JsonSchema::string(Some(
                "Optional working directory to run the command in; defaults to the turn cwd."
                    .to_string(),
            )),
        ),
        (
            "shell".to_string(),
            JsonSchema::string(Some(
                "Shell binary to launch. Defaults to the user's default shell.".to_string(),
            )),
        ),
        (
            "host_id".to_string(),
            JsonSchema::string(Some(
                "Optional registered exec host to run the command on; omit for the default host."
                    .to_string(),
            )),
        ),
        (
            "tty".to_string(),
            JsonSchema::boolean(Some(
                "Whether to allocate a TTY for the command. Defaults to false (plain pipes); set to true to open a PTY and access TTY process."
                    .to_string(),
            )),
        ),
        (
            "yield_time_ms".to_string(),
            JsonSchema::number(Some(
                "How long to wait (in milliseconds) for output before yielding.".to_string(),
            )),
        ),
        (
            "max_output_tokens".to_string(),
            JsonSchema::number(Some(
                "Maximum number of tokens to return. Excess output will be truncated.".to_string(),
            )),
        ),
    ]);
    if options.allow_login_shell {
        properties.insert(
            "login".to_string(),
            JsonSchema::boolean(Some(
                "Whether to run the shell with -l/-i semantics. Defaults to true.".to_string(),
            )),
        );
    }
    properties.extend(create_approval_parameters(
        options.exec_permission_approvals_enabled,
    ));

    ToolSpec::Function(ResponsesApiTool {
        name: "exec_command".to_string(),
        description: if cfg!(windows) {
            format!(
                "Runs a command in a PTY, returning output or a session ID for ongoing interaction.\n\n{}",
                windows_destructive_filesystem_guidance()
            )
        } else {
            "Runs a command in a PTY, returning output or a session ID for ongoing interaction."
                .to_string()
        },
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["cmd".to_string()]),
            Some(false.into()),
        ),
        output_schema: Some(unified_exec_output_schema()),
    })
}

pub fn create_write_stdin_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "session_id".to_string(),
            JsonSchema::number(Some(
                "Identifier of the running unified exec session.".to_string(),
            )),
        ),
        (
            "chars".to_string(),
            JsonSchema::string(Some(
                "Bytes to write to stdin (may be empty to poll).".to_string(),
            )),
        ),
        (
            "yield_time_ms".to_string(),
            JsonSchema::number(Some(
                "How long to wait (in milliseconds) for output before yielding.".to_string(),
            )),
        ),
        (
            "max_output_tokens".to_string(),
            JsonSchema::number(Some(
                "Maximum number of tokens to return. Excess output will be truncated.".to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "write_stdin".to_string(),
        description:
            "Writes characters to an existing unified exec session and returns recent output."
                .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["session_id".to_string()]),
            Some(false.into()),
        ),
        output_schema: Some(unified_exec_output_schema()),
    })
}

pub fn create_shell_tool(options: ShellToolOptions) -> ToolSpec {
    let mut properties = BTreeMap::from([
        (
            "command".to_string(),
            JsonSchema::array(
                JsonSchema::string(/*description*/ None),
                Some("The command to execute".to_string()),
            ),
        ),
        (
            "workdir".to_string(),
            JsonSchema::string(Some(
                "The working directory to execute the command in".to_string(),
            )),
        ),
        (
            "timeout_ms".to_string(),
            JsonSchema::number(Some(
                "The timeout for the command in milliseconds".to_string(),
            )),
        ),
    ]);
    properties.extend(create_approval_parameters(
        options.exec_permission_approvals_enabled,
    ));

    let description = if cfg!(windows) {
        format!(
            r#"Runs a Powershell command (Windows) and returns its output. Arguments to `shell` will be passed to CreateProcessW(). Most commands should be prefixed with ["powershell.exe", "-Command"].

Examples of valid command strings:

- ls -a (show hidden): ["powershell.exe", "-Command", "Get-ChildItem -Force"]
- recursive find by name: ["powershell.exe", "-Command", "Get-ChildItem -Recurse -Filter *.py"]
- recursive grep: ["powershell.exe", "-Command", "Get-ChildItem -Path C:\\myrepo -Recurse | Select-String -Pattern 'TODO' -CaseSensitive"]
- ps aux | grep python: ["powershell.exe", "-Command", "Get-Process | Where-Object {{ $_.ProcessName -like '*python*' }}"]
- setting an env var: ["powershell.exe", "-Command", "$env:FOO='bar'; echo $env:FOO"]
- running an inline Python script: ["powershell.exe", "-Command", "@'\\nprint('Hello, world!')\\n'@ | python -"]

{}"#,
            windows_destructive_filesystem_guidance()
        )
    } else {
        r#"Runs a shell command and returns its output.
- The arguments to `shell` will be passed to execvp(). Most terminal commands should be prefixed with ["bash", "-lc"].
- Always set the `workdir` param when using the shell function. Do not use `cd` unless absolutely necessary."#
            .to_string()
    };

    ToolSpec::Function(ResponsesApiTool {
        name: "shell".to_string(),
        description,
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["command".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}

pub fn create_shell_command_tool(options: CommandToolOptions) -> ToolSpec {
    let mut properties = BTreeMap::from([
        (
            "command".to_string(),
            JsonSchema::string(Some(
                "The shell script to execute in the user's default shell".to_string(),
            )),
        ),
        (
            "workdir".to_string(),
            JsonSchema::string(Some(
                "The working directory to execute the command in".to_string(),
            )),
        ),
        (
            "timeout_ms".to_string(),
            JsonSchema::number(Some(
                "The timeout for the command in milliseconds".to_string(),
            )),
        ),
    ]);
    if options.allow_login_shell {
        properties.insert(
            "login".to_string(),
            JsonSchema::boolean(Some(
                "Whether to run the shell with login shell semantics. Defaults to true."
                    .to_string(),
            )),
        );
    }
    properties.extend(create_approval_parameters(
        options.exec_permission_approvals_enabled,
    ));

    let description = if cfg!(windows) {
        format!(
            r#"Runs a Powershell command (Windows) and returns its output.

Examples of valid command strings:

- ls -a (show hidden): "Get-ChildItem -Force"
- recursive find by name: "Get-ChildItem -Recurse -Filter *.py"
- recursive grep: "Get-ChildItem -Path C:\\myrepo -Recurse | Select-String -Pattern 'TODO' -CaseSensitive"
- ps aux | grep python: "Get-Process | Where-Object {{ $_.ProcessName -like '*python*' }}"
- setting an env var: "$env:FOO='bar'; echo $env:FOO"
- running an inline Python script: "@'\\nprint('Hello, world!')\\n'@ | python -"

{}"#,
            windows_destructive_filesystem_guidance()
        )
    } else {
        r#"Runs a shell command and returns its output.
- Always set the `workdir` param when using the shell_command function. Do not use `cd` unless absolutely necessary."#
            .to_string()
    };

    ToolSpec::Function(ResponsesApiTool {
        name: "shell_command".to_string(),
        description,
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["command".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}

pub fn create_request_permissions_tool(description: String) -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "reason".to_string(),
            JsonSchema::string(Some(
                "Optional short explanation for why additional permissions are needed.".to_string(),
            )),
        ),
        ("permissions".to_string(), permission_profile_schema()),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "request_permissions".to_string(),
        description,
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["permissions".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}

pub fn request_permissions_tool_description() -> String {
    "Request additional filesystem or network permissions from the user and wait for the client to grant a subset of the requested permission profile. Granted permissions apply automatically to later shell-like commands in the current turn, or for the rest of the session if the client approves them at session scope."
        .to_string()
}

fn unified_exec_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "chunk_id": {
                "type": "string",
                "description": "Chunk identifier included when the response reports one."
            },
            "wall_time_seconds": {
                "type": "number",
                "description": "Elapsed wall time spent waiting for output in seconds."
            },
            "exit_code": {
                "type": "number",
                "description": "Process exit code when the command finished during this call."
            },
            "session_id": {
                "type": "number",
                "description": "Session identifier to pass to write_stdin when the process is still running."
            },
            "original_token_count": {
                "type": "number",
                "description": "Approximate token count before output truncation."
            },
            "output": {
                "type": "string",
                "description": "Command output text, possibly truncated."
            }
        },
        "required": ["wall_time_seconds", "output"],
        "additionalProperties": false
    })
}

fn create_approval_parameters(
    exec_permission_approvals_enabled: bool,
) -> BTreeMap<String, JsonSchema> {
    let mut properties = BTreeMap::from([
        (
            "sandbox_permissions".to_string(),
            JsonSchema::string(Some(
                if exec_permission_approvals_enabled {
                    "Sandbox permissions for the command. Use \"with_additional_permissions\" to request additional sandboxed filesystem or network permissions (preferred), or \"require_escalated\" to request running without sandbox restrictions; defaults to \"use_default\"."
                } else {
                    "Sandbox permissions for the command. Set to \"require_escalated\" to request running without sandbox restrictions; defaults to \"use_default\"."
                }
                .to_string(),
            )),
        ),
        (
            "justification".to_string(),
            JsonSchema::string(Some(
                r#"Only set if sandbox_permissions is \"require_escalated\".
                    Request approval from the user to run this command outside the sandbox.
                    Phrased as a simple question that summarizes the purpose of the
                    command as it relates to the task at hand - e.g. 'Do you want to
                    fetch and pull the latest version of this git branch?'"#
                    .to_string(),
            )),
        ),
        (
            "prefix_rule".to_string(),
            JsonSchema::array(JsonSchema::string(/*description*/ None), Some(
                    r#"Only specify when sandbox_permissions is `require_escalated`.
                        Suggest a prefix command pattern that will allow you to fulfill similar requests from the user in the future.
                        Should be a short but reasonable prefix, e.g. [\"git\", \"pull\"] or [\"uv\", \"run\"] or [\"pytest\"]."#.to_string(),
                )),
        ),
    ]);

    if exec_permission_approvals_enabled {
        properties.insert(
            "additional_permissions".to_string(),
            permission_profile_schema(),
        );
    }

    properties
}

fn permission_profile_schema() -> JsonSchema {
    JsonSchema::object(
        BTreeMap::from([
            ("network".to_string(), network_permissions_schema()),
            ("file_system".to_string(), file_system_permissions_schema()),
        ]),
        /*required*/ None,
        Some(false.into()),
    )
}

fn network_permissions_schema() -> JsonSchema {
    JsonSchema::object(
        BTreeMap::from([(
            "enabled".to_string(),
            JsonSchema::boolean(Some("Set to true to request network access.".to_string())),
        )]),
        /*required*/ None,
        Some(false.into()),
    )
}

fn file_system_permissions_schema() -> JsonSchema {
    JsonSchema::object(
        BTreeMap::from([
            (
                "read".to_string(),
                JsonSchema::array(
                    JsonSchema::string(/*description*/ None),
                    Some("Absolute paths to grant read access to.".to_string()),
                ),
            ),
            (
                "write".to_string(),
                JsonSchema::array(
                    JsonSchema::string(/*description*/ None),
                    Some("Absolute paths to grant write access to.".to_string()),
                ),
            ),
        ]),
        /*required*/ None,
        Some(false.into()),
    )
}

fn windows_destructive_filesystem_guidance() -> &'static str {
    r#"Windows safety rules:
- Do not compose destructive filesystem commands across shells. Do not enumerate paths in PowerShell and then pass them to `cmd /c`, batch builtins, or another shell for deletion or moving. Use one shell end-to-end, prefer native PowerShell cmdlets such as `Remove-Item` / `Move-Item` with `-LiteralPath`, and avoid string-built shell commands for file operations.
- Before any recursive delete or move on Windows, verify the resolved absolute target paths stay within the intended workspace or explicitly named target directory. Never issue a recursive delete or move against a computed path if the final target has not been checked."#
}

#[cfg(test)]
#[path = "local_tool_tests.rs"]
mod tests;

use anyhow::Context;
use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use codex_app_server_protocol::CommandExecOutputDeltaNotification;
use codex_app_server_protocol::CommandExecOutputStream;
use codex_app_server_protocol::CommandExecParams;
use codex_app_server_protocol::CommandExecResizeParams;
use codex_app_server_protocol::CommandExecResponse;
use codex_app_server_protocol::CommandExecTerminalSize;
use codex_app_server_protocol::CommandExecTerminateParams;
use codex_app_server_protocol::CommandExecWriteParams;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxPolicy;
use codex_exec_server::CODEX_EXEC_SERVER_URL_ENV_VAR;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_READ_ONLY;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout;

use super::connection_handling_websocket::DEFAULT_READ_TIMEOUT;
use super::connection_handling_websocket::assert_no_message;
use super::connection_handling_websocket::connect_websocket;
use super::connection_handling_websocket::create_config_toml;
use super::connection_handling_websocket::read_jsonrpc_message;
use super::connection_handling_websocket::send_initialize_request;
use super::connection_handling_websocket::send_request;
use super::connection_handling_websocket::spawn_websocket_server;

#[cfg(windows)]
fn require_absent_windows_environment(names: &[&str]) -> Result<()> {
    for name in names {
        anyhow::ensure!(
            std::env::var_os(name).is_none(),
            "cannot establish lowercase Windows environment fixture because {name} is already set"
        );
    }
    Ok(())
}

#[tokio::test]
async fn command_exec_without_streams_can_be_terminated() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let process_id = "sleep-1".to_string();
    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec!["sh".to_string(), "-lc".to_string(), "sleep 30".to_string()],
            process_id: Some(process_id.clone()),
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .await?;
    let terminate_request_id = mcp
        .send_command_exec_terminate_request(CommandExecTerminateParams { process_id })
        .await?;

    let terminate_response = mcp
        .read_stream_until_response_message(RequestId::Integer(terminate_request_id))
        .await?;
    assert_eq!(terminate_response.result, serde_json::json!({}));

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_ne!(
        response.exit_code, 0,
        "terminated command should not succeed"
    );
    assert_eq!(response.stdout, "");
    assert_eq!(response.stderr, "");

    Ok(())
}

#[tokio::test]
async fn command_exec_without_process_id_keeps_buffered_compatibility() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                "sh".to_string(),
                "-lc".to_string(),
                "printf 'legacy-out'; printf 'legacy-err' >&2".to_string(),
            ],
            process_id: None,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_eq!(
        response,
        CommandExecResponse {
            exit_code: 0,
            stdout: "legacy-out".to_string(),
            stderr: "legacy-err".to_string(),
        }
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_env_overrides_merge_with_server_environment_and_support_unset() -> Result<()>
{
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = TestAppServer::new_with_env(
        codex_home.path(),
        &[("COMMAND_EXEC_BASELINE", Some("server"))],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                "/bin/sh".to_string(),
                "-lc".to_string(),
                "printf '%s|%s|%s|%s' \"$COMMAND_EXEC_BASELINE\" \"$COMMAND_EXEC_EXTRA\" \"${RUST_LOG-unset}\" \"$CODEX_HOME\"".to_string(),
            ],
            process_id: None,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: Some(HashMap::from([
                (
                    "COMMAND_EXEC_BASELINE".to_string(),
                    Some("request".to_string()),
                ),
                ("COMMAND_EXEC_EXTRA".to_string(), Some("added".to_string())),
                ("RUST_LOG".to_string(), None),
            ])),
            size: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_eq!(
        response,
        CommandExecResponse {
            exit_code: 0,
            stdout: format!("request|added|unset|{}", codex_home.path().display()),
            stderr: String::new(),
        }
    );

    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn command_exec_env_overrides_use_windows_case_folding() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    require_absent_windows_environment(&[
        "GIT_ALLOW_PROTOCOL",
        "GIT_NO_LAZY_FETCH",
        "CASEFOLD_REMOVE_ME",
    ])?;
    let mut mcp = TestAppServer::new_with_env(
        codex_home.path(),
        &[
            ("git_allow_protocol", Some("ext")),
            ("git_no_lazy_fetch", Some("0")),
            ("casefold_remove_me", Some("inherited")),
        ],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let system_root = std::env::var_os("SystemRoot").context("SystemRoot should be set")?;
    let powershell = Path::new(&system_root)
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");
    let inherited_script = r#"
$entries = [System.Environment]::GetEnvironmentVariables('Process')
function Exact-Keys([string] $name) {
    @($entries.Keys | Where-Object {
        [string]::Equals(
            [string] $_,
            $name,
            [System.StringComparison]::Ordinal
        )
    })
}
$allowLower = @(Exact-Keys 'git_allow_protocol')
$allowUpper = @(Exact-Keys 'GIT_ALLOW_PROTOCOL')
$noLazyLower = @(Exact-Keys 'git_no_lazy_fetch')
$noLazyUpper = @(Exact-Keys 'GIT_NO_LAZY_FETCH')
$removedLower = @(Exact-Keys 'casefold_remove_me')
$removedUpper = @(Exact-Keys 'CASEFOLD_REMOVE_ME')
if ($allowLower.Count -ne 1 -or [string] $entries[$allowLower[0]] -ne 'ext') { exit 21 }
if ($allowUpper.Count -ne 0) { exit 22 }
if ($noLazyLower.Count -ne 1 -or [string] $entries[$noLazyLower[0]] -ne '0') { exit 23 }
if ($noLazyUpper.Count -ne 0) { exit 24 }
if ($removedLower.Count -ne 1 -or [string] $entries[$removedLower[0]] -ne 'inherited') { exit 25 }
if ($removedUpper.Count -ne 0) { exit 26 }
[Console]::Out.Write('lowercase-inherited')
"#;
    let inherited_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                powershell.display().to_string(),
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                inherited_script.to_string(),
            ],
            process_id: None,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .await?;

    let inherited_response = mcp
        .read_stream_until_response_message(RequestId::Integer(inherited_request_id))
        .await?;
    let inherited_response: CommandExecResponse = to_response(inherited_response)?;
    assert_eq!(
        inherited_response,
        CommandExecResponse {
            exit_code: 0,
            stdout: "lowercase-inherited".to_string(),
            stderr: String::new(),
        }
    );

    let overridden_script = r#"
$entries = [System.Environment]::GetEnvironmentVariables('Process')
function Matching-Keys([string] $name) {
    @($entries.Keys | Where-Object {
        [string]::Equals(
            [string] $_,
            $name,
            [System.StringComparison]::OrdinalIgnoreCase
        )
    })
}
$allow = @(Matching-Keys 'GIT_ALLOW_PROTOCOL')
$noLazy = @(Matching-Keys 'GIT_NO_LAZY_FETCH')
$removed = @(Matching-Keys 'CASEFOLD_REMOVE_ME')
if ($allow.Count -ne 1 -or [string] $entries[$allow[0]] -ne '') { exit 11 }
if ($noLazy.Count -ne 1 -or [string] $entries[$noLazy[0]] -ne '1') { exit 12 }
if ($removed.Count -ne 0) { exit 13 }
[Console]::Out.Write('casefold-ok')
"#;
    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                powershell.display().to_string(),
                "-NoProfile".to_string(),
                "-NonInteractive".to_string(),
                "-Command".to_string(),
                overridden_script.to_string(),
            ],
            process_id: None,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: Some(HashMap::from([
                ("GIT_ALLOW_PROTOCOL".to_string(), Some(String::new())),
                ("GIT_NO_LAZY_FETCH".to_string(), Some("1".to_string())),
                ("CASEFOLD_REMOVE_ME".to_string(), None),
            ])),
            size: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_eq!(
        response,
        CommandExecResponse {
            exit_code: 0,
            stdout: "casefold-ok".to_string(),
            stderr: String::new(),
        }
    );

    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn command_exec_empty_git_allowlist_blocks_local_file_transport_on_windows() -> Result<()> {
    let repository_root = TempDir::new()?;
    let repository = repository_root.path().join("remote.git");
    let init = std::process::Command::new("git")
        .args(["init", "-q", "--bare"])
        .arg(&repository)
        .output()
        .context("initialize bare Git repository")?;
    anyhow::ensure!(
        init.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    let repository_url = url::Url::from_file_path(&repository)
        .map_err(|()| anyhow::anyhow!("convert repository path to file URL"))?
        .to_string();

    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    require_absent_windows_environment(&["GIT_ALLOW_PROTOCOL", "GIT_NO_LAZY_FETCH"])?;
    let mut mcp = TestAppServer::new_with_env(
        codex_home.path(),
        &[
            ("git_allow_protocol", Some("file")),
            ("git_no_lazy_fetch", Some("0")),
        ],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let control_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                "git".to_string(),
                "ls-remote".to_string(),
                repository_url.clone(),
            ],
            process_id: None,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .await?;

    let control_response = mcp
        .read_stream_until_response_message(RequestId::Integer(control_request_id))
        .await?;
    let control_response: CommandExecResponse = to_response(control_response)?;
    assert_eq!(
        control_response,
        CommandExecResponse {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
        },
        "inherited lowercase allowlist should permit file:// through command/exec"
    );

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec!["git".to_string(), "ls-remote".to_string(), repository_url],
            process_id: None,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: Some(HashMap::from([
                ("GIT_ALLOW_PROTOCOL".to_string(), Some(String::new())),
                ("GIT_NO_LAZY_FETCH".to_string(), Some("1".to_string())),
            ])),
            size: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_ne!(response.exit_code, 0, "empty allowlist should deny file://");
    assert_eq!(response.stdout, "");
    assert!(
        response.stderr.contains("transport 'file' not allowed"),
        "expected Git transport denial, got: {}",
        response.stderr
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_accepts_permission_profile() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                "sh".to_string(),
                "-lc".to_string(),
                "printf 'profile'".to_string(),
            ],
            process_id: None,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
            permission_profile: Some(BUILT_IN_PERMISSION_PROFILE_READ_ONLY.to_string()),
        })
        .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_eq!(
        response,
        CommandExecResponse {
            exit_code: 0,
            stdout: "profile".to_string(),
            stderr: String::new(),
        }
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_permission_profile_starts_selected_network_proxy() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    insert_networked_permission_profile_config(
        codex_home.path(),
        /*default_permissions*/ None,
    )?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                "sh".to_string(),
                "-lc".to_string(),
                "printf '%s' \"${CODEX_NETWORK_PROXY_ACTIVE-unset}\"".to_string(),
            ],
            process_id: None,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
            permission_profile: Some("networked".to_string()),
        })
        .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_eq!(
        response,
        CommandExecResponse {
            exit_code: 0,
            stdout: "1".to_string(),
            stderr: String::new(),
        }
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_permission_profile_does_not_reuse_default_network_proxy() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    insert_networked_permission_profile_config(codex_home.path(), Some("networked"))?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                "sh".to_string(),
                "-lc".to_string(),
                "printf '%s' \"${CODEX_NETWORK_PROXY_ACTIVE-unset}\"".to_string(),
            ],
            process_id: None,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
            permission_profile: Some(BUILT_IN_PERMISSION_PROFILE_READ_ONLY.to_string()),
        })
        .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_eq!(
        response,
        CommandExecResponse {
            exit_code: 0,
            stdout: "unset".to_string(),
            stderr: String::new(),
        }
    );

    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn command_exec_permission_profile_project_roots_use_command_cwd() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    let command_dir = codex_home.path().join("command-cwd");
    std::fs::create_dir(&command_dir)?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    insert_command_exec_config(
        codex_home.path(),
        r#"
[permissions.command-cwd.filesystem]
":root" = "read"
":workspace_roots" = "write"
"#,
    )?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                "sh".to_string(),
                "-lc".to_string(),
                "printf child > child.txt && ! printf parent > ../parent.txt".to_string(),
            ],
            process_id: None,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: Some("command-cwd".into()),
            env: None,
            size: None,
            sandbox_policy: None,
            permission_profile: Some("command-cwd".to_string()),
        })
        .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_eq!(
        response.exit_code, 0,
        "parent cwd write should fail under command project-root profile: {response:?}"
    );
    assert_eq!(
        std::fs::read_to_string(command_dir.join("child.txt"))?,
        "child"
    );
    assert!(
        !codex_home.path().join("parent.txt").exists(),
        "permissionProfile :workspace_roots write should not grant the server cwd when command cwd differs"
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_returns_error_when_local_environment_is_disabled() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = TestAppServer::new_with_env(
        codex_home.path(),
        &[(CODEX_EXEC_SERVER_URL_ENV_VAR, Some("none"))],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec!["sh".to_string(), "-lc".to_string(), "true".to_string()],
            process_id: None,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .await?;

    let error = mcp
        .read_stream_until_error_message(RequestId::Integer(command_request_id))
        .await?;
    assert_eq!(error.error.message, "local environment is not configured");

    Ok(())
}

#[tokio::test]
async fn command_exec_rejects_sandbox_policy_with_permission_profile() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec!["sh".to_string(), "-lc".to_string(), "true".to_string()],
            process_id: None,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
            permission_profile: Some(BUILT_IN_PERMISSION_PROFILE_READ_ONLY.to_string()),
        })
        .await?;

    let error = mcp
        .read_stream_until_error_message(RequestId::Integer(command_request_id))
        .await?;
    assert_eq!(
        error.error.message,
        "`permissionProfile` cannot be combined with `sandboxPolicy`"
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_rejects_disable_timeout_with_timeout_ms() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec!["sh".to_string(), "-lc".to_string(), "sleep 1".to_string()],
            process_id: Some("invalid-timeout-1".to_string()),
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: true,
            timeout_ms: Some(1_000),
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .await?;

    let error = mcp
        .read_stream_until_error_message(RequestId::Integer(command_request_id))
        .await?;
    assert_eq!(
        error.error.message,
        "command/exec cannot set both timeoutMs and disableTimeout"
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_rejects_disable_output_cap_with_output_bytes_cap() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec!["sh".to_string(), "-lc".to_string(), "sleep 1".to_string()],
            process_id: Some("invalid-cap-1".to_string()),
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: Some(1024),
            disable_output_cap: true,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .await?;

    let error = mcp
        .read_stream_until_error_message(RequestId::Integer(command_request_id))
        .await?;
    assert_eq!(
        error.error.message,
        "command/exec cannot set both outputBytesCap and disableOutputCap"
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_rejects_negative_timeout_ms() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec!["sh".to_string(), "-lc".to_string(), "sleep 1".to_string()],
            process_id: Some("negative-timeout-1".to_string()),
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: Some(-1),
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .await?;

    let error = mcp
        .read_stream_until_error_message(RequestId::Integer(command_request_id))
        .await?;
    assert_eq!(
        error.error.message,
        "command/exec timeoutMs must be non-negative, got -1"
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_without_process_id_rejects_streaming() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec!["sh".to_string(), "-lc".to_string(), "cat".to_string()],
            process_id: None,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: true,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .await?;

    let error = mcp
        .read_stream_until_error_message(RequestId::Integer(command_request_id))
        .await?;
    assert_eq!(
        error.error.message,
        "command/exec tty or streaming requires a client-supplied processId"
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_non_streaming_respects_output_cap() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                "sh".to_string(),
                "-lc".to_string(),
                "printf 'abcdef'; printf 'uvwxyz' >&2".to_string(),
            ],
            process_id: Some("cap-1".to_string()),
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: Some(5),
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_eq!(
        response,
        CommandExecResponse {
            exit_code: 0,
            stdout: "abcde".to_string(),
            stderr: "uvwxy".to_string(),
        }
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_streaming_does_not_buffer_output() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let process_id = "stream-cap-1".to_string();
    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                "sh".to_string(),
                "-lc".to_string(),
                "printf 'abcdefghij'; sleep 30".to_string(),
            ],
            process_id: Some(process_id.clone()),
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: true,
            output_bytes_cap: Some(5),
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .await?;

    let output = collect_command_exec_output_until(
        CommandExecDeltaReader::Mcp(&mut mcp),
        process_id.as_str(),
        "capped stdout",
        |_output, delta| delta.stream == CommandExecOutputStream::Stdout && delta.cap_reached,
    )
    .await?;
    assert_eq!(output.stdout, "abcde");
    let terminate_request_id = mcp
        .send_command_exec_terminate_request(CommandExecTerminateParams {
            process_id: process_id.clone(),
        })
        .await?;
    let terminate_response = mcp
        .read_stream_until_response_message(RequestId::Integer(terminate_request_id))
        .await?;
    assert_eq!(terminate_response.result, serde_json::json!({}));

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_ne!(
        response.exit_code, 0,
        "terminated command should not succeed"
    );
    assert_eq!(response.stdout, "");
    assert_eq!(response.stderr, "");

    Ok(())
}

#[tokio::test]
async fn command_exec_pipe_streams_output_and_accepts_write() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let process_id = "pipe-1".to_string();
    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                "sh".to_string(),
                "-lc".to_string(),
                "printf 'out-start\\n'; printf 'err-start\\n' >&2; IFS= read line; printf 'out:%s\\n' \"$line\"; printf 'err:%s\\n' \"$line\" >&2".to_string(),
            ],
            process_id: Some(process_id.clone()),
            tty: false,
            stream_stdin: true,
            stream_stdout_stderr: true,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .await?;

    wait_for_command_exec_outputs_contains(
        &mut mcp,
        process_id.as_str(),
        "out-start\n",
        "err-start\n",
    )
    .await?;

    let write_request_id = mcp
        .send_command_exec_write_request(CommandExecWriteParams {
            process_id: process_id.clone(),
            delta_base64: Some(STANDARD.encode("hello\n")),
            close_stdin: true,
        })
        .await?;
    let write_response = mcp
        .read_stream_until_response_message(RequestId::Integer(write_request_id))
        .await?;
    assert_eq!(write_response.result, serde_json::json!({}));

    wait_for_command_exec_outputs_contains(
        &mut mcp,
        process_id.as_str(),
        "out:hello\n",
        "err:hello\n",
    )
    .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_eq!(
        response,
        CommandExecResponse {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
        }
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_tty_implies_streaming_and_reports_pty_output() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let process_id = "tty-1".to_string();
    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                "sh".to_string(),
                "-lc".to_string(),
                "stty -echo; if [ -t 0 ]; then printf 'tty\\n'; else printf 'notty\\n'; fi; IFS= read line; printf 'echo:%s\\n' \"$line\"".to_string(),
            ],
            process_id: Some(process_id.clone()),
            tty: true,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
            permission_profile: None,
        })
        .await?;

    wait_for_command_exec_output_contains(
        &mut mcp,
        process_id.as_str(),
        CommandExecOutputStream::Stdout,
        "tty\n",
    )
    .await?;

    let write_request_id = mcp
        .send_command_exec_write_request(CommandExecWriteParams {
            process_id: process_id.clone(),
            delta_base64: Some(STANDARD.encode("world\n")),
            close_stdin: true,
        })
        .await?;
    let write_response = mcp
        .read_stream_until_response_message(RequestId::Integer(write_request_id))
        .await?;
    assert_eq!(write_response.result, serde_json::json!({}));

    wait_for_command_exec_output_contains(
        &mut mcp,
        process_id.as_str(),
        CommandExecOutputStream::Stdout,
        "echo:world\n",
    )
    .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_eq!(response.exit_code, 0);
    assert_eq!(response.stdout, "");
    assert_eq!(response.stderr, "");

    Ok(())
}

#[tokio::test]
async fn command_exec_tty_supports_initial_size_and_resize() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let process_id = "tty-size-1".to_string();
    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                "sh".to_string(),
                "-lc".to_string(),
                "stty -echo; printf 'start:%s\\n' \"$(stty size)\"; IFS= read _line; printf 'after:%s\\n' \"$(stty size)\"".to_string(),
            ],
            process_id: Some(process_id.clone()),
            tty: true,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: Some(CommandExecTerminalSize {
                rows: 31,
                cols: 101,
            }),
            sandbox_policy: None,
            permission_profile: None,
        })
        .await?;

    wait_for_command_exec_output_contains(
        &mut mcp,
        process_id.as_str(),
        CommandExecOutputStream::Stdout,
        "start:31 101\n",
    )
    .await?;

    let resize_request_id = mcp
        .send_command_exec_resize_request(CommandExecResizeParams {
            process_id: process_id.clone(),
            size: CommandExecTerminalSize {
                rows: 45,
                cols: 132,
            },
        })
        .await?;
    let resize_response = mcp
        .read_stream_until_response_message(RequestId::Integer(resize_request_id))
        .await?;
    assert_eq!(resize_response.result, serde_json::json!({}));

    let write_request_id = mcp
        .send_command_exec_write_request(CommandExecWriteParams {
            process_id: process_id.clone(),
            delta_base64: Some(STANDARD.encode("go\n")),
            close_stdin: true,
        })
        .await?;
    let write_response = mcp
        .read_stream_until_response_message(RequestId::Integer(write_request_id))
        .await?;
    assert_eq!(write_response.result, serde_json::json!({}));

    wait_for_command_exec_output_contains(
        &mut mcp,
        process_id.as_str(),
        CommandExecOutputStream::Stdout,
        "after:45 132\n",
    )
    .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_eq!(response.exit_code, 0);
    assert_eq!(response.stdout, "");
    assert_eq!(response.stderr, "");

    Ok(())
}

#[tokio::test]
async fn command_exec_process_ids_are_connection_scoped_and_disconnect_terminates_process()
-> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let marker = format!(
        "codex-command-exec-marker-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );

    let (mut process, bind_addr) = spawn_websocket_server(codex_home.path()).await?;

    let mut ws1 = connect_websocket(bind_addr).await?;
    let mut ws2 = connect_websocket(bind_addr).await?;

    send_initialize_request(&mut ws1, /*id*/ 1, "ws_client_one").await?;
    read_initialize_response(&mut ws1, /*request_id*/ 1).await?;
    send_initialize_request(&mut ws2, /*id*/ 2, "ws_client_two").await?;
    read_initialize_response(&mut ws2, /*request_id*/ 2).await?;

    send_request(
        &mut ws1,
        "command/exec",
        /*id*/ 101,
        Some(serde_json::json!({
            "command": [
                "python3",
                "-c",
                "import time; print('ready', flush=True); time.sleep(30)",
                marker,
            ],
            "processId": "shared-process",
            "streamStdoutStderr": true,
        })),
    )
    .await?;

    collect_command_exec_output_until(
        CommandExecDeltaReader::Websocket(&mut ws1),
        "shared-process",
        "websocket ready output",
        |output, _delta| output.stdout.contains("ready\n"),
    )
    .await?;
    wait_for_process_marker(&marker, /*should_exist*/ true).await?;

    send_request(
        &mut ws2,
        "command/exec/terminate",
        /*id*/ 102,
        Some(serde_json::json!({
            "processId": "shared-process",
        })),
    )
    .await?;

    let terminate_error = loop {
        let message = read_jsonrpc_message(&mut ws2).await?;
        if let JSONRPCMessage::Error(error) = message
            && error.id == RequestId::Integer(102)
        {
            break error;
        }
    };
    assert_eq!(
        terminate_error.error.message,
        "no active command/exec for process id \"shared-process\""
    );
    wait_for_process_marker(&marker, /*should_exist*/ true).await?;

    assert_no_message(&mut ws2, Duration::from_millis(250)).await?;
    ws1.close(None).await?;

    wait_for_process_marker(&marker, /*should_exist*/ false).await?;

    process
        .kill()
        .await
        .context("failed to stop websocket app-server process")?;
    Ok(())
}

async fn read_command_exec_delta(
    mcp: &mut TestAppServer,
) -> Result<CommandExecOutputDeltaNotification> {
    let notification = mcp
        .read_stream_until_notification_message("command/exec/outputDelta")
        .await?;
    decode_delta_notification(notification)
}

async fn wait_for_command_exec_output_contains(
    mcp: &mut TestAppServer,
    process_id: &str,
    stream: CommandExecOutputStream,
    expected: &str,
) -> Result<()> {
    let stream_name = match stream {
        CommandExecOutputStream::Stdout => "stdout",
        CommandExecOutputStream::Stderr => "stderr",
    };
    collect_command_exec_output_until(
        CommandExecDeltaReader::Mcp(mcp),
        process_id,
        format!("{stream_name} containing {expected:?}"),
        |output, _delta| match stream {
            CommandExecOutputStream::Stdout => output.stdout.contains(expected),
            CommandExecOutputStream::Stderr => output.stderr.contains(expected),
        },
    )
    .await?;
    Ok(())
}

async fn wait_for_command_exec_outputs_contains(
    mcp: &mut TestAppServer,
    process_id: &str,
    stdout_expected: &str,
    stderr_expected: &str,
) -> Result<()> {
    collect_command_exec_output_until(
        CommandExecDeltaReader::Mcp(mcp),
        process_id,
        format!("stdout containing {stdout_expected:?} and stderr containing {stderr_expected:?}"),
        |output, _delta| {
            output.stdout.contains(stdout_expected) && output.stderr.contains(stderr_expected)
        },
    )
    .await?;
    Ok(())
}

enum CommandExecDeltaReader<'a> {
    Mcp(&'a mut TestAppServer),
    Websocket(&'a mut super::connection_handling_websocket::WsClient),
}

#[derive(Default)]
struct CollectedCommandExecOutput {
    stdout: String,
    stderr: String,
}

async fn collect_command_exec_output_until(
    mut reader: CommandExecDeltaReader<'_>,
    process_id: &str,
    waiting_for: impl Into<String>,
    mut should_stop: impl FnMut(
        &CollectedCommandExecOutput,
        &CommandExecOutputDeltaNotification,
    ) -> bool,
) -> Result<CollectedCommandExecOutput> {
    let waiting_for = waiting_for.into();
    let deadline = Instant::now() + DEFAULT_READ_TIMEOUT;
    let mut output = CollectedCommandExecOutput::default();

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let delta = timeout(remaining, async {
            match &mut reader {
                CommandExecDeltaReader::Mcp(mcp) => read_command_exec_delta(mcp).await,
                CommandExecDeltaReader::Websocket(stream) => {
                    read_command_exec_delta_ws(stream).await
                }
            }
        })
        .await
        .with_context(|| {
            format!(
                "timed out waiting for {waiting_for} in command/exec output for {process_id}; collected stdout={:?}, stderr={:?}",
                output.stdout, output.stderr
            )
        })??;
        assert_eq!(delta.process_id, process_id);

        let delta_text = String::from_utf8(STANDARD.decode(&delta.delta_base64)?)?;
        let delta_text = delta_text.replace('\r', "");
        match delta.stream {
            CommandExecOutputStream::Stdout => output.stdout.push_str(&delta_text),
            CommandExecOutputStream::Stderr => output.stderr.push_str(&delta_text),
        }
        if should_stop(&output, &delta) {
            return Ok(output);
        }
    }
}

async fn read_command_exec_delta_ws(
    stream: &mut super::connection_handling_websocket::WsClient,
) -> Result<CommandExecOutputDeltaNotification> {
    loop {
        let message = read_jsonrpc_message(stream).await?;
        let JSONRPCMessage::Notification(notification) = message else {
            continue;
        };
        if notification.method == "command/exec/outputDelta" {
            return decode_delta_notification(notification);
        }
    }
}

fn decode_delta_notification(
    notification: JSONRPCNotification,
) -> Result<CommandExecOutputDeltaNotification> {
    let params = notification
        .params
        .context("command/exec/outputDelta notification should include params")?;
    serde_json::from_value(params).context("deserialize command/exec/outputDelta notification")
}

fn insert_networked_permission_profile_config(
    codex_home: &Path,
    default_permissions: Option<&str>,
) -> Result<()> {
    let default_permissions = default_permissions
        .map(|default_permissions| format!("default_permissions = \"{default_permissions}\"\n\n"))
        .unwrap_or_default();
    let inserted_config = format!(
        r#"{default_permissions}[features]
network_proxy = true

[permissions.networked.filesystem]
":root" = "read"

[permissions.networked.network]
enabled = true
proxy_url = "http://127.0.0.1:0"
enable_socks5 = false

"#
    );
    insert_command_exec_config(codex_home, &inserted_config)?;
    Ok(())
}

fn insert_command_exec_config(codex_home: &Path, inserted_config: &str) -> Result<()> {
    let config_path = codex_home.join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    let marker = "\n[model_providers.mock_provider]\n";
    let (prefix, suffix) = config
        .split_once(marker)
        .context("test config should include mock provider table")?;
    let config = format!("{prefix}\n{inserted_config}{marker}{suffix}");
    std::fs::write(config_path, config)?;
    Ok(())
}

async fn read_initialize_response(
    stream: &mut super::connection_handling_websocket::WsClient,
    request_id: i64,
) -> Result<()> {
    loop {
        let message = read_jsonrpc_message(stream).await?;
        if let JSONRPCMessage::Response(response) = message
            && response.id == RequestId::Integer(request_id)
        {
            return Ok(());
        }
    }
}

async fn wait_for_process_marker(marker: &str, should_exist: bool) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if process_with_marker_exists(marker)? == should_exist {
            return Ok(());
        }
        if Instant::now() >= deadline {
            let expectation = if should_exist { "appear" } else { "exit" };
            anyhow::bail!("process marker {marker:?} did not {expectation} before timeout");
        }
        sleep(Duration::from_millis(50)).await;
    }
}

fn process_with_marker_exists(marker: &str) -> Result<bool> {
    let output = std::process::Command::new("ps")
        .args(["-axo", "command"])
        .output()
        .context("spawn ps -axo command")?;
    let stdout = String::from_utf8(output.stdout).context("decode ps output")?;
    Ok(stdout.lines().any(|line| line.contains(marker)))
}

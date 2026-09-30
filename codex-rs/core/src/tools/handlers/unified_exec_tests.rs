use super::*;
use crate::shell::ShellType;
use crate::shell::default_user_shell;
use crate::shell::get_shell;
use codex_exec_server::Environment;
use codex_tools::UnifiedExecShellMode;
use codex_tools::ZshForkConfig;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use std::sync::Arc;

use crate::environment_selection::TurnEnvironmentState;
use crate::function_tool::FunctionCallError;
use crate::session::step_context::StepContext;
use crate::session::tests::make_session_and_context;
use crate::tools::context::ExecCommandToolOutput;
use crate::tools::context::ToolCallSource;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::hook_names::HookToolName;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use crate::turn_diff_tracker::TurnDiffTracker;
use tokio::sync::Mutex;

const TEST_TRUNCATION_POLICY: TruncationPolicy = TruncationPolicy::Tokens(10_000);

async fn invocation_for_payload(
    tool_name: &str,
    call_id: &str,
    payload: ToolPayload,
) -> ToolInvocation {
    let (session, turn) = make_session_and_context().await;
    let turn = Arc::new(turn);
    ToolInvocation {
        session: session.into(),
        step_context: StepContext::for_test(Arc::clone(&turn)),
        turn,
        cancellation_token: tokio_util::sync::CancellationToken::new(),
        tracker: Arc::new(Mutex::new(TurnDiffTracker::new())),
        call_id: call_id.to_string(),
        tool_name: codex_tools::ToolName::plain(tool_name),
        source: ToolCallSource::Direct,
        payload,
    }
}

#[test]
fn test_get_command_uses_default_shell_when_unspecified() -> anyhow::Result<()> {
    let json = r#"{"cmd": "echo hello"}"#;

    let args: ExecCommandArgs = parse_arguments(json)?;

    assert!(args.shell.is_none());

    let resolved = get_command(
        &args,
        Arc::new(default_user_shell()),
        &UnifiedExecShellMode::Direct,
        /*allow_login_shell*/ true,
    )
    .map_err(anyhow::Error::msg)?;
    let command = resolved.command;

    assert_eq!(command.len(), 3);
    assert_eq!(command[2], "echo hello");
    Ok(())
}

#[test]
fn test_get_command_respects_explicit_bash_shell() -> anyhow::Result<()> {
    let json = r#"{"cmd": "echo hello", "shell": "/bin/bash"}"#;

    let args: ExecCommandArgs = parse_arguments(json)?;

    assert_eq!(args.shell.as_deref(), Some("/bin/bash"));

    let resolved = get_command(
        &args,
        Arc::new(default_user_shell()),
        &UnifiedExecShellMode::Direct,
        /*allow_login_shell*/ true,
    )
    .map_err(anyhow::Error::msg)?;
    let command = resolved.command;

    assert_eq!(command.last(), Some(&"echo hello".to_string()));
    if command
        .iter()
        .any(|arg| arg.eq_ignore_ascii_case("-Command"))
    {
        assert!(command.contains(&"-NoProfile".to_string()));
    }
    Ok(())
}

/// Nested commands must find the first reported tool directory after login startup without duplicating existing entries.
#[cfg(unix)]
#[test]
fn test_get_command_keeps_path_prepends_after_login_startup() -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::tempdir()?;
    let path_dir = temp_dir.path().join("codex '[*] path");
    let extra_path_dir = temp_dir.path().join("extra tools");
    for directory in [&path_dir, &extra_path_dir] {
        std::fs::create_dir(directory)?;
        let rg = directory.join("rg");
        std::fs::write(&rg, "#!/bin/sh\n")?;
        std::fs::set_permissions(&rg, std::fs::Permissions::from_mode(0o755))?;
    }
    let rg = path_dir.join("rg");
    let startup = temp_dir.path().join("startup");
    std::fs::write(
        &startup,
        "export PATH=\"$CODEX_TEST_LOGIN_PATH\" CODEX_TEST_LOGIN_PATH_RESET=1\n",
    )?;
    let path_dirs = [
        PathUri::from_host_native_path(&path_dir)?,
        PathUri::from_host_native_path(&extra_path_dir)?,
    ];
    let args: ExecCommandArgs = parse_arguments(
        r#"{"cmd":"sh -c 'test \"$CODEX_TEST_LOGIN_PATH_RESET\" = 1 && printf \"%s\\n\" \"$PATH\" && command -v rg'"}"#,
    )?;
    let shell = Arc::new(get_shell(ShellType::Bash).expect("bash should be available"));
    let resolved = get_command(
        &args,
        Arc::clone(&shell),
        &UnifiedExecShellMode::Direct,
        /*allow_login_shell*/ true,
    )
    .map_err(anyhow::Error::msg)?;
    assert_eq!(
        resolved.command,
        shell.derive_exec_args(&args.cmd, /*use_login_shell*/ true)
    );

    let execution_command = resolved
        .shell
        .derive_exec_args_with_path_prepends(&args.cmd, &path_dirs)
        .expect("login shell should restore PATH");
    let path_dir = path_dirs[0].inferred_native_path_string();
    let extra_path_dir = path_dirs[1].inferred_native_path_string();
    let already_present = format!(
        "{}:{path_dir}:{extra_path_dir}:/usr/bin:/bin",
        temp_dir.path().display()
    );
    for (login_path, expected_path) in [
        (
            "/usr/bin:/bin".to_string(),
            format!("{path_dir}:{extra_path_dir}:/usr/bin:/bin"),
        ),
        (already_present.clone(), already_present),
    ] {
        let output = std::process::Command::new(&execution_command[0])
            .args(&execution_command[1..])
            .env("BASH_ENV", &startup)
            .env("CODEX_TEST_LOGIN_PATH", login_path)
            .env_remove("CODEX_TEST_LOGIN_PATH_RESET")
            .env("PATH", &path_dir)
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout)?,
            format!("{expected_path}\n{}\n", rg.display())
        );
    }
    Ok(())
}

/// The requested login shell and its command still run when startup forbids changing PATH.
#[cfg(unix)]
#[test]
fn test_get_command_runs_login_shell_when_path_is_readonly() -> anyhow::Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let startup = temp_dir.path().join("startup");
    // An explicit exit from a login Bash runs ~/.bash_logout; keep the runner's file out of the test.
    std::fs::write(temp_dir.path().join(".bash_logout"), ":\n")?;
    std::fs::write(
        &startup,
        "if shopt -q login_shell; then\n    export HOME=\"$CODEX_TEST_LOGIN_HOME\"\n    export PATH=/usr/bin:/bin\n    readonly PATH\n    set -e\nfi\n",
    )?;
    let path_dir = PathUri::from_host_native_path(temp_dir.path().join("codex-path"))?;
    let args: ExecCommandArgs = parse_arguments(
        r#"{"cmd":"shopt -q login_shell || exit 90; case $- in *e*) ;; *) exit 91 ;; esac; /bin/sh -c 'printf \"%s\\n\" \"$PATH\"'; exit 23","login":true}"#,
    )?;
    let shell = Arc::new(get_shell(ShellType::Bash).expect("bash should be available"));
    let resolved = get_command(
        &args,
        shell,
        &UnifiedExecShellMode::Direct,
        /*allow_login_shell*/ true,
    )
    .map_err(anyhow::Error::msg)?;
    let command = resolved
        .shell
        .derive_exec_args_with_path_prepends(&args.cmd, &[path_dir])
        .expect("the reported directory can be put on PATH");
    let output = std::process::Command::new(&command[0])
        .args(&command[1..])
        .env("BASH_ENV", startup)
        .env("CODEX_TEST_LOGIN_HOME", temp_dir.path())
        .env_remove("SHLVL")
        .output()?;

    assert_eq!(
        (
            output.status.code(),
            String::from_utf8(output.stdout)?,
            String::from_utf8(output.stderr)?
        ),
        (Some(23), "/usr/bin:/bin\n".to_string(), String::new())
    );
    Ok(())
}

/// The package path only affects Codex's own POSIX login shells when the executor reports a POSIX path.
#[test]
fn test_get_command_does_not_change_nested_login_or_powershell() -> anyhow::Result<()> {
    for (shell_type, shell_path, arguments, use_login_shell, path) in [
        (
            ShellType::Bash,
            "/bin/bash",
            r#"{"cmd":"bash -lc 'command -v rg'","login":false}"#,
            false,
            "file:///executor/codex-path",
        ),
        (
            ShellType::PowerShell,
            "pwsh",
            r#"{"cmd":"Get-Command rg"}"#,
            true,
            "file:///executor/codex-path",
        ),
        (
            ShellType::Bash,
            "/bin/bash",
            r#"{"cmd":"command -v rg"}"#,
            true,
            "file:///C:/executor/codex-path",
        ),
    ] {
        let args: ExecCommandArgs = parse_arguments(arguments)?;
        let path_dir = PathUri::parse(path)?;
        let shell = Arc::new(Shell {
            shell_type,
            shell_path: PathBuf::from(shell_path),
        });
        let resolved = get_command(
            &args,
            Arc::clone(&shell),
            &UnifiedExecShellMode::Direct,
            /*allow_login_shell*/ true,
        )
        .map_err(anyhow::Error::msg)?;
        assert_eq!(
            (
                resolved.command,
                resolved
                    .shell
                    .derive_exec_args_with_path_prepends(&args.cmd, &[path_dir])
            ),
            (shell.derive_exec_args(&args.cmd, use_login_shell), None)
        );
    }
    Ok(())
}

#[test]
fn test_get_command_resolves_powershell_by_type() -> anyhow::Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let powershell_path = temp_dir.path().join(if cfg!(windows) {
        "powershell.exe"
    } else {
        "powershell"
    });
    std::fs::write(&powershell_path, "")?;
    let json = serde_json::json!({
        "cmd": "echo hello",
        "shell": powershell_path,
    })
    .to_string();

    let args: ExecCommandArgs = parse_arguments(&json)?;

    assert_eq!(
        args.shell.as_deref(),
        Some(powershell_path.to_string_lossy().as_ref())
    );

    let resolved = get_command(
        &args,
        Arc::new(default_user_shell()),
        &UnifiedExecShellMode::Direct,
        /*allow_login_shell*/ true,
    )
    .map_err(anyhow::Error::msg)?;
    let expected_shell = get_shell(ShellType::PowerShell)
        .unwrap_or_else(|| codex_shell_command::shell_detect::ultimate_fallback_shell().into());
    assert_eq!(
        resolved.command,
        expected_shell.derive_exec_args("echo hello", /*use_login_shell*/ true)
    );
    assert_eq!(resolved.shell.shell, expected_shell);
    Ok(())
}

#[test]
fn test_get_command_respects_explicit_cmd_shell() -> anyhow::Result<()> {
    let json = r#"{"cmd": "echo hello", "shell": "cmd"}"#;

    let args: ExecCommandArgs = parse_arguments(json)?;

    assert_eq!(args.shell.as_deref(), Some("cmd"));

    let resolved = get_command(
        &args,
        Arc::new(default_user_shell()),
        &UnifiedExecShellMode::Direct,
        /*allow_login_shell*/ true,
    )
    .map_err(anyhow::Error::msg)?;
    let command = resolved.command;

    assert_eq!(command[2], "echo hello");
    Ok(())
}

#[test]
fn test_get_command_rejects_explicit_login_when_disallowed() -> anyhow::Result<()> {
    let json = r#"{"cmd": "echo hello", "login": true}"#;

    let args: ExecCommandArgs = parse_arguments(json)?;
    let err = get_command(
        &args,
        Arc::new(default_user_shell()),
        &UnifiedExecShellMode::Direct,
        /*allow_login_shell*/ false,
    )
    .expect_err("explicit login should be rejected");

    assert!(
        err.contains("login shell is disabled by config"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[tokio::test]
async fn exec_command_rejects_login_when_selected_environment_disallows_it() {
    let (session, mut turn) = make_session_and_context().await;
    assert!(turn.config.permissions.allow_login_shell);
    let TurnEnvironmentState::Ready(environment) = turn
        .initial_environments
        .environments
        .first_mut()
        .expect("primary environment")
    else {
        panic!("primary environment should be ready");
    };
    environment.config_mut().allow_login_shell = false;

    let turn = Arc::new(turn);
    let invocation = ToolInvocation {
        session: session.into(),
        step_context: StepContext::for_test(Arc::clone(&turn)),
        turn,
        cancellation_token: tokio_util::sync::CancellationToken::new(),
        tracker: Arc::new(Mutex::new(TurnDiffTracker::new())),
        call_id: "login-disallowed".to_string(),
        tool_name: codex_tools::ToolName::plain("exec_command"),
        source: ToolCallSource::Direct,
        payload: ToolPayload::Function {
            arguments: serde_json::json!({ "cmd": "echo hello", "login": true }).to_string(),
        },
    };

    let Err(FunctionCallError::RespondToModel(message)) =
        ExecCommandHandler::default().handle(invocation).await
    else {
        panic!("expected login-shell rejection");
    };
    assert_eq!(
        message,
        "login shell is disabled by config; omit `login` or set it to false."
    );
}

#[test]
fn test_get_command_rejects_explicit_shell_in_zsh_fork_mode() -> anyhow::Result<()> {
    let json = r#"{"cmd": "echo hello", "shell": "/bin/bash"}"#;
    let args: ExecCommandArgs = parse_arguments(json)?;
    let shell_zsh_path = AbsolutePathBuf::from_absolute_path(if cfg!(windows) {
        r"C:\opt\codex\zsh"
    } else {
        "/opt/codex/zsh"
    })?;
    let shell_mode = UnifiedExecShellMode::ZshFork(ZshForkConfig {
        shell_zsh_path,
        main_execve_wrapper_exe: AbsolutePathBuf::from_absolute_path(if cfg!(windows) {
            r"C:\opt\codex\codex-execve-wrapper"
        } else {
            "/opt/codex/codex-execve-wrapper"
        })?,
    });

    let err = get_command(
        &args,
        Arc::new(default_user_shell()),
        &shell_mode,
        /*allow_login_shell*/ true,
    )
    .expect_err("explicit shell should be rejected");

    assert!(
        err.contains("`shell` is not supported for local zsh-fork exec"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[tokio::test]
async fn shell_mode_for_environment_uses_direct_mode_for_remote_environments() -> anyhow::Result<()>
{
    let shell_zsh_path = AbsolutePathBuf::from_absolute_path(if cfg!(windows) {
        r"C:\opt\codex\zsh"
    } else {
        "/opt/codex/zsh"
    })?;
    let shell_mode = UnifiedExecShellMode::ZshFork(ZshForkConfig {
        shell_zsh_path,
        main_execve_wrapper_exe: AbsolutePathBuf::from_absolute_path(if cfg!(windows) {
            r"C:\opt\codex\codex-execve-wrapper"
        } else {
            "/opt/codex/codex-execve-wrapper"
        })?,
    });
    let local_environment = Environment::default_for_tests();
    let remote_environment =
        Environment::create_for_tests(Some("ws://127.0.0.1:1/remote-exec-server".to_string()))?;

    assert_eq!(
        shell_mode_for_environment(&shell_mode, &local_environment),
        shell_mode
    );
    assert_eq!(
        shell_mode_for_environment(&shell_mode, &remote_environment),
        UnifiedExecShellMode::Direct
    );

    Ok(())
}

#[tokio::test]
#[cfg(not(windows))]
async fn exec_command_reuses_foreign_windows_grant() {
    use crate::session::tests::make_session_and_context_with_auth_and_config_and_rx;
    use codex_features::Feature;
    use codex_protocol::models::AdditionalPermissionProfile;
    use codex_protocol::models::FileSystemPermissions;
    use codex_utils_path_uri::PathUri;

    let (session, mut turn, _events) = make_session_and_context_with_auth_and_config_and_rx(
        codex_login::CodexAuth::from_api_key("Test API Key"),
        Vec::new(),
        |config| {
            config
                .features
                .enable(Feature::RequestPermissionsTool)
                .expect("test setup should allow request permissions");
        },
    )
    .await;

    let cwd = PathUri::parse("file:///C:/workspace").expect("valid Windows cwd");
    let granted_permissions = AdditionalPermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_path_uris(
            /*read*/ Some(Vec::new()),
            /*write*/
            Some(vec![
                PathUri::parse("file:///C:/workspace/granted").expect("valid Windows grant"),
            ]),
        )),
        ..Default::default()
    };
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());
    let turn_state = {
        let active_turn = session.active_turn.lock().await;
        Arc::clone(&active_turn.as_ref().expect("active turn").turn_state)
    };
    turn_state.lock().await.record_granted_permissions(
        codex_exec_server::REMOTE_ENVIRONMENT_ID,
        granted_permissions.clone(),
    );

    {
        let turn = Arc::get_mut(&mut turn).expect("turn should be uniquely owned");
        let TurnEnvironmentState::Ready(environment) = turn
            .initial_environments
            .environments
            .first_mut()
            .expect("primary environment")
        else {
            panic!("primary environment should be ready");
        };
        environment.selection.environment_id = codex_exec_server::REMOTE_ENVIRONMENT_ID.to_string();
        environment.selection.cwd = cwd.clone();
        environment.selection.workspace_roots = vec![cwd.clone()];
        environment.config_mut().workspace_roots = vec![cwd];
        environment.environment = Arc::new(
            Environment::create_for_tests(Some("ws://127.0.0.1:1/remote-exec-server".to_string()))
                .expect("remote environment"),
        );
    }

    let response = ExecCommandHandler::default()
        .handle(ToolInvocation {
            session: Arc::clone(&session),
            step_context: StepContext::for_test(Arc::clone(&turn)),
            turn,
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            tracker: Arc::new(Mutex::new(TurnDiffTracker::new())),
            call_id: "foreign-windows-grant".to_string(),
            tool_name: codex_tools::ToolName::plain("exec_command"),
            source: ToolCallSource::Direct,
            payload: ToolPayload::Function {
                arguments: serde_json::json!({
                    "cmd": "*** Begin Patch\n*** Add File: granted/file.txt\n+text\n*** End Patch",
                    "workdir": "nested",
                    "sandbox_permissions": "with_additional_permissions",
                    "additional_permissions": granted_permissions,
                })
                .to_string(),
            },
        })
        .await;

    let Err(FunctionCallError::RespondToModel(message)) = response else {
        panic!("raw patch should stop before remote execution");
    };
    assert!(
        message.contains("apply_patch verification failed"),
        "matching foreign grant should reach patch interception: {message}"
    );
}

#[tokio::test]
async fn exec_command_pre_tool_use_payload_uses_raw_command() {
    let payload = ToolPayload::Function {
        arguments: serde_json::json!({ "cmd": "printf exec command" }).to_string(),
    };
    let (session, turn) = make_session_and_context().await;
    let turn = Arc::new(turn);
    let handler = ExecCommandHandler::default();

    assert_eq!(
        handler.pre_tool_use_payload(&ToolInvocation {
            session: session.into(),
            step_context: StepContext::for_test(Arc::clone(&turn)),
            turn,
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            tracker: Arc::new(Mutex::new(TurnDiffTracker::new())),
            call_id: "call-43".to_string(),
            tool_name: codex_tools::ToolName::plain("exec_command"),
            source: crate::tools::context::ToolCallSource::Direct,
            payload,
        }),
        Some(crate::tools::registry::PreToolUsePayload {
            tool_name: HookToolName::bash(),
            tool_input: serde_json::json!({ "command": "printf exec command" }),
        })
    );
}

#[tokio::test]
async fn exec_command_pre_tool_use_payload_skips_write_stdin() {
    let payload = ToolPayload::Function {
        arguments: serde_json::json!({ "chars": "echo hi" }).to_string(),
    };
    let (session, turn) = make_session_and_context().await;
    let turn = Arc::new(turn);
    let handler = WriteStdinHandler;

    assert_eq!(
        handler.pre_tool_use_payload(&ToolInvocation {
            session: session.into(),
            step_context: StepContext::for_test(Arc::clone(&turn)),
            turn,
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            tracker: Arc::new(Mutex::new(TurnDiffTracker::new())),
            call_id: "call-44".to_string(),
            tool_name: codex_tools::ToolName::plain("write_stdin"),
            source: crate::tools::context::ToolCallSource::Direct,
            payload,
        }),
        None
    );
}

#[tokio::test]
async fn exec_command_post_tool_use_payload_uses_output_for_noninteractive_one_shot_commands() {
    let payload = ToolPayload::Function {
        arguments: serde_json::json!({ "cmd": "echo three", "tty": false }).to_string(),
    };
    let output = ExecCommandToolOutput {
        event_call_id: "call-43".to_string(),
        chunk_id: "chunk-1".to_string(),
        wall_time: std::time::Duration::from_millis(498),
        raw_output: b"three".to_vec(),
        truncation_policy: TEST_TRUNCATION_POLICY,
        max_output_tokens: None,
        process_id: None,
        exit_code: Some(0),
        original_token_count: None,
        output_omitted_bytes: None,
        hook_command: Some("echo three".to_string()),
    };
    let invocation = invocation_for_payload("exec_command", "call-43", payload).await;
    let handler = ExecCommandHandler::default();
    assert_eq!(
        handler.post_tool_use_payload(&invocation, &output),
        Some(crate::tools::registry::PostToolUsePayload {
            tool_name: HookToolName::bash(),
            tool_use_id: "call-43".to_string(),
            tool_input: serde_json::json!({ "command": "echo three" }),
            tool_response: serde_json::json!("three"),
        })
    );
}

#[tokio::test]
async fn exec_command_post_tool_use_payload_uses_output_for_interactive_completion() {
    let payload = ToolPayload::Function {
        arguments: serde_json::json!({ "cmd": "echo three", "tty": true }).to_string(),
    };
    let output = ExecCommandToolOutput {
        event_call_id: "call-44".to_string(),
        chunk_id: "chunk-1".to_string(),
        wall_time: std::time::Duration::from_millis(498),
        raw_output: b"three".to_vec(),
        truncation_policy: TEST_TRUNCATION_POLICY,
        max_output_tokens: None,
        process_id: None,
        exit_code: Some(0),
        original_token_count: None,
        output_omitted_bytes: None,
        hook_command: Some("echo three".to_string()),
    };
    let invocation = invocation_for_payload("exec_command", "call-44", payload).await;
    let handler = ExecCommandHandler::default();

    assert_eq!(
        handler.post_tool_use_payload(&invocation, &output),
        Some(crate::tools::registry::PostToolUsePayload {
            tool_name: HookToolName::bash(),
            tool_use_id: "call-44".to_string(),
            tool_input: serde_json::json!({ "command": "echo three" }),
            tool_response: serde_json::json!("three"),
        })
    );
}

#[tokio::test]
async fn exec_command_post_tool_use_payload_skips_running_sessions() {
    let payload = ToolPayload::Function {
        arguments: serde_json::json!({ "cmd": "echo three", "tty": false }).to_string(),
    };
    let output = ExecCommandToolOutput {
        event_call_id: "event-45".to_string(),
        chunk_id: "chunk-1".to_string(),
        wall_time: std::time::Duration::from_millis(498),
        raw_output: b"three".to_vec(),
        truncation_policy: TEST_TRUNCATION_POLICY,
        max_output_tokens: None,
        process_id: Some(45),
        exit_code: None,
        original_token_count: None,
        output_omitted_bytes: None,
        hook_command: Some("echo three".to_string()),
    };
    let invocation = invocation_for_payload("exec_command", "call-45", payload).await;
    let handler = ExecCommandHandler::default();
    assert_eq!(handler.post_tool_use_payload(&invocation, &output), None);
}

#[tokio::test]
async fn write_stdin_post_tool_use_payload_uses_original_exec_call_id_and_command_on_completion() {
    let payload = ToolPayload::Function {
        arguments: serde_json::json!({
            "session_id": 45,
            "chars": "",
        })
        .to_string(),
    };
    let output = ExecCommandToolOutput {
        event_call_id: "exec-call-45".to_string(),
        chunk_id: "chunk-2".to_string(),
        wall_time: std::time::Duration::from_millis(498),
        raw_output: b"finished\n".to_vec(),
        truncation_policy: TEST_TRUNCATION_POLICY,
        max_output_tokens: None,
        process_id: None,
        exit_code: Some(0),
        original_token_count: None,
        output_omitted_bytes: None,
        hook_command: Some("sleep 1; echo finished".to_string()),
    };
    let invocation = invocation_for_payload("write_stdin", "write-stdin-call", payload).await;
    let handler = WriteStdinHandler;

    assert_eq!(
        handler.post_tool_use_payload(&invocation, &output),
        Some(crate::tools::registry::PostToolUsePayload {
            tool_name: HookToolName::bash(),
            tool_use_id: "exec-call-45".to_string(),
            tool_input: serde_json::json!({ "command": "sleep 1; echo finished" }),
            tool_response: serde_json::json!("finished\n"),
        })
    );
}

#[tokio::test]
async fn write_stdin_post_tool_use_payload_keeps_parallel_session_metadata_separate() {
    let payload = ToolPayload::Function {
        arguments: serde_json::json!({ "session_id": 45, "chars": "" }).to_string(),
    };
    let output_a = ExecCommandToolOutput {
        event_call_id: "exec-call-a".to_string(),
        chunk_id: "chunk-a".to_string(),
        wall_time: std::time::Duration::from_millis(498),
        raw_output: b"alpha\n".to_vec(),
        truncation_policy: TEST_TRUNCATION_POLICY,
        max_output_tokens: None,
        process_id: None,
        exit_code: Some(0),
        original_token_count: None,
        output_omitted_bytes: None,
        hook_command: Some("sleep 2; echo alpha".to_string()),
    };
    let output_b = ExecCommandToolOutput {
        event_call_id: "exec-call-b".to_string(),
        chunk_id: "chunk-b".to_string(),
        wall_time: std::time::Duration::from_millis(498),
        raw_output: b"beta\n".to_vec(),
        truncation_policy: TEST_TRUNCATION_POLICY,
        max_output_tokens: None,
        process_id: None,
        exit_code: Some(0),
        original_token_count: None,
        output_omitted_bytes: None,
        hook_command: Some("sleep 1; echo beta".to_string()),
    };
    let invocation_b = invocation_for_payload("write_stdin", "write-call-b", payload.clone()).await;
    let invocation_a = invocation_for_payload("write_stdin", "write-call-a", payload).await;
    let handler = WriteStdinHandler;

    let payloads = [
        handler.post_tool_use_payload(&invocation_b, &output_b),
        handler.post_tool_use_payload(&invocation_a, &output_a),
    ];

    assert_eq!(
        payloads,
        [
            Some(crate::tools::registry::PostToolUsePayload {
                tool_name: HookToolName::bash(),
                tool_use_id: "exec-call-b".to_string(),
                tool_input: serde_json::json!({ "command": "sleep 1; echo beta" }),
                tool_response: serde_json::json!("beta\n"),
            }),
            Some(crate::tools::registry::PostToolUsePayload {
                tool_name: HookToolName::bash(),
                tool_use_id: "exec-call-a".to_string(),
                tool_input: serde_json::json!({ "command": "sleep 2; echo alpha" }),
                tool_response: serde_json::json!("alpha\n"),
            }),
        ]
    );
}

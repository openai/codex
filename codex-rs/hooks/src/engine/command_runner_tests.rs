use std::collections::HashMap;
use std::ffi::OsString;
#[cfg(windows)]
use std::fs;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_channel::Receiver;
use codex_protocol::ThreadId;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookSource;
use codex_protocol::shell_environment::CODEX_EXEC_SERVER_NOISE_AUTH_TOKEN_ENV_VAR;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tempfile::tempdir;
use tokio::time::sleep;
use tokio::time::timeout;

use super::super::ClaudeHooksEngine;
use super::super::ConfiguredHandlerKind;
use super::super::tests::mcp_executor;
use super::CommandHookRuntime;
use super::CommandShell;
use super::ConfiguredHandler;
use super::MAX_CONCURRENT_ASYNC_HOOKS;
use super::build_command;
use super::default_shell_program;
use super::run_command;
use crate::events::user_prompt_submit::UserPromptSubmitRequest;

#[cfg(unix)]
#[tokio::test]
async fn hook_shell_startup_does_not_stop_on_controlling_terminal() {
    const CHILD_ENV: &str = "CODEX_HOOK_TERMINAL_TEST_CHILD";
    const TEST_NAME: &str =
        "engine::command_runner::tests::hook_shell_startup_does_not_stop_on_controlling_terminal";

    if std::env::var_os(CHILD_ENV).is_none() {
        // Re-exec under a controlling terminal even when the test runner has none.
        let executable = std::env::current_exe().expect("current test executable");
        let mut env = std::env::vars().collect::<HashMap<_, _>>();
        env.insert(CHILD_ENV.to_string(), "1".to_string());
        let codex_utils_pty::SpawnedProcess {
            session: _session,
            mut stdout_rx,
            exit_rx,
            ..
        } = codex_utils_pty::spawn_pty_process(
            executable.to_str().expect("UTF-8 test executable path"),
            &[
                TEST_NAME.to_string(),
                "--exact".to_string(),
                "--nocapture".to_string(),
            ],
            &std::env::current_dir().expect("current test directory"),
            &env,
            /*arg0*/ &None,
            codex_utils_pty::TerminalSize::default(),
            &[],
        )
        .await
        .expect("spawn test with a controlling terminal");

        let (exit_code, output) = timeout(Duration::from_secs(10), async {
            let output = async {
                let mut output = Vec::new();
                while let Some(chunk) = stdout_rx.recv().await {
                    output.extend_from_slice(&chunk);
                }
                output
            };
            tokio::join!(exit_rx, output)
        })
        .await
        .expect("terminal hook test should finish");
        let output = String::from_utf8_lossy(&output);
        assert_eq!(
            exit_code.expect("terminal hook test exit status"),
            0,
            "{output}"
        );
        assert!(
            output.contains(TEST_NAME),
            "child did not run test: {output}"
        );
        return;
    }

    std::fs::File::open("/dev/tty").expect("test process must have a controlling terminal");
    let temp = tempdir().expect("create temp dir");
    let startup_path = temp.path().join("bashenv");
    std::fs::write(
        &startup_path,
        "command -v stty >/dev/null || exit 1\nstty sane < /dev/tty\n",
    )
    .expect("write shell startup fixture");
    let command = "printf hook-ran";
    let env = HashMap::from([(
        "BASH_ENV".to_string(),
        startup_path
            .to_str()
            .expect("UTF-8 startup path")
            .to_string(),
    )]);
    let handler = ConfiguredHandler {
        builtin: false,
        event_name: HookEventName::SessionStart,
        matcher: None,
        timeout_sec: 2,
        status_message: None,
        additional_context_limit: Default::default(),
        source_path: AbsolutePathBuf::try_from(temp.path().join("hooks.json"))
            .expect("absolute hook configuration path")
            .into(),
        source: HookSource::User,
        display_order: 0,
        kind: ConfiguredHandlerKind::Command {
            command: command.to_string(),
            r#async: false,
            env: env.clone(),
        },
    };
    let (result_sender, _result_receiver) = async_channel::unbounded();
    let runtime = CommandHookRuntime::new(
        CommandShell {
            program: "/bin/bash".to_string(),
            args: vec!["-c".to_string()],
        },
        Arc::new(std::env::vars_os().collect()),
        ThreadId::new(),
        result_sender,
    );

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    static FORKS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use std::sync::atomic::Ordering;
        extern "C" fn record_parent_fork() {
            FORKS.fetch_add(1, Ordering::SeqCst);
        }
        // Registration cannot be undone, so keep it in this isolated TTY child.
        // SAFETY: The callback only updates a lock-free atomic in the parent.
        assert_eq!(
            unsafe {
                libc::pthread_atfork(
                    /*prepare*/ None,
                    Some(record_parent_fork),
                    /*child*/ None,
                )
            },
            0
        );
        let mut baseline = tokio::process::Command::new("/bin/sh");
        baseline.args(["-c", "exit 0"]);
        // SAFETY: A no-op callback forces fork for the positive control.
        unsafe {
            baseline.pre_exec(|| Ok(()));
        }
        assert!(baseline.status().await.expect("fork control").success());
        assert_eq!(FORKS.load(Ordering::SeqCst), 1);
    }

    let result = run_command(&runtime, &handler, command, &env, "{}", temp.path()).await;
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        let native_chdir_supported = {
            // SAFETY: Only inspect symbol availability, matching the launcher.
            !unsafe {
                libc::dlsym(
                    libc::RTLD_DEFAULT,
                    c"posix_spawn_file_actions_addchdir_np".as_ptr(),
                )
            }
            .is_null()
        };
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        let native_chdir_supported = true;
        if native_chdir_supported {
            assert_eq!(
                FORKS.load(std::sync::atomic::Ordering::SeqCst),
                1,
                "hook must avoid forking its parent"
            );
        }
    }
    assert_eq!(result.exit_code, Some(0), "stderr: {}", result.stderr);
    assert_eq!(result.stdout, "hook-ran");
    assert_eq!(result.error, None);
}

#[cfg(windows)]
#[tokio::test]
async fn cmd_shell_runs_quoted_hook_command_path() {
    let temp = tempdir().expect("create temp dir");
    let hook_dir = temp.path().join("hook with spaces");
    fs::create_dir(&hook_dir).expect("create hook dir");
    let hook_path = hook_dir.join("hook.cmd");
    fs::write(
        &hook_path,
        "@echo off\r\nif not \"%~1\"==\"notify\" exit /B 7\r\necho hook-ran\r\n",
    )
    .expect("write hook command");
    let source_path =
        AbsolutePathBuf::try_from(hook_path.clone()).expect("absolute hook command path");
    let command = format!(r#""{}" notify"#, hook_path.display());
    let env = HashMap::new();
    let handler = ConfiguredHandler {
        builtin: false,
        event_name: HookEventName::SessionStart,
        matcher: None,
        timeout_sec: 10,
        status_message: None,
        additional_context_limit: Default::default(),
        source_path: source_path.into(),
        source: HookSource::User,
        display_order: 0,
        kind: ConfiguredHandlerKind::Command {
            command: command.clone(),
            r#async: false,
            env: env.clone(),
        },
    };
    let shells = [
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
        CommandShell {
            program: std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string()),
            args: vec!["/c".to_string()],
        },
    ];

    for shell in shells {
        let (result_sender, _result_receiver) = async_channel::unbounded();
        let runtime = CommandHookRuntime::new(
            shell,
            Arc::new(std::env::vars_os().collect()),
            ThreadId::new(),
            result_sender,
        );
        let result = run_command(&runtime, &handler, &command, &env, "{}", temp.path()).await;

        assert_eq!(result.exit_code, Some(0), "stderr: {}", result.stderr);
        assert_eq!(result.stdout.trim(), "hook-ran");
        assert!(result.error.is_none());
    }
}

#[tokio::test]
async fn fast_exiting_hook_preserves_stdout_when_stdin_is_not_consumed() {
    let temp = tempdir().expect("create temp dir");
    let source_path = AbsolutePathBuf::try_from(temp.path().join("hooks.json"))
        .expect("absolute hook configuration path");
    let command = "echo hook-ran";
    let env = HashMap::new();
    let handler = ConfiguredHandler {
        builtin: false,
        event_name: HookEventName::SessionStart,
        matcher: None,
        timeout_sec: 10,
        status_message: None,
        additional_context_limit: Default::default(),
        source_path: source_path.into(),
        source: HookSource::User,
        display_order: 0,
        kind: ConfiguredHandlerKind::Command {
            command: command.to_string(),
            r#async: false,
            env: env.clone(),
        },
    };
    let input_json = format!(r#"{{"padding":"{}"}}"#, "x".repeat(1024 * 1024));
    let (runtime, _result_receiver) = runtime();

    let result = run_command(&runtime, &handler, command, &env, &input_json, temp.path()).await;

    assert_eq!(result.exit_code, Some(0), "stderr: {}", result.stderr);
    assert_eq!(result.stdout.trim(), "hook-ran");
    assert_eq!(result.error, None);
}

#[tokio::test]
async fn hook_drains_output_and_times_out_while_stdin_is_blocked() {
    let temp = tempdir().expect("create temp dir");
    let mut handler = write_handler(
        &temp,
        r#"from pathlib import Path
import sys
import time

sys.stdout.write("x" * 1024 * 1024)
sys.stdout.flush()
sys.stderr.write("x" * 1024 * 1024)
sys.stderr.flush()
Path("output-drained").touch()
time.sleep(30)
"#,
    );
    handler.timeout_sec = 2;
    let ConfiguredHandlerKind::Command { command, env, .. } = &handler.kind else {
        panic!("expected command hook");
    };
    let input_json = format!(r#"{{"padding":"{}"}}"#, "x".repeat(1024 * 1024));
    let (runtime, _result_receiver) = runtime();

    // Keep user shell startup files out of the pipe I/O timeout test.
    let runtime = runtime.reconfigured(if cfg!(windows) {
        CommandShell {
            program: "cmd.exe".to_string(),
            args: vec!["/D".to_string(), "/C".to_string()],
        }
    } else {
        CommandShell {
            program: "/bin/sh".to_string(),
            args: vec!["-c".to_string()],
        }
    });

    let result = timeout(
        Duration::from_secs(10),
        run_command(&runtime, &handler, command, env, &input_json, temp.path()),
    )
    .await
    .expect("the hook timeout must also cover blocked stdin writes");

    assert!(temp.path().join("output-drained").exists());
    assert_eq!(result.exit_code, None);
    assert_eq!(result.error, Some("hook timed out after 2s".to_string()));
}

#[tokio::test]
async fn command_hook_does_not_expose_configured_noise_auth_token() {
    let temp = tempdir().expect("create temp dir");
    let source_path = AbsolutePathBuf::try_from(temp.path().join("hooks.json"))
        .expect("absolute hook configuration path");
    let command = if cfg!(windows) { "set" } else { "env" };
    let env = HashMap::from([
        (
            CODEX_EXEC_SERVER_NOISE_AUTH_TOKEN_ENV_VAR.to_ascii_lowercase(),
            "configured-noise-token".to_string(),
        ),
        ("CODEX_HOOK_SAFE_ENV".to_string(), "visible".to_string()),
    ]);
    let handler = ConfiguredHandler {
        builtin: false,
        event_name: HookEventName::SessionStart,
        matcher: None,
        timeout_sec: 10,
        status_message: None,
        additional_context_limit: Default::default(),
        source_path: source_path.into(),
        source: HookSource::User,
        display_order: 0,
        kind: ConfiguredHandlerKind::Command {
            command: command.to_string(),
            r#async: false,
            env: env.clone(),
        },
    };
    let (runtime, _result_receiver) = runtime();

    let result = run_command(&runtime, &handler, command, &env, "{}", temp.path()).await;

    assert_eq!(result.exit_code, Some(0), "stderr: {}", result.stderr);
    assert!(result.stdout.contains("CODEX_HOOK_SAFE_ENV=visible"));
    assert!(!result.stdout.lines().any(|line| {
        line.split_once('=').is_some_and(|(name, _)| {
            name.eq_ignore_ascii_case(CODEX_EXEC_SERVER_NOISE_AUTH_TOKEN_ENV_VAR)
        })
    }));
    assert_eq!(result.error, None);
}

#[tokio::test]
async fn build_command_replays_snapshot_before_hook_overrides_and_scrubbing() {
    #[cfg(unix)]
    let non_unicode_value = OsString::from_vec(vec![b'v', 0xff]);
    let environment = vec![
        (
            OsString::from("CODEX_HOOK_SNAPSHOT"),
            OsString::from("captured"),
        ),
        (
            OsString::from("CODEX_HOOK_OVERRIDE"),
            OsString::from("captured"),
        ),
        (
            OsString::from(CODEX_EXEC_SERVER_NOISE_AUTH_TOKEN_ENV_VAR),
            OsString::from("captured-noise-token"),
        ),
        #[cfg(unix)]
        (OsString::from("CODEX_HOOK_NON_UNICODE"), non_unicode_value),
    ];
    let env = HashMap::from([
        ("CODEX_HOOK_OVERRIDE".to_string(), "configured".to_string()),
        ("CODEX_HOOK_SAFE_ENV".to_string(), "visible".to_string()),
        (
            CODEX_EXEC_SERVER_NOISE_AUTH_TOKEN_ENV_VAR.to_string(),
            "configured-noise-token".to_string(),
        ),
    ]);
    #[cfg(not(windows))]
    let (program, args, command_line) = (
        "/bin/sh",
        vec!["-c".to_string()],
        "printf '%s\\n' \"$CODEX_HOOK_SNAPSHOT\" \"$CODEX_HOOK_OVERRIDE\" \"$CODEX_HOOK_SAFE_ENV\" \"${CODEX_EXEC_SERVER_NOISE_AUTH_TOKEN-absent}\" \"$CODEX_HOOK_NON_UNICODE\"",
    );
    #[cfg(windows)]
    let (program, args, command_line) = (
        "cmd.exe",
        vec!["/D".to_string(), "/C".to_string()],
        "echo %CODEX_HOOK_SNAPSHOT%&echo %CODEX_HOOK_OVERRIDE%&echo %CODEX_HOOK_SAFE_ENV%&if defined CODEX_EXEC_SERVER_NOISE_AUTH_TOKEN (echo leaked) else (echo absent)",
    );
    let command = build_command(
        &CommandShell {
            program: program.to_string(),
            args,
        },
        command_line,
        &environment,
        &env,
    );
    #[cfg(not(unix))]
    let mut command = command;
    let output = command
        .spawn()
        .expect("spawn hook")
        .wait_with_output()
        .await
        .expect("hook output");
    assert!(output.status.success(), "{output:?}");
    #[cfg(unix)]
    assert_eq!(
        output.stdout,
        b"captured\nconfigured\nvisible\nabsent\nv\xff\n"
    );
    #[cfg(windows)]
    assert_eq!(
        output.stdout,
        b"captured\r\nconfigured\r\nvisible\r\nabsent\r\n"
    );
}

#[test]
fn fallback_shell_uses_snapshot() {
    #[cfg(windows)]
    let (name, program) = ("comspec", r"C:\captured\cmd.exe");
    #[cfg(not(windows))]
    let (name, program) = ("SHELL", "/captured/shell");
    assert_eq!(
        default_shell_program(&[(OsString::from(name), OsString::from(program))]),
        OsString::from(program),
    );
}

const ASYNC_HOOK_TEST_TIMEOUT: Duration = Duration::from_secs(30);

fn runtime() -> (CommandHookRuntime, Receiver<HookCompletedEvent>) {
    runtime_with_environment(Arc::new(std::env::vars_os().collect()))
}

fn runtime_with_environment(
    environment: Arc<Vec<(OsString, OsString)>>,
) -> (CommandHookRuntime, Receiver<HookCompletedEvent>) {
    let thread_id = ThreadId::new();
    let (result_sender, result_receiver) = async_channel::unbounded();
    let runtime = CommandHookRuntime::new(
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
        environment,
        thread_id,
        result_sender,
    );
    (runtime, result_receiver)
}

fn write_handler(temp: &TempDir, source: &str) -> ConfiguredHandler {
    let script_path = temp.path().join("async_hook.py");
    std::fs::write(&script_path, source).expect("write async test hook");
    ConfiguredHandler {
        builtin: false,
        event_name: HookEventName::UserPromptSubmit,
        matcher: None,
        timeout_sec: 10,
        status_message: None,
        additional_context_limit: Default::default(),
        source_path: AbsolutePathBuf::try_from(temp.path().join("hooks.json"))
            .expect("absolute test hook path")
            .into(),
        source: HookSource::User,
        display_order: 0,
        kind: ConfiguredHandlerKind::Command {
            command: format!("python3 {}", script_path.display()),
            r#async: true,
            env: HashMap::new(),
        },
    }
}

async fn schedule(runtime: &CommandHookRuntime, handler: ConfiguredHandler, cwd: &Path) {
    let engine = ClaudeHooksEngine {
        handlers: vec![handler],
        warnings: Vec::new(),
        required_load_errors: Vec::new(),
        command_runtime: runtime.clone(),
        mcp_executor: mcp_executor(),
    };
    engine
        .run_user_prompt_submit(UserPromptSubmitRequest {
            session_id: ThreadId::new(),
            turn_id: "async-test-turn".to_string(),
            subagent: None,
            cwd: AbsolutePathBuf::try_from(cwd.to_path_buf()).expect("absolute test hook cwd"),
            transcript_path: None,
            model: "test-model".to_string(),
            permission_mode: "default".to_string(),
            prompt: "test prompt".to_string(),
        })
        .await;
}

#[tokio::test]
async fn async_hook_marks_invalid_structured_output_as_failed() {
    let temp = TempDir::new().expect("async test directory");
    let (runtime, results) = runtime();
    let handler = write_handler(
        &temp,
        r#"import sys

sys.stdin.read()
print('{"systemMessage": 123}')
"#,
    );

    schedule(&runtime, handler, temp.path()).await;
    let hook_result = timeout(ASYNC_HOOK_TEST_TIMEOUT, results.recv())
        .await
        .expect("invalid async output should still finish its background task")
        .expect("result receiver should remain open");

    assert_eq!(hook_result.run.status, HookRunStatus::Failed);
    assert_eq!(
        hook_result.run.entries,
        vec![HookOutputEntry {
            kind: HookOutputEntryKind::Error,
            text: "hook returned invalid user prompt submit JSON output".to_string(),
        }]
    );

    runtime.shutdown().await;
}

#[tokio::test]
async fn async_hook_result_survives_runtime_reconfiguration() {
    let temp = TempDir::new().expect("async test directory");
    let mut environment = std::env::vars_os().collect::<Vec<_>>();
    environment.push((
        OsString::from("CODEX_HOOK_CAPTURED_ENV"),
        OsString::from("captured"),
    ));
    let (previous, results) = runtime_with_environment(Arc::new(environment));
    let release_path = temp.path().join("release");
    let handler = write_handler(
        &temp,
        &format!(
            r#"import json
import os
from pathlib import Path
import sys
import time

json.load(sys.stdin)
while not Path(r"{}").exists():
    time.sleep(0.01)
print(os.environ["CODEX_HOOK_CAPTURED_ENV"])
"#,
            release_path.display()
        ),
    );
    schedule(&previous, handler, temp.path()).await;

    let reconfigured = previous.reconfigured(CommandShell {
        program: String::new(),
        args: Vec::new(),
    });
    assert!(Arc::ptr_eq(
        &previous.environment,
        &reconfigured.environment
    ));
    std::fs::write(release_path, "ready").expect("release async hook");

    let hook_result = timeout(ASYNC_HOOK_TEST_TIMEOUT, results.recv())
        .await
        .expect("in-flight hook should survive runtime reconfiguration")
        .expect("result receiver should remain open");
    assert_eq!(
        hook_result.run.entries,
        vec![HookOutputEntry {
            kind: HookOutputEntryKind::Context,
            text: "captured".to_string(),
        }]
    );

    reconfigured.shutdown().await;
}

#[tokio::test]
async fn async_hooks_limit_concurrent_processes_without_dropping_waiting_jobs() {
    let temp = TempDir::new().expect("async test directory");
    let (runtime, results) = runtime();
    let started_dir = temp.path().join("started");
    let release_path = temp.path().join("release");
    std::fs::create_dir(&started_dir).expect("create hook marker directory");
    let mut handler = write_handler(
        &temp,
        &format!(
            r#"import os
from pathlib import Path
import sys
import time

sys.stdin.read()
Path(r"{started}", str(os.getpid())).touch()
while not Path(r"{release}").exists():
    time.sleep(0.01)
print("{{}}")
"#,
            started = started_dir.display(),
            release = release_path.display(),
        ),
    );
    handler.timeout_sec = ASYNC_HOOK_TEST_TIMEOUT.as_secs();

    for _ in 0..=MAX_CONCURRENT_ASYNC_HOOKS {
        schedule(&runtime, handler.clone(), temp.path()).await;
    }

    let started_count = || {
        std::fs::read_dir(&started_dir)
            .expect("read hook marker directory")
            .count()
    };
    timeout(ASYNC_HOOK_TEST_TIMEOUT, async {
        while started_count() < MAX_CONCURRENT_ASYNC_HOOKS {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("all available async hook slots should start");
    assert_eq!(started_count(), MAX_CONCURRENT_ASYNC_HOOKS);

    std::fs::write(release_path, "ready").expect("release running async hooks");
    for _ in 0..=MAX_CONCURRENT_ASYNC_HOOKS {
        timeout(ASYNC_HOOK_TEST_TIMEOUT, results.recv())
            .await
            .expect("waiting async hook should eventually finish")
            .expect("result receiver should remain open");
    }
    assert_eq!(started_count(), MAX_CONCURRENT_ASYNC_HOOKS + 1);

    runtime.shutdown().await;
}

#[tokio::test]
async fn shutdown_aborts_in_flight_async_hooks_without_delivering_context() {
    let temp = TempDir::new().expect("async test directory");
    let (runtime, results) = runtime();
    let started_path = temp.path().join("started");
    let release_path = temp.path().join("release");
    let handler = write_handler(
        &temp,
        &format!(
            r#"import json
from pathlib import Path
import sys
import time

json.load(sys.stdin)
Path(r"{started}").write_text("started", encoding="utf-8")
while not Path(r"{release}").exists():
    time.sleep(0.01)
print(json.dumps({{
    "hookSpecificOutput": {{
        "hookEventName": "UserPromptSubmit",
        "additionalContext": "must not be delivered after shutdown"
    }}
}}))
"#,
            started = started_path.display(),
            release = release_path.display(),
        ),
    );
    schedule(&runtime, handler, temp.path()).await;

    timeout(ASYNC_HOOK_TEST_TIMEOUT, async {
        while !started_path.exists() {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("async hook should start before runtime shutdown");

    runtime.shutdown().await;
    drop(runtime);
    std::fs::write(release_path, "ready").expect("release shutdown hook");
    assert!(
        timeout(Duration::from_millis(150), results.recv())
            .await
            .expect("shutdown should close the result channel")
            .is_err(),
        "shutdown must not deliver a late async result"
    );
}

#[cfg(unix)]
async fn process_is_running(pid: &str) -> bool {
    let output = tokio::process::Command::new("ps")
        .args(["-p", pid, "-o", "stat="])
        .output()
        .await
        .expect("inspect hook process state");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert!(output.status.success() || output.status.code() == Some(1));
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .any(|state| !state.starts_with('Z'))
}

#[cfg(unix)]
#[tokio::test]
async fn cancelled_and_timed_out_hooks_kill_their_process_group() {
    enum Termination {
        Cancellation,
        Timeout,
    }

    for termination in [Termination::Cancellation, Termination::Timeout] {
        let temp = tempdir().expect("create temp dir");
        let mut handler = write_handler(
            &temp,
            r#"import os
from pathlib import Path
import subprocess
import time

child = subprocess.Popen(['sleep', '60'])
Path('hook-pids.tmp').write_text(f'{os.getpid()} {child.pid}')
Path('hook-pids.tmp').rename('hook-pids')
time.sleep(60)
"#,
        );
        handler.timeout_sec = 2;
        let ConfiguredHandlerKind::Command { command, env, .. } = &handler.kind else {
            panic!("expected command hook");
        };
        let (mut runtime, _result_receiver) = runtime();
        runtime.shell.program = "/bin/sh".into();
        runtime.shell.args = vec!["-c".into()];
        let mut run = Box::pin(run_command(
            &runtime,
            &handler,
            command,
            env,
            "{}",
            temp.path(),
        ));
        let pids = timeout(Duration::from_secs(10), async {
            loop {
                if let Ok(pids) = std::fs::read_to_string(temp.path().join("hook-pids")) {
                    break pids;
                }
                tokio::select! {
                    result = &mut run => panic!("hook exited before cancellation: {result:?}"),
                    _ = sleep(Duration::from_millis(10)) => {}
                }
            }
        })
        .await
        .expect("hook and descendant must start");
        match termination {
            Termination::Cancellation => drop(run),
            Termination::Timeout => {
                let result = timeout(Duration::from_secs(10), run)
                    .await
                    .expect("hook timeout completes");
                assert_eq!(result.error, Some("hook timed out after 2s".to_string()));
            }
        }
        timeout(Duration::from_secs(10), async {
            loop {
                let mut all_stopped = true;
                for pid in pids.split_whitespace() {
                    all_stopped &= !process_is_running(pid).await;
                }
                if all_stopped {
                    break;
                }
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("cancellation and timeout must stop both hook and descendant");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn completed_hook_preserves_background_descendant() {
    for exit_code in [0, 23] {
        let temp = tempdir().expect("create temp dir");
        let handler = write_handler(
            &temp,
            &format!(
                r#"import subprocess

subprocess.Popen(
    ['sh', '-c', 'i=0; while [ ! -f release-descendant ] && [ "$i" -lt 500 ]; do sleep 0.01; i=$((i + 1)); done; printf survived > descendant-result.tmp; mv descendant-result.tmp descendant-result'],
    stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
raise SystemExit({exit_code})
"#
            ),
        );
        let ConfiguredHandlerKind::Command { command, env, .. } = &handler.kind else {
            panic!("expected command hook");
        };
        let (mut runtime, _result_receiver) = runtime();
        runtime.shell.program = "/bin/sh".into();
        runtime.shell.args = vec!["-c".into()];
        let result = run_command(&runtime, &handler, command, env, "{}", temp.path()).await;
        // Release even on a failed assertion so a failed test does not leak a waiter.
        std::fs::write(temp.path().join("release-descendant"), "").expect("release descendant");
        assert_eq!((result.exit_code, result.error), (Some(exit_code), None));
        let result_path = temp.path().join("descendant-result");
        timeout(Duration::from_secs(10), async {
            while !result_path.exists() {
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("completed hook must leave its descendant running");
        assert_eq!(std::fs::read_to_string(result_path).unwrap(), "survived");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn exited_hook_leader_with_open_descendant_output_still_times_out() {
    let temp = tempdir().expect("create temp dir");
    let mut handler = write_handler(
        &temp,
        r#"from pathlib import Path
import subprocess

child = subprocess.Popen(['sleep', '60'])
Path('descendant-pid').write_text(str(child.pid))
"#,
    );
    handler.timeout_sec = 2;
    let ConfiguredHandlerKind::Command { command, env, .. } = &handler.kind else {
        panic!("expected command hook");
    };
    let (mut runtime, _result_receiver) = runtime();
    runtime.shell.program = "/bin/sh".into();
    runtime.shell.args = vec!["-c".into()];
    let result = run_command(&runtime, &handler, command, env, "{}", temp.path()).await;
    assert_eq!(result.error, Some("hook timed out after 2s".to_string()));
    let pid = std::fs::read_to_string(temp.path().join("descendant-pid"))
        .expect("descendant started before its parent exited");
    timeout(Duration::from_secs(10), async {
        while process_is_running(pid.trim()).await {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("hook timeout must stop the descendant after the leader exits");
}

#[cfg(unix)]
#[tokio::test]
async fn hook_rejects_nul_arguments_before_execution() {
    for nul_in_shell_args in [false, true] {
        let root = tempdir().expect("tempdir");
        let (mut runtime, _receiver) = runtime();
        runtime.shell = CommandShell {
            program: "/usr/bin/touch".to_string(),
            args: if nul_in_shell_args {
                vec!["bad\0arg".to_string()]
            } else {
                Vec::new()
            },
        };
        let command = if nul_in_shell_args { "ran" } else { "bad\0arg" };
        let handler = write_handler(&root, "");
        let result = run_command(
            &runtime,
            &handler,
            command,
            &HashMap::new(),
            "",
            root.path(),
        )
        .await;
        assert!(result.error.is_some(), "invalid hook ran: {result:?}");
        assert_eq!(result.exit_code, None);
        assert!(!root.path().join("ran").exists());
        assert!(!root.path().join("<string-with-nul>").exists());
    }
}

#[cfg(unix)]
#[tokio::test]
async fn hook_rejects_nul_in_configured_and_default_shell_program() {
    for configured in [false, true] {
        let root = tempdir().expect("tempdir");
        let (mut runtime, _receiver) = runtime_with_environment(Arc::new(vec![(
            OsString::from("SHELL"),
            OsString::from("/bin/sh\0"),
        )]));
        if configured {
            runtime.shell.program = "/bin/sh\0".to_string();
        }
        let handler = write_handler(&root, "");
        let result = run_command(
            &runtime,
            &handler,
            "printf ran > ran",
            &HashMap::new(),
            "",
            root.path(),
        )
        .await;
        assert_eq!(
            result.error.as_deref(),
            Some("nul byte found in provided data")
        );
        assert_eq!(result.exit_code, None);
        assert!(!root.path().join("ran").exists());
    }
}

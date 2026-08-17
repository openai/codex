use std::collections::HashMap;
#[cfg(windows)]
use std::fs;
use std::path::Path;
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
use super::CommandHookRuntime;
use super::CommandShell;
use super::ConfiguredHandler;
use super::MAX_CONCURRENT_ASYNC_HOOKS;
use super::run_command;
use crate::events::user_prompt_submit::UserPromptSubmitRequest;

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
        event_name: HookEventName::SessionStart,
        matcher: None,
        timeout_sec: 10,
        status_message: None,
        additional_context_limit: Default::default(),
        source_path,
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
        let runtime = CommandHookRuntime::new(shell, ThreadId::new(), result_sender);
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
        event_name: HookEventName::SessionStart,
        matcher: None,
        timeout_sec: 10,
        status_message: None,
        additional_context_limit: Default::default(),
        source_path,
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
        event_name: HookEventName::SessionStart,
        matcher: None,
        timeout_sec: 10,
        status_message: None,
        additional_context_limit: Default::default(),
        source_path,
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

const ASYNC_HOOK_TEST_TIMEOUT: Duration = Duration::from_secs(30);

fn runtime() -> (CommandHookRuntime, Receiver<HookCompletedEvent>) {
    let thread_id = ThreadId::new();
    let (result_sender, result_receiver) = async_channel::unbounded();
    let runtime = CommandHookRuntime::new(
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
        thread_id,
        result_sender,
    );
    (runtime, result_receiver)
}

fn write_handler(temp: &TempDir, source: &str) -> ConfiguredHandler {
    let script_path = temp.path().join("async_hook.py");
    std::fs::write(&script_path, source).expect("write async test hook");
    ConfiguredHandler {
        event_name: HookEventName::UserPromptSubmit,
        matcher: None,
        timeout_sec: 10,
        status_message: None,
        additional_context_limit: Default::default(),
        source_path: AbsolutePathBuf::try_from(temp.path().join("hooks.json"))
            .expect("absolute test hook path"),
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
        mcp_executor: None,
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
    let (previous, results) = runtime();
    let release_path = temp.path().join("release");
    let handler = write_handler(
        &temp,
        &format!(
            r#"import json
from pathlib import Path
import sys
import time

json.load(sys.stdin)
while not Path(r"{}").exists():
    time.sleep(0.01)
print("survived reconfiguration")
"#,
            release_path.display()
        ),
    );
    schedule(&previous, handler, temp.path()).await;

    let reconfigured = previous.reconfigured(CommandShell {
        program: String::new(),
        args: Vec::new(),
    });
    std::fs::write(release_path, "ready").expect("release async hook");

    let hook_result = timeout(ASYNC_HOOK_TEST_TIMEOUT, results.recv())
        .await
        .expect("in-flight hook should survive runtime reconfiguration")
        .expect("result receiver should remain open");
    assert_eq!(
        hook_result.run.entries,
        vec![HookOutputEntry {
            kind: HookOutputEntryKind::Context,
            text: "survived reconfiguration".to_string(),
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

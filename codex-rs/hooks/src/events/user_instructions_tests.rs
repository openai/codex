use codex_protocol::ThreadId;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookScope;
use codex_protocol::protocol::HookSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;

use super::UserInstructionsRequest;
use super::UserInstructionsResult;
use super::parse_completed;
use super::preview;
use super::run;
use crate::engine::CommandShell;
use crate::engine::ConfiguredHandler;
use crate::engine::command_runner::CommandRunResult;

#[test]
fn successful_stdout_is_trimmed_literal_text_without_output_entry() {
    let parsed = parse_completed(
        &handler(),
        run_result(Some(0), "  {\"literal\":true}\n", ""),
        /*turn_id*/ None,
    );

    assert_eq!(
        parsed.data.result,
        Some(UserInstructionsResult {
            text: "{\"literal\":true}".to_string(),
            source_path: handler_source_uri(),
        })
    );
    assert_eq!(parsed.data.warning, None);
    assert_eq!(parsed.completed.turn_id, None);
    assert_eq!(parsed.completed.run.status, HookRunStatus::Completed);
    assert_eq!(parsed.completed.run.entries, Vec::new());
}

#[test]
fn empty_stdout_warns_without_instructions() {
    let parsed = parse_completed(
        &handler(),
        run_result(Some(0), " \n", ""),
        /*turn_id*/ None,
    );

    assert_eq!(parsed.data.result, None);
    assert_eq!(
        parsed.data.warning,
        Some(format!(
            "UserInstructions hook from {} returned no instructions",
            handler_source_uri()
        ))
    );
    assert_eq!(parsed.completed.run.status, HookRunStatus::Completed);
    assert_eq!(
        parsed.completed.run.entries,
        vec![HookOutputEntry {
            kind: HookOutputEntryKind::Warning,
            text: "returned no instructions".to_string(),
        }]
    );
}

#[test]
fn nonzero_exit_warns_and_includes_stderr() {
    let parsed = parse_completed(
        &handler(),
        run_result(Some(7), "ignored", "provider unavailable\n"),
        /*turn_id*/ None,
    );

    assert_eq!(parsed.data.result, None);
    assert_eq!(
        parsed.data.warning,
        Some(format!(
            "UserInstructions hook from {} failed: hook exited with code 7: provider unavailable",
            handler_source_uri()
        ))
    );
    assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
    assert_eq!(
        parsed.completed.run.entries,
        vec![HookOutputEntry {
            kind: HookOutputEntryKind::Error,
            text: "hook exited with code 7: provider unavailable".to_string(),
        }]
    );
}

#[test]
fn timeout_warns_without_instructions() {
    let mut result = run_result(/*exit_code*/ None, "ignored", "");
    result.error = Some("hook timed out after 1s".to_string());

    let parsed = parse_completed(&handler(), result, /*turn_id*/ None);

    assert_eq!(parsed.data.result, None);
    assert_eq!(
        parsed.data.warning,
        Some(format!(
            "UserInstructions hook from {} failed: hook timed out after 1s",
            handler_source_uri()
        ))
    );
    assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
    assert_eq!(
        parsed.completed.run.entries,
        vec![HookOutputEntry {
            kind: HookOutputEntryKind::Error,
            text: "hook timed out after 1s".to_string(),
        }]
    );
}

#[tokio::test]
async fn handler_runs_and_returns_runtime_source() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let cwd = AbsolutePathBuf::try_from(temp_dir.path().to_path_buf()).expect("absolute temp dir");
    let command = if cfg!(windows) {
        "echo   literal instructions"
    } else {
        "printf '  literal instructions  '"
    };
    let handler = handler_with_command(command);
    let request = request(cwd);

    let preview_runs = preview(std::slice::from_ref(&handler), &request);
    assert_eq!(preview_runs.len(), 1);
    assert_eq!(preview_runs[0].scope, HookScope::Thread);

    let outcome = run(
        std::slice::from_ref(&handler),
        &CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
        request,
    )
    .await;

    assert_eq!(outcome.warnings, Vec::<String>::new());
    assert_eq!(outcome.hook_events.len(), 1);
    assert_eq!(outcome.hook_events[0].run.entries, Vec::new());
    assert_eq!(
        outcome.results,
        vec![super::UserInstructionsResult {
            text: "literal instructions".to_string(),
            source_path: PathUri::from_abs_path(&handler.source_path),
        }]
    );
}

#[tokio::test]
async fn no_handlers_is_unconfigured() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let cwd = AbsolutePathBuf::try_from(temp_dir.path().to_path_buf()).expect("absolute temp dir");
    let request = request(cwd);

    assert!(preview(&[], &request).is_empty());
    assert_eq!(
        run(
            &[],
            &CommandShell {
                program: String::new(),
                args: Vec::new(),
            },
            request,
        )
        .await,
        super::UserInstructionsOutcome::default()
    );
}

#[tokio::test]
async fn multiple_handlers_run_and_preserve_configured_order() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let cwd = AbsolutePathBuf::try_from(temp_dir.path().to_path_buf()).expect("absolute temp dir");
    let first_command = if cfg!(windows) {
        "echo first instructions"
    } else {
        "printf 'first instructions'"
    };
    let second_command = if cfg!(windows) {
        "echo second instructions"
    } else {
        "printf 'second instructions'"
    };
    let mut first = handler_with_command(first_command);
    first.source_path = test_path_buf("/tmp/first-hooks.json").abs();
    let mut second = handler_with_command(second_command);
    second.source_path = test_path_buf("/tmp/second-hooks.json").abs();
    second.display_order = 1;
    let handlers = vec![first, second];
    let request = request(cwd);

    assert_eq!(preview(&handlers, &request).len(), 2);
    let outcome = run(
        &handlers,
        &CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
        request,
    )
    .await;

    assert_eq!(
        outcome.results,
        vec![
            UserInstructionsResult {
                text: "first instructions".to_string(),
                source_path: PathUri::from_abs_path(&handlers[0].source_path),
            },
            UserInstructionsResult {
                text: "second instructions".to_string(),
                source_path: PathUri::from_abs_path(&handlers[1].source_path),
            },
        ]
    );
    assert_eq!(outcome.hook_events.len(), 2);
    assert_eq!(outcome.warnings, Vec::<String>::new());
}

#[tokio::test]
async fn failed_handler_does_not_discard_successful_sibling_output() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let cwd = AbsolutePathBuf::try_from(temp_dir.path().to_path_buf()).expect("absolute temp dir");
    let failed_command = if cfg!(windows) { "exit /b 7" } else { "exit 7" };
    let successful_command = if cfg!(windows) {
        "echo surviving instructions"
    } else {
        "printf 'surviving instructions'"
    };
    let failed = handler_with_command(failed_command);
    let mut successful = handler_with_command(successful_command);
    successful.source_path = test_path_buf("/tmp/successful-hooks.json").abs();
    successful.display_order = 1;
    let handlers = vec![failed, successful];

    let outcome = run(
        &handlers,
        &CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
        request(cwd),
    )
    .await;

    assert_eq!(
        outcome.results,
        vec![UserInstructionsResult {
            text: "surviving instructions".to_string(),
            source_path: PathUri::from_abs_path(&handlers[1].source_path),
        }]
    );
    assert_eq!(outcome.hook_events.len(), 2);
    assert_eq!(
        outcome.warnings,
        vec![format!(
            "UserInstructions hook from {} failed: hook exited with code 7",
            PathUri::from_abs_path(&handlers[0].source_path)
        )]
    );
}

fn handler() -> ConfiguredHandler {
    handler_with_command("echo hook")
}

fn handler_source_uri() -> PathUri {
    PathUri::from_abs_path(&handler().source_path)
}

fn handler_with_command(command: &str) -> ConfiguredHandler {
    ConfiguredHandler {
        event_name: HookEventName::UserInstructions,
        matcher: None,
        command: command.to_string(),
        timeout_sec: 600,
        status_message: None,
        source_path: test_path_buf("/tmp/hooks.json").abs(),
        source: HookSource::User,
        display_order: 0,
        env: std::collections::HashMap::new(),
    }
}

fn request(cwd: AbsolutePathBuf) -> UserInstructionsRequest {
    UserInstructionsRequest {
        session_id: ThreadId::new(),
        cwd: PathUri::from_abs_path(&cwd),
        command_cwd: cwd,
        transcript_path: None,
        model: "gpt-test".to_string(),
        permission_mode: "default".to_string(),
    }
}

fn run_result(exit_code: Option<i32>, stdout: &str, stderr: &str) -> CommandRunResult {
    CommandRunResult {
        started_at: 1,
        completed_at: 2,
        duration_ms: 1,
        exit_code,
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
        error: None,
    }
}

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_protocol::protocol::HookEventName;
use codex_utils_output_truncation::approx_token_count;
use pretty_assertions::assert_eq;

use super::ASYNC_HOOK_COMPLETION_TOKEN_LIMIT;
use super::ASYNC_HOOK_FLUSH_TOKEN_LIMIT;
use super::AsyncCommandRuntime;
use super::deliverable_output;
use crate::engine::command_runner::CommandRunResult;

#[test]
fn async_output_ignores_control_fields_and_delivers_context() {
    let pre = output(
        HookEventName::PreToolUse,
        r#"{"continue":"not-a-bool","systemMessage":"ignore","decision":"block","reason":"ignore","hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":42,"updatedInput":{"command":"rewrite"},"additionalContext":"pre context"}}"#,
    );
    let permission = output(
        HookEventName::PermissionRequest,
        r#"{"continue":false,"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"ignore"}}}"#,
    );
    let post = output(
        HookEventName::PostToolUse,
        r#"{"continue":false,"decision":"block","reason":"ignore","hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"post context"}}"#,
    );
    let plain_start = output(HookEventName::SessionStart, "plain start context");
    let plain_pre = output(HookEventName::PreToolUse, "ignored plain output");

    assert_eq!(
        (
            pre.as_deref(),
            permission,
            post.as_deref(),
            plain_start.as_deref(),
            plain_pre,
        ),
        (
            Some("pre context"),
            None,
            Some("post context"),
            Some("plain start context"),
            None,
        )
    );
}

#[test]
fn async_output_surfaces_parse_and_runtime_failures() {
    let invalid = deliverable_output(
        HookEventName::PreToolUse,
        &result(Some(0), r#"{"unfinished":"#, "", /*error*/ None),
    );
    let nonzero = deliverable_output(
        HookEventName::PreToolUse,
        &result(Some(2), "", "denied", /*error*/ None),
    );
    let runtime = deliverable_output(
        HookEventName::PreToolUse,
        &result(
            /*exit_code*/ None,
            "",
            "",
            Some("spawn failed".to_string()),
        ),
    );

    assert_eq!(
        (invalid, nonzero, runtime),
        (
            Some("Async PreToolUse hook returned invalid JSON output".to_string()),
            Some("Async hook exited with code 2".to_string()),
            Some("Async hook failed to run: spawn failed".to_string()),
        )
    );
}

#[test]
fn queue_preserves_duplicate_completions_and_holds_arrivals_after_boundary() {
    let runtime = AsyncCommandRuntime::default();
    runtime.push(HookEventName::PreToolUse, "same".to_string());
    runtime.push(HookEventName::PostToolUse, "same".to_string());
    runtime.push(HookEventName::Stop, "last".to_string());

    let boundary = runtime.ready_boundary();
    runtime.push(HookEventName::SessionStart, "next-turn".to_string());

    let text = runtime
        .flush_through(boundary)
        .expect("queued async output");
    assert_eq!(text.matches("same").count(), 2);
    assert!(
        ["PreToolUse", "PostToolUse", "last"]
            .map(|needle| text.find(needle).expect("queued output"))
            .is_sorted()
    );
    assert!(!text.contains("next-turn"));

    let text = runtime
        .flush_through(runtime.ready_boundary())
        .expect("completion after boundary");
    assert!(text.contains("next-turn"));
    assert!(runtime.flush_through(runtime.ready_boundary()).is_none());
}

#[test]
fn queue_bounds_items_and_flushes_a_contiguous_prefix() {
    let runtime = AsyncCommandRuntime::default();
    let large = "large-output ".repeat(ASYNC_HOOK_COMPLETION_TOKEN_LIMIT);
    for event_name in [
        HookEventName::PreToolUse,
        HookEventName::PostToolUse,
        HookEventName::Stop,
    ] {
        runtime.push(event_name, large.clone());
    }
    runtime.push(HookEventName::SessionStart, "small-tail".to_string());
    assert!(runtime.lock_pending().iter().all(|completion| {
        approx_token_count(&completion.text) <= ASYNC_HOOK_COMPLETION_TOKEN_LIMIT
    }));

    let mut completed = 0;
    loop {
        let text = runtime
            .flush_through(runtime.ready_boundary())
            .expect("bounded async output");
        completed += text.matches("<async_hook_output event=").count();
        assert!(approx_token_count(&text) <= ASYNC_HOOK_FLUSH_TOKEN_LIMIT);
        if let Some(tail) = text.find("small-tail") {
            if let Some(large) = text.find("tokens truncated") {
                assert!(large < tail);
            }
            assert_eq!(completed, 4);
            break;
        }
    }
    assert!(runtime.flush_through(runtime.ready_boundary()).is_none());
}

#[tokio::test]
async fn shutdown_cancels_and_joins_detached_tasks() {
    let runtime = AsyncCommandRuntime::default();
    let cancellation = runtime.state.cancellation.clone();
    let stopped = Arc::new(AtomicBool::new(false));
    let task_stopped = Arc::clone(&stopped);
    runtime.state.tasks.spawn(async move {
        cancellation.cancelled().await;
        task_stopped.store(true, Ordering::SeqCst);
    });

    runtime.shutdown().await;

    assert!(stopped.load(Ordering::SeqCst));
}

fn output(event_name: HookEventName, stdout: &str) -> Option<String> {
    deliverable_output(event_name, &result(Some(0), stdout, "", /*error*/ None))
}

fn result(
    exit_code: Option<i32>,
    stdout: &str,
    stderr: &str,
    error: Option<String>,
) -> CommandRunResult {
    CommandRunResult {
        started_at: 1,
        completed_at: 2,
        duration_ms: 1,
        exit_code,
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
        error,
    }
}

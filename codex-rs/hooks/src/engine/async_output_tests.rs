use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_output_truncation::approx_token_count;
use pretty_assertions::assert_eq;

use super::ASYNC_HOOK_COMPLETION_TOKEN_LIMIT;
use super::ASYNC_HOOK_FLUSH_TOKEN_LIMIT;
use super::AsyncCommandRuntime;
use super::deliverable_output;
use crate::engine::CommandShell;
use crate::engine::ConfiguredHandler;
use crate::engine::command_runner::CommandRunResult;
use crate::events::permission_request;
use crate::events::post_tool_use;
use crate::events::pre_tool_use;

#[test]
fn async_output_delivers_only_event_context() {
    let pre = output(
        HookEventName::PreToolUse,
        r#"{"continue":false,"systemMessage":"ignore","decision":"block","reason":"ignore","hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","updatedInput":{"command":"rewrite"},"additionalContext":"pre context"}}"#,
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

#[tokio::test]
async fn async_tool_hooks_return_no_lifecycle_or_control_results() {
    let runtime = AsyncCommandRuntime::default();
    let shell = CommandShell {
        program: String::new(),
        args: Vec::new(),
    };
    let cwd = AbsolutePathBuf::current_dir().expect("current dir");

    let pre_request = pre_tool_use::PreToolUseRequest {
        session_id: ThreadId::new(),
        turn_id: "turn-1".to_string(),
        subagent: None,
        cwd: cwd.clone(),
        transcript_path: None,
        model: "gpt-test".to_string(),
        permission_mode: "default".to_string(),
        tool_name: "Bash".to_string(),
        matcher_aliases: Vec::new(),
        tool_use_id: "tool-1".to_string(),
        tool_input: serde_json::json!({"command": "original"}),
    };
    let pre_handlers = [async_handler(HookEventName::PreToolUse, cwd.clone())];
    assert!(pre_tool_use::preview(&pre_handlers, &pre_request).is_empty());
    let pre = pre_tool_use::run(&pre_handlers, &shell, &runtime, pre_request).await;
    assert_eq!(
        (
            pre.hook_events,
            pre.should_block,
            pre.block_reason,
            pre.additional_contexts,
            pre.updated_input,
        ),
        (Vec::new(), false, None, Vec::new(), None),
    );

    let permission_request = permission_request::PermissionRequestRequest {
        session_id: ThreadId::new(),
        turn_id: "turn-1".to_string(),
        subagent: None,
        cwd: cwd.to_path_buf(),
        transcript_path: None,
        model: "gpt-test".to_string(),
        permission_mode: "default".to_string(),
        tool_name: "Bash".to_string(),
        matcher_aliases: Vec::new(),
        run_id_suffix: "tool-1".to_string(),
        tool_input: serde_json::json!({"command": "original"}),
    };
    let permission_handlers = [async_handler(HookEventName::PermissionRequest, cwd.clone())];
    assert!(permission_request::preview(&permission_handlers, &permission_request).is_empty());
    let permission =
        permission_request::run(&permission_handlers, &shell, &runtime, permission_request).await;
    assert_eq!(
        (permission.hook_events, permission.decision),
        (Vec::new(), None),
    );

    let post_request = post_tool_use::PostToolUseRequest {
        session_id: ThreadId::new(),
        turn_id: "turn-1".to_string(),
        subagent: None,
        cwd: cwd.clone(),
        transcript_path: None,
        model: "gpt-test".to_string(),
        permission_mode: "default".to_string(),
        tool_name: "Bash".to_string(),
        matcher_aliases: Vec::new(),
        tool_use_id: "tool-1".to_string(),
        tool_input: serde_json::json!({"command": "original"}),
        tool_response: serde_json::json!({"ok": true}),
    };
    let post_handlers = [async_handler(HookEventName::PostToolUse, cwd)];
    assert!(post_tool_use::preview(&post_handlers, &post_request).is_empty());
    let post = post_tool_use::run(&post_handlers, &shell, &runtime, post_request).await;
    assert_eq!(
        (
            post.hook_events,
            post.should_stop,
            post.stop_reason,
            post.additional_contexts,
            post.feedback_message,
        ),
        (Vec::new(), false, None, Vec::new(), None),
    );

    runtime.shutdown().await;
}

#[test]
fn queue_preserves_duplicate_completions_until_commit() {
    let runtime = AsyncCommandRuntime::default();
    runtime.push(HookEventName::PreToolUse, "same".to_string());
    runtime.push(HookEventName::PostToolUse, "same".to_string());
    runtime.push(HookEventName::Stop, "last".to_string());

    let batch = runtime.prepare_batch().expect("queued output batch");
    assert_eq!(runtime.prepare_batch(), Some(batch.clone()));
    let text = batch.clone().into_text();
    assert_eq!(text.matches("same").count(), 2);
    assert!(
        ["PreToolUse", "PostToolUse", "last"]
            .map(|needle| text.find(needle).expect("queued output"))
            .is_sorted()
    );
    runtime.commit(&batch);
    assert!(runtime.prepare_batch().is_none());
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
        let batch = runtime.prepare_batch().expect("bounded output batch");
        let text = batch.clone().into_text();
        assert!(approx_token_count(&text) <= ASYNC_HOOK_FLUSH_TOKEN_LIMIT);
        completed += batch.completion_count;
        runtime.commit(&batch);
        if let Some(tail) = text.find("small-tail") {
            if let Some(large) = text.find("tokens truncated") {
                assert!(large < tail);
            }
            assert_eq!(completed, 4);
            break;
        }
    }
    assert!(runtime.prepare_batch().is_none());
}

#[tokio::test]
async fn shutdown_cancels_and_joins_detached_tasks() {
    let runtime = AsyncCommandRuntime::default();
    let cancellation = runtime.state.cancellation.clone();
    let stopped = Arc::new(AtomicBool::new(false));
    let task_stopped = Arc::clone(&stopped);
    runtime.spawn(async move {
        cancellation.cancelled().await;
        task_stopped.store(true, Ordering::SeqCst);
    });

    runtime.shutdown().await;

    assert!(stopped.load(Ordering::SeqCst));
}

fn output(event_name: HookEventName, stdout: &str) -> Option<String> {
    deliverable_output(event_name, &result(Some(0), stdout, "", /*error*/ None))
}

fn async_handler(event_name: HookEventName, source_path: AbsolutePathBuf) -> ConfiguredHandler {
    ConfiguredHandler {
        event_name,
        matcher: Some("^Bash$".to_string()),
        command: "exit 0".to_string(),
        timeout_sec: 5,
        r#async: true,
        status_message: None,
        source_path,
        source: HookSource::User,
        display_order: 0,
        env: std::collections::HashMap::new(),
    }
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

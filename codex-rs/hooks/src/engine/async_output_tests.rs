#[cfg(not(target_os = "windows"))]
use std::fs;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
#[cfg(not(target_os = "windows"))]
use std::time::Duration;

#[cfg(not(target_os = "windows"))]
use codex_protocol::ThreadId;
use codex_protocol::protocol::HookEventName;
#[cfg(not(target_os = "windows"))]
use codex_protocol::protocol::HookSource;
#[cfg(not(target_os = "windows"))]
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_output_truncation::approx_token_count;
use pretty_assertions::assert_eq;
#[cfg(not(target_os = "windows"))]
use tempfile::tempdir;
#[cfg(not(target_os = "windows"))]
use tokio::time::sleep;
#[cfg(not(target_os = "windows"))]
use tokio::time::timeout;

use super::ASYNC_HOOK_COMPLETION_TOKEN_LIMIT;
use super::ASYNC_HOOK_FLUSH_TOKEN_LIMIT;
use super::AsyncHookOutputQueue;
use super::deliverable_output;
#[cfg(not(target_os = "windows"))]
use crate::engine::CommandShell;
#[cfg(not(target_os = "windows"))]
use crate::engine::ConfiguredHandler;
use crate::engine::command_runner::CommandRunResult;
#[cfg(not(target_os = "windows"))]
use crate::events::permission_request;
#[cfg(not(target_os = "windows"))]
use crate::events::post_tool_use;
#[cfg(not(target_os = "windows"))]
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

    assert_eq!(
        (pre.as_deref(), permission, post.as_deref()),
        (Some("pre context"), None, Some("post context"))
    );
}

#[test]
fn async_output_surfaces_parse_and_runtime_failures() {
    let invalid = deliverable_output(
        HookEventName::PreToolUse,
        &result(Some(0), "plain stdout", "", /*error*/ None),
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

#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn async_tool_hooks_do_not_return_control_or_lifecycle() {
    let temp = tempdir().expect("create temp dir");
    let script_path = temp.path().join("async_controls.py");
    fs::write(
        &script_path,
        r#"import json
from pathlib import Path
import sys

name = sys.argv[1]
outputs = {
    "pre_block": {"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny", "permissionDecisionReason": "blocked", "additionalContext": "pre block context"}},
    "pre_rewrite": {"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "allow", "updatedInput": {"command": "rewrite"}, "additionalContext": "pre rewrite context"}},
    "permission": {"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "deny", "message": "denied"}}},
    "post": {"continue": False, "decision": "block", "reason": "stop", "hookSpecificOutput": {"hookEventName": "PostToolUse", "additionalContext": "post context"}},
}
json.load(sys.stdin)
print(json.dumps(outputs[name]))
(Path(__file__).parent / f"{name}.completed").write_text("done", encoding="utf-8")
"#,
    )
    .expect("write async controls script");
    let cwd = AbsolutePathBuf::try_from(temp.path().to_path_buf()).expect("absolute temp dir");
    let queue = AsyncHookOutputQueue::default();
    let shell = CommandShell {
        program: String::new(),
        args: Vec::new(),
    };
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
    let pre_handlers = [
        async_handler(HookEventName::PreToolUse, &script_path, "pre_block"),
        async_handler(HookEventName::PreToolUse, &script_path, "pre_rewrite"),
    ];
    assert!(pre_tool_use::preview(&pre_handlers, &pre_request).is_empty());
    let pre = pre_tool_use::run(&pre_handlers, &shell, &queue, pre_request).await;
    assert_eq!(
        (
            pre.hook_events,
            pre.should_block,
            pre.block_reason,
            pre.additional_contexts,
            pre.updated_input,
        ),
        (Vec::new(), false, None, Vec::new(), None)
    );

    let permission_request = permission_request::PermissionRequestRequest {
        session_id: ThreadId::new(),
        turn_id: "turn-1".to_string(),
        subagent: None,
        cwd: temp.path().to_path_buf(),
        transcript_path: None,
        model: "gpt-test".to_string(),
        permission_mode: "default".to_string(),
        tool_name: "Bash".to_string(),
        matcher_aliases: Vec::new(),
        run_id_suffix: "tool-1".to_string(),
        tool_input: serde_json::json!({"command": "original"}),
    };
    let permission_handlers = [async_handler(
        HookEventName::PermissionRequest,
        &script_path,
        "permission",
    )];
    assert!(permission_request::preview(&permission_handlers, &permission_request).is_empty());
    let permission =
        permission_request::run(&permission_handlers, &shell, &queue, permission_request).await;
    assert_eq!(
        (permission.hook_events, permission.decision),
        (Vec::new(), None)
    );

    let post_request = post_tool_use::PostToolUseRequest {
        session_id: ThreadId::new(),
        turn_id: "turn-1".to_string(),
        subagent: None,
        cwd,
        transcript_path: None,
        model: "gpt-test".to_string(),
        permission_mode: "default".to_string(),
        tool_name: "Bash".to_string(),
        matcher_aliases: Vec::new(),
        tool_use_id: "tool-1".to_string(),
        tool_input: serde_json::json!({"command": "original"}),
        tool_response: serde_json::json!({"ok": true}),
    };
    let post_handlers = [async_handler(
        HookEventName::PostToolUse,
        &script_path,
        "post",
    )];
    assert!(post_tool_use::preview(&post_handlers, &post_request).is_empty());
    let post = post_tool_use::run(&post_handlers, &shell, &queue, post_request).await;
    assert_eq!(
        (
            post.hook_events,
            post.should_stop,
            post.stop_reason,
            post.additional_contexts,
            post.feedback_message,
        ),
        (Vec::new(), false, None, Vec::new(), None)
    );

    timeout(Duration::from_secs(10), async {
        for name in ["pre_block", "pre_rewrite", "permission", "post"] {
            while !temp.path().join(format!("{name}.completed")).exists() {
                sleep(Duration::from_millis(10)).await;
            }
        }
    })
    .await
    .expect("async control hooks complete");
    let delivered = timeout(Duration::from_secs(10), async {
        loop {
            if let Some(batch) = queue.pending_batch() {
                let delivered = batch.into_text();
                if ["pre block context", "pre rewrite context", "post context"]
                    .iter()
                    .all(|context| delivered.contains(context))
                {
                    break delivered;
                }
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("informational async outputs");
    assert!(delivered.contains("pre block context"));
    assert!(delivered.contains("pre rewrite context"));
    assert!(delivered.contains("post context"));
    assert!(!delivered.contains("denied"));
    queue.shutdown().await;
}

#[test]
fn queue_preserves_duplicate_completions_until_commit() {
    let queue = AsyncHookOutputQueue::default();
    queue.push(HookEventName::PreToolUse, "same".to_string());
    queue.push(HookEventName::PostToolUse, "same".to_string());
    queue.push(HookEventName::Stop, "last".to_string());

    let batch = queue.pending_batch().expect("queued output batch");
    assert_eq!(queue.pending_batch(), Some(batch.clone()));
    let text = batch.clone().into_text();
    assert_eq!(text.matches("same").count(), 2);
    assert!(
        ["PreToolUse", "PostToolUse", "last"]
            .map(|needle| text.find(needle).expect("queued output"))
            .is_sorted()
    );
    queue.commit(&batch);
    assert!(queue.pending_batch().is_none());
}

#[test]
fn queue_bounds_items_and_flushes_a_contiguous_prefix() {
    let queue = AsyncHookOutputQueue::default();
    let large = "large-output ".repeat(ASYNC_HOOK_COMPLETION_TOKEN_LIMIT);
    for event_name in [
        HookEventName::PreToolUse,
        HookEventName::PostToolUse,
        HookEventName::Stop,
    ] {
        queue.push(event_name, large.clone());
    }
    queue.push(HookEventName::SessionStart, "small-tail".to_string());
    assert!(queue.lock_pending().iter().all(|completion| {
        approx_token_count(&completion.text) <= ASYNC_HOOK_COMPLETION_TOKEN_LIMIT
    }));

    let mut completed = 0;
    loop {
        let batch = queue.pending_batch().expect("bounded output batch");
        let text = batch.clone().into_text();
        assert!(approx_token_count(&text) <= ASYNC_HOOK_FLUSH_TOKEN_LIMIT);
        completed += batch.completion_count;
        queue.commit(&batch);
        if let Some(tail) = text.find("small-tail") {
            if let Some(large) = text.find("tokens truncated") {
                assert!(large < tail);
            }
            assert_eq!(completed, 4);
            break;
        }
    }
    assert!(queue.pending_batch().is_none());
}

#[tokio::test]
async fn shutdown_cancels_and_joins_detached_tasks() {
    let queue = AsyncHookOutputQueue::default();
    let cancellation = queue.cancellation_token();
    let stopped = Arc::new(AtomicBool::new(false));
    let task_stopped = Arc::clone(&stopped);
    queue.spawn(async move {
        cancellation.cancelled().await;
        task_stopped.store(true, Ordering::SeqCst);
    });

    queue.shutdown().await;

    assert!(stopped.load(Ordering::SeqCst));
}

fn output(event_name: HookEventName, stdout: &str) -> Option<String> {
    deliverable_output(event_name, &result(Some(0), stdout, "", /*error*/ None))
}

#[cfg(not(target_os = "windows"))]
fn async_handler(
    event_name: HookEventName,
    script_path: &std::path::Path,
    name: &str,
) -> ConfiguredHandler {
    ConfiguredHandler {
        event_name,
        matcher: Some("^Bash$".to_string()),
        command: format!("python3 {} {name}", script_path.display()),
        timeout_sec: 5,
        r#async: true,
        status_message: None,
        source_path: AbsolutePathBuf::try_from(script_path.to_path_buf()).expect("absolute script"),
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

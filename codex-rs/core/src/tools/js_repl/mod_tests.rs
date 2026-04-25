use super::*;
use crate::session::tests::make_session_and_context;
use crate::session::tests::make_session_and_context_with_dynamic_tools_and_rx;
use crate::turn_diff_tracker::TurnDiffTracker;
use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::DEFAULT_IMAGE_DETAIL;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::openai_models::InputModality;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SandboxPolicy;
use core_test_support::PathBufExt;
use core_test_support::TempDirExt;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn set_danger_full_access(turn: &mut crate::session::turn_context::TurnContext) {
    turn.sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");
    turn.file_system_sandbox_policy = FileSystemSandboxPolicy::from(turn.sandbox_policy.get());
    turn.network_sandbox_policy = NetworkSandboxPolicy::from(turn.sandbox_policy.get());
}

#[test]
fn node_version_parses_v_prefix_and_suffix() {
    let version = NodeVersion::parse("v25.1.0-nightly.2024").unwrap();
    assert_eq!(
        version,
        NodeVersion {
            major: 25,
            minor: 1,
            patch: 0,
        }
    );
}

#[test]
fn truncate_utf8_prefix_by_bytes_preserves_character_boundaries() {
    let input = "aé🙂z";
    assert_eq!(truncate_utf8_prefix_by_bytes(input, /*max_bytes*/ 0), "");
    assert_eq!(truncate_utf8_prefix_by_bytes(input, /*max_bytes*/ 1), "a");
    assert_eq!(truncate_utf8_prefix_by_bytes(input, /*max_bytes*/ 2), "a");
    assert_eq!(truncate_utf8_prefix_by_bytes(input, /*max_bytes*/ 3), "aé");
    assert_eq!(truncate_utf8_prefix_by_bytes(input, /*max_bytes*/ 6), "aé");
    assert_eq!(
        truncate_utf8_prefix_by_bytes(input, /*max_bytes*/ 7),
        "aé🙂"
    );
    assert_eq!(
        truncate_utf8_prefix_by_bytes(input, /*max_bytes*/ 8),
        "aé🙂z"
    );
}

#[test]
fn stderr_tail_applies_line_and_byte_limits() {
    let mut lines = VecDeque::new();
    let per_line_cap = JS_REPL_STDERR_TAIL_LINE_MAX_BYTES.min(JS_REPL_STDERR_TAIL_MAX_BYTES);
    let long = "x".repeat(per_line_cap + 128);
    let bounded = push_stderr_tail_line(&mut lines, &long);
    assert_eq!(bounded.len(), per_line_cap);

    for i in 0..50 {
        let line = format!("line-{i}-{}", "y".repeat(200));
        push_stderr_tail_line(&mut lines, &line);
    }

    assert!(lines.len() <= JS_REPL_STDERR_TAIL_LINE_LIMIT);
    assert!(lines.iter().all(|line| line.len() <= per_line_cap));
    assert!(stderr_tail_formatted_bytes(&lines) <= JS_REPL_STDERR_TAIL_MAX_BYTES);
    assert_eq!(
        format_stderr_tail(&lines).len(),
        stderr_tail_formatted_bytes(&lines)
    );
}

#[test]
fn model_kernel_failure_details_are_structured_and_truncated() {
    let snapshot = KernelDebugSnapshot {
        pid: Some(42),
        status: "exited(code=1)".to_string(),
        stderr_tail: "s".repeat(JS_REPL_MODEL_DIAG_STDERR_MAX_BYTES + 400),
    };
    let stream_error = "e".repeat(JS_REPL_MODEL_DIAG_ERROR_MAX_BYTES + 200);
    let message = with_model_kernel_failure_message(
        "js_repl kernel exited unexpectedly",
        "stdout_eof",
        Some(&stream_error),
        &snapshot,
    );
    assert!(message.starts_with("js_repl kernel exited unexpectedly\n\njs_repl diagnostics: "));
    let (_prefix, encoded) = message
        .split_once("js_repl diagnostics: ")
        .expect("diagnostics suffix should be present");
    let parsed: serde_json::Value =
        serde_json::from_str(encoded).expect("diagnostics should be valid json");
    assert_eq!(
        parsed.get("reason").and_then(|v| v.as_str()),
        Some("stdout_eof")
    );
    assert_eq!(
        parsed.get("kernel_pid").and_then(serde_json::Value::as_u64),
        Some(42)
    );
    assert_eq!(
        parsed.get("kernel_status").and_then(|v| v.as_str()),
        Some("exited(code=1)")
    );
    assert!(
        parsed
            .get("kernel_stderr_tail")
            .and_then(|v| v.as_str())
            .expect("kernel_stderr_tail should be present")
            .len()
            <= JS_REPL_MODEL_DIAG_STDERR_MAX_BYTES
    );
    assert!(
        parsed
            .get("stream_error")
            .and_then(|v| v.as_str())
            .expect("stream_error should be present")
            .len()
            <= JS_REPL_MODEL_DIAG_ERROR_MAX_BYTES
    );
}

#[test]
fn write_error_diagnostics_only_attach_for_likely_kernel_failures() {
    let running = KernelDebugSnapshot {
        pid: Some(7),
        status: "running".to_string(),
        stderr_tail: "<empty>".to_string(),
    };
    let exited = KernelDebugSnapshot {
        pid: Some(7),
        status: "exited(code=1)".to_string(),
        stderr_tail: "<empty>".to_string(),
    };
    assert!(!should_include_model_diagnostics_for_write_error(
        "failed to flush kernel message: other io error",
        &running
    ));
    assert!(should_include_model_diagnostics_for_write_error(
        "failed to write to kernel: Broken pipe (os error 32)",
        &running
    ));
    assert!(should_include_model_diagnostics_for_write_error(
        "failed to write to kernel: some other io error",
        &exited
    ));
}

#[test]
fn js_repl_internal_tool_guard_matches_expected_names() {
    assert!(is_js_repl_internal_tool("js_repl"));
    assert!(is_js_repl_internal_tool("js_repl_reset"));
    assert!(!is_js_repl_internal_tool("shell_command"));
    assert!(!is_js_repl_internal_tool("list_mcp_resources"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wait_for_exec_tool_calls_map_drains_inflight_calls_without_hanging() {
    let exec_tool_calls = Arc::new(Mutex::new(HashMap::new()));

    for _ in 0..128 {
        let exec_id = Uuid::new_v4().to_string();
        exec_tool_calls
            .lock()
            .await
            .insert(exec_id.clone(), ExecToolCalls::default());
        assert!(
            JsReplManager::begin_exec_tool_call(&exec_tool_calls, &exec_id)
                .await
                .is_some()
        );

        let wait_map = Arc::clone(&exec_tool_calls);
        let wait_exec_id = exec_id.clone();
        let waiter = tokio::spawn(async move {
            JsReplManager::wait_for_exec_tool_calls_map(&wait_map, &wait_exec_id).await;
        });

        let finish_map = Arc::clone(&exec_tool_calls);
        let finish_exec_id = exec_id.clone();
        let finisher = tokio::spawn(async move {
            tokio::task::yield_now().await;
            JsReplManager::finish_exec_tool_call(&finish_map, &finish_exec_id).await;
        });

        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("wait_for_exec_tool_calls_map should not hang")
            .expect("wait task should not panic");
        finisher.await.expect("finish task should not panic");

        JsReplManager::clear_exec_tool_calls_map(&exec_tool_calls, &exec_id).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reset_waits_for_exec_lock_before_clearing_exec_tool_calls() {
    let manager = JsReplManager::new(/*node_path*/ None, Vec::new())
        .await
        .expect("manager should initialize");
    let permit = manager
        .exec_lock
        .clone()
        .acquire_owned()
        .await
        .expect("lock should be acquirable");
    let exec_id = Uuid::new_v4().to_string();
    manager.register_exec_tool_calls(&exec_id).await;

    let reset_manager = Arc::clone(&manager);
    let mut reset_task = tokio::spawn(async move { reset_manager.reset().await });
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(
        !reset_task.is_finished(),
        "reset should wait until execute lock is released"
    );
    assert!(
        manager.exec_tool_calls.lock().await.contains_key(&exec_id),
        "reset must not clear tool-call contexts while execute lock is held"
    );

    drop(permit);

    tokio::time::timeout(Duration::from_secs(1), &mut reset_task)
        .await
        .expect("reset should complete after execute lock release")
        .expect("reset task should not panic")
        .expect("reset should succeed");
    assert!(
        !manager.exec_tool_calls.lock().await.contains_key(&exec_id),
        "reset should clear tool-call contexts after lock acquisition"
    );
}

#[test]
fn summarize_tool_call_response_for_multimodal_function_output() {
    let response = ResponseInputItem::FunctionCallOutput {
        call_id: "call-1".to_string(),
        output: FunctionCallOutputPayload::from_content_items(vec![
            FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,abcd".to_string(),
                detail: Some(DEFAULT_IMAGE_DETAIL),
            },
        ]),
    };

    let actual = JsReplManager::summarize_tool_call_response(&response);

    assert_eq!(
        actual,
        JsReplToolCallResponseSummary {
            response_type: Some("function_call_output".to_string()),
            payload_kind: Some(JsReplToolCallPayloadKind::FunctionContentItems),
            payload_text_preview: None,
            payload_text_length: None,
            payload_item_count: Some(1),
            text_item_count: Some(0),
            image_item_count: Some(1),
            structured_content_present: None,
            result_is_error: None,
        }
    );
}

#[tokio::test]
async fn emitted_image_content_item_preserves_explicit_non_original_detail() {
    let (_session, turn) = make_session_and_context().await;
    let content_item = emitted_image_content_item(
        &turn,
        "data:image/png;base64,AAA".to_string(),
        Some(ImageDetail::Low),
    );
    assert_eq!(
        content_item,
        FunctionCallOutputContentItem::InputImage {
            image_url: "data:image/png;base64,AAA".to_string(),
            detail: Some(ImageDetail::Low),
        }
    );
}

#[tokio::test]
async fn emitted_image_content_item_allows_explicit_original_detail_when_supported() {
    let (_session, mut turn) = make_session_and_context().await;
    turn.model_info.supports_image_detail_original = true;

    let content_item = emitted_image_content_item(
        &turn,
        "data:image/png;base64,AAA".to_string(),
        Some(ImageDetail::Original),
    );

    assert_eq!(
        content_item,
        FunctionCallOutputContentItem::InputImage {
            image_url: "data:image/png;base64,AAA".to_string(),
            detail: Some(ImageDetail::Original),
        }
    );
}

#[tokio::test]
async fn emitted_image_content_item_defaults_to_high_for_unsupported_original_detail() {
    let (_session, turn) = make_session_and_context().await;

    let content_item = emitted_image_content_item(
        &turn,
        "data:image/png;base64,AAA".to_string(),
        Some(ImageDetail::Original),
    );

    assert_eq!(
        content_item,
        FunctionCallOutputContentItem::InputImage {
            image_url: "data:image/png;base64,AAA".to_string(),
            detail: Some(DEFAULT_IMAGE_DETAIL),
        }
    );
}

#[test]
fn validate_emitted_image_url_accepts_case_insensitive_data_scheme() {
    assert_eq!(
        validate_emitted_image_url("DATA:image/png;base64,AAA"),
        Ok(())
    );
}

#[test]
fn validate_emitted_image_url_rejects_non_data_scheme() {
    assert_eq!(
        validate_emitted_image_url("https://example.com/image.png"),
        Err("codex.emitImage only accepts data URLs".to_string())
    );
}

#[test]
fn summarize_tool_call_response_for_multimodal_custom_output() {
    let response = ResponseInputItem::CustomToolCallOutput {
        call_id: "call-1".to_string(),
        name: None,
        output: FunctionCallOutputPayload::from_content_items(vec![
            FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,abcd".to_string(),
                detail: Some(DEFAULT_IMAGE_DETAIL),
            },
        ]),
    };

    let actual = JsReplManager::summarize_tool_call_response(&response);

    assert_eq!(
        actual,
        JsReplToolCallResponseSummary {
            response_type: Some("custom_tool_call_output".to_string()),
            payload_kind: Some(JsReplToolCallPayloadKind::CustomContentItems),
            payload_text_preview: None,
            payload_text_length: None,
            payload_item_count: Some(1),
            text_item_count: Some(0),
            image_item_count: Some(1),
            structured_content_present: None,
            result_is_error: None,
        }
    );
}

#[test]
fn summarize_tool_call_error_marks_error_payload() {
    let actual = JsReplManager::summarize_tool_call_error("tool failed");

    assert_eq!(
        actual,
        JsReplToolCallResponseSummary {
            response_type: None,
            payload_kind: Some(JsReplToolCallPayloadKind::Error),
            payload_text_preview: Some("tool failed".to_string()),
            payload_text_length: Some("tool failed".len()),
            payload_item_count: None,
            text_item_count: None,
            image_item_count: None,
            structured_content_present: None,
            result_is_error: None,
        }
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reset_clears_inflight_exec_tool_calls_without_waiting() {
    let manager = JsReplManager::new(/*node_path*/ None, Vec::new())
        .await
        .expect("manager should initialize");
    let exec_id = Uuid::new_v4().to_string();
    manager.register_exec_tool_calls(&exec_id).await;
    assert!(
        JsReplManager::begin_exec_tool_call(&manager.exec_tool_calls, &exec_id)
            .await
            .is_some()
    );

    let wait_manager = Arc::clone(&manager);
    let wait_exec_id = exec_id.clone();
    let waiter = tokio::spawn(async move {
        wait_manager.wait_for_exec_tool_calls(&wait_exec_id).await;
    });
    tokio::task::yield_now().await;

    tokio::time::timeout(Duration::from_secs(1), manager.reset())
        .await
        .expect("reset should not hang")
        .expect("reset should succeed");

    tokio::time::timeout(Duration::from_secs(1), waiter)
        .await
        .expect("waiter should be released")
        .expect("wait task should not panic");

    assert!(manager.exec_tool_calls.lock().await.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reset_aborts_inflight_exec_tool_tasks() {
    let manager = JsReplManager::new(/*node_path*/ None, Vec::new())
        .await
        .expect("manager should initialize");
    let exec_id = Uuid::new_v4().to_string();
    manager.register_exec_tool_calls(&exec_id).await;
    let reset_cancel = JsReplManager::begin_exec_tool_call(&manager.exec_tool_calls, &exec_id)
        .await
        .expect("exec should be registered");

    let task = tokio::spawn(async move {
        tokio::select! {
            _ = reset_cancel.cancelled() => "cancelled",
            _ = tokio::time::sleep(Duration::from_secs(60)) => "timed_out",
        }
    });

    tokio::time::timeout(Duration::from_secs(1), manager.reset())
        .await
        .expect("reset should not hang")
        .expect("reset should succeed");

    let outcome = tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .expect("cancelled task should resolve promptly")
        .expect("task should not panic");
    assert_eq!(outcome, "cancelled");
}

async fn can_run_js_repl_runtime_tests() -> bool {
    // These white-box runtime tests are required on macOS. Linux relies on
    // the codex-linux-sandbox arg0 dispatch path, which is exercised in
    // integration tests instead.
    cfg!(target_os = "macos")
}
fn write_js_repl_test_package_source(base: &Path, name: &str, source: &str) -> anyhow::Result<()> {
    let pkg_dir = base.join("node_modules").join(name);
    fs::create_dir_all(&pkg_dir)?;
    fs::write(
        pkg_dir.join("package.json"),
        format!(
            "{{\n  \"name\": \"{name}\",\n  \"version\": \"1.0.0\",\n  \"type\": \"module\",\n  \"exports\": {{\n    \"import\": \"./index.js\"\n  }}\n}}\n"
        ),
    )?;
    fs::write(pkg_dir.join("index.js"), source)?;
    Ok(())
}

fn write_js_repl_test_package(base: &Path, name: &str, value: &str) -> anyhow::Result<()> {
    write_js_repl_test_package_source(base, name, &format!("export const value = \"{value}\";\n"))?;
    Ok(())
}

fn write_js_repl_test_module(base: &Path, relative: &str, contents: &str) -> anyhow::Result<()> {
    let module_path = base.join(relative);
    if let Some(parent) = module_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(module_path, contents)?;
    Ok(())
}

#[tokio::test]
async fn js_repl_timeout_does_not_deadlock() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let result = tokio::time::timeout(
        Duration::from_secs(3),
        manager.execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "while (true) {}".to_string(),
                timeout_ms: Some(50),
            },
        ),
    )
    .await
    .expect("execute should return, not deadlock")
    .expect_err("expected timeout error");

    assert_eq!(
        result.to_string(),
        "js_repl execution timed out; kernel reset, rerun your request"
    );
    Ok(())
}

#[tokio::test]
async fn js_repl_timeout_kills_kernel_process() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            JsReplArgs {
                code: "console.log('warmup');".to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await?;

    let child = {
        let guard = manager.kernel.lock().await;
        let state = guard.as_ref().expect("kernel should exist after warmup");
        Arc::clone(&state.child)
    };

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "while (true) {}".to_string(),
                timeout_ms: Some(50),
            },
        )
        .await
        .expect_err("expected timeout error");

    assert_eq!(
        result.to_string(),
        "js_repl execution timed out; kernel reset, rerun your request"
    );

    let exit_state = {
        let mut child = child.lock().await;
        child.try_wait()?
    };
    assert!(
        exit_state.is_some(),
        "timed out js_repl execution should kill previous kernel process"
    );
    Ok(())
}

#[tokio::test]
async fn interrupt_turn_exec_clears_matching_submitted_exec() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let manager = JsReplManager::new(/*node_path*/ None, Vec::new())
        .await
        .expect("manager should initialize");
    let (_session, turn) = make_session_and_context().await;
    let turn = Arc::new(turn);
    let dependency_env = HashMap::new();
    let mut state = manager
        .start_kernel(Arc::clone(&turn), &dependency_env, /*thread_id*/ None)
        .await
        .map_err(anyhow::Error::msg)?;
    let child = Arc::clone(&state.child);
    state.top_level_exec_state = TopLevelExecState::Submitted {
        turn_id: turn.sub_id.clone(),
        exec_id: "exec-1".to_string(),
    };
    *manager.kernel.lock().await = Some(state);
    manager.register_exec_tool_calls("exec-1").await;

    assert!(manager.interrupt_turn_exec(&turn.sub_id).await?);
    assert!(manager.kernel.lock().await.is_none());
    assert!(manager.exec_tool_calls.lock().await.is_empty());

    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let exited = {
                let mut child = child.lock().await;
                child.try_wait()?.is_some()
            };
            if exited {
                return Ok::<(), anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("kernel should exit after interrupt cleanup")?;

    Ok(())
}

#[tokio::test]
async fn interrupt_turn_exec_resets_matching_pending_kernel_start() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let manager = JsReplManager::new(/*node_path*/ None, Vec::new())
        .await
        .expect("manager should initialize");
    let (_session, turn) = make_session_and_context().await;
    let turn = Arc::new(turn);
    let dependency_env = HashMap::new();
    let mut state = manager
        .start_kernel(Arc::clone(&turn), &dependency_env, /*thread_id*/ None)
        .await
        .map_err(anyhow::Error::msg)?;
    state.top_level_exec_state = TopLevelExecState::FreshKernel {
        turn_id: turn.sub_id.clone(),
        exec_id: None,
    };
    let child = Arc::clone(&state.child);
    *manager.kernel.lock().await = Some(state);

    assert!(manager.interrupt_turn_exec(&turn.sub_id).await?);
    assert!(manager.kernel.lock().await.is_none());

    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let exited = {
                let mut child = child.lock().await;
                child.try_wait()?.is_some()
            };
            if exited {
                return Ok::<(), anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("kernel should exit after interrupt cleanup")?;

    Ok(())
}

#[tokio::test]
async fn interrupt_turn_exec_does_not_reset_reused_kernel_before_submit() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let manager = JsReplManager::new(/*node_path*/ None, Vec::new())
        .await
        .expect("manager should initialize");
    let (_session, turn) = make_session_and_context().await;
    let turn = Arc::new(turn);
    let dependency_env = HashMap::new();
    let mut state = manager
        .start_kernel(Arc::clone(&turn), &dependency_env, /*thread_id*/ None)
        .await
        .map_err(anyhow::Error::msg)?;
    state.top_level_exec_state = TopLevelExecState::ReusedKernelPending {
        turn_id: turn.sub_id.clone(),
        exec_id: "exec-1".to_string(),
    };
    *manager.kernel.lock().await = Some(state);

    assert!(!manager.interrupt_turn_exec(&turn.sub_id).await?);
    assert!(manager.kernel.lock().await.is_some());

    manager.reset().await.map_err(anyhow::Error::msg)
}

#[tokio::test]
async fn interrupt_active_exec_stops_aborted_kernel_before_later_exec() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let dir = tempdir()?;
    let (session, mut turn) = make_session_and_context().await;
    turn.cwd = dir.abs();
    set_danger_full_access(&mut turn);
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let first_file = dir.path().join("1.txt");
    let second_file = dir.path().join("2.txt");
    let first_file_js = serde_json::to_string(&first_file.to_string_lossy().to_string())?;
    let second_file_js = serde_json::to_string(&second_file.to_string_lossy().to_string())?;
    let code = format!(
        r#"
const {{ promises: fs }} = await import("fs");

const paths = [{first_file_js}, {second_file_js}];
for (let i = 0; i < paths.length; i++) {{
  await fs.writeFile(paths[i], `${{i + 1}}`);
  if (i + 1 < paths.length) {{
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }}
}}
"#
    );

    let handle = tokio::spawn({
        let manager = Arc::clone(&manager);
        let session = Arc::clone(&session);
        let turn = Arc::clone(&turn);
        let tracker = Arc::clone(&tracker);
        async move {
            manager
                .execute(
                    session,
                    turn,
                    tracker,
                    JsReplArgs {
                        code,
                        timeout_ms: Some(15_000),
                    },
                )
                .await
        }
    });

    tokio::time::timeout(Duration::from_secs(3), async {
        while !first_file.exists() {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("first file should be written before interrupt");

    let child = {
        let guard = manager.kernel.lock().await;
        let state = guard
            .as_ref()
            .expect("kernel should exist while exec is running");
        Arc::clone(&state.child)
    };

    handle.abort();
    assert!(manager.interrupt_turn_exec(&turn.sub_id).await?);

    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let exited = {
                let mut child = child.lock().await;
                child.try_wait()?.is_some()
            };
            if exited {
                return Ok::<(), anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("kernel should exit after interrupt")?;

    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(first_file.exists());
    assert!(!second_file.exists());

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "console.log('after interrupt');".to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await?;
    assert!(result.output.contains("after interrupt"));

    Ok(())
}

#[tokio::test]
async fn js_repl_forced_kernel_exit_recovers_on_next_exec() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            JsReplArgs {
                code: "console.log('warmup');".to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await?;

    let child = {
        let guard = manager.kernel.lock().await;
        let state = guard.as_ref().expect("kernel should exist after warmup");
        Arc::clone(&state.child)
    };
    JsReplManager::kill_kernel_child(&child, "test_crash").await;
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let cleared = {
                let guard = manager.kernel.lock().await;
                guard
                    .as_ref()
                    .is_none_or(|state| !Arc::ptr_eq(&state.child, &child))
            };
            if cleared {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("host should clear dead kernel state promptly");

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "console.log('after-kill');".to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await?;
    assert!(result.output.contains("after-kill"));
    Ok(())
}

#[tokio::test]
async fn js_repl_uncaught_exception_returns_exec_error_and_recovers() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = crate::session::tests::make_session_and_context().await;
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            JsReplArgs {
                code: "console.log('warmup');".to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await?;

    let child = {
        let guard = manager.kernel.lock().await;
        let state = guard.as_ref().expect("kernel should exist after warmup");
        Arc::clone(&state.child)
    };

    let err = tokio::time::timeout(
            Duration::from_secs(3),
            manager.execute(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::clone(&tracker),
                JsReplArgs {
                    code: "setTimeout(() => { throw new Error('boom'); }, 0);\nawait new Promise(() => {});".to_string(),
                    timeout_ms: Some(10_000),
                },
            ),
        )
        .await
        .expect("uncaught exception should fail promptly")
        .expect_err("expected uncaught exception to fail the exec");

    let message = err.to_string();
    assert!(message.contains("js_repl kernel uncaught exception: boom"));
    assert!(message.contains("kernel reset."));
    assert!(message.contains("Catch or handle async errors"));
    assert!(!message.contains("js_repl kernel exited unexpectedly"));

    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let exited = {
                let mut child = child.lock().await;
                child.try_wait()?.is_some()
            };
            if exited {
                return Ok::<(), anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("uncaught exception should terminate the previous kernel process")?;

    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let cleared = {
                let guard = manager.kernel.lock().await;
                guard
                    .as_ref()
                    .is_none_or(|state| !Arc::ptr_eq(&state.child, &child))
            };
            if cleared {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("host should clear dead kernel state promptly");

    let next = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "console.log('after reset');".to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await?;
    assert!(next.output.contains("after reset"));
    Ok(())
}

#[tokio::test]
async fn js_repl_waits_for_unawaited_tool_calls_before_completion() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    set_danger_full_access(&mut turn);

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let marker = turn
        .cwd
        .join(format!("js-repl-unawaited-marker-{}.txt", Uuid::new_v4()));
    let marker_json = serde_json::to_string(&marker.to_string_lossy().to_string())?;
    let result = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: format!(
                        r#"
const marker = {marker_json};
void codex.tool("shell_command", {{ command: `sleep 0.35; printf js_repl_unawaited_done > "${{marker}}"` }});
console.log("cell-complete");
"#
                    ),
                    timeout_ms: Some(10_000),
                },
            )
            .await?;
    assert!(result.output.contains("cell-complete"));
    let marker_contents = tokio::fs::read_to_string(&marker).await?;
    assert_eq!(marker_contents, "js_repl_unawaited_done");
    let _ = tokio::fs::remove_file(&marker).await;
    Ok(())
}

#[tokio::test]
async fn js_repl_persisted_tool_helpers_work_across_cells() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    set_danger_full_access(&mut turn);

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let global_marker = turn
        .cwd
        .join(format!("js-repl-global-helper-{}.txt", Uuid::new_v4()));
    let lexical_marker = turn
        .cwd
        .join(format!("js-repl-lexical-helper-{}.txt", Uuid::new_v4()));
    let global_marker_json = serde_json::to_string(&global_marker.to_string_lossy().to_string())?;
    let lexical_marker_json = serde_json::to_string(&lexical_marker.to_string_lossy().to_string())?;

    manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            JsReplArgs {
                code: format!(
                    r#"
const globalMarker = {global_marker_json};
const lexicalMarker = {lexical_marker_json};
const savedTool = codex.tool;
globalThis.globalToolHelper = {{
  run: () => savedTool("shell_command", {{ command: `printf global_helper > "${{globalMarker}}"` }}),
}};
const lexicalToolHelper = {{
  run: () => savedTool("shell_command", {{ command: `printf lexical_helper > "${{lexicalMarker}}"` }}),
}};
"#
                ),
                timeout_ms: Some(10_000),
            },
        )
        .await?;

    let next = manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            JsReplArgs {
                code: r#"
await globalToolHelper.run();
await lexicalToolHelper.run();
console.log("helpers-ran");
"#
                .to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await?;

    assert!(next.output.contains("helpers-ran"));
    assert_eq!(
        tokio::fs::read_to_string(&global_marker).await?,
        "global_helper"
    );
    assert_eq!(
        tokio::fs::read_to_string(&lexical_marker).await?,
        "lexical_helper"
    );
    let _ = tokio::fs::remove_file(&global_marker).await;
    let _ = tokio::fs::remove_file(&lexical_marker).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_does_not_auto_attach_image_via_view_image_tool() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    set_danger_full_access(&mut turn);

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
const fs = await import("node:fs/promises");
const path = await import("node:path");
const imagePath = path.join(codex.tmpDir, "js-repl-view-image.png");
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await fs.writeFile(imagePath, png);
const out = await codex.tool("view_image", { path: imagePath });
console.log(out.type);
"#;

    let result = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
            },
        )
        .await?;
    assert!(result.output.contains("function_call_output"));
    assert!(result.content_items.is_empty());
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_can_emit_image_via_view_image_tool() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, mut turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }
    turn.approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    set_danger_full_access(&mut turn);

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
const fs = await import("node:fs/promises");
const path = await import("node:path");
const imagePath = path.join(codex.tmpDir, "js-repl-view-image-explicit.png");
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await fs.writeFile(imagePath, png);
const out = await codex.tool("view_image", { path: imagePath });
await codex.emitImage(out);
console.log(out.type);
"#;

    let result = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
            },
        )
        .await?;
    assert!(result.output.contains("function_call_output"));
    assert_eq!(
            result.content_items.as_slice(),
            [FunctionCallOutputContentItem::InputImage {
                image_url:
                    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
                        .to_string(),
                detail: Some(DEFAULT_IMAGE_DETAIL),
            }]
            .as_slice()
        );
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_can_emit_image_from_bytes_and_mime_type() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await codex.emitImage({ bytes: png, mimeType: "image/png" });
"#;

    let result = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
            },
        )
        .await?;
    assert_eq!(
            result.content_items.as_slice(),
            [FunctionCallOutputContentItem::InputImage {
                image_url:
                    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
                        .to_string(),
                detail: Some(DEFAULT_IMAGE_DETAIL),
            }]
            .as_slice()
        );
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_can_emit_multiple_images_in_one_cell() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
await codex.emitImage(
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
);
await codex.emitImage(
  "data:image/gif;base64,R0lGODdhAQABAIAAAP///////ywAAAAAAQABAAACAkQBADs="
);
"#;

    let result = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
            },
        )
        .await?;
    assert_eq!(
            result.content_items.as_slice(),
            [
                FunctionCallOutputContentItem::InputImage {
                    image_url:
                        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
                            .to_string(),
                    detail: Some(DEFAULT_IMAGE_DETAIL),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url:
                        "data:image/gif;base64,R0lGODdhAQABAIAAAP///////ywAAAAAAQABAAACAkQBADs="
                            .to_string(),
                    detail: Some(DEFAULT_IMAGE_DETAIL),
                },
            ]
            .as_slice()
        );
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_waits_for_unawaited_emit_image_before_completion() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
void codex.emitImage(
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
);
console.log("cell-complete");
"#;

    let result = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
            },
        )
        .await?;
    assert!(result.output.contains("cell-complete"));
    assert_eq!(
            result.content_items.as_slice(),
            [FunctionCallOutputContentItem::InputImage {
                image_url:
                    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
                        .to_string(),
                detail: Some(DEFAULT_IMAGE_DETAIL),
            }]
            .as_slice()
        );
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_persisted_emit_image_helpers_work_across_cells() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let data_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

    manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            JsReplArgs {
                code: format!(
                    r#"
const dataUrl = "{data_url}";
const savedEmitImage = codex.emitImage;
globalThis.globalEmitHelper = {{
  run: () => savedEmitImage(dataUrl),
}};
const lexicalEmitHelper = {{
  run: () => savedEmitImage(dataUrl),
}};
"#
                ),
                timeout_ms: Some(15_000),
            },
        )
        .await?;

    let next = manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            JsReplArgs {
                code: r#"
await globalEmitHelper.run();
await lexicalEmitHelper.run();
console.log("helpers-ran");
"#
                .to_string(),
                timeout_ms: Some(15_000),
            },
        )
        .await?;

    assert!(next.output.contains("helpers-ran"));
    assert_eq!(
        next.content_items,
        vec![
            FunctionCallOutputContentItem::InputImage {
                image_url: data_url.to_string(),
                detail: Some(DEFAULT_IMAGE_DETAIL),
            },
            FunctionCallOutputContentItem::InputImage {
                image_url: data_url.to_string(),
                detail: Some(DEFAULT_IMAGE_DETAIL),
            },
        ]
    );
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_unawaited_emit_image_errors_fail_cell() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
void codex.emitImage({ bytes: new Uint8Array(), mimeType: "image/png" });
console.log("cell-complete");
"#;

    let err = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
            },
        )
        .await
        .expect_err("unawaited invalid emitImage should fail");
    assert!(err.to_string().contains("expected non-empty bytes"));
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_caught_emit_image_error_does_not_fail_cell() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
try {
  await codex.emitImage({ bytes: new Uint8Array(), mimeType: "image/png" });
} catch (error) {
  console.log(error.message);
}
console.log("cell-complete");
"#;

    let result = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
            },
        )
        .await?;
    assert!(result.output.contains("expected non-empty bytes"));
    assert!(result.output.contains("cell-complete"));
    assert!(result.content_items.is_empty());
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_emit_image_requires_explicit_mime_type_for_bytes() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await codex.emitImage({ bytes: png });
"#;

    let err = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
            },
        )
        .await
        .expect_err("missing mimeType should fail");
    assert!(err.to_string().contains("expected a non-empty mimeType"));
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_emit_image_rejects_unsupported_byte_mime_type() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
await codex.emitImage({
  bytes: Buffer.from([255, 0, 0, 255]),
  mimeType: "image/rgba",
});
"#;

    let err = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
            },
        )
        .await
        .expect_err("unsupported byte MIME type should fail");
    assert!(
        err.to_string()
            .contains("only supports image/png, image/jpeg, image/webp, or image/gif")
    );
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_emit_image_rejects_non_data_url() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
await codex.emitImage("https://example.com/image.png");
"#;

    let err = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
            },
        )
        .await
        .expect_err("non-data URLs should fail");
    assert!(err.to_string().contains("only accepts data URLs"));
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[test]
fn validate_emitted_image_url_rejects_unsupported_mime_type() {
    assert_eq!(
        validate_emitted_image_url("data:image/rgba;base64,AAAA").expect_err("unsupported MIME"),
        "codex.emitImage only supports image/png, image/jpeg, image/webp, or image/gif"
    );
}

#[test]
fn validate_emitted_image_url_accepts_supported_mime_type_case_insensitive() {
    assert!(validate_emitted_image_url("DATA:image/PNG;base64,AAAA").is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_emit_image_accepts_case_insensitive_data_url() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
await codex.emitImage("DATA:image/png;base64,AAA");
"#;

    let result = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
            },
        )
        .await?;
    assert_eq!(
        result.content_items.as_slice(),
        [FunctionCallOutputContentItem::InputImage {
            image_url: "DATA:image/png;base64,AAA".to_string(),
            detail: Some(DEFAULT_IMAGE_DETAIL),
        }]
        .as_slice()
    );
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_emit_image_rejects_invalid_detail() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await codex.emitImage({ bytes: png, mimeType: "image/png", detail: "ultra" });
"#;

    let err = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
            },
        )
        .await
        .expect_err("invalid detail should fail");
    assert!(err.to_string().contains("expected detail to be one of"));
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_emit_image_treats_null_detail_as_omitted() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await codex.emitImage({ bytes: png, mimeType: "image/png", detail: null });
"#;

    let result = manager
        .execute(
            Arc::clone(&session),
            turn,
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
            },
        )
        .await?;
    assert_eq!(
            result.content_items.as_slice(),
            [FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==".to_string(),
                detail: Some(DEFAULT_IMAGE_DETAIL),
            }]
            .as_slice()
        );
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_emit_image_rejects_mixed_content() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn, rx_event) =
        make_session_and_context_with_dynamic_tools_and_rx(vec![DynamicToolSpec {
            namespace: None,
            name: "inline_image".to_string(),
            description: "Returns inline text and image content.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            defer_loading: false,
        }])
        .await;
    if !turn
        .model_info
        .input_modalities
        .contains(&InputModality::Image)
    {
        return Ok(());
    }

    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
const out = await codex.tool("inline_image", {});
await codex.emitImage(out);
"#;
    let image_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

    let session_for_response = Arc::clone(&session);
    let response_watcher = async move {
        loop {
            let event = tokio::time::timeout(Duration::from_secs(2), rx_event.recv()).await??;
            if let EventMsg::DynamicToolCallRequest(request) = event.msg {
                session_for_response
                    .notify_dynamic_tool_response(
                        &request.call_id,
                        DynamicToolResponse {
                            content_items: vec![
                                DynamicToolCallOutputContentItem::InputText {
                                    text: "inline image note".to_string(),
                                },
                                DynamicToolCallOutputContentItem::InputImage {
                                    image_url: image_url.to_string(),
                                },
                            ],
                            success: true,
                        },
                    )
                    .await;
                return Ok::<(), anyhow::Error>(());
            }
        }
    };

    let (result, response_watcher_result) = tokio::join!(
        manager.execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
            },
        ),
        response_watcher,
    );
    response_watcher_result?;
    let err = result.expect_err("mixed content should fail");
    assert!(
        err.to_string()
            .contains("does not accept mixed text and image content")
    );
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_dynamic_tool_response_preserves_js_line_separator_text() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    for (tool_name, description, expected_text, literal) in [
        (
            "line_separator_tool",
            "Returns text containing U+2028.",
            "alpha\u{2028}omega".to_string(),
            r#""alpha\u2028omega""#,
        ),
        (
            "paragraph_separator_tool",
            "Returns text containing U+2029.",
            "alpha\u{2029}omega".to_string(),
            r#""alpha\u2029omega""#,
        ),
    ] {
        let (session, turn, rx_event) =
            make_session_and_context_with_dynamic_tools_and_rx(vec![DynamicToolSpec {
                namespace: None,
                name: tool_name.to_string(),
                description: description.to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
                defer_loading: false,
            }])
            .await;

        *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;
        let code = format!(
            r#"
const out = await codex.tool("{tool_name}", {{}});
const text = typeof out === "string" ? out : out?.output;
console.log(text === {literal});
console.log(text);
"#
        );

        let session_for_response = Arc::clone(&session);
        let expected_text_for_response = expected_text.clone();
        let response_watcher = async move {
            loop {
                let event = tokio::time::timeout(Duration::from_secs(2), rx_event.recv()).await??;
                if let EventMsg::DynamicToolCallRequest(request) = event.msg {
                    session_for_response
                        .notify_dynamic_tool_response(
                            &request.call_id,
                            DynamicToolResponse {
                                content_items: vec![DynamicToolCallOutputContentItem::InputText {
                                    text: expected_text_for_response.clone(),
                                }],
                                success: true,
                            },
                        )
                        .await;
                    return Ok::<(), anyhow::Error>(());
                }
            }
        };

        let (result, response_watcher_result) = tokio::join!(
            manager.execute(
                Arc::clone(&session),
                Arc::clone(&turn),
                tracker,
                JsReplArgs {
                    code,
                    timeout_ms: Some(15_000),
                },
            ),
            response_watcher,
        );
        response_watcher_result?;

        let result = result?;
        assert_eq!(result.output, format!("true\n{expected_text}"));
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_can_call_hidden_dynamic_tools() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn, rx_event) =
        make_session_and_context_with_dynamic_tools_and_rx(vec![DynamicToolSpec {
            namespace: Some("codex_app".to_string()),
            name: "hidden_dynamic_tool".to_string(),
            description: "A hidden dynamic tool.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "city": { "type": "string" }
                },
                "required": ["city"],
                "additionalProperties": false
            }),
            defer_loading: true,
        }])
        .await;

    *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;
    let code = r#"
const out = await codex.tool("codex_app_hidden_dynamic_tool", { city: "Paris" });
console.log(JSON.stringify(out));
"#;

    let session_for_response = Arc::clone(&session);
    let response_watcher = async move {
        loop {
            let event = tokio::time::timeout(Duration::from_secs(2), rx_event.recv()).await??;
            if let EventMsg::DynamicToolCallRequest(request) = event.msg {
                session_for_response
                    .notify_dynamic_tool_response(
                        &request.call_id,
                        DynamicToolResponse {
                            content_items: vec![DynamicToolCallOutputContentItem::InputText {
                                text: "hidden-ok".to_string(),
                            }],
                            success: true,
                        },
                    )
                    .await;
                return Ok::<(), anyhow::Error>(());
            }
        }
    };

    let (result, response_watcher_result) = tokio::join!(
        manager.execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            tracker,
            JsReplArgs {
                code: code.to_string(),
                timeout_ms: Some(15_000),
            },
        ),
        response_watcher,
    );

    let result = result?;
    response_watcher_result?;
    assert!(result.output.contains("hidden-ok"));
    assert!(session.get_pending_input().await.is_empty());

    Ok(())
}

#[tokio::test]
async fn js_repl_prefers_env_node_module_dirs_over_config() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let env_base = tempdir()?;
    write_js_repl_test_package(env_base.path(), "repl_probe", "env")?;

    let config_base = tempdir()?;
    let cwd_dir = tempdir()?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy.r#set.insert(
        "CODEX_JS_REPL_NODE_MODULE_DIRS".to_string(),
        env_base.path().to_string_lossy().to_string(),
    );
    turn.cwd = cwd_dir.abs();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        vec![config_base.path().to_path_buf()],
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "const mod = await import(\"repl_probe\"); console.log(mod.value);"
                    .to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await?;
    assert!(result.output.contains("env"));
    Ok(())
}

#[tokio::test]
async fn js_repl_resolves_from_first_config_dir() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let first_base = tempdir()?;
    let second_base = tempdir()?;
    write_js_repl_test_package(first_base.path(), "repl_probe", "first")?;
    write_js_repl_test_package(second_base.path(), "repl_probe", "second")?;

    let cwd_dir = tempdir()?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.abs();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        vec![
            first_base.path().to_path_buf(),
            second_base.path().to_path_buf(),
        ],
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "const mod = await import(\"repl_probe\"); console.log(mod.value);"
                    .to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await?;
    assert!(result.output.contains("first"));
    Ok(())
}

#[tokio::test]
async fn js_repl_falls_back_to_cwd_node_modules() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let config_base = tempdir()?;
    let cwd_dir = tempdir()?;
    write_js_repl_test_package(cwd_dir.path(), "repl_probe", "cwd")?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.abs();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        vec![config_base.path().to_path_buf()],
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "const mod = await import(\"repl_probe\"); console.log(mod.value);"
                    .to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await?;
    assert!(result.output.contains("cwd"));
    Ok(())
}

#[tokio::test]
async fn js_repl_accepts_node_modules_dir_entries() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let base_dir = tempdir()?;
    let cwd_dir = tempdir()?;
    write_js_repl_test_package(base_dir.path(), "repl_probe", "normalized")?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.abs();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        vec![base_dir.path().join("node_modules")],
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "const mod = await import(\"repl_probe\"); console.log(mod.value);"
                    .to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await?;
    assert!(result.output.contains("normalized"));
    Ok(())
}

#[tokio::test]
async fn js_repl_supports_relative_file_imports() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let cwd_dir = tempdir()?;
    write_js_repl_test_module(
        cwd_dir.path(),
        "child.js",
        "export const value = \"child\";\n",
    )?;
    write_js_repl_test_module(
        cwd_dir.path(),
        "parent.js",
        "import { value as childValue } from \"./child.js\";\nexport const value = `${childValue}-parent`;\n",
    )?;
    write_js_repl_test_module(
        cwd_dir.path(),
        "local.mjs",
        "export const value = \"mjs\";\n",
    )?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.abs();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let result = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "const parent = await import(\"./parent.js\"); const other = await import(\"./local.mjs\"); console.log(parent.value); console.log(other.value);".to_string(),
                    timeout_ms: Some(10_000),
                },
            )
            .await?;
    assert!(result.output.contains("child-parent"));
    assert!(result.output.contains("mjs"));
    Ok(())
}

#[tokio::test]
async fn js_repl_supports_absolute_file_imports() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let module_dir = tempdir()?;
    let cwd_dir = tempdir()?;
    write_js_repl_test_module(
        module_dir.path(),
        "absolute.js",
        "export const value = \"absolute\";\n",
    )?;
    let absolute_path_json =
        serde_json::to_string(&module_dir.path().join("absolute.js").display().to_string())?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.abs();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: format!(
                    "const mod = await import({absolute_path_json}); console.log(mod.value);"
                ),
                timeout_ms: Some(10_000),
            },
        )
        .await?;
    assert!(result.output.contains("absolute"));
    Ok(())
}

#[tokio::test]
async fn js_repl_imported_local_files_can_access_repl_globals() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let cwd_dir = tempdir()?;
    let expected_home_dir = serde_json::to_string("/tmp/codex-home")?;
    write_js_repl_test_module(
        cwd_dir.path(),
        "globals.js",
        &format!(
            "const expectedHomeDir = {expected_home_dir};\nconsole.log(`tmp:${{codex.tmpDir === tmpDir}}`);\nconsole.log(`cwd:${{typeof codex.cwd}}:${{codex.cwd.length > 0}}`);\nconsole.log(`home:${{codex.homeDir === expectedHomeDir}}`);\nconsole.log(`tool:${{typeof codex.tool}}`);\nconsole.log(\"local-file-console-ok\");\n"
        ),
    )?;

    let (session, mut turn) = make_session_and_context().await;
    session
        .set_dependency_env(HashMap::from([(
            "HOME".to_string(),
            "/tmp/codex-home".to_string(),
        )]))
        .await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.abs();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "await import(\"./globals.js\");".to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await?;
    assert!(result.output.contains("tmp:true"));
    assert!(result.output.contains("cwd:string:true"));
    assert!(result.output.contains("home:true"));
    assert!(result.output.contains("tool:function"));
    assert!(result.output.contains("local-file-console-ok"));
    Ok(())
}

#[tokio::test]
async fn js_repl_reimports_local_files_after_edit() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let cwd_dir = tempdir()?;
    let helper_path = cwd_dir.path().join("helper.js");
    fs::write(&helper_path, "export const value = \"v1\";\n")?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.abs();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let first = manager
            .execute(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::clone(&tracker),
                JsReplArgs {
                    code: "const { value: firstValue } = await import(\"./helper.js\");\nconsole.log(firstValue);".to_string(),
                    timeout_ms: Some(10_000),
                },
            )
            .await?;
    assert!(first.output.contains("v1"));

    fs::write(&helper_path, "export const value = \"v2\";\n")?;

    let second = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "console.log(firstValue);\nconst { value: secondValue } = await import(\"./helper.js\");\nconsole.log(secondValue);".to_string(),
                    timeout_ms: Some(10_000),
                },
            )
            .await?;
    assert!(second.output.contains("v1"));
    assert!(second.output.contains("v2"));
    Ok(())
}

#[tokio::test]
async fn js_repl_reimports_local_files_after_fixing_failure() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let cwd_dir = tempdir()?;
    let helper_path = cwd_dir.path().join("broken.js");
    fs::write(&helper_path, "throw new Error(\"boom\");\n")?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.abs();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let err = manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            JsReplArgs {
                code: "await import(\"./broken.js\");".to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await
        .expect_err("expected broken module import to fail");
    assert!(err.to_string().contains("boom"));

    fs::write(&helper_path, "export const value = \"fixed\";\n")?;

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "console.log((await import(\"./broken.js\")).value);".to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await?;
    assert!(result.output.contains("fixed"));
    Ok(())
}

#[tokio::test]
async fn js_repl_local_files_expose_node_like_import_meta() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let cwd_dir = tempdir()?;
    let pkg_dir = cwd_dir.path().join("node_modules").join("repl_meta_pkg");
    fs::create_dir_all(&pkg_dir)?;
    fs::write(
        pkg_dir.join("package.json"),
        "{\n  \"name\": \"repl_meta_pkg\",\n  \"version\": \"1.0.0\",\n  \"type\": \"module\",\n  \"exports\": {\n    \"import\": \"./index.js\"\n  }\n}\n",
    )?;
    fs::write(
        pkg_dir.join("index.js"),
        "import { sep } from \"node:path\";\nexport const value = `pkg:${typeof sep}`;\n",
    )?;
    write_js_repl_test_module(
        cwd_dir.path(),
        "child.js",
        "export const value = \"child-export\";\n",
    )?;
    write_js_repl_test_module(
        cwd_dir.path(),
        "meta.js",
        "console.log(import.meta.url);\nconsole.log(import.meta.filename);\nconsole.log(import.meta.dirname);\nconsole.log(import.meta.main);\nconsole.log(import.meta.resolve(\"./child.js\"));\nconsole.log(import.meta.resolve(\"repl_meta_pkg\"));\nconsole.log(import.meta.resolve(\"node:fs\"));\nconsole.log((await import(import.meta.resolve(\"./child.js\"))).value);\nconsole.log((await import(import.meta.resolve(\"repl_meta_pkg\"))).value);\n",
    )?;
    let child_path = fs::canonicalize(cwd_dir.path().join("child.js"))?;
    let child_url = url::Url::from_file_path(&child_path)
        .expect("child path should convert to file URL")
        .to_string();

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.abs();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let result = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "await import(\"./meta.js\");".to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await?;
    let cwd_display = cwd_dir.path().display().to_string();
    let meta_path_display = cwd_dir.path().join("meta.js").display().to_string();
    assert!(result.output.contains("file://"));
    assert!(result.output.contains(&meta_path_display));
    assert!(result.output.contains(&cwd_display));
    assert!(result.output.contains("false"));
    assert!(result.output.contains(&child_url));
    assert!(result.output.contains("repl_meta_pkg"));
    assert!(result.output.contains("node:fs"));
    assert!(result.output.contains("child-export"));
    assert!(result.output.contains("pkg:string"));
    Ok(())
}

#[tokio::test]
async fn js_repl_rejects_top_level_static_imports_with_clear_error() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let (session, turn) = make_session_and_context().await;
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let err = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "import \"./local.js\";".to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await
        .expect_err("expected top-level static import to be rejected");
    assert!(
        err.to_string()
            .contains("Top-level static import \"./local.js\" is not supported in js_repl")
    );
    Ok(())
}

#[tokio::test]
async fn js_repl_local_files_reject_static_bare_imports() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let cwd_dir = tempdir()?;
    write_js_repl_test_package(cwd_dir.path(), "repl_counter", "pkg")?;
    write_js_repl_test_module(
        cwd_dir.path(),
        "entry.js",
        "import { value } from \"repl_counter\";\nconsole.log(value);\n",
    )?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.abs();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let err = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "await import(\"./entry.js\");".to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await
        .expect_err("expected static bare import to be rejected");
    assert!(
        err.to_string()
            .contains("Static import \"repl_counter\" is not supported from js_repl local files")
    );
    Ok(())
}

#[tokio::test]
async fn js_repl_rejects_unsupported_file_specifiers() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let cwd_dir = tempdir()?;
    write_js_repl_test_module(cwd_dir.path(), "local.ts", "export const value = \"ts\";\n")?;
    write_js_repl_test_module(cwd_dir.path(), "local", "export const value = \"noext\";\n")?;
    fs::create_dir_all(cwd_dir.path().join("dir"))?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.abs();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let unsupported_extension = manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            JsReplArgs {
                code: "await import(\"./local.ts\");".to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await
        .expect_err("expected unsupported extension to be rejected");
    assert!(
        unsupported_extension
            .to_string()
            .contains("Only .js and .mjs files are supported")
    );

    let extensionless = manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            JsReplArgs {
                code: "await import(\"./local\");".to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await
        .expect_err("expected extensionless import to be rejected");
    assert!(
        extensionless
            .to_string()
            .contains("Only .js and .mjs files are supported")
    );

    let directory = manager
        .execute(
            Arc::clone(&session),
            Arc::clone(&turn),
            Arc::clone(&tracker),
            JsReplArgs {
                code: "await import(\"./dir\");".to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await
        .expect_err("expected directory import to be rejected");
    assert!(
        directory
            .to_string()
            .contains("Directory imports are not supported")
    );

    let unsupported_url = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "await import(\"https://example.com/test.js\");".to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await
        .expect_err("expected unsupported url import to be rejected");
    assert!(
        unsupported_url
            .to_string()
            .contains("Unsupported import specifier")
    );
    Ok(())
}

#[tokio::test]
async fn js_repl_blocks_sensitive_builtin_imports_from_local_files() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let cwd_dir = tempdir()?;
    write_js_repl_test_module(
        cwd_dir.path(),
        "blocked.js",
        "import process from \"node:process\";\nconsole.log(process.pid);\n",
    )?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.abs();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let err = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "await import(\"./blocked.js\");".to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await
        .expect_err("expected blocked builtin import to be rejected");
    assert!(
        err.to_string()
            .contains("Importing module \"node:process\" is not allowed in js_repl")
    );
    Ok(())
}

#[tokio::test]
async fn js_repl_local_files_do_not_escape_node_module_search_roots() -> anyhow::Result<()> {
    if !can_run_js_repl_runtime_tests().await {
        return Ok(());
    }

    let parent_dir = tempdir()?;
    write_js_repl_test_package(parent_dir.path(), "repl_probe", "parent")?;
    let cwd_dir = parent_dir.path().join("workspace");
    fs::create_dir_all(&cwd_dir)?;
    write_js_repl_test_module(
        &cwd_dir,
        "entry.js",
        "const { value } = await import(\"repl_probe\");\nconsole.log(value);\n",
    )?;

    let (session, mut turn) = make_session_and_context().await;
    turn.shell_environment_policy
        .r#set
        .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
    turn.cwd = cwd_dir.abs();
    turn.js_repl = Arc::new(JsReplHandle::with_node_path(
        turn.config.js_repl_node_path.clone(),
        Vec::new(),
    ));

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
    let manager = turn.js_repl.manager().await?;

    let err = manager
        .execute(
            session,
            turn,
            tracker,
            JsReplArgs {
                code: "await import(\"./entry.js\");".to_string(),
                timeout_ms: Some(10_000),
            },
        )
        .await
        .expect_err("expected parent node_modules lookup to be rejected");
    assert!(err.to_string().contains("repl_probe"));
    Ok(())
}

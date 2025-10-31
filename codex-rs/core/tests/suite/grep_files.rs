#![cfg(not(target_os = "windows"))]

use anyhow::Result;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;
use std::process::Command as StdCommand;
use wiremock::matchers::any;

const MODEL_WITH_TOOL: &str = "test-gpt-5-codex";

fn ripgrep_available() -> bool {
    StdCommand::new("rg")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

macro_rules! skip_if_ripgrep_missing {
    ($ret:expr $(,)?) => {{
        if !ripgrep_available() {
            eprintln!("rg not available in PATH; skipping test");
            return $ret;
        }
    }};
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grep_files_tool_collects_matches() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_ripgrep_missing!(Ok(()));

    let server = start_mock_server().await;
    let test = build_test_codex(&server).await?;

    let search_dir = test.cwd.path().join("src");
    std::fs::create_dir_all(&search_dir)?;
    let alpha = search_dir.join("alpha.rs");
    let beta = search_dir.join("beta.rs");
    let gamma = search_dir.join("gamma.txt");
    std::fs::write(&alpha, "alpha needle\n")?;
    std::fs::write(&beta, "beta needle\n")?;
    std::fs::write(&gamma, "needle in text but excluded\n")?;

    let call_id = "grep-files-collect";
    let arguments = serde_json::json!({
        "pattern": "needle",
        "path": search_dir.to_string_lossy(),
        "include": "*.rs",
    })
    .to_string();

    mount_tool_sequence(&server, call_id, &arguments, "grep_files").await;
    submit_turn(&test, "please find uses of needle").await?;

    let bodies = recorded_bodies(&server).await?;
    let tool_output = find_tool_output(&bodies, call_id).expect("tool output present");
    let payload = tool_output.get("output").expect("output field present");
    let (content_opt, success_opt) = extract_content_and_success(payload);
    let content = content_opt.expect("content present");
    let success = success_opt.unwrap_or(true);
    assert!(success, "expected success for matches, got {payload:?}");

    let entries = collect_file_names(content);
    assert_eq!(entries.len(), 2, "content: {content}");
    assert!(
        entries.contains("alpha.rs"),
        "missing alpha.rs in {entries:?}"
    );
    assert!(
        entries.contains("beta.rs"),
        "missing beta.rs in {entries:?}"
    );
    assert!(
        !entries.contains("gamma.txt"),
        "txt file should be filtered out: {entries:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grep_files_tool_reports_empty_results() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_ripgrep_missing!(Ok(()));

    let server = start_mock_server().await;
    let test = build_test_codex(&server).await?;

    let search_dir = test.cwd.path().join("logs");
    std::fs::create_dir_all(&search_dir)?;
    std::fs::write(search_dir.join("output.txt"), "no hits here")?;

    let call_id = "grep-files-empty";
    let arguments = serde_json::json!({
        "pattern": "needle",
        "path": search_dir.to_string_lossy(),
        "limit": 5,
    })
    .to_string();

    mount_tool_sequence(&server, call_id, &arguments, "grep_files").await;
    submit_turn(&test, "search again").await?;

    let bodies = recorded_bodies(&server).await?;
    let tool_output = find_tool_output(&bodies, call_id).expect("tool output present");
    let payload = tool_output.get("output").expect("output field present");
    let (content_opt, success_opt) = extract_content_and_success(payload);
    let content = content_opt.expect("content present");
    if let Some(success) = success_opt {
        assert!(!success, "expected success=false payload: {payload:?}");
    }
    assert_eq!(content, "No matches found.");

    Ok(())
}

#[allow(clippy::expect_used)]
async fn build_test_codex(server: &wiremock::MockServer) -> Result<TestCodex> {
    let mut builder = test_codex().with_config(|config| {
        config.model = MODEL_WITH_TOOL.to_string();
        config.model_family =
            find_family_for_model(MODEL_WITH_TOOL).expect("model family for test model");
    });
    builder.build(server).await
}

async fn submit_turn(test: &TestCodex, prompt: &str) -> Result<()> {
    let session_model = test.session_configured.model.clone();

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: prompt.into(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TaskComplete(_))
    })
    .await;
    Ok(())
}

async fn mount_tool_sequence(
    server: &wiremock::MockServer,
    call_id: &str,
    arguments: &str,
    tool_name: &str,
) {
    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, tool_name, arguments),
        ev_completed("resp-1"),
    ]);
    responses::mount_sse_once_match(server, any(), first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    responses::mount_sse_once_match(server, any(), second_response).await;
}

#[allow(clippy::expect_used)]
async fn recorded_bodies(server: &wiremock::MockServer) -> Result<Vec<Value>> {
    let requests = server.received_requests().await.expect("requests recorded");
    Ok(requests
        .iter()
        .map(|req| req.body_json::<Value>().expect("request json"))
        .collect())
}

fn find_tool_output<'a>(requests: &'a [Value], call_id: &str) -> Option<&'a Value> {
    requests.iter().find_map(|body| {
        body.get("input")
            .and_then(Value::as_array)
            .and_then(|items| {
                items.iter().find(|item| {
                    item.get("type").and_then(Value::as_str) == Some("function_call_output")
                        && item.get("call_id").and_then(Value::as_str) == Some(call_id)
                })
            })
    })
}

fn collect_file_names(content: &str) -> HashSet<String> {
    content
        .lines()
        .filter_map(|line| {
            if line.trim().is_empty() {
                return None;
            }
            Path::new(line)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .collect()
}

fn extract_content_and_success(value: &Value) -> (Option<&str>, Option<bool>) {
    match value {
        Value::String(text) => (Some(text.as_str()), None),
        Value::Object(obj) => (
            obj.get("content").and_then(Value::as_str),
            obj.get("success").and_then(Value::as_bool),
        ),
        _ => (None, None),
    }
}

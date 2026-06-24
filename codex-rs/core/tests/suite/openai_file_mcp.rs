#![cfg(not(target_os = "windows"))]

use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_utils_path_uri::PathUri;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::CALENDAR_EXTRACT_TEXT_TOOL_NAME;
use core_test_support::apps_test_server::DIRECT_CALENDAR_EXTRACT_TEXT_TOOL as DOCUMENT_EXTRACT_HOOK_MATCHER;
use core_test_support::apps_test_server::DOCUMENT_EXTRACT_TEXT_RESOURCE_URI;
use core_test_support::apps_test_server::SEARCH_CALENDAR_EXTRACT_TEXT_TOOL as DOCUMENT_EXTRACT_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_NAMESPACE as DOCUMENT_EXTRACT_NAMESPACE;
use core_test_support::apps_test_server::apps_enabled_builder;
use core_test_support::apps_test_server::recorded_apps_tool_call_by_name;
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::ev_tool_search_call;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::namespace_child_tool;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_target_windows;
use core_test_support::test_codex::TestCodex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_json;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const STREAMED_FILE_SIZE: usize = 13 * 1024 * 1024;

fn write_post_tool_use_hook(home: &Path) -> Result<()> {
    let script_path = home.join("post_tool_use_hook.py");
    let log_path = home.join("post_tool_use_hook_log.jsonl");
    let script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)

with Path(r"{log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

print(json.dumps({{
    "hookSpecificOutput": {{
        "hookEventName": "PostToolUse",
        "additionalContext": "observed apps file payload"
    }}
}}))
"#,
        log_path = log_path.display(),
    );
    let hooks = serde_json::json!({
        "hooks": {
            "PostToolUse": [{
                "matcher": DOCUMENT_EXTRACT_HOOK_MATCHER,
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                    "statusMessage": "running apps file post tool use hook",
                }]
            }]
        }
    });

    fs::write(&script_path, script).context("write post tool use hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn read_post_tool_use_hook_inputs(home: &Path) -> Result<Vec<Value>> {
    fs::read_to_string(home.join("post_tool_use_hook_log.jsonl"))
        .context("read post tool use hook log")?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).context("parse post tool use hook input"))
        .collect()
}

fn uploaded_file(server: &MockServer, file_size_bytes: u64) -> Value {
    json!({
        "download_url": format!("{}/download/file_123", server.uri()),
        "file_id": "file_123",
        "mime_type": "text/plain",
        "file_name": "report.txt",
        "uri": "sediment://file_123",
        "file_size_bytes": file_size_bytes,
    })
}

async fn mount_file_upload_mocks(server: &MockServer, file_size_bytes: u64) {
    Mock::given(method("POST"))
        .and(path("/files"))
        .and(header("chatgpt-account-id", "account_id"))
        .and(body_json(json!({
            "file_name": "report.txt",
            "file_size": file_size_bytes,
            "use_case": "codex",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "file_id": "file_123",
            "upload_url": format!("{}/upload/file_123", server.uri()),
        })))
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/upload/file_123"))
        .and(header("content-length", file_size_bytes.to_string()))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/files/file_123/uploaded"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "download_url": format!("{}/download/file_123", server.uri()),
            "file_name": "report.txt",
            "mime_type": "text/plain",
            "file_size_bytes": file_size_bytes,
        })))
        .expect(1)
        .mount(server)
        .await;
}

async fn run_extract_turn(test: &TestCodex, server: &MockServer) -> Result<ResponseMock> {
    let mock = mount_sse_sequence(
        server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_tool_search_call(
                    "extract-search-1",
                    &json!({
                        "query": "extract text from uploaded document",
                        "limit": 1,
                    }),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_function_call_with_namespace(
                    "extract-call-1",
                    DOCUMENT_EXTRACT_NAMESPACE,
                    DOCUMENT_EXTRACT_TOOL,
                    &json!({"file": "report.txt"}).to_string(),
                ),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;

    test.submit_turn_with_approval_and_permission_profile(
        "Extract the report text with the app tool.",
        AskForApproval::Never,
        PermissionProfile::Disabled,
    )
    .await?;

    Ok(mock)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_apps_file_params_upload_environment_files_before_mcp_tool_call() -> Result<()> {
    // TODO(anp): Remove after file-upload fixtures support target-native Windows paths.
    skip_if_target_windows!(Ok(()), "uses a host-native file-upload path");

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    mount_file_upload_mocks(&server, STREAMED_FILE_SIZE as u64).await;

    let mut builder = apps_enabled_builder(apps_server.chatgpt_base_url.clone())
        .with_workspace_setup(|cwd, fs| async move {
            let report_path = PathUri::from_abs_path(&cwd.join("report.txt"));
            fs.write_file(
                &report_path,
                vec![b'x'; STREAMED_FILE_SIZE],
                /*sandbox*/ None,
            )
            .await?;
            Ok(())
        });
    let test = builder.build_with_auto_env(&server).await?;
    let mock = run_extract_turn(&test, &server).await?;

    let requests = mock.requests();
    let search_output = requests[1].tool_search_output("extract-search-1");
    let missing_tool_message = format!(
        "missing tool {DOCUMENT_EXTRACT_NAMESPACE}{DOCUMENT_EXTRACT_TOOL} in tool_search output: {search_output:?}"
    );
    let extract_tool = namespace_child_tool(
        &search_output,
        DOCUMENT_EXTRACT_NAMESPACE,
        DOCUMENT_EXTRACT_TOOL,
    )
    .expect(&missing_tool_message);
    assert_eq!(
        extract_tool.pointer("/parameters/properties/file"),
        Some(&json!({
            "type": "string",
            "description": "Document file payload. This parameter expects an absolute local file path. If you want to upload a file, provide the absolute path to that file here."
        }))
    );

    let apps_tool_call =
        recorded_apps_tool_call_by_name(&server, CALENDAR_EXTRACT_TEXT_TOOL_NAME).await;

    assert_eq!(
        apps_tool_call.pointer("/params/arguments/file"),
        Some(&uploaded_file(&server, STREAMED_FILE_SIZE as u64))
    );
    assert_eq!(
        apps_tool_call.pointer("/params/_meta/_codex_apps"),
        Some(&json!({
            "call_id": "extract-call-1",
            "resource_uri": DOCUMENT_EXTRACT_TEXT_RESOURCE_URI,
            "contains_mcp_source": true,
            "connector_id": "calendar",
        }))
    );

    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_apps_file_params_pass_uploaded_file_to_post_tool_use_hook() -> Result<()> {
    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    mount_file_upload_mocks(&server, /*file_size_bytes*/ 11).await;

    let mut builder = apps_enabled_builder(apps_server.chatgpt_base_url.clone())
        .with_pre_build_hook(move |home| {
            if let Err(error) = write_post_tool_use_hook(home) {
                panic!("failed to write apps file post tool use hook fixture: {error}");
            }
        })
        .with_config(move |config| {
            trust_discovered_hooks(config);
        });
    let test = builder.build(&server).await?;
    tokio::fs::write(test.cwd.path().join("report.txt"), b"hello world").await?;
    let _responses = run_extract_turn(&test, &server).await?;

    let hook_inputs = read_post_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(
        hook_inputs[0]["tool_input"]["file"],
        uploaded_file(&server, /*file_size_bytes*/ 11)
    );

    server.verify().await;
    Ok(())
}

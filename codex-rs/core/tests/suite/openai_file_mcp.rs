#![cfg(not(target_os = "windows"))]

use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use codex_core::config::Config;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::DOCUMENT_EXTRACT_TEXT_RESOURCE_URI;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_json;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const DOCUMENT_EXTRACT_NAMESPACE: &str = "mcp__codex_apps__calendar";
const DOCUMENT_EXTRACT_TOOL: &str = "_extract_text";
const DOCUMENT_EXTRACT_HOOK_MATCHER: &str = "mcp__codex_apps__calendar_extract_text";

fn configure_apps(config: &mut Config, chatgpt_base_url: &str) {
    if let Err(err) = config.features.enable(Feature::Apps) {
        panic!("test config should allow feature update: {err}");
    }
    config.chatgpt_base_url = chatgpt_base_url.to_string();
}

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_apps_file_params_upload_local_paths_before_mcp_tool_call() -> Result<()> {
    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;

    Mock::given(method("POST"))
        .and(path("/files"))
        .and(header("chatgpt-account-id", "account_id"))
        .and(body_json(json!({
            "file_name": "report.txt",
            "file_size": 11,
            "use_case": "codex",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "file_id": "file_123",
            "upload_url": format!("{}/upload/file_123", server.uri()),
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/upload/file_123"))
        .and(header("content-length", "11"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/files/file_123/uploaded"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "download_url": format!("{}/download/file_123", server.uri()),
            "file_name": "report.txt",
            "mime_type": "text/plain",
            "file_size_bytes": 11,
        })))
        .expect(1)
        .mount(&server)
        .await;

    let call_id = "extract-call-1";
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call_with_namespace(
                    call_id,
                    DOCUMENT_EXTRACT_NAMESPACE,
                    DOCUMENT_EXTRACT_TOOL,
                    &json!({"file": "report.txt"}).to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_pre_build_hook(move |home| {
            if let Err(error) = write_post_tool_use_hook(home) {
                panic!("failed to write apps file post tool use hook fixture: {error}");
            }
        })
        .with_config(move |config| {
            configure_apps(config, apps_server.chatgpt_base_url.as_str());
            if let Err(err) = config.features.enable(Feature::CodexHooks) {
                panic!("test config should allow feature update: {err}");
            }
        });
    let test = builder.build(&server).await?;
    tokio::fs::write(test.cwd.path().join("report.txt"), b"hello world").await?;

    test.submit_turn_with_approval_and_permission_profile(
        "Extract the report text with the app tool.",
        AskForApproval::Never,
        PermissionProfile::Disabled,
    )
    .await?;

    let requests = mock.requests();
    let Some(extract_tool) =
        requests[0].tool_by_name(DOCUMENT_EXTRACT_NAMESPACE, DOCUMENT_EXTRACT_TOOL)
    else {
        let body = requests[0].body_json();
        panic!(
            "missing tool {DOCUMENT_EXTRACT_NAMESPACE}{DOCUMENT_EXTRACT_TOOL} in /v1/responses request: {body:?}"
        )
    };
    assert_eq!(
        extract_tool.pointer("/parameters/properties/file"),
        Some(&json!({
            "type": "string",
            "description": "Document file payload. This parameter expects an absolute local file path. If you want to upload a file, provide the absolute path to that file here."
        }))
    );

    let apps_tool_call = server
        .received_requests()
        .await
        .unwrap_or_default()
        .into_iter()
        .find_map(|request| {
            let body: Value = serde_json::from_slice(&request.body).ok()?;
            (request.url.path() == "/api/codex/apps"
                && body.get("method").and_then(Value::as_str) == Some("tools/call")
                && body.pointer("/params/name").and_then(Value::as_str)
                    == Some("calendar_extract_text"))
            .then_some(body)
        })
        .expect("apps calendar_extract_text tools/call request should be recorded");

    assert_eq!(
        apps_tool_call.pointer("/params/arguments/file"),
        Some(&json!({
            "download_url": format!("{}/download/file_123", server.uri()),
            "file_id": "file_123",
            "mime_type": "text/plain",
            "file_name": "report.txt",
            "uri": "sediment://file_123",
            "file_size_bytes": 11,
        }))
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

    let hook_inputs = read_post_tool_use_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    assert_eq!(
        hook_inputs[0]["tool_input"]["file"],
        json!({
            "download_url": format!("{}/download/file_123", server.uri()),
            "file_id": "file_123",
            "mime_type": "text/plain",
            "file_name": "report.txt",
            "uri": "sediment://file_123",
            "file_size_bytes": 11,
        })
    );

    server.verify().await;
    Ok(())
}

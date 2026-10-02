//! Exercises an MCP server applying harness-provided sandbox state to a real CLI subprocess.
//! The server passes request metadata unchanged; filesystem effects verify enforcement.

use std::path::PathBuf;
use std::process::Stdio;

use anyhow::Context;
use anyhow::Result;
use codex_config::McpServerConfig;
use codex_mcp::MCP_SANDBOX_STATE_META_CAPABILITY;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_cargo_bin::cargo_bin;
use codex_utils_process::background_command;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_remote;
use core_test_support::skip_if_sandbox;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::tempdir;
use wiremock::Mock;
use wiremock::Request;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::split_wall_time_wrapped_output;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_server_enforces_sandbox_state_with_codex_sandbox() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_remote!(
        Ok(()),
        "the HTTP MCP fixture executes the host CLI against host filesystem paths"
    );

    let responses_server = responses::start_mock_server().await;
    let mcp_server = responses::start_mock_server().await;
    let sandbox_home = tempdir()?;
    let server_cwd = tempdir()?;
    let outside_workspace = tempdir()?;
    let denied_path = outside_workspace.path().canonicalize()?.join("denied.txt");
    // Establish that this is an existing, writable file before applying the sandbox.
    std::fs::write(&denied_path, "original")?;
    #[cfg(windows)]
    std::fs::write(
        sandbox_home.path().join("config.toml"),
        "[windows]\nsandbox = \"unelevated\"\n",
    )?;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(SandboxMcpServer {
            codex: cargo_bin("codex")?,
            codex_home: sandbox_home.path().to_path_buf(),
            cwd: server_cwd.path().to_path_buf(),
        })
        .mount(&mcp_server)
        .await;

    let server_config: McpServerConfig = serde_json::from_value(json!({
        "url": format!("{}/mcp", mcp_server.uri()),
        "default_tools_approval_mode": "approve",
    }))?;
    let fixture = test_codex()
        .with_config(move |config| {
            config.workspace_roots = vec![config.cwd.clone()];
            config
                .mcp_servers
                .set([("sandbox".to_string(), server_config)].into())
                .expect("test config should allow the MCP server");
        })
        .build_with_auto_env(&responses_server)
        .await?;
    wait_for_mcp_server(&fixture.codex, "sandbox").await?;

    let calls = [
        ("allowed-write", json!("allowed.txt")),
        ("denied-write", json!(denied_path)),
    ];
    let mut events = vec![responses::ev_response_created("resp-1")];
    for (call_id, target_path) in &calls {
        events.push(responses::ev_function_call_with_namespace(
            call_id,
            "mcp__sandbox",
            "write_file",
            &json!({"path": target_path}).to_string(),
        ));
    }
    events.push(responses::ev_completed("resp-1"));
    let first_request = responses::mount_sse_once(&responses_server, responses::sse(events)).await;
    let final_request = responses::mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("resp-2"),
            responses::ev_assistant_message("msg-1", "done"),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    fixture
        .submit_turn_with_permission_profile(
            "Write one file in the workspace and try overwriting the file outside it.",
            PermissionProfile::workspace_write_with(
                &[],
                NetworkSandboxPolicy::Restricted,
                // Both directories are temporary; exclude the usual ambient tmp write grants.
                /*exclude_tmpdir_env_var*/
                true,
                /*exclude_slash_tmp*/ true,
            ),
        )
        .await?;

    first_request.single_request();
    let request = final_request.single_request();
    for ((call_id, _), expected_success) in calls.iter().zip([true, false]) {
        let output = request.function_call_output(call_id);
        let text = output["output"].as_str().context("MCP tool output")?;
        let result: Value = serde_json::from_str(split_wall_time_wrapped_output(text))?;
        assert_eq!(result["success"], json!(expected_success), "{result}");
    }
    // A relative tool argument must resolve against sandboxCwd, not the server's cwd.
    assert_eq!(
        (
            std::fs::read_to_string(fixture.workspace_path("allowed.txt"))?,
            std::fs::read_to_string(&denied_path)?,
            server_cwd.path().join("allowed.txt").exists(),
        ),
        ("sandboxed".to_string(), "original".to_string(), false),
    );
    fixture.codex.shutdown_and_wait().await?;
    Ok(())
}

struct SandboxMcpServer {
    codex: PathBuf,
    codex_home: PathBuf,
    cwd: PathBuf,
}

impl Respond for SandboxMcpServer {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = request.body_json().expect("valid MCP request");
        let result = match body["method"].as_str() {
            Some("initialize") => json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {
                    "tools": {},
                    "experimental": {MCP_SANDBOX_STATE_META_CAPABILITY: {}},
                },
                "serverInfo": {"name": "sandbox-test", "version": "1.0.0"},
            }),
            Some("notifications/initialized") => return ResponseTemplate::new(/*s*/ 202),
            Some("tools/list") => json!({"tools": [{
                "name": "write_file",
                "description": "Write a file using the sandbox state supplied by Codex.",
                "inputSchema": {
                    "type": "object",
                    "properties": {"path": {"type": "string"}},
                    "required": ["path"],
                    "additionalProperties": false,
                },
            }]}),
            Some("tools/call") => {
                let state = body["params"]["_meta"]
                    .get(MCP_SANDBOX_STATE_META_CAPABILITY)
                    .expect("Codex must supply sandbox state in request metadata");
                let target_path = body["params"]["arguments"]["path"]
                    .as_str()
                    .expect("write_file path argument");
                let mut command = background_command(&self.codex);
                command
                    .current_dir(&self.cwd)
                    .env("CODEX_HOME", &self.codex_home)
                    .env_remove("CODEX_ACCESS_TOKEN")
                    .env_remove("OPENAI_API_KEY")
                    .env("CODEX_TEST_WRITE_PATH", target_path)
                    .stdin(Stdio::null())
                    .args(["sandbox", "--sandbox-state-json", &state.to_string(), "--"]);
                #[cfg(not(windows))]
                command.args([
                    "/bin/sh",
                    "-c",
                    "printf sandboxed > \"$CODEX_TEST_WRITE_PATH\"",
                ]);
                #[cfg(windows)]
                command.args([
                    "powershell.exe",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    // Use a cmdlet that works in PowerShell's Constrained Language Mode.
                    "Set-Content -LiteralPath $env:CODEX_TEST_WRITE_PATH -Value 'sandboxed' -Encoding Ascii -NoNewline -ErrorAction Stop",
                ]);
                let output = command.output().expect("launch codex sandbox");
                let report = json!({
                    "success": output.status.success(),
                    "stdout": String::from_utf8_lossy(&output.stdout),
                    "stderr": String::from_utf8_lossy(&output.stderr),
                });
                json!({
                    "content": [{"type": "text", "text": report.to_string()}],
                    "structuredContent": report,
                    "isError": !output.status.success(),
                })
            }
            other => panic!("unexpected MCP method: {other:?}"),
        };
        ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "jsonrpc": "2.0", "id": body["id"], "result": result,
        }))
    }
}

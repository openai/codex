//! Covers model request history through authenticated MCP and retained cwd/environment roots.
//! This scenario does not establish filesystem sandbox enforcement.

use super::*;
use crate::legacy_core::config::Config;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::*;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use pretty_assertions::assert_eq;

async fn model_call(
    server: &mut crate::app_server_session::AppServerSession,
    model: &wiremock::MockServer,
    thread: &str,
    tool: &str,
    arguments: Value,
) -> color_eyre::Result<(Value, Vec<responses::ResponsesRequest>, usize)> {
    model.reset().await;
    let call = if tool == "exec_command" {
        responses::ev_function_call(tool, tool, &arguments.to_string())
    } else {
        responses::ev_function_call_with_namespace(
            tool,
            "mcp__codex_worktrees",
            tool,
            &arguments.to_string(),
        )
    };
    let mock = responses::mount_sse_sequence(
        model,
        vec![
            responses::sse(vec![call, responses::ev_completed("call")]),
            responses::sse(vec![
                responses::ev_assistant_message("reply", "Done."),
                responses::ev_completed("done"),
            ]),
        ],
    )
    .await;
    let _: TurnStartResponse = server
        .request_handle()
        .request_typed(ClientRequest::TurnStart {
            request_id: request_id(),
            params: TurnStartParams {
                thread_id: thread.to_owned(),
                input: vec![UserInput::Text {
                    text: format!("Run {tool}."),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;
    let approvals = tokio::time::timeout(Duration::from_secs(/*secs*/ 30), async {
        let mut approvals = 0;
        while let Some(event) = server.next_event().await {
            match event {
                AppServerEvent::ServerRequest(request) => {
                    let ServerRequest::McpServerElicitationRequest { request_id, params } = *request else { panic!("unexpected server request: {request:?}"); };
                    assert_eq!(params.thread_id, thread);
                    approvals += 1;
                    server.resolve_server_request(request_id, json!({"action":"accept","content":null,"_meta":null})).await?;
                }
                AppServerEvent::ServerNotification(notification) if matches!(notification.as_ref(), ServerNotification::TurnCompleted(event) if event.thread_id == thread) => break,
                _ => {},
            }
        }
        Ok::<_, color_eyre::Report>(approvals)
    }).await??;
    let requests = mock.requests();
    assert_eq!(
        requests.len(),
        2,
        "model must observe the MCP result before answering"
    );
    let output = requests[1].function_call_output(tool)["output"].clone();
    let texts = if let Some(text) = output.as_str() {
        vec![text]
    } else {
        output
            .as_array()
            .or_else(|| output["content"].as_array())
            .expect("content blocks")
            .iter()
            .filter_map(|item| item["text"].as_str())
            .collect::<Vec<_>>()
    };
    if tool == "exec_command" {
        return Ok((Value::String(texts.join("\n")), requests, approvals));
    }
    let result = texts
        .iter()
        .find_map(|text| serde_json::from_str::<Value>(text).ok())
        .unwrap_or_else(|| panic!("missing JSON result for {tool}: {output:#}"));
    Ok((result, requests, approvals))
}

pub(super) async fn exercise(
    server: &mut crate::app_server_session::AppServerSession,
    model: &wiremock::MockServer,
    config: &Config,
    repository: &Path,
) -> color_eyre::Result<()> {
    let mut config = config.clone();
    config.cwd = codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(
        dunce::canonicalize(repository)?,
    )?;
    config
        .features
        .enable(codex_features::Feature::ToolCallMcpElicitation)?;
    config.approvals_reviewer = codex_protocol::config_types::ApprovalsReviewer::User;
    config
        .permissions
        .approval_policy
        .set(codex_protocol::protocol::AskForApproval::OnRequest)?;
    let started = server.start_thread(&config).await?;
    let thread = started.session.thread_id.to_string();
    let other = server
        .start_thread(&config)
        .await?
        .session
        .thread_id
        .to_string();
    let before = server
        .thread_read(started.session.thread_id, /*include_turns*/ false)
        .await?;
    let (pending, mut history, approvals) = model_call(
        server,
        model,
        &thread,
        "create_worktree",
        json!({"allowAsync":true}),
    )
    .await?;
    assert_eq!(approvals, 0);
    assert_eq!(pending["type"], "pending");
    // Wait for the asynchronous host operation, then read its status through the model.
    let mut overrides = None;
    server.thread_tool_transport().configure_mcp(&mut overrides);
    let overrides = overrides.unwrap();
    let mcp = &overrides["mcp_servers.codex_worktrees"];
    let client = codex_http_client::HttpClientBuilder::new().build_direct()?;
    tokio::time::timeout(Duration::from_secs(/*secs*/ 20), async {
        loop {
            let response = client.post(mcp["url"].as_str().unwrap())
                .header("Authorization", mcp["http_headers"]["Authorization"].as_str().unwrap())
                .header("Accept", "application/json, text/event-stream")
                .header("MCP-Protocol-Version", "2026-07-28")
                .header("MCP-Method", "tools/call").header("MCP-Name", "get_worktree_creation_status")
                .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
                    "name":"get_worktree_creation_status", "arguments":{"operationId":pending["operationId"]},
                    "_meta":{"threadId":thread,"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}
                }})).send().await.unwrap().text().await.unwrap();
            let result: Value = serde_json::from_str(response.lines().find_map(|line| line.strip_prefix("data: ")).unwrap_or(&response)).unwrap();
            let result: Value = serde_json::from_str(result["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
            if result["type"] != "pending" { break; }
            tokio::time::sleep(Duration::from_millis(/*millis*/ 20)).await;
        }
    }).await?;
    let (created, requests, approvals) = model_call(
        server,
        model,
        &thread,
        "get_worktree_creation_status",
        json!({"operationId":pending["operationId"]}),
    )
    .await?;
    history.extend(requests);
    assert_eq!(approvals, 0);
    assert_eq!(created["type"], "created");
    let root = PathBuf::from(created["worktree"]["root"].as_str().unwrap());
    let (listed, requests, approvals) =
        model_call(server, model, &thread, "list_worktrees", json!({})).await?;
    history.extend(requests);
    assert_eq!(approvals, 0);
    assert_eq!(listed["data"][0]["root"], created["worktree"]["root"]);
    let other_attachments: ThreadAttachmentListResponse = server
        .request_handle()
        .request_typed(ClientRequest::ThreadAttachmentList {
            request_id: request_id(),
            params: ThreadAttachmentListParams {
                thread_id: other,
                cursor: None,
                limit: Some(100),
            },
        })
        .await?;
    assert!(other_attachments.data.is_empty());
    let after = server
        .thread_read(started.session.thread_id, /*include_turns*/ false)
        .await?;
    // These assert retained roots, not filesystem permission enforcement.
    assert_eq!(
        (before.cwd, before.environments),
        (after.cwd, after.environments)
    );
    // Normalize fixture paths and terminal timing before formatting so truncated
    // result fingerprints remain stable across hosts and runs.
    let source = dunce::canonicalize(repository)?;
    let bodies = history
        .iter()
        .map(|request| {
            let mut body = request.body_json();
            // Keep every request and input item, but focus schema fingerprints on
            // the worktree tools so unrelated built-in changes do not churn this test.
            body["tools"]
                .as_array_mut()
                .expect("request tools")
                .retain(|tool| tool["name"] == "mcp__codex_worktrees");
            let mut text = body.to_string();
            for (path, label) in [
                (&root.join(""), "<WORKTREE>/"),
                (&root, "<WORKTREE>"),
                (&source, "<SOURCE>"),
                (&repository.to_path_buf(), "<SOURCE>"),
            ] {
                let quoted = serde_json::to_string(path).unwrap();
                let escaped = &quoted[1..quoted.len() - 1];
                let twice = serde_json::to_string(escaped).unwrap();
                text = text
                    .replace(&twice[1..twice.len() - 1], label)
                    .replace(escaped, label);
            }
            let text = regex_lite::Regex::new(r"Chunk ID: [a-zA-Z0-9]+")
                .unwrap()
                .replace_all(&text, "Chunk ID: <CHUNK>")
                .into_owned();
            let text = regex_lite::Regex::new(r"Wall time: [0-9.]+ seconds")
                .unwrap()
                .replace_all(&text, "Wall time: <DURATION> seconds")
                .into_owned();
            let text = regex_lite::Regex::new(r"session ID [0-9]+")
                .unwrap()
                .replace_all(&text, "session ID <SESSION>")
                .into_owned();
            serde_json::from_str::<Value>(&text).unwrap()
        })
        .collect::<Vec<_>>();
    let entries = bodies
        .iter()
        .map(context_snapshot::SnapshotEntry::body)
        .collect::<Vec<_>>();
    insta::assert_snapshot!(
        "managed_worktree_model_creation",
        context_snapshot::format_context_snapshot(
            "Model request history for creating, polling and listing a task-attached checkout through MCP, without a separate creation confirmation. Separate assertions verify retained cwd/environment roots, not sandbox enforcement.",
            &entries,
            &ContextSnapshotOptions::default()
                .rewrite_known_segments()
                .include_request_settings(),
        )
    );
    Ok(())
}

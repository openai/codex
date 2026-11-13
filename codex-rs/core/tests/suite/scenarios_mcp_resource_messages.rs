//! Catalog resource-tool text reaches direct calls and Code Mode without changing MCP execution.

use super::super::rmcp_client::remote_aware_environment_id;
use super::super::rmcp_client::remote_aware_stdio_server_bin;
use anyhow::Result;
use codex_core::ConfigRefreshOutcome;
use codex_features::Feature;
use codex_mcp::McpServerKey;
use codex_protocol::mcp::McpServerConnectionStatus;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::McpStartupStatus;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use test_case::test_case;

#[test_case(ToolMode::Direct; "direct")]
#[test_case(ToolMode::CodeModeOnly; "code_mode")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_resource_messages(tool_mode: ToolMode) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "requires a Windows test_stdio_server binary");
    let server = responses::start_mock_server().await;
    let command = remote_aware_stdio_server_bin()?;
    let calls = [
        ("list_mcp_resources", r#"{"server":"resources"}"#),
        ("list_mcp_resource_templates", r#"{"server":"resources"}"#),
        (
            "read_mcp_resource",
            r#"{"server":"resources","uri":"memo://codex/example-note"}"#,
        ),
    ];
    let mut messages = serde_json::Map::new();
    for (name, _) in calls {
        let mut parameters = json!({
            "type": "object",
            "properties": {"server": {"type": "string", "description": format!("Catalog server for {name}.")}},
            "additionalProperties": false,
        });
        if name == "read_mcp_resource" {
            parameters["properties"]["uri"] = json!({"type": "string", "description": "Catalog URI returned by resource discovery."});
            parameters["required"] = json!(["server", "uri"]);
        } else {
            parameters["properties"]["cursor"] =
                json!({"type": "string", "description": "Catalog cursor from the previous page."});
        }
        messages.insert(
            name.to_string(),
            json!({
                "description": format!("Catalog instructions for {name}."),
                "parameters": parameters.to_string(),
            }),
        );
    }
    let catalog = serde_json::from_value(json!(messages))?;
    let mut test = test_codex()
        .with_config(move |config| {
            super::configure_scenario_catalog(config);
            config.code_mode.disable_in_process_fallback = true;
            config
                .features
                .enable(Feature::CodeModeHost)
                .expect("enable Code Mode host");
            config
                .mcp_servers
                .set(HashMap::from([(
                    "resources".to_string(),
                    serde_json::from_value(json!({
                        "command": command,
                        "environment_id": remote_aware_environment_id(),
                        "cwd": config.cwd,
                    }))
                    .expect("MCP resource fixture"),
                )]))
                .expect("configure resources server");
        })
        .with_model_info_override("gpt-6-astra", move |model| {
            model.tool_mode = Some(tool_mode);
            model.use_responses_lite = false;
            let model_messages = model.model_messages.as_mut().expect("model messages");
            model_messages
                .tools
                .get_or_insert_with(Default::default)
                .mcp_resources = Some(catalog);
        })
        .build_with_auto_env(&server)
        .await?;
    wait_for_mcp_server(&test.codex, "resources").await?;
    let mut options = test.start_thread_options().await;
    options.session_source = Some(SessionSource::SubAgent(SubAgentSource::Other(
        "resource-discovery".to_string(),
    )));
    test.codex = test.thread_manager.start_thread(options).await?.thread;
    let resource_calls = |id: &str, calls: &[(&str, &str)]| {
        let mut events = vec![responses::ev_response_created(id)];
        if tool_mode == ToolMode::CodeModeOnly {
            let script = calls
                .iter()
                .map(|(name, args)| format!("text(await tools.{name}({args}));"))
                .collect::<Vec<_>>()
                .join("\n");
            events.push(responses::ev_custom_tool_call(id, "exec", &script));
        } else {
            events.extend(calls.iter().map(|(name, args)| {
                responses::ev_function_call(&format!("{id}-{name}"), name, args)
            }));
        }
        events.push(responses::ev_completed(id));
        responses::sse(events)
    };
    let broad = [
        ("list_mcp_resources", "{}"),
        ("list_mcp_resource_templates", "{}"),
    ];
    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            resource_calls("broad", &broad),
            responses::sse_completed("broad-done"),
            resource_calls("resources-call", &calls),
            responses::sse_completed("done"),
        ],
    )
    .await;
    test.submit_turn("List available MCP resources and templates.")
        .await?;
    let (config, _) = test.codex.current_mcp_config_and_runtime_context().await;
    assert_eq!(
        test.codex
            .mcp_connection_statuses(&config)
            .await
            .get(&McpServerKey::unscoped("resources")),
        Some(&McpServerConnectionStatus::NotStarted)
    );
    test.submit_turn("Query the deferred server by name and read its example note.")
        .await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), 4);
    if tool_mode == ToolMode::Direct {
        for (tool, key) in [
            ("list_mcp_resources", "resources"),
            ("list_mcp_resource_templates", "resourceTemplates"),
        ] {
            let output = requests[1]
                .function_call_output_text(&format!("broad-{tool}"))
                .expect("aggregate result");
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&output)?,
                json!({key: [], "deferredServers": ["resources"]})
            );
        }
    }
    let body = requests[0].body_json();
    for (name, message) in &messages {
        if tool_mode == ToolMode::CodeModeOnly {
            assert!(
                requests[0]
                    .body_contains_text(message["description"].as_str().expect("description"))
            );
            assert!(requests[0].body_contains_text(&format!("Catalog server for {name}.")));
        } else {
            let tool = body["tools"]
                .as_array()
                .expect("tools")
                .iter()
                .find(|tool| tool["name"] == *name)
                .expect("resource tool");
            assert_eq!(
                tool,
                &json!({
                    "type": "function", "name": name, "description": message["description"],
                    "parameters": serde_json::from_str::<serde_json::Value>(message["parameters"].as_str().expect("parameters"))?,
                    "strict": false,
                })
            );
        }
    }
    assert!(requests[3].body_contains_text("Example Note"));
    assert!(requests[3].body_contains_text("memo://codex/{slug}"));
    assert!(
        requests[3]
            .body_contains_text("This is a sample MCP resource served by the rmcp test server.")
    );
    if tool_mode == ToolMode::CodeModeOnly {
        insta::assert_snapshot!(
            "mcp_resource_messages",
            context_snapshot::format_request_history_snapshot(
                "Broad discovery names the dormant server. Calling it by name returns its resources and templates through Code Mode.",
                &requests,
                &ContextSnapshotOptions::default().include_request_settings(),
            )
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resource_helpers_across_server_changes() -> Result<()> {
    let server = responses::start_mock_server().await;
    let mcp = responses::start_mock_server().await;
    // Empty tools isolate the resource helper definitions from the separate
    // deferred and MCP type guidance when the first server comes and goes.
    let empty_server =
        AppsTestServer::mount_with_tools(&mcp, Arc::new(Mutex::new(Vec::new()))).await?;
    let test = test_codex()
        .with_config(move |config| {
            super::configure_scenario_catalog(config);
            config.code_mode.disable_in_process_fallback = true;
            config.features.enable(Feature::CodeModeHost).unwrap();
        })
        .with_model_info_override("gpt-6-astra", |model| {
            model.tool_mode = Some(ToolMode::CodeModeOnly);
            model.use_responses_lite = false;
        })
        .build_with_auto_env(&server)
        .await?;
    let script = r#"text(await tools.list_mcp_resources({}));
    text(await tools.list_mcp_resource_templates({}));
    try {
      await tools.read_mcp_resource({ server: "missing", uri: "memo://missing" });
    } catch (error) {
      text(String(error));
    }"#;
    let mut events = Vec::new();
    for phase in ["before", "registered", "removed"] {
        events.push(responses::sse(vec![
            responses::ev_custom_tool_call(phase, "exec", script),
            responses::ev_completed(phase),
        ]));
        events.push(responses::sse_completed(&format!("{phase}-done")));
    }
    let mock = responses::mount_sse_sequence(&server, events).await;
    test.submit_turn("Inspect resources without any configured server.")
        .await?;
    let current = test.codex.config().await;
    let mut next = current.as_ref().clone();
    next.mcp_servers.set(HashMap::from([(
        "resources".to_string(),
        serde_json::from_value(json!({
            "url": format!("{}/api/codex/ps/mcp", empty_server.chatgpt_base_url),
        }))?,
    )]))?;
    assert_eq!(
        test.codex.refresh_mcp_config(current, next).await,
        ConfigRefreshOutcome::Published,
    );
    let startup = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::McpStartupUpdate(update)
            if update.server == "resources"
                && !matches!(update.status, McpStartupStatus::Starting) =>
        {
            Some(update.status.clone())
        }
        _ => None,
    })
    .await;
    assert!(matches!(startup, McpStartupStatus::Ready), "{startup:?}");
    test.submit_turn("Inspect resources with one server.")
        .await?;
    let current = test.codex.config().await;
    let mut next = current.as_ref().clone();
    next.mcp_servers.set(HashMap::new())?;
    assert_eq!(
        test.codex.refresh_mcp_config(current, next).await,
        ConfigRefreshOutcome::Published,
    );
    test.submit_turn("Inspect resources after removing the last server.")
        .await?;
    let requests = mock.requests();
    let tools = requests
        .iter()
        .map(|request| request.body_json()["tools"].clone())
        .collect::<Vec<_>>();
    assert_eq!(requests.len(), 6);
    assert_eq!(tools, vec![tools[0].clone(); requests.len()]);
    for (request, phase) in
        requests
            .iter()
            .skip(1)
            .step_by(2)
            .zip(["before", "registered", "removed"])
    {
        let output = request.custom_tool_call_output(phase).to_string();
        assert!(output.contains(r#"{\"resources\":[]}"#), "{output}");
        assert!(output.contains(r#"{\"resourceTemplates\":[]}"#), "{output}");
        assert!(output.contains("unknown MCP server 'missing'"), "{output}");
    }
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

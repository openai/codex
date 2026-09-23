//! Catalog resource-tool text reaches direct calls and Code Mode without changing MCP execution.

use super::super::rmcp_client::remote_aware_environment_id;
use super::super::rmcp_client::remote_aware_stdio_server_bin;
use anyhow::Result;
use codex_features::Feature;
use codex_protocol::openai_models::ToolMode;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
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
    let test = test_codex()
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
    let mut events = vec![responses::ev_response_created("resources-response")];
    events.extend(if tool_mode == ToolMode::CodeModeOnly {
        let script = calls
            .iter()
            .map(|(name, arguments)| format!("text(await tools.{name}({arguments}));"))
            .collect::<Vec<_>>()
            .join("\n");
        vec![responses::ev_custom_tool_call(
            "resources-call",
            "exec",
            &script,
        )]
    } else {
        calls
            .iter()
            .map(|(name, arguments)| responses::ev_function_call(name, name, arguments))
            .collect()
    });
    events.push(responses::ev_completed("resources-response"));
    let mock = responses::mount_sse_sequence(
        &server,
        vec![responses::sse(events), responses::sse_completed("done")],
    )
    .await;
    test.submit_turn("List the available resources and templates, then read the example note.")
        .await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
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
    assert!(requests[1].body_contains_text("Example Note"));
    assert!(requests[1].body_contains_text("memo://codex/{slug}"));
    assert!(
        requests[1]
            .body_contains_text("This is a sample MCP resource served by the rmcp test server.")
    );
    if tool_mode == ToolMode::CodeModeOnly {
        insta::assert_snapshot!(
            "mcp_resource_messages",
            context_snapshot::format_request_history_snapshot(
                "Catalog resource descriptions and parameter guidance accompany discovery and reading through Code Mode.",
                &requests,
                &ContextSnapshotOptions::default().include_request_settings(),
            )
        );
    }
    Ok(())
}

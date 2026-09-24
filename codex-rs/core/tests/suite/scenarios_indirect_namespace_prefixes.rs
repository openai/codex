//! Catalog namespace and MCP server prefixes follow the current model without changing dispatch.

use super::super::code_mode::custom_tool_output_last_non_empty_text;
use super::super::rmcp_client::remote_aware_environment_id;
use super::super::rmcp_client::remote_aware_stdio_server_bin;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use test_case::test_case;

#[test_case(ToolMode::CodeModeOnly; "code_mode_only")]
#[test_case(ToolMode::CodeMode; "code_mode_with_direct_tools")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_namespace_prefixes_follow_the_selected_model(
    tool_mode: ToolMode,
) -> anyhow::Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));
    let include_search = tool_mode == ToolMode::CodeMode;
    let requests_per_turn = if include_search { 3 } else { 2 };
    let server = responses::start_mock_server().await;
    let command = remote_aware_stdio_server_bin()?;
    let environment_id = remote_aware_environment_id();
    let mut builder = test_codex().with_config(move |config| {
        super::configure_scenario_catalog(config);
        config
            .features
            .enable(Feature::MultiAgentV2)
            .expect("enable MAv2");
        config.multi_agent_v2.non_code_mode_only = false;
        config.multi_agent_v2.root_agent_usage_hint_text = Some("Coordinate sub-agents.".into());
        config
            .mcp_servers
            .set(HashMap::from([(
                "reports.one".to_string(),
                serde_json::from_value(json!({
                    "command": command,
                    "environment_id": environment_id,
                    "cwd": config.cwd,
                    "env": {"MCP_TEST_SERVER_INSTRUCTIONS": "Reports."},
                    "enabled_tools": ["echo"],
                    "default_tools_approval_mode": "approve",
                }))
                .expect("MCP fixture config"),
            )]))
            .expect("MCP fixture allowed");
    });
    for (slug, prefixes, mcp_prefix) in [
        (
            "prefixes-conflicting",
            json!({"mcp__reports_one": "Different report context."}),
            "Reports context.",
        ),
        (
            "prefixes-cleared",
            json!({"functions": "", "collaboration": ""}),
            "",
        ),
        (
            "gpt-5.5",
            json!({
                "functions": "  Functions context.  ",
                "collaboration": "Agents context.",
            }),
            "Reports context.",
        ),
    ] {
        builder = builder.with_model_info_override(slug, move |model| {
            model.tool_mode = Some(tool_mode);
            model.supports_search_tool = include_search;
            model.model_messages.as_mut().expect("model messages").tools = Some(
                serde_json::from_value(json!({
                    "indirect_description_prefixes": {
                        "namespaces": prefixes,
                        "mcp_servers": {"reports.one": mcp_prefix},
                    },
                }))
                .expect("sparse catalog tool messages"),
            );
        });
    }
    let test = builder.build_with_auto_env(&server).await?;
    wait_for_mcp_server(&test.codex, "reports.one").await?;
    let script = r#"
const descriptions = ["view_image", "collaboration__list_agents", "mcp__reports_one__echo"]
  .map(name => ALL_TOOLS.find(t => t.name === name).description);
const report = await tools.mcp__reports_one__echo({message: "ping"});
const agents = await tools.collaboration__list_agents({});
text({descriptions, echo: report.structuredContent.echo, agent: agents.agents[0].agent_name});
"#;
    let mock = responses::mount_sse_sequence(
        &server,
        [
            ("find", "run", "done"),
            ("find-cleared", "run-cleared", "done-cleared"),
        ]
        .into_iter()
        .flat_map(|(find, run, done)| {
            let mut responses = Vec::new();
            if include_search {
                responses.push(responses::sse(vec![
                    responses::ev_tool_search_call(find, &json!({"query": "echo"})),
                    responses::ev_completed(find),
                ]));
            }
            responses.extend([
                responses::sse(vec![
                    responses::ev_custom_tool_call(run, "exec", script),
                    responses::ev_completed(run),
                ]),
                responses::sse(vec![
                    responses::ev_assistant_message(done, "Done."),
                    responses::ev_completed(done),
                ]),
            ]);
            responses
        })
        .collect(),
    )
    .await;
    test.submit_turn("Find echo, call it, and list the agents.")
        .await?;
    test.codex
        .submit(Op::ThreadSettings {
            thread_settings: ThreadSettingsOverrides {
                model: Some("prefixes-cleared".to_string()),
                ..Default::default()
            },
            reply: None,
        })
        .await?;
    test.submit_text_turn("Find echo again, call it, and list the agents.")
        .await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), requests_per_turn * 2);
    let prefixes = ["Functions context.", "Agents context.", "Reports context."];
    let direct_specs = [0, requests_per_turn].map(|index| {
        let mut tools = requests[index].body_json()["tools"]
            .as_array()
            .expect("tools")
            .clone();
        assert_eq!(
            tools.iter().any(|tool| tool["type"] == "tool_search"),
            include_search,
        );
        let exec = tools.remove(
            tools
                .iter()
                .position(|tool| tool["name"] == "exec")
                .expect("exec"),
        );
        let description = exec["description"].as_str().expect("description");
        for &prefix in &prefixes {
            assert_eq!(
                description.matches(prefix).count(),
                usize::from(!include_search && index == 0),
                "{description}"
            );
        }
        assert!(
            prefixes
                .iter()
                .all(|prefix| !json!(tools).to_string().contains(*prefix))
        );
        tools
    });
    assert_eq!(direct_specs[0], direct_specs[1]);
    if include_search {
        let loaded = requests[1].tool_search_output("find");
        let mut cleared = requests[requests_per_turn + 1].tool_search_output("find-cleared");
        assert_eq!(cleared["tools"][0]["description"], "Reports.");
        cleared["tools"][0]["description"] = json!("Reports context.\n\nReports.");
        assert_eq!(loaded["tools"], cleared["tools"]);
    }
    let mut outputs = Vec::new();
    for (index, call_id) in [
        (requests_per_turn - 1, "run"),
        (requests_per_turn * 2 - 1, "run-cleared"),
    ] {
        let output: Value = serde_json::from_str(
            &custom_tool_output_last_non_empty_text(&requests[index], call_id)
                .expect("exec JSON output"),
        )?;
        assert_eq!(
            (&output["echo"], &output["agent"]),
            (&json!("ECHOING: ping"), &json!("/root")),
        );
        outputs.push(output);
    }
    let mut expected = outputs[1].clone();
    for (description, prefix) in expected["descriptions"]
        .as_array_mut()
        .expect("tool descriptions")
        .iter_mut()
        .zip(prefixes)
    {
        *description = json!(format!(
            "{prefix}\n\n{}",
            description.as_str().expect("tool description")
        ));
    }
    assert_eq!(outputs[0], expected);
    if tool_mode == ToolMode::CodeModeOnly {
        insta::assert_snapshot!(
            "catalog_indirect_namespace_prefixes",
            context_snapshot::format_request_history_snapshot(
                "Catalog namespace and raw MCP server prefixes annotate embedded docs and ALL_TOOLS; a model switch clears them without changing nested dispatch.",
                &requests,
                &ContextSnapshotOptions::default()
                    .rewrite_known_segments()
                    .include_request_settings(),
            )
        );
    }
    if include_search {
        test.codex
            .start_or_steer_turn(
                TurnInputRequest::user_input(vec![UserInput::Text {
                    text: "Find echo with conflicting prefixes.".to_string(),
                    text_elements: Vec::new(),
                }])
                .with_thread_settings(ThreadSettingsOverrides {
                    model: Some("prefixes-conflicting".to_string()),
                    ..Default::default()
                }),
            )
            .await?;
        let EventMsg::TurnComplete(completed) = wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await
        else {
            unreachable!("event predicate guarantees turn completion");
        };
        assert_eq!(
            completed
                .error
                .expect("conflicting prefixes must fail the turn")
                .message,
            "Conflicting indirect description prefixes for namespace `mcp__reports_one`: `namespaces.mcp__reports_one` and `mcp_servers.reports.one`",
        );
        assert_eq!(
            server
                .received_requests()
                .await
                .expect("received requests")
                .iter()
                .filter(|request| request.url.path() == "/v1/responses")
                .count(),
            requests.len(),
            "conflicting prefixes must fail before another model request",
        );
    }
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

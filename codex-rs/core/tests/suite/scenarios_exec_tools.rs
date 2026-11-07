//! Exec tools append changed entries and refresh frozen exec sections only at compaction.

use anyhow::Result;
use codex_features::Feature;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_mcp_server;
use core_test_support::wait_for_successful_turn;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;

use super::super::rmcp_client::remote_aware_environment_id;
use super::super::rmcp_client::remote_aware_stdio_server_bin;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn changed_sections_and_entries_append_without_repeating_the_catalog() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "requires a Windows test_stdio_server fixture");
    let server = responses::start_mock_server().await;
    let command = remote_aware_stdio_server_bin()?;
    let environment_id = remote_aware_environment_id();
    let server_config = |tools: &[&str], instructions: &str, omitted: &[&str]| {
        serde_json::from_value(json!({
            "command": command,
            "environment_id": environment_id,
            "env": {"MCP_TEST_SERVER_INSTRUCTIONS": instructions},
            "enabled_tools": tools,
            "omit_tools_from": omitted,
            "default_tools_approval_mode": "approve",
        }))
    };
    let original_instructions = "Echo service instructions.";
    let updated_instructions = "Echo service instructions. Durations are milliseconds.";
    let initial_server = server_config(&["echo"], original_instructions, &["deferred"])?;
    let mut builder = test_codex()
        .with_config(move |config| {
            super::configure_scenario_catalog(config);
            for feature in [
                Feature::IncrementalTools,
                Feature::IncrementalExecTools,
                Feature::DeferredToolWorldState,
            ] {
                config
                    .features
                    .enable(feature)
                    .expect("enable catalog updates");
            }
            config
                .features
                .disable(Feature::CodeModeOnlyStrictThirdPartyTools)
                .expect("allow eager MCP tools");
            config.code_mode.disable_in_process_fallback = true;
            config
                .mcp_servers
                .set(HashMap::from([(
                    "echo_service".to_string(),
                    initial_server,
                )]))
                .expect("set MCP fixture");
        })
        .with_model_info_override("catalog-initial", |model| {
            model.use_responses_lite = true;
            model.tool_mode = Some(ToolMode::CodeModeOnly);
            model.supports_search_tool = true;
        });
    for (slug, description, deferred_guidance, shared_types) in [
        (
            "catalog-updated",
            "Run JavaScript. Await every tool call before returning.",
            "Find deferred tools in ALL_TOOLS.",
            "type CallToolResult<T = unknown> = { content: unknown[]; structuredContent?: T; isError?: boolean };",
        ),
        (
            "catalog-compacted",
            "Run JavaScript. Await every tool call and inspect its result before returning.",
            "Find deferred tools by name in ALL_TOOLS.",
            "type CallToolResult<T = unknown> = { content: unknown[]; structuredContent?: T; isError?: boolean; _meta?: unknown };",
        ),
    ] {
        builder = builder.with_model_info_override(slug, move |model| {
            model.use_responses_lite = true;
            model.tool_mode = Some(ToolMode::CodeModeOnly);
            model.supports_search_tool = true;
            model.model_messages.as_mut().expect("model messages").tools = Some(
                serde_json::from_value(json!({
                    "code_mode": {
                        "exec": {"description": description},
                        "deferred_nested_tools_guidance": deferred_guidance,
                        "mcp_typescript_preamble": shared_types,
                    },
                    "mcp_resources": {"read_mcp_resource": {
                        "description": "Read a selected resource revision.",
                        "parameters": json!({
                            "type": "object",
                            "properties": {"server": {"type": "string"}, "uri": {"type": "string"}, "revision": {"type": "integer"}},
                            "required": ["server", "uri"],
                            "additionalProperties": false,
                        }).to_string(),
                    }},
                })).expect("updated tool messages"),
            );
        });
    }
    let test = builder
        .with_model("catalog-initial")
        .build_with_auto_env(&server)
        .await?;
    wait_for_mcp_server(&test.codex, "echo_service").await?;
    let mut events = (0..12)
        .map(|index| responses::sse_completed(&format!("response-{index}")))
        .collect::<Vec<_>>();
    events.push(responses::sse(vec![
        json!({"type": "response.output_item.done", "item": {
            "type": "compaction", "encrypted_content": "CATALOG_CHECKPOINT",
        }}),
        responses::ev_completed("response-12"),
    ]));
    events.push(responses::sse_completed("response-13"));
    let mock = responses::mount_sse_sequence(&server, events).await;
    test.submit_text_turn("Initial eager catalog: echo.")
        .await?;
    // Refresh through the public MCP configuration and call boundaries, as production does.
    let ready = Some(json!({"message": "catalog ready"}));
    for (tools, instructions, prompt) in [
        (
            vec!["echo", "cwd"],
            original_instructions,
            "Nested tool addition: cwd.",
        ),
        (
            vec!["echo", "cwd"],
            updated_instructions,
            "Namespace instructions redefinition: durations.",
        ),
        (
            vec!["echo"],
            updated_instructions,
            "Nested tool removal: cwd.",
        ),
    ] {
        let current = test.codex.config().await;
        let mut updated = current.as_ref().clone();
        updated.mcp_servers.set(HashMap::from([(
            "echo_service".to_string(),
            server_config(&tools, instructions, &["deferred"])?,
        )]))?;
        let _ = test.codex.refresh_mcp_config(current, updated).await;
        test.codex
            .call_mcp_tool("echo_service", "echo", ready.clone(), /*meta*/ None)
            .await?;
        test.submit_text_turn(prompt).await?;
    }
    test.submit_text_turn("Unchanged catalog: emit no update.")
        .await?;

    for (tools, instructions, omitted, prompt) in [
        (
            vec!["echo"],
            "",
            vec!["deferred"],
            "Namespace instructions removal: keep echo.",
        ),
        (vec!["echo"], "", vec![], "Eager to deferred: echo."),
        (
            vec!["echo"],
            updated_instructions,
            vec!["deferred"],
            "Deferred to eager: echo.",
        ),
        (vec![], "", vec![], "Remove all echo service entries."),
        (
            vec!["echo", "cwd"],
            updated_instructions,
            vec!["deferred"],
            "Re-enable echo service entries: echo and cwd.",
        ),
    ] {
        let current = test.codex.config().await;
        let mut updated = current.as_ref().clone();
        updated.mcp_servers.set(if tools.is_empty() {
            HashMap::new()
        } else {
            HashMap::from([(
                "echo_service".to_string(),
                server_config(&tools, instructions, &omitted)?,
            )])
        })?;
        let _ = test.codex.refresh_mcp_config(current, updated).await;
        let result = test
            .codex
            .call_mcp_tool("echo_service", "echo", ready.clone(), /*meta*/ None)
            .await;
        if tools.is_empty() {
            assert!(result.is_err(), "removed tools must not remain callable");
        } else {
            result?;
        }
        test.submit_text_turn(prompt).await?;
    }
    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some("catalog-updated".to_string()),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn(
        "Combined redefinition: resource revision, common instructions, and shared types.",
    )
    .await?;
    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some("catalog-compacted".to_string()),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("Only common instructions and shared types change: wait for compaction.")
        .await?;
    test.codex.submit(Op::Compact).await?;
    wait_for_successful_turn(&test.codex).await;
    test.submit_text_turn("After compaction: restore the full current catalog.")
        .await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 14);
    let mut previous = Vec::new();
    let mut declarations = Vec::new();
    // Snapshots normalize IDs and metadata, so check the retained raw prefix separately.
    for (index, request) in requests.iter().enumerate() {
        let input = request.input();
        let suffix = if index == 13 {
            input.as_slice()
        } else {
            assert_eq!(
                &input[..previous.len()],
                previous.as_slice(),
                "retained context changed"
            );
            &input[previous.len()..]
        };
        declarations.push(
            suffix
                .iter()
                .filter(|item| item["type"] == "additional_tools")
                .filter_map(|item| responses::namespace_child_tool(item, "functions", "exec"))
                .cloned()
                .collect::<Vec<_>>(),
        );
        previous = input;
    }
    // Native declarations are fingerprinted in snapshots; keep their invariants readable here.
    let mut native_fields = declarations[0][0].clone();
    native_fields["description"] = json!(null);
    for exec in declarations.iter().flatten() {
        let mut actual = exec.clone();
        actual["description"] = json!(null);
        assert_eq!(actual, native_fields, "compaction preserves native fields");
    }
    for request in &requests[10..13] {
        let retained_input = serde_json::to_string(&request.input())?;
        for frozen in [
            "Run JavaScript. Await every tool call",
            "Find deferred tools in ALL_TOOLS.",
            "Find deferred tools by name in ALL_TOOLS.",
            "content: unknown[]; structuredContent?: T; isError?: boolean",
        ] {
            assert!(
                !retained_input.contains(frozen),
                "frozen documentation appeared before compaction: {frozen}"
            );
        }
    }
    let restored = declarations[13][0]["description"]
        .as_str()
        .expect("compacted catalog");
    for expected in [
        "Await every tool call and inspect its result",
        "Find deferred tools by name in ALL_TOOLS.",
        "isError?: boolean; _meta?: unknown",
        "revision?: number",
        "### `mcp__echo_service__echo`",
        "### `mcp__echo_service__cwd`",
        updated_instructions,
    ] {
        assert!(restored.contains(expected), "compaction omitted {expected}");
    }
    let snapshot = context_snapshot::format_request_history_snapshot(
        "Code Mode appends complete changed tool entries and namespace instructions in combined developer messages. Namespace clearing, eager/deferred movement, remove-all and re-enabling preserve context. Common instructions and shared types stay frozen while nested methods update; compaction installs the latest full catalog.",
        &requests,
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    );
    insta::assert_snapshot!("incremental_exec_tools", snapshot);
    Ok(())
}

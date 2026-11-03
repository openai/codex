//! A server named "environment" adds, replaces, then removes MCP tools between turns.
//! The execution environment runs throughout; this tests MCP refresh, not executor shutdown.

use anyhow::Result;
use codex_core::ConfigRefreshOutcome;
use codex_features::Feature;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::McpStartupStatus;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn strict_3p_mcp_refresh_preserves_cache_while_exec_tracks_available_tools() -> Result<()> {
    let server = responses::start_mock_server().await;
    let first_mcp = responses::start_mock_server().await;
    let second_mcp = responses::start_mock_server().await;
    let initial = AppsTestServer::mount_with_tools(
        &first_mcp,
        Arc::new(Mutex::new(vec![json!({
            "name": "lookup",
            "description": "First environment tool.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
        })])),
    )
    .await?;
    let changed = AppsTestServer::mount_with_tools(
        &second_mcp,
        Arc::new(Mutex::new(vec![
            json!({
                "name": "search",
                "description": "Replacement environment tool.",
                "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
            }),
            json!({
                "name": "resolve",
                "description": "Additional tool from the refreshed environment.",
                "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
            }),
        ])),
    )
    .await?;
    let test = test_codex()
        .with_config(|config| {
            super::configure_scenario_catalog(config);
            config.code_mode.disable_in_process_fallback = true;
            config.features.enable(Feature::CodeModeHost).unwrap();
            config
                .features
                .enable(Feature::CodeModeOnlyStrictThirdPartyTools)
                .unwrap();
            config.code_mode.direct_only_tool_namespaces = vec!["mcp__environment".to_string()];
            config.code_mode.excluded_tool_namespaces = vec!["mcp__environment".to_string()];
        })
        .with_model_info_override("gpt-6-astra", |model| {
            model.tool_mode = Some(ToolMode::CodeModeOnly);
            model.use_responses_lite = true;
            model.supports_search_tool = false;
        })
        .build_with_auto_env(&server)
        .await?;

    let script = r#"const available = ALL_TOOLS.filter(t => t.name.startsWith("mcp__environment__")).sort((a, b) => a.name.localeCompare(b.name));
const results = [];
for (const t of available) {
  const result = await tools[t.name]({});
  results.push({name: t.name, result: result.content[0].text});
}
text(JSON.stringify(results));"#;
    let mut events = Vec::new();
    for phase in ["before", "ready", "changed", "gone"] {
        events.push(responses::sse(vec![
            responses::ev_custom_tool_call(phase, "exec", script),
            responses::ev_completed(phase),
        ]));
        events.push(responses::sse_completed(&format!("{phase}-done")));
    }
    let mock = responses::mount_sse_sequence(&server, events).await;

    test.submit_turn("List and call the tools provided by the environment MCP server.")
        .await?;
    for endpoint in [
        Some(initial.chatgpt_base_url),
        Some(changed.chatgpt_base_url),
        None,
    ] {
        let current = test.codex.config().await;
        let mut next = current.as_ref().clone();
        let mut servers = HashMap::new();
        if let Some(url) = &endpoint {
            servers.insert(
                "environment".to_string(),
                serde_json::from_value(json!({
                    "url": format!("{url}/api/codex/ps/mcp"),
                    "omit_tools_from": ["direct", "deferred", "code_mode"],
                }))?,
            );
        }
        next.mcp_servers.set(servers)?;
        assert_eq!(
            test.codex.refresh_mcp_config(current, next).await,
            ConfigRefreshOutcome::Published,
        );
        if endpoint.is_some() {
            let status = wait_for_event_match(&test.codex, |event| match event {
                EventMsg::McpStartupUpdate(update)
                    if update.server == "environment"
                        && !matches!(update.status, McpStartupStatus::Starting) =>
                {
                    Some(update.status.clone())
                }
                _ => None,
            })
            .await;
            assert!(matches!(status, McpStartupStatus::Ready), "{status:?}");
        }
        test.submit_turn("List and call the tools provided by the environment MCP server.")
            .await?;
    }

    let requests = mock.requests();
    // Each of four turns needs two requests: one calls exec, the next includes its output.
    assert_eq!(requests.len(), 8);
    let prefix = requests[0].inputs_of_type("additional_tools");
    assert!(
        !prefix.is_empty(),
        "Responses Lite must supply a real tool prefix"
    );
    for (index, request) in requests.iter().enumerate() {
        assert_eq!(
            request.inputs_of_type("additional_tools"),
            prefix,
            "request {}: MCP registration or refresh changed the eager tool prefix",
            index + 1,
        );
    }
    for (request, (phase, expected)) in requests.iter().skip(1).step_by(2).zip([
        ("before", json!([])),
        (
            "ready",
            json!([{"name": "mcp__environment__lookup", "result": "called lookup for  at  with "}]),
        ),
        (
            "changed",
            json!([
                {"name": "mcp__environment__resolve", "result": "called resolve for  at  with "},
                {"name": "mcp__environment__search", "result": "called search for  at  with "},
            ]),
        ),
        ("gone", json!([])),
    ]) {
        let output =
            super::super::code_mode::custom_tool_output_last_non_empty_text(request, phase)
                .expect("exec reports the current MCP catalog and dispatched tool results");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&output)?,
            expected,
            "phase {phase}: exec must discover and call exactly the currently registered MCP tools",
        );
    }
    let labels = [
        "turn 1: no MCP server registered",
        "turn 1: exec discovery and call results",
        "turn 2: register MCP with lookup; omit-all, direct-only and excluded",
        "turn 2: exec discovery and call results",
        "turn 3: same MCP name, new endpoint; replace lookup with resolve and search",
        "turn 3: exec discovery and call results",
        "turn 4: remove MCP server from config",
        "turn 4: exec discovery and call results",
    ];
    let sections = labels.into_iter().zip(&requests).collect::<Vec<_>>();
    let snapshot = context_snapshot::format_labeled_requests_snapshot(
        "Strict Code Mode Only: MCP config changes between turns, with omit-all and namespace skips. Exec calls the live catalog. One window means the captured prefix stayed stable. The execution environment remains running.",
        &sections,
        &ContextSnapshotOptions::default()
            .rewrite_known_segments()
            .include_request_settings(),
    );
    // The starting exec schema varies across Cargo and Bazel builds. The assertions above
    // still compare every real prefix byte; the formatter has already grouped the windows.
    let snapshot = regex_lite::Regex::new(
        r"(?m)(^00:additional_tools/developer \(\d+; hash=|^ +- namespace/functions; hash=)[0-9a-f]{16}",
    )?
    .replace_all(&snapshot, "${1}<INITIAL_BUILD_HASH>");
    insta::assert_snapshot!("strict_3p_mcp_catalog_cache", snapshot);
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

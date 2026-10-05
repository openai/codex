//! Request-history coverage for opt-in incremental tools on Responses Lite.

use anyhow::Result;
use codex_core::ConfigRefreshOutcome;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_protocol::openai_models::CodeModeToolMessages;
use codex_protocol::openai_models::ToolMessage;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::ThreadSettingsOverrides;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;

use super::super::rmcp_client::remote_aware_environment_id;
use super::super::rmcp_client::remote_aware_stdio_server_bin;

#[test_case(true; "responses_lite")]
#[test_case(false; "responses_api")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn incremental_tools_append_changed_catalog_without_rewriting_history(
    use_responses_lite: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_sequence(
        &server,
        (1..=4)
            .map(|index| responses::sse(vec![responses::ev_completed(&format!("resp-{index}"))]))
            .collect(),
    )
    .await;
    let test = test_codex()
        .with_model_info_override("gpt-5.4", move |model| {
            model.use_responses_lite = use_responses_lite;
            model.tool_mode = Some(ToolMode::CodeMode);
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::IncrementalTools)
                .expect("enable incremental tools");
            config.base_instructions =
                Some("Use the available tools to help the user.".to_string());
            config.code_mode.disable_in_process_fallback = true;
            let catalog = config.model_catalog.as_mut().expect("model catalog");
            let mut updated = catalog
                .models
                .iter()
                .find(|model| model.slug == "gpt-5.4")
                .expect("source model")
                .clone();
            updated.slug = "updated-tools-model".to_string();
            updated
                .model_messages
                .get_or_insert_default()
                .tools
                .get_or_insert_default()
                .code_mode = Some(CodeModeToolMessages {
                exec: Some(ToolMessage {
                    description: Some("Updated execution instructions.".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            });
            catalog.models.push(updated);
        })
        .build_with_auto_env(&server)
        .await?;
    test.submit_turn("Begin work.").await?;
    test.submit_turn("Continue with the same tools.").await?;
    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some("updated-tools-model".to_string()),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("Continue with the updated execution instructions.")
        .await?;
    test.submit_text_turn("Continue with the same updated tools.")
        .await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 4);
    if !use_responses_lite {
        for request in &requests {
            let body = request.body_json();
            assert!(
                body["tools"]
                    .as_array()
                    .is_some_and(|tools| !tools.is_empty())
            );
            assert_eq!(
                request.instructions_text(),
                "Use the available tools to help the user."
            );
        }
        assert_eq!(
            requests[0].body_json()["tools"],
            requests[1].body_json()["tools"]
        );
        assert_ne!(
            requests[1].body_json()["tools"],
            requests[2].body_json()["tools"]
        );
        assert_eq!(
            requests[2].body_json()["tools"],
            requests[3].body_json()["tools"]
        );
        return Ok(());
    }
    assert!(requests[1].input().starts_with(&requests[0].input()));
    assert!(requests[2].input().starts_with(&requests[1].input()));
    assert!(requests[3].input().starts_with(&requests[2].input()));
    assert!(
        requests
            .iter()
            .all(|request| request.body_json().get("tools").is_none())
    );
    for request in &requests {
        let input = request.input();
        assert_eq!(input[0]["type"], "additional_tools");
        assert_eq!(input[1]["role"], "developer");
        assert_eq!(
            input[1]["content"][0]["text"],
            "Use the available tools to help the user."
        );
        assert!(request.body_json().get("instructions").is_none());
    }
    let initial = requests[0].inputs_of_type("additional_tools");
    assert_eq!(initial.len(), 1);
    assert_eq!(
        initial[0]["tools"]
            .as_array()
            .expect("initial tool declarations")
            .iter()
            .map(|tool| tool["type"].clone())
            .collect::<Vec<_>>(),
        vec![json!("namespace"), json!("tool_search")]
    );
    assert_eq!(requests[1].inputs_of_type("additional_tools"), initial);
    let changed = requests[2].inputs_of_type("additional_tools");
    assert_eq!(requests[2].body_json()["model"], "updated-tools-model");
    assert_eq!(&changed[..initial.len()], &initial);
    assert_eq!(changed.len(), initial.len() + 1);
    let diff = changed.last().expect("changed tool declarations")["tools"]
        .as_array()
        .expect("changed tool array");
    assert_eq!(diff.len(), 1);
    assert_eq!(diff[0]["name"], "functions");
    assert_eq!(initial[0]["tools"][0]["description"], json!(""));
    assert_eq!(
        diff[0]["description"],
        json!(
            "This is an incremental namespace update. Previously declared tools remain available for direct calls unless explicitly marked unavailable. If a tool is redefined here, its latest definition replaces the earlier one."
        )
    );
    let changed_tools = diff[0]["tools"].as_array().expect("namespace members");
    assert_eq!(changed_tools.len(), 1);
    assert_eq!(changed_tools[0]["name"], "exec");
    assert!(
        changed
            .last()
            .expect("changed tool declarations")
            .to_string()
            .contains("Updated execution instructions.")
    );
    assert_eq!(requests[3].inputs_of_type("additional_tools"), changed);

    test.codex.ensure_rollout_materialized().await;
    test.codex.flush_rollout().await?;
    let rollout =
        tokio::fs::read_to_string(test.codex.rollout_path().expect("rollout path")).await?;
    let catalogs = rollout
        .lines()
        .map(codex_rollout::parse_rollout_line)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter_map(|line| match line.item {
            RolloutItem::WorldState(item) => item.state.get("top_level_tools").cloned(),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(catalogs.len(), 2);
    assert_ne!(catalogs[0], catalogs[1]);
    assert_eq!(
        catalogs[1]
            .as_object()
            .expect("updated tool catalog")
            .iter()
            .filter(|(key, hash)| catalogs[0].get(*key) != Some(*hash))
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["functions.exec"]
    );
    assert!(catalogs.iter().all(|catalog| {
        catalog
            .as_object()
            .expect("tool catalog")
            .values()
            .all(|hash| hash.as_str().is_some_and(|hash| hash.len() == 40))
    }));
    insta::assert_snapshot!(
        "incremental_tools",
        context_snapshot::format_request_history_snapshot(
            "Tool definitions enter history in one batch; a catalog change appends only the changed exec definition with an incremental namespace hint. The next unchanged turn appends no tool definitions.",
            &requests,
            &ContextSnapshotOptions::default()
                .rewrite_known_segments()
                .include_request_settings(),
        )
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_removal_appends_a_standalone_notice_without_rewriting_history() -> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let command = remote_aware_stdio_server_bin()?;
    let environment_id = remote_aware_environment_id();
    let server_config = |enabled_tools: &[&str]| {
        serde_json::from_value(json!({
            "command": command,
            "environment_id": environment_id,
            "env": {"MCP_TEST_SERVER_INSTRUCTIONS": "Echo service instructions."},
            "enabled_tools": enabled_tools,
            "default_tools_approval_mode": "approve",
        }))
    };
    let initial_server = server_config(&["echo", "cwd"])?;
    let reduced_server = server_config(&["echo"])?;
    let test = test_codex()
        .with_model_info_override("gpt-5.5", |model| {
            model.use_responses_lite = true;
            model.supports_search_tool = false;
            model.tool_mode = Some(ToolMode::Direct);
        })
        .with_config(move |config| {
            config
                .features
                .enable(Feature::IncrementalTools)
                .expect("enable incremental tools");
            config
                .mcp_servers
                .set(std::collections::HashMap::from([(
                    "echo_service".to_string(),
                    initial_server,
                )]))
                .expect("set MCP fixture");
        })
        .build_with_auto_env(&server)
        .await?;
    wait_for_mcp_server(&test.codex, "echo_service").await?;
    let mock = responses::mount_sse_sequence(
        &server,
        (1..=3)
            .map(|index| responses::sse(vec![responses::ev_completed(&format!("resp-{index}"))]))
            .collect(),
    )
    .await;
    test.submit_text_turn("Begin with the echo service.")
        .await?;

    // Remove one member while keeping the namespace instructions and echo schema unchanged.
    let current_config = test.codex.config().await;
    let mut reduced_config = current_config.as_ref().clone();
    reduced_config
        .mcp_servers
        .set(std::collections::HashMap::from([(
            "echo_service".to_string(),
            reduced_server,
        )]))?;
    assert_eq!(
        test.codex
            .refresh_mcp_config(current_config, reduced_config)
            .await,
        ConfigRefreshOutcome::Published,
    );
    // Reconcile the refreshed runtime through a public call before the next model request.
    let result = test
        .codex
        .call_mcp_tool(
            "echo_service",
            "echo",
            Some(json!({"message": "ready after removal"})),
            /*meta*/ None,
        )
        .await?;
    assert_ne!(result.is_error, Some(true));
    test.submit_text_turn("Continue after disabling the cwd tool.")
        .await?;
    test.submit_text_turn("Continue with the remaining echo tool.")
        .await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 3);
    assert!(requests[1].has_content_kinds(&["tools.removed_definition"]));
    let initial_tools = requests[0].inputs_of_type("additional_tools");
    for pair in requests.windows(2) {
        assert!(pair[1].input().starts_with(&pair[0].input()));
        assert_eq!(pair[1].inputs_of_type("additional_tools"), initial_tools);
    }
    let removed_input = requests[1].input();
    let updates = &removed_input[requests[0].input().len()..];
    assert_eq!(
        updates.len(),
        2,
        "removal appends a standalone notice and user input"
    );
    assert_eq!(updates[0]["role"], "developer");
    assert_eq!(
        updates[0]["content"],
        json!([{
            "type": "input_text",
            "text": "The following tools are no longer available. Do not call them:\n- mcp__echo_service.cwd",
        }])
    );
    let unchanged_input = requests[2].input();
    let follow_up = &unchanged_input[removed_input.len()..];
    assert_eq!(follow_up.len(), 1, "unchanged turn appends only user input");
    assert_eq!(follow_up[0]["role"], "user");
    assert_eq!(
        follow_up[0]["content"],
        json!([{"type": "input_text", "text": "Continue with the remaining echo tool."}])
    );
    insta::assert_snapshot!(
        "incremental_tool_removals",
        context_snapshot::format_request_history_snapshot(
            "An MCP configuration refresh removes one tool while preserving its namespace and other member. A standalone removal notice preserves earlier declarations and is not repeated on the next turn.",
            &requests,
            &ContextSnapshotOptions::default()
                .rewrite_known_segments()
                .include_request_settings(),
        )
    );
    Ok(())
}

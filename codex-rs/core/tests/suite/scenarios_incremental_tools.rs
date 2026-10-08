//! Request-history coverage for opt-in incremental tools on Responses Lite.

use anyhow::Result;
use codex_features::Feature;
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
    insta::assert_snapshot!(
        if use_responses_lite {
            "incremental_tools"
        } else {
            "incremental_tools_responses_api"
        },
        context_snapshot::format_request_history_snapshot(
            if use_responses_lite {
                "Tool definitions enter history in one batch; a catalog change appends one developer notice before only the changed exec definition, without modifying namespace descriptions. The next unchanged turn appends only user input."
            } else {
                "Regular Responses requests carry the full current tool catalog without an incremental notice; changed execution instructions replace the catalog on subsequent requests."
            },
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
    let restored_server = server_config(&["echo", "cwd"])?;
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
        (1..=7)
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
    let _ = test
        .codex
        .refresh_mcp_config(current_config, reduced_config)
        .await;
    // Reconcile the refreshed runtime through a public call before the next model request.
    test.codex
        .call_mcp_tool(
            "echo_service",
            "echo",
            Some(json!({"message": "ready after removal"})),
            /*meta*/ None,
        )
        .await?;
    test.submit_text_turn("Continue after disabling the cwd tool.")
        .await?;
    test.submit_text_turn("Continue with the remaining echo tool.")
        .await?;

    // Remove the whole server, then reconcile even though its tools can no longer be called.
    let current_config = test.codex.config().await;
    let mut removed_config = current_config.as_ref().clone();
    removed_config.mcp_servers.set(Default::default())?;
    let _ = test
        .codex
        .refresh_mcp_config(current_config, removed_config)
        .await;
    let _ = test
        .codex
        .call_mcp_tool(
            "echo_service",
            "echo",
            Some(json!({"message": "unavailable"})),
            /*meta*/ None,
        )
        .await;
    test.submit_text_turn("Continue after removing the echo service.")
        .await?;
    test.submit_text_turn("Continue without the echo service.")
        .await?;

    // Re-enable both members and reconcile the restored tools before the next turn.
    let current_config = test.codex.config().await;
    let mut restored_config = current_config.as_ref().clone();
    restored_config
        .mcp_servers
        .set(std::collections::HashMap::from([(
            "echo_service".to_string(),
            restored_server,
        )]))?;
    let _ = test
        .codex
        .refresh_mcp_config(current_config, restored_config)
        .await;
    for (tool, arguments) in [
        ("echo", json!({"message": "ready after restoration"})),
        ("cwd", json!({})),
    ] {
        test.codex
            .call_mcp_tool("echo_service", tool, Some(arguments), /*meta*/ None)
            .await?;
    }
    test.submit_text_turn("Continue with the restored echo service.")
        .await?;
    test.submit_text_turn("Continue with the same restored tools.")
        .await?;

    let requests = mock.requests();
    insta::assert_snapshot!(
        "incremental_tool_removals",
        context_snapshot::format_request_history_snapshot(
            "MCP refreshes remove one tool, then its whole namespace, before restoring both tools. Removal notices append alone; restoration appends one developer notice before both namespace declarations. Unchanged turns do not repeat updates.",
            &requests,
            &ContextSnapshotOptions::default()
                .rewrite_known_segments()
                .include_request_settings(),
        )
    );
    Ok(())
}

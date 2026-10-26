//! Request-history coverage for incremental tools on Responses Lite.

use anyhow::Result;
use codex_core::ForkSnapshot;
use codex_core::McpServersDisabled;
use codex_core::compact::AutoCompactionDisabled;
use codex_features::Feature;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_otel::TOOL_INCREMENTAL_UPDATES_METRIC;
use codex_otel::TOOL_REGISTRATIONS_METRIC;
use codex_otel::TOOL_SPEC_DESCRIPTION_BYTES_METRIC;
use codex_protocol::openai_models::CodeModeToolMessages;
use codex_protocol::openai_models::IncrementalToolMessages;
use codex_protocol::openai_models::ToolMessage;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_mcp_server;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use opentelemetry_sdk::metrics::data::ScopeMetrics;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;
use test_case::test_case;

use super::super::rmcp_client::remote_aware_environment_id;
use super::super::rmcp_client::remote_aware_stdio_server_bin;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prediction_fork_keeps_inherited_tools() -> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let mcp = serde_json::from_value(json!({
        "command": remote_aware_stdio_server_bin()?,
        "environment_id": remote_aware_environment_id(),
        "enabled_tools": ["echo"],
        "required": true,
    }))?;
    let mut test = test_codex()
        .with_model_info_override("gpt-5.4", |model| {
            model.use_responses_lite = true;
            model.supports_search_tool = false;
            model.tool_mode = Some(ToolMode::CodeModeOnly);
        })
        .with_config(move |config| {
            config.workspace_roots = vec![config.cwd.clone()];
            config.base_instructions = Some("Use the available tools to help the user.".into());
            config
                .features
                .enable(Feature::IncrementalTools)
                .expect("enable incremental tools");
            config
                .mcp_servers
                .set(std::collections::HashMap::from([(
                    "echo_service".into(),
                    mcp,
                )]))
                .expect("set MCP server");
        })
        .build_with_auto_env(&server)
        .await?;
    wait_for_mcp_server(&test.codex, "echo_service").await?;
    let mock = responses::mount_sse_sequence(
        &server,
        ["parent", "prediction", "continued-parent"]
            .into_iter()
            .map(|id| responses::sse(vec![responses::ev_completed(id)]))
            .collect(),
    )
    .await;
    test.submit_text_turn("Start with the echo service.")
        .await?;

    let parent = std::sync::Arc::clone(&test.codex);
    parent.flush_rollout().await?;
    let (mut options, settings) = test
        .thread_manager
        .fork_options_from_parent(test.session_configured.thread_id)
        .await?;
    // Match experimentalPredictionMode with experimentalDisablePredictionMcp enabled.
    options.config.ephemeral = true;
    options
        .config
        .features
        .enable(Feature::ReasoningEffortOverride)?;
    options.thread_extension_init.insert(AutoCompactionDisabled);
    options.thread_extension_init.insert(McpServersDisabled);
    let child = test
        .thread_manager
        .fork_legacy_thread(
            ForkSnapshot::Interrupted,
            options,
            parent.rollout_path().expect("parent rollout"),
        )
        .await?;
    child.thread.restore_thread_settings(settings).await?;
    test.codex = child.thread;
    test.submit_text_turn("Predict the next message.").await?;
    test.codex = parent;
    test.submit_text_turn("Continue using the echo service.")
        .await?;

    let requests = mock.requests();
    let parent = requests[0].body_json();
    let child = requests[1].body_json();
    let prefix = parent["input"].as_array().expect("parent input");
    let exec = prefix
        .iter()
        .filter(|item| item["type"] == "additional_tools")
        .find_map(|item| responses::namespace_child_tool(item, "functions", "exec"))
        .expect("parent declares functions.exec");
    assert!(
        exec["description"]
            .as_str()
            .expect("exec description")
            .contains("mcp__echo_service__echo"),
        "the inherited functions.exec must describe the MCP helper",
    );
    assert_eq!(
        &child["input"].as_array().expect("child input")[..prefix.len()],
        prefix
    );
    insta::assert_snapshot!(
        "prediction_fork_inherited_tools",
        context_snapshot::format_request_history_snapshot(
            "Request 1 starts the parent thread. Request 2 is the prediction child's only turn. Request 3 continues the original parent and verifies that it still has its MCP tools.",
            &requests,
            &ContextSnapshotOptions::default()
                .rewrite_known_segments()
                .include_request_settings(),
        )
    );
    Ok(())
}

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
        (1..=if use_responses_lite { 6 } else { 4 })
            .map(|index| {
                let mut events = Vec::new();
                if index == 5 {
                    events.push(json!({
                        "type": "response.output_item.done",
                        "item": {
                            "type": "compaction",
                            "encrypted_content": "INCREMENTAL_TOOLS_CHECKPOINT",
                        },
                    }));
                }
                events.push(responses::ev_completed(&format!("resp-{index}")));
                responses::sse(events)
            })
            .collect(),
    )
    .await;
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory(
            "test",
            "codex-core",
            env!("CARGO_PKG_VERSION"),
            InMemoryMetricExporter::default(),
        )
        .with_runtime_reader(),
    )?;
    let mut test = test_codex()
        .with_model_info_override("gpt-5.4", move |model| {
            model.use_responses_lite = use_responses_lite;
            model.tool_mode = Some(ToolMode::CodeMode);
            if use_responses_lite {
                model
                    .model_messages
                    .as_mut()
                    .expect("model messages")
                    .instructions_template =
                    Some("Use the available tools to help the user.".to_string());
            }
        })
        .with_config(move |config| {
            if !use_responses_lite {
                config.base_instructions =
                    Some("Use the available tools to help the user.".to_string());
            }
            config.code_mode.disable_in_process_fallback = true;
            let catalog = config.model_catalog.as_mut().expect("model catalog");
            let mut updated = catalog
                .models
                .iter()
                .find(|model| model.slug == "gpt-5.4")
                .expect("source model")
                .clone();
            updated.slug = "updated-tools-model".to_string();
            if use_responses_lite {
                updated
                    .model_messages
                    .as_mut()
                    .expect("model messages")
                    .instructions_template = Some("Review results before responding.".to_string());
            }
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
    let mut options = test.start_thread_options().await;
    options.thread_extension_init.insert(metrics.clone());
    let thread = test.thread_manager.start_thread(options).await?;
    test.codex.shutdown_and_wait().await?;
    test.codex = thread.thread;
    test.session_configured = thread.session_configured;
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
    if use_responses_lite {
        test.codex.submit(Op::Compact).await?;
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        test.submit_text_turn("Continue after compaction.").await?;
    }

    let requests = mock.requests();
    assert_eq!(requests.len(), if use_responses_lite { 6 } else { 4 });
    let snapshot = metrics.snapshot()?;
    let mut registration_samples = 0;
    let mut counts = BTreeMap::<_, u64>::new();
    let mut bytes = BTreeMap::new();
    for metric in snapshot.scope_metrics().flat_map(ScopeMetrics::metrics) {
        match metric.name() {
            TOOL_REGISTRATIONS_METRIC => {
                let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = metric.data() else {
                    panic!("tool registrations must be a histogram");
                };
                for point in histogram.data_points() {
                    registration_samples += point.count();
                    assert_eq!(
                        point
                            .attributes()
                            .find(|tag| tag.key.as_str() == "incremental_tools")
                            .map(|tag| tag.value.to_string()),
                        Some(use_responses_lite.to_string())
                    );
                }
            }
            TOOL_INCREMENTAL_UPDATES_METRIC => {
                let AggregatedMetrics::U64(MetricData::Sum(sum)) = metric.data() else {
                    panic!("tool updates must be a counter");
                };
                for point in sum.data_points() {
                    let tags = point
                        .attributes()
                        .map(|tag| (tag.key.as_str(), tag.value.to_string()))
                        .collect::<BTreeMap<_, _>>();
                    *counts
                        .entry((tags["action"].clone(), tags["tool"].clone()))
                        .or_default() += point.value();
                }
            }
            TOOL_SPEC_DESCRIPTION_BYTES_METRIC => {
                let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = metric.data() else {
                    panic!("tool description bytes must be a histogram");
                };
                for point in histogram.data_points() {
                    let tool = point
                        .attributes()
                        .find(|tag| tag.key.as_str() == "tool")
                        .expect("tool tag")
                        .value
                        .to_string();
                    let sample = bytes.entry(tool).or_insert((0, 0.0));
                    sample.0 += point.count();
                    sample.1 += point.sum();
                }
            }
            _ => {}
        }
    }
    // Six exposure samples per sampling request, excluding compaction.
    assert_eq!(
        registration_samples,
        6 * if use_responses_lite { 5 } else { 4 }
    );
    if use_responses_lite {
        let body = requests[2].body_json();
        let exec = body["input"]
            .as_array()
            .expect("request input")
            .iter()
            .rev()
            .filter(|item| item["type"] == "additional_tools")
            .find_map(|item| responses::namespace_child_tool(item, "functions", "exec"))
            .expect("changed exec definition");
        let description_bytes = exec["description"]
            .as_str()
            .expect("exec description")
            .len() as f64;
        assert_eq!(
            counts,
            BTreeMap::from([(
                ("schema_changed".to_string(), "functions.exec".to_string()),
                1
            )])
        );
        assert_eq!(
            bytes,
            BTreeMap::from([("functions.exec".to_string(), (1, description_bytes))])
        );
    } else {
        assert_eq!(counts, BTreeMap::new());
        assert_eq!(bytes, BTreeMap::new());
    }
    if use_responses_lite {
        for request in &requests[..5] {
            assert_eq!(
                request.instructions_text(),
                "Use the available tools to help the user."
            );
        }
        assert!(!requests[5].body_contains_text("Use the available tools to help the user."));
        assert_eq!(
            requests[5].instructions_text(),
            "Review results before responding."
        );
    }
    insta::assert_snapshot!(
        if use_responses_lite {
            "incremental_tools"
        } else {
            "incremental_tools_responses_api"
        },
        context_snapshot::format_request_history_snapshot(
            if use_responses_lite {
                "Tool definitions precede the initial base instructions. A model switch appends one developer notice, changed tools, and replacement instructions without rewriting the prefix; compaction starts a new window with the updated base."
            } else {
                "Regular Responses requests carry the full current tool catalog without an incremental notice; changed execution instructions replace the catalog on subsequent requests."
            },
            &requests,
            &ContextSnapshotOptions::default()
                .rewrite_known_segments()
                .include_request_settings(),
        )
    );
    metrics.shutdown()?;
    Ok(())
}

#[test_case(false; "bundled")]
#[test_case(true; "catalog_messages")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_removal_appends_a_standalone_notice_without_rewriting_history(
    catalog_messages: bool,
) -> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let command = remote_aware_stdio_server_bin()?;
    let environment_id = remote_aware_environment_id();
    let server_config = |enabled_tools: &[&str], instructions: &str| {
        serde_json::from_value(json!({
            "command": command,
            "environment_id": environment_id,
            "env": {"MCP_TEST_SERVER_INSTRUCTIONS": instructions},
            "enabled_tools": enabled_tools,
            "default_tools_approval_mode": "approve",
        }))
    };
    let initial_server = server_config(&["echo", "cwd"], "Echo service instructions.")?;
    let reduced_server = server_config(&["echo"], "Echo service instructions.")?;
    let restored_server = server_config(&["echo", "cwd"], "Echo service instructions.")?;
    let test = test_codex()
        .with_model_info_override("gpt-5.5", move |model| {
            model.use_responses_lite = true;
            model.supports_search_tool = false;
            model.tool_mode = Some(ToolMode::Direct);
            if catalog_messages {
                model
                    .model_messages
                    .get_or_insert_default()
                    .tools
                    .get_or_insert_default()
                    .incremental_tools = Some(IncrementalToolMessages {
                    tool_update_hint: Some(
                        "Catalog update: retain earlier tools unless explicitly removed."
                            .to_string(),
                    ),
                    removed_tools_header: Some("Unavailable tools:".to_string()),
                    removed_namespaces_header: Some("Unavailable namespaces:".to_string()),
                    namespace_instructions_prefix: Some("Catalog {name}: ".to_string()),
                    namespace_instructions_cleared: Some("Cleared {name}.".to_string()),
                });
            }
        })
        .with_config(move |config| {
            config
                .features
                .enable(Feature::IncrementalTools)
                .expect("enable incremental tools");
            if catalog_messages {
                config
                    .features
                    .enable(Feature::NonPrefixedMcpToolNames)
                    .expect("enable unprefixed MCP namespaces");
                config.non_prefixed_mcp_tool_servers = Some(vec!["functions".to_string()]);
            }
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
    let response_count = if catalog_messages { 11 } else { 7 };
    let mock = responses::mount_sse_sequence(
        &server,
        (1..=response_count)
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

    if catalog_messages {
        // The functions namespace preserves empty instructions instead of adding fallback text.
        // Move the service there, then change and clear instructions with unchanged tool schemas.
        for (instructions, prompt) in [
            (
                "Use these echo tools.",
                "Continue with the echo tools in the functions namespace.",
            ),
            (
                "Keep {name} and {instructions} literal.",
                "Continue with updated echo instructions.",
            ),
            ("", "Continue after clearing echo instructions."),
        ] {
            let current_config = test.codex.config().await;
            let mut updated_config = current_config.as_ref().clone();
            updated_config
                .mcp_servers
                .set(std::collections::HashMap::from([(
                    "functions".to_string(),
                    server_config(&["echo", "cwd"], instructions)?,
                )]))?;
            let _ = test
                .codex
                .refresh_mcp_config(current_config, updated_config)
                .await;
            test.codex
                .call_mcp_tool(
                    "functions",
                    "echo",
                    Some(json!({"message": "ready after instructions refresh"})),
                    /*meta*/ None,
                )
                .await?;
            test.submit_text_turn(prompt).await?;
        }
        test.submit_text_turn("Continue with the same cleared instructions.")
            .await?;
    }

    let requests = mock.requests();
    let (snapshot_name, scenario) = if catalog_messages {
        (
            "incremental_tools_catalog_messages",
            "Catalog overrides supply the update hint, both removal headers, and namespace instructions notices. MCP refreshes remove and restore tools, then change and clear only the namespace instructions. Runtime instructions remain verbatim, and unchanged turns do not repeat notices.",
        )
    } else {
        (
            "incremental_tool_removals",
            "MCP refreshes remove one tool, then its whole namespace, before restoring both tools. Removal notices append alone; restoration appends one developer notice before both namespace declarations. Unchanged turns do not repeat updates.",
        )
    };
    insta::assert_snapshot!(
        snapshot_name,
        context_snapshot::format_request_history_snapshot(
            scenario,
            &requests,
            &ContextSnapshotOptions::default()
                .rewrite_known_segments()
                .include_request_settings(),
        )
    );
    Ok(())
}

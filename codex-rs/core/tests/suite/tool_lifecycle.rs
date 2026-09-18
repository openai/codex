use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_extension_api::McpToolResultInput;
use codex_extension_api::McpToolSource;
use codex_extension_api::ResponseItem;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
use codex_extension_api::ToolStartInput;
use codex_features::Feature;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::models::ContentItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::user_input::UserInput;
use codex_utils_path_uri::PathUri;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::SEARCH_CALENDAR_LIST_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_NAMESPACE;
use core_test_support::apps_test_server::apps_enabled_builder;
use core_test_support::apps_test_server::recorded_apps_tool_call_by_call_id;
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::test_env;
use core_test_support::wait_for_event;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use test_case::test_case;
use tokio::sync::Notify;
use tokio::sync::oneshot;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;

#[path = "command_lifecycle_tests.rs"]
mod command_lifecycle;

struct RecordedHistory {
    call_id: String,
    arguments: String,
    items: Vec<ResponseItem>,
    mcp_tool: Option<(String, Option<String>, McpToolSource)>,
}

#[derive(Default)]
struct ConversationHistoryRecorder {
    histories: Mutex<Vec<RecordedHistory>>,
}

#[derive(Clone, Copy)]
enum AppsServerOwner {
    Host,
    Extension,
}

struct ExtensionOwnedAppsServer {
    url: String,
}

impl McpServerContributor<Config> for ExtensionOwnedAppsServer {
    fn id(&self) -> &'static str {
        "extension_owned_apps_lifecycle_test"
    }

    fn contribute<'a>(
        &'a self,
        _context: McpServerContributionContext<'a, Config>,
    ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async move {
            let config = serde_json::from_value(json!({ "url": self.url }))
                .expect("test Apps MCP server config should be valid");
            vec![McpServerContribution::Set {
                name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                config: Box::new(config),
            }]
        })
    }
}

impl ToolLifecycleContributor for ConversationHistoryRecorder {
    fn on_tool_start<'a>(&'a self, input: ToolStartInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move {
            self.histories
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(RecordedHistory {
                    call_id: input.call_id.to_owned(),
                    arguments: input.payload.log_payload().into_owned(),
                    items: input.conversation_history.items().cloned().collect(),
                    mcp_tool: input.mcp_tool.map(|tool| {
                        (
                            tool.tool_info().server_name.clone(),
                            tool.tool_info().connector_id.clone(),
                            tool.source().clone(),
                        )
                    }),
                });
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_start_receives_conversation_history() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let first_call_id = "first-plan-call";
    let second_call_id = "second-plan-call";
    responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_reasoning_item("reasoning-1", &["inspect the workspace"], &[]),
                responses::ev_function_call(
                    first_call_id,
                    "update_plan",
                    &json!({
                        "plan": [{ "step": "Inspect workspace", "status": "in_progress" }]
                    })
                    .to_string(),
                ),
                responses::ev_completed("first-response"),
            ]),
            responses::sse(vec![
                responses::ev_function_call(
                    second_call_id,
                    "update_plan",
                    &json!({
                        "plan": [{ "step": "Inspect workspace", "status": "completed" }]
                    })
                    .to_string(),
                ),
                responses::ev_completed("second-response"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("assistant-1", "done"),
                responses::ev_completed("third-response"),
            ]),
        ],
    )
    .await;

    let recorder = Arc::new(ConversationHistoryRecorder::default());
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.tool_lifecycle_contributor(recorder.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| config.update_plan_enabled = true)
        .build_with_auto_env(&server)
        .await?;

    let user_prompt = "Inspect the workspace and update the plan.";
    test.submit_text_turn(user_prompt).await?;

    let histories = recorder
        .histories
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let call_ids = histories
        .iter()
        .map(|history| history.call_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(call_ids, vec![first_call_id, second_call_id]);
    let arguments = histories
        .iter()
        .map(|history| serde_json::from_str::<serde_json::Value>(&history.arguments))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        arguments,
        vec![
            json!({ "plan": [{ "step": "Inspect workspace", "status": "in_progress" }] }),
            json!({ "plan": [{ "step": "Inspect workspace", "status": "completed" }] }),
        ]
    );

    let first_history = &histories[0].items;
    assert!(first_history.iter().any(|item| matches!(
        item,
        ResponseItem::Message { role, content, .. }
            if role == "user"
                && content.iter().any(|content| matches!(
                    content,
                    ContentItem::InputText { text } if text == user_prompt
                ))
    )));
    assert!(
        first_history
            .iter()
            .any(|item| matches!(item, ResponseItem::Reasoning { .. }))
    );
    assert!(first_history.iter().any(|item| matches!(
        item,
        ResponseItem::FunctionCall { call_id, .. } if call_id == first_call_id
    )));

    let second_history = &histories[1].items;
    assert!(second_history.iter().any(|item| matches!(
        item,
        ResponseItem::FunctionCallOutput {
            call_id: Some(call_id),
            ..
        } if call_id == first_call_id
    )));
    assert!(second_history.iter().any(|item| matches!(
        item,
        ResponseItem::FunctionCall { call_id, .. } if call_id == second_call_id
    )));

    Ok(())
}

#[test_case(AppsServerOwner::Host, McpToolSource::Connector; "host_owned_apps")]
#[test_case(AppsServerOwner::Extension, McpToolSource::Other; "extension_owned_apps")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_start_receives_executed_mcp_call_for_connector(
    owner: AppsServerOwner,
    expected_source: McpToolSource,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let call_id = "calendar-lifecycle-call";
    responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call_with_namespace(
                    call_id,
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_LIST_TOOL,
                    "{}",
                ),
                responses::ev_completed("first-response"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("assistant-1", "done"),
                responses::ev_completed("second-response"),
            ]),
        ],
    )
    .await;

    let recorder = Arc::new(ConversationHistoryRecorder::default());
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.tool_lifecycle_contributor(recorder.clone());
    if matches!(owner, AppsServerOwner::Extension) {
        extensions.mcp_server_contributor(Arc::new(ExtensionOwnedAppsServer {
            url: format!("{}/api/codex/ps/mcp", apps_server.chatgpt_base_url),
        }));
    }
    let test = apps_enabled_builder(apps_server.chatgpt_base_url)
        .with_extensions(Arc::new(extensions.build()))
        .build_with_auto_env(&server)
        .await?;
    wait_for_mcp_server(&test.codex, CODEX_APPS_MCP_SERVER_NAME).await?;

    test.submit_text_turn("List my calendar events.").await?;

    {
        let histories = recorder
            .histories
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let [history] = histories.as_slice() else {
            panic!("expected one tool start, got {}", histories.len());
        };
        assert_eq!(history.call_id, call_id);
        assert_eq!(
            history.mcp_tool,
            Some((
                CODEX_APPS_MCP_SERVER_NAME.to_string(),
                Some("calendar".to_string()),
                expected_source,
            )),
        );
    }

    let executed_call = recorded_apps_tool_call_by_call_id(&server, call_id).await;
    assert_eq!(
        executed_call
            .pointer("/params/name")
            .and_then(Value::as_str),
        Some("calendar_list_events")
    );

    Ok(())
}

#[derive(Clone, Copy)]
enum McpCallMode {
    Direct,
    CodeMode,
}

#[derive(Clone, Copy)]
enum McpResultProcessing {
    Unchanged,
    Replace,
    Fail,
}

struct BlockingMcpResultContributor {
    entered: Mutex<Option<oneshot::Sender<CallToolResult>>>,
    release: Notify,
    replacement: Option<CallToolResult>,
}

impl ToolLifecycleContributor for BlockingMcpResultContributor {
    fn on_mcp_tool_result<'a>(&'a self, input: McpToolResultInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move {
            assert_eq!(input.mcp_tool.source(), &McpToolSource::Connector);
            assert_eq!(input.mcp_tool.tool_info().tool.name, "calendar_list_events");
            assert_eq!(input.arguments, &json!({"query": "callback test"}));
            self.entered
                .lock()
                .expect("callback gate lock")
                .take()
                .expect("callback should run once")
                .send(input.result.clone())
                .expect("test should be waiting for the result");
            self.release.notified().await;
            if let Some(replacement) = &self.replacement {
                *input.result = replacement.clone();
            }
        })
    }
}

#[test_case(McpCallMode::Direct, McpResultProcessing::Unchanged; "direct_unchanged")]
#[test_case(McpCallMode::Direct, McpResultProcessing::Replace; "direct_replaced")]
#[test_case(McpCallMode::Direct, McpResultProcessing::Fail; "direct_error")]
#[test_case(McpCallMode::CodeMode, McpResultProcessing::Replace; "code_mode_replaced")]
#[test_case(McpCallMode::CodeMode, McpResultProcessing::Fail; "code_mode_error")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_result_processing_precedes_completion(
    mode: McpCallMode,
    processing: McpResultProcessing,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let server_result: CallToolResult = serde_json::from_value(json!({
        "content": [{"type": "text", "text": "server result"}],
        "structuredContent": {"text": "server result"},
        "isError": false,
        "_meta": {"private-mcp-metadata": "server"},
    }))?;
    let response_result = server_result.clone();
    Mock::given(body_partial_json(json!({"method": "tools/call"})))
        .respond_with(move |request: &Request| {
            let body: Value = serde_json::from_slice(&request.body).expect("MCP request JSON");
            ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": body["id"],
                "result": response_result,
            }))
        })
        .with_priority(1)
        .mount(&server)
        .await;

    let replacement = match processing {
        McpResultProcessing::Unchanged => None,
        McpResultProcessing::Replace | McpResultProcessing::Fail => {
            Some(serde_json::from_value(json!({
                "content": [{"type": "text", "text": "extension result"}],
                "structuredContent": {"text": "extension result"},
                "isError": matches!(processing, McpResultProcessing::Fail),
                "_meta": {"private-mcp-metadata": "extension"},
            }))?)
        }
    };
    let expected_result = replacement.clone().unwrap_or_else(|| server_result.clone());
    let (entered_tx, entered_rx) = oneshot::channel();
    let contributor = Arc::new(BlockingMcpResultContributor {
        entered: Mutex::new(Some(entered_tx)),
        release: Notify::new(),
        replacement,
    });
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.tool_lifecycle_contributor(contributor.clone());
    let test = apps_enabled_builder(apps_server.chatgpt_base_url)
        .with_extensions(Arc::new(extensions.build()))
        .with_config(move |config| {
            if matches!(mode, McpCallMode::CodeMode) {
                let _ = config.features.enable(Feature::CodeMode);
            }
        })
        .build_with_auto_env(&server)
        .await?;
    wait_for_mcp_server(&test.codex, CODEX_APPS_MCP_SERVER_NAME).await?;

    let call_id = "mcp-result-call";
    let call = match mode {
        McpCallMode::Direct => responses::ev_function_call_with_namespace(
            call_id,
            SEARCH_CALENDAR_NAMESPACE,
            SEARCH_CALENDAR_LIST_TOOL,
            r#"{"query":"callback test"}"#,
        ),
        McpCallMode::CodeMode => responses::ev_custom_tool_call(
            call_id,
            "exec",
            r#"text(await tools.mcp__codex_apps__calendar_list_events({query: "callback test"}));"#,
        ),
    };
    responses::mount_sse_once(
        &server,
        responses::sse(vec![call, responses::ev_completed("first-response")]),
    )
    .await;
    let follow_up = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("second-response")]),
    )
    .await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "List my calendar events.".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;

    assert_eq!(
        timeout(Duration::from_secs(30), entered_rx).await??,
        server_result
    );
    assert!(
        timeout(
            Duration::from_millis(100),
            wait_for_event(&test.codex, |event| matches!(
                event,
                EventMsg::McpToolCallEnd(_) | EventMsg::TurnComplete(_)
            )),
        )
        .await
        .is_err(),
        "completion must wait for the extension"
    );
    assert!(follow_up.requests().is_empty());

    contributor.release.notify_one();
    let EventMsg::McpToolCallEnd(end) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::McpToolCallEnd(_))
    })
    .await
    else {
        unreachable!();
    };
    assert_eq!(
        end.result.expect("MCP server returned a result"),
        expected_result
    );
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let request = follow_up.single_request();
    let output = match mode {
        McpCallMode::Direct => request.function_call_output(call_id),
        McpCallMode::CodeMode => request.custom_tool_call_output(call_id),
    };
    let output = output["output"].to_string();
    assert!(
        output.contains(
            expected_result.content[0]["text"]
                .as_str()
                .expect("fixture should contain text")
        )
    );
    assert!(!output.contains("private-mcp-metadata"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_start_receives_frozen_host_plugin_root() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "requires a native test_stdio_server");

    let server = responses::start_mock_server().await;
    let codex_home = Arc::new(tempfile::tempdir()?);
    let plugin_root = super::plugins::write_sample_plugin_manifest_and_config(codex_home.as_ref());
    let test_env = test_env().await?;
    let server_config = json!({
        "command": super::rmcp_client::remote_aware_stdio_server_bin()?,
        "environment_id": super::rmcp_client::remote_aware_environment_id(),
        "cwd": test_env.cwd(),
    });
    fs::write(
        plugin_root.join(".mcp.json"),
        serde_json::to_vec(&json!({"mcpServers": {"sample": server_config}}))?,
    )?;

    let recorder = Arc::new(ConversationHistoryRecorder::default());
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.tool_lifecycle_contributor(recorder.clone());
    let test = test_codex()
        .with_home(codex_home)
        .with_extensions(Arc::new(extensions.build()))
        .with_model_info_override("gpt-5.4", |model| model.supports_search_tool = false)
        .build_with_environment(&server, test_env)
        .await?;
    wait_for_mcp_server(&test.codex, "sample").await?;
    responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call_with_namespace(
                    "plugin-lifecycle-call",
                    "mcp__sample",
                    "echo",
                    r#"{"message":"ping"}"#,
                ),
                responses::ev_completed("first-response"),
            ]),
            responses::sse(vec![responses::ev_completed("second-response")]),
        ],
    )
    .await;
    test.submit_text_turn("Call the sample echo tool.").await?;

    let histories = recorder
        .histories
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let [history] = histories.as_slice() else {
        panic!("expected one tool start, got {}", histories.len());
    };
    let expected_source = McpToolSource::Plugin {
        id: "sample@test".to_string(),
        root: PathUri::from_host_native_path(&plugin_root)?,
    };
    assert_eq!(
        history.mcp_tool,
        Some(("sample".to_string(), None, expected_source)),
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_start_receives_rewritten_payload_and_post_hook_history() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "command hooks require a host-native executor");

    let server = responses::start_mock_server().await;
    let call_id = "rewritten-plan-call";
    let original_input = json!({
        "plan": [{ "step": "Original step", "status": "in_progress" }]
    });
    let rewritten_input = json!({
        "plan": [{ "step": "Rewritten step", "status": "completed" }]
    });
    let additional_context = "Only available after the pre-tool hook.";
    responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call(call_id, "update_plan", &original_input.to_string()),
                responses::ev_completed("first-response"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("assistant-1", "done"),
                responses::ev_completed("second-response"),
            ]),
        ],
    )
    .await;

    let hook_output = json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
            "updatedInput": rewritten_input,
            "additionalContext": additional_context,
        }
    });
    let recorder = Arc::new(ConversationHistoryRecorder::default());
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.tool_lifecycle_contributor(recorder.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_pre_build_hook(move |home| {
            write_pre_tool_hook(home, "^update_plan$", &hook_output)
                .expect("write pre-tool hook fixture");
        })
        .with_config(trust_discovered_hooks)
        .with_config(|config| config.update_plan_enabled = true)
        .build_with_auto_env(&server)
        .await?;

    test.submit_text_turn("Update the plan.").await?;

    let histories = recorder
        .histories
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let [history] = histories.as_slice() else {
        panic!("expected one tool start, got {}", histories.len());
    };
    assert_eq!(
        (
            history.call_id.as_str(),
            serde_json::from_str::<serde_json::Value>(&history.arguments)?,
        ),
        (call_id, rewritten_input)
    );
    assert!(history.items.iter().any(|item| matches!(
        item,
        ResponseItem::Message { role, content, .. }
            if role == "developer"
                && content.iter().any(|content| matches!(
                    content,
                    ContentItem::InputText { text } if text == additional_context
                ))
    )));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_start_is_not_called_when_pre_tool_hook_prevents_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "command hooks require a host-native executor");

    for (tool_name, matcher, arguments, hook_output) in [
        (
            "update_plan",
            "^update_plan$",
            json!({ "plan": [{ "step": "Blocked step", "status": "in_progress" }] }),
            json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "deny",
                    "permissionDecisionReason": "blocked by lifecycle test",
                }
            }),
        ),
        (
            "exec_command",
            "^Bash$",
            json!({ "cmd": "echo original" }),
            json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "allow",
                    "updatedInput": { "command": 123 },
                }
            }),
        ),
    ] {
        let server = responses::start_mock_server().await;
        let call_id = format!("prevented-{tool_name}-call");
        responses::mount_sse_sequence(
            &server,
            vec![
                responses::sse(vec![
                    responses::ev_function_call(&call_id, tool_name, &arguments.to_string()),
                    responses::ev_completed("first-response"),
                ]),
                responses::sse(vec![
                    responses::ev_assistant_message("assistant-1", "done"),
                    responses::ev_completed("second-response"),
                ]),
            ],
        )
        .await;

        let recorder = Arc::new(ConversationHistoryRecorder::default());
        let mut extensions = ExtensionRegistryBuilder::<Config>::new();
        extensions.tool_lifecycle_contributor(recorder.clone());
        let test = test_codex()
            .with_extensions(Arc::new(extensions.build()))
            .with_pre_build_hook(move |home| {
                write_pre_tool_hook(home, matcher, &hook_output)
                    .expect("write pre-tool hook fixture");
            })
            .with_config(trust_discovered_hooks)
            .with_config(|config| config.update_plan_enabled = true)
            .build_with_auto_env(&server)
            .await?;

        test.submit_text_turn("Run the tool.").await?;

        assert!(
            recorder
                .histories
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_empty(),
            "tool start should not run for {tool_name}"
        );
    }

    Ok(())
}

fn write_pre_tool_hook(home: &Path, matcher: &str, output: &serde_json::Value) -> Result<()> {
    let script_path = home.join("tool_lifecycle_hook.py");
    let output_json = serde_json::to_string(output).context("serialize pre-tool hook output")?;
    fs::write(
        &script_path,
        format!("import json\nimport sys\njson.load(sys.stdin)\nprint({output_json:?})\n"),
    )
    .context("write pre-tool hook script")?;
    let hooks = json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": matcher,
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                }]
            }]
        }
    });
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write pre-tool hooks.json")?;

    Ok(())
}

//! Exercises sampling through JSON-RPC, including cache identity and cleanup.

use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::DynamicToolFunctionSpec;
use codex_app_server_protocol::DynamicToolSpec;
use codex_app_server_protocol::HooksListParams;
use codex_app_server_protocol::HooksListResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadSampleCancelResponse;
use codex_app_server_protocol::ThreadSampleParams;
use codex_app_server_protocol::ThreadSampleResponse;
use codex_app_server_protocol::ThreadSource;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_core::config::ConfigBuilder;
use codex_core::test_support::construct_model_info_offline;
use codex_features::Feature;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::openai_models::ModelServiceTier;
use codex_protocol::openai_models::ReasoningEffort;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::skip_if_host_windows;
use core_test_support::skip_if_remote;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use test_case::test_case;
use test_case::test_matrix;
use tokio::sync::Notify;
use tokio::time::timeout;
use wiremock::ResponseTemplate;

const WAIT: Duration = Duration::from_secs(40);

async fn client(
    home: &TempDir,
    server: &wiremock::MockServer,
    overrides: bool,
) -> Result<TestAppServer> {
    client_with_effort_updates(home, server, overrides, EffortUpdates::Supported, "").await
}

enum EffortUpdates {
    Supported,
    Unsupported,
}

async fn client_with_effort_updates(
    home: &TempDir,
    server: &wiremock::MockServer,
    overrides: bool,
    effort_updates: EffortUpdates,
    extra_config: &str,
) -> Result<TestAppServer> {
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(codex_config::LoaderOverrides::without_managed_config_for_tests())
        .build()
        .await?;
    let mut model = construct_model_info_offline("gpt-5.4", &config);
    model.supports_reasoning_effort_updates = matches!(effort_updates, EffortUpdates::Supported);
    model.use_responses_lite = true;
    model.service_tiers = vec![ModelServiceTier {
        id: "priority".to_string(),
        name: "Fast".to_string(),
        description: "Priority processing.".to_string(),
    }];
    let catalog = home.path().join("catalog.json");
    std::fs::write(
        &catalog,
        serde_json::to_string(&json!({"models": [model]}))?,
    )?;
    let mut config = MockResponsesConfig::new(&server.uri()).with_model("gpt-5.4").with_provider_name("OpenAI")
        .with_root_config(&format!("model_catalog_json = {}\nmodel_reasoning_effort = \"high\"\nmodel_post_turn_compact_threshold_percent = 0", serde_json::to_string(&catalog)?));
    config = if overrides {
        config.enable_feature(Feature::ReasoningEffortOverride)
    } else {
        config.disable_feature(Feature::ReasoningEffortOverride)
    };
    config.with_extra_config(extra_config).write(home.path())?;
    TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized_with_timeout(WAIT)
        .await
}

fn input(text: &str) -> Vec<UserInput> {
    vec![UserInput::Text {
        text: text.to_string(),
        text_elements: Vec::new(),
    }]
}

fn comparable_request(body: Value) -> Value {
    let mut body = responses::strip_response_item_ids_from_json(body);
    body.as_object_mut()
        .expect("request object")
        .remove("client_metadata");
    for item in body["input"].as_array_mut().expect("request input") {
        if let Some(metadata) = item
            .get_mut("internal_chat_message_metadata_passthrough")
            .and_then(Value::as_object_mut)
        {
            metadata.remove("create_time");
            metadata.remove("turn_id");
        }
    }
    body
}

async fn sample(
    client: &mut TestAppServer,
    params: ThreadSampleParams,
) -> Result<ThreadSampleResponse> {
    let id = client
        .send_raw_request("thread/sample", Some(serde_json::to_value(params)?))
        .await?;
    timeout(WAIT, client.read_response(id)).await?
}

async fn ready_parent(
    client: &mut TestAppServer,
    params: ThreadStartParams,
) -> Result<(String, String)> {
    let parent = client.start_thread(params).await?.thread.id;
    let turn = client
        .start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: parent.clone(),
            input: input("Initial turn"),
            ..Default::default()
        })
        .await?
        .turn
        .id;
    Ok((parent, turn))
}

async fn prediction_fork(client: &mut TestAppServer, parent: &str) -> Result<String> {
    let id = client
        .send_thread_fork_request(ThreadForkParams {
            thread_id: parent.to_string(),
            experimental_prediction_mode: true,
            ephemeral: true,
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let fork: ThreadForkResponse = timeout(WAIT, client.read_response(id)).await??;
    Ok(fork.thread.id)
}

#[test_matrix(
    [ThreadHistoryMode::Legacy, ThreadHistoryMode::Paginated],
    [TurnStatus::Completed, TurnStatus::Failed, TurnStatus::Interrupted]
)]
#[tokio::test]
async fn sample_preserves_context_and_keeps_source_unchanged(
    history_mode: ThreadHistoryMode,
    parent_status: TurnStatus,
) -> Result<()> {
    let server = responses::start_mock_server().await;
    let compact_parent =
        history_mode == ThreadHistoryMode::Paginated && parent_status == TurnStatus::Completed;
    let mut parent_responses = vec![responses::sse(vec![
        responses::ev_reasoning_item(
            "parent-reasoning",
            &["Check the release."],
            &["Keep the existing context."],
        ),
        if parent_status == TurnStatus::Interrupted {
            responses::ev_function_call("parent-call", "parent_tool", "{}")
        } else {
            responses::ev_assistant_message("answer", "Done")
        },
        if parent_status == TurnStatus::Failed {
            json!({"type":"response.failed","response":{"id":"parent","error":{"code":"insufficient_quota","message":"quota exceeded"}}})
        } else {
            responses::ev_completed_with_tokens(
                "parent",
                /*total_tokens*/ if compact_parent { 330_000 } else { 100 },
            )
        },
    ])];
    if compact_parent {
        parent_responses.push(responses::sse(vec![
            json!({"type":"response.output_item.done","item":{
                "type":"compaction", "encrypted_content":"sample-test-compacted-parent",
            }}),
            responses::ev_completed_with_tokens("compacted", /*total_tokens*/ 200),
        ]));
    }
    let sample_index = parent_responses.len();
    parent_responses.extend([
        responses::sse(vec![
            json!({"type":"response.output_item.done","item":{"type":"message","id":"commentary","role":"assistant","phase":"commentary","content":[{"type":"output_text","text":"Checking the context"}]}}),
            responses::ev_reasoning_item("reasoning", &["Summary"], &["Reasoning"]),
            responses::ev_assistant_message("prediction", "Continue"),
            json!({"type":"response.completed","response":{"id":"sample","usage":{"input_tokens":100,"input_tokens_details":{"cached_tokens":80},"output_tokens":5,"total_tokens":105}}}),
        ]),
        responses::sse(vec![responses::ev_completed("prediction-fork")]),
        responses::sse(vec![responses::ev_completed("follow-up")]),
    ]);
    let mock = responses::mount_sse_sequence(&server, parent_responses).await;
    let home = TempDir::new()?;
    let mut client = client(&home, &server, /*overrides*/ true).await?;
    let parent = client
        .start_thread(ThreadStartParams {
            history_mode: Some(history_mode),
            base_instructions: Some("Retain these base instructions.".to_string()),
            developer_instructions: Some("Retain this system policy context.".to_string()),
            dynamic_tools: Some(vec![DynamicToolSpec::Function(DynamicToolFunctionSpec {
                name: "parent_tool".to_string(),
                description: "The parent's client-provided tool description.".to_string(),
                input_schema: json!({"type":"object","properties":{}}),
                defer_loading: false,
            })]),
            config: Some(std::collections::HashMap::from([
                ("features.request_permissions_tool".to_string(), json!(true)),
                ("model_auto_compact_token_limit".to_string(), json!(200_000)),
                (
                    "model_post_turn_compact_threshold_percent".to_string(),
                    json!(if compact_parent { 75 } else { 0 }),
                ),
            ])),
            ..Default::default()
        })
        .await?;
    let start_id = client
        .send_turn_start_request(TurnStartParams {
            thread_id: parent.thread.id.clone(),
            input: input("Initial turn"),
            turn_trigger: Some("parent-trigger".to_string()),
            service_tier: Some(Some("priority".to_string())),
            collaboration_mode: Some(CollaborationMode {
                mode: ModeKind::Plan,
                settings: Settings {
                    model: "gpt-5.4".to_string(),
                    reasoning_effort: Some(ReasoningEffort::High),
                    developer_instructions: Some(
                        "Keep the parent's planning instructions.".to_string(),
                    ),
                },
            }),
            disabled_plugin_ids: Some(vec!["unused@plugin".to_string()]),
            ..Default::default()
        })
        .await?;
    let started: TurnStartResponse = timeout(WAIT, client.read_response(start_id)).await??;
    if parent_status == TurnStatus::Interrupted {
        assert!(matches!(
            timeout(WAIT, client.read_stream_until_request_message()).await??,
            ServerRequest::DynamicToolCall { .. }
        ));
        let sample_id = client
            .send_raw_request(
                "thread/sample",
                Some(json!({"threadId": parent.thread.id, "expectedTurnId": started.turn.id, "prompt": "Predict"})),
            )
            .await?;
        let error = timeout(
            WAIT,
            client.read_stream_until_error_message(RequestId::Integer(sample_id)),
        )
        .await??;
        assert_eq!(error.error.message, "sample source must be idle");
        let interrupt_id = client
            .send_turn_interrupt_request(TurnInterruptParams {
                thread_id: parent.thread.id.clone(),
                turn_id: started.turn.id,
            })
            .await?;
        let _: TurnInterruptResponse = timeout(WAIT, client.read_response(interrupt_id)).await??;
    }
    let completed: TurnCompletedNotification =
        timeout(WAIT, client.read_notification("turn/completed")).await??;
    assert_eq!(completed.turn.status, parent_status);
    let path = parent.thread.path.as_ref().expect("persistent source path");
    let before = std::fs::read(path)?;
    client.clear_message_buffer();
    let result = sample(
        &mut client,
        ThreadSampleParams {
            thread_id: parent.thread.id.clone(),
            expected_turn_id: completed.turn.id.clone(),
            prompt: "Continue identically".to_string(),
            effort: Some(ReasoningEffort::Medium),
            thread_source: Some(ThreadSource::Feature("composer_predictions".to_string())),
            turn_trigger: Some("composer_predictions".to_string()),
            responsesapi_client_metadata: Some(HashMap::from([
                (
                    "generation_trigger".to_string(),
                    "composer_prediction".to_string(),
                ),
                ("turn_trigger".to_string(), "client-supplied".to_string()),
            ])),
            ..Default::default()
        },
    )
    .await?;
    assert_eq!(
        result,
        ThreadSampleResponse {
            source_turn_id: completed.turn.id,
            response_id: "sample".to_string(),
            text: Some("Continue".to_string()),
            input_tokens: Some(100),
            cached_input_tokens: Some(80),
            output_tokens: Some(5),
        }
    );
    assert_eq!(std::fs::read(path)?, before);
    assert!(
        !client
            .pending_notification_methods()
            .iter()
            .any(|name| name == "thread/started")
    );
    let list_id = client
        .send_raw_request("thread/loaded/list", Some(json!({})))
        .await?;
    let loaded: ThreadLoadedListResponse = timeout(WAIT, client.read_response(list_id)).await??;
    assert_eq!(loaded.data, vec![parent.thread.id.clone()]);
    let fork = prediction_fork(&mut client, &parent.thread.id).await?;
    for thread_id in [fork, parent.thread.id] {
        client
            .start_turn_and_wait_for_completion(TurnStartParams {
                thread_id,
                input: input("Continue identically"),
                effort: Some(ReasoningEffort::Medium),
                ..Default::default()
            })
            .await?;
    }
    let requests = mock.requests();
    assert_eq!(requests.len(), sample_index + 3);
    if history_mode == ThreadHistoryMode::Legacy && parent_status == TurnStatus::Completed {
        insta::assert_snapshot!(
            "sample_inherits_parent_context",
            context_snapshot::format_request_history_snapshot(
                "A parent uses high effort with custom instructions, client tools, permission tools, and priority service. A medium-effort sample, prediction fork, and subsequent parent turn preserve the same request context and high request-effort baseline.",
                &requests,
                &ContextSnapshotOptions::default().include_request_settings(),
            )
        );
    }
    if compact_parent {
        assert_eq!(
            requests[sample_index].inputs_of_type("compaction")[0]["encrypted_content"],
            "sample-test-compacted-parent",
        );
    }
    let prediction = &requests[sample_index];
    assert_eq!(
        comparable_request(prediction.body_json()),
        comparable_request(requests[sample_index + 1].body_json())
    );
    assert_eq!(
        comparable_request(prediction.body_json()),
        comparable_request(requests[sample_index + 2].body_json())
    );
    assert_eq!(prediction.body_json()["service_tier"], "priority");
    assert!(prediction.body_json().to_string().contains("parent_tool"));
    assert!(
        prediction
            .body_json()
            .to_string()
            .contains("request_permissions")
    );
    let metadata: Value = serde_json::from_str(
        &prediction
            .header("x-codex-turn-metadata")
            .expect("turn metadata"),
    )?;
    assert_eq!(metadata["thread_source"], "composer_predictions");
    assert_eq!(metadata["turn_trigger"], "composer_predictions");
    assert_eq!(metadata["generation_trigger"], "composer_prediction");
    let body_metadata: Value = serde_json::from_str(
        prediction.body_json()["client_metadata"]["x-codex-turn-metadata"]
            .as_str()
            .expect("body turn metadata"),
    )?;
    assert_eq!(body_metadata, metadata);
    let parent_metadata: Value = serde_json::from_str(
        &requests[sample_index + 2]
            .header("x-codex-turn-metadata")
            .expect("parent turn metadata"),
    )?;
    assert!(parent_metadata.get("generation_trigger").is_none());
    Ok(())
}

#[test_case(true, ReasoningEffort::High, ReasoningEffort::Low, ReasoningEffort::Medium; "pinned parent")]
#[test_case(false, ReasoningEffort::High, ReasoningEffort::Low, ReasoningEffort::Medium; "parent overrides disabled")]
#[test_case(true, ReasoningEffort::Low, ReasoningEffort::Medium, ReasoningEffort::High; "increasing pinned effort")]
#[test_case(false, ReasoningEffort::Low, ReasoningEffort::Medium, ReasoningEffort::High; "increasing effort without parent overrides")]
#[tokio::test]
async fn sample_preserves_effort_baseline(
    overrides: bool,
    initial_effort: ReasoningEffort,
    selected_effort: ReasoningEffort,
    sample_effort: ReasoningEffort,
) -> Result<()> {
    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_sequence(
        &server,
        ["initial", "selected", "sample", "prediction-fork"]
            .map(|id| {
                responses::sse(vec![
                    responses::ev_reasoning_item(
                        &format!("reasoning-{id}"),
                        &["Preserve the context."],
                        &["Continue from the previous response."],
                    ),
                    responses::ev_assistant_message(&format!("answer-{id}"), "Ready."),
                    responses::ev_completed(id),
                ])
            })
            .to_vec(),
    )
    .await;
    let home = TempDir::new()?;
    let mut client = client(&home, &server, overrides).await?;
    let (parent_id, _) = ready_parent(
        &mut client,
        ThreadStartParams {
            config: Some(std::collections::HashMap::from([(
                "model_reasoning_effort".to_string(),
                serde_json::to_value(&initial_effort)?,
            )])),
            ..Default::default()
        },
    )
    .await?;
    let completed = client
        .start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: parent_id.clone(),
            input: input("Second turn"),
            effort: Some(selected_effort.clone()),
            turn_trigger: Some("parent-trigger".to_string()),
            ..Default::default()
        })
        .await?;
    sample(
        &mut client,
        ThreadSampleParams {
            thread_id: parent_id.clone(),
            expected_turn_id: completed.turn.id,
            prompt: "Predict".to_string(),
            effort: Some(sample_effort.clone()),
            ..Default::default()
        },
    )
    .await?;
    let fork = prediction_fork(&mut client, &parent_id).await?;
    client
        .start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: fork,
            input: input("Predict"),
            effort: Some(sample_effort.clone()),
            ..Default::default()
        })
        .await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), 4);
    assert_eq!(
        requests[0].body_json()["reasoning"]["effort"],
        json!(initial_effort)
    );
    assert_eq!(
        comparable_request(requests[2].body_json()),
        comparable_request(requests[3].body_json())
    );
    let mut parent = comparable_request(requests[1].body_json());
    let mut child = comparable_request(requests[2].body_json());
    assert_eq!(
        child["reasoning"]["effort"],
        json!(if overrides {
            initial_effort
        } else {
            selected_effort
        })
    );
    let parent_input = parent["input"].take();
    let child_input = child["input"].take();
    assert_eq!(child, parent);
    let parent_input = parent_input.as_array().expect("parent input");
    let child_input = child_input.as_array().expect("sample input");
    assert_eq!(&child_input[..parent_input.len()], parent_input.as_slice());
    assert_eq!(
        child_input.last(),
        Some(&json!({"type":"configuration_update","reasoning":{"effort":sample_effort}}))
    );
    let metadata: Value = serde_json::from_str(
        &requests[2]
            .header("x-codex-turn-metadata")
            .expect("turn metadata"),
    )?;
    assert_eq!(metadata["thread_source"], "thread_sample");
    assert!(metadata.get("turn_trigger").is_none());
    Ok(())
}

#[tokio::test]
async fn sample_skips_inherited_stop_hooks() -> Result<()> {
    skip_if_host_windows!(Ok(()));
    skip_if_remote!(Ok(()), "command hook uses host-local paths");
    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_sequence(
        &server,
        ["parent", "sample"]
            .map(|id| {
                responses::sse(vec![
                    responses::ev_assistant_message(id, id),
                    responses::ev_completed(id),
                ])
            })
            .to_vec(),
    )
    .await;
    let home = TempDir::new()?;
    let log = home.path().join("stop.log");
    let script = home.path().join("stop.sh");
    std::fs::write(
        &script,
        format!(
            "cat >/dev/null\nif [ -f '{log}' ]; then\n  printf '%s' '{{\"decision\":\"block\",\"reason\":\"Continue working\"}}'\nfi\nprintf 'stop\\n' >> '{log}'\n",
            log = log.display(),
        ),
    )?;
    let extra_config = format!(
        "[[hooks.Stop]]\n[[hooks.Stop.hooks]]\ntype = \"command\"\ncommand = {}\n",
        serde_json::to_string(&format!("sh '{}'", script.display()))?,
    );
    let mut client = client_with_effort_updates(
        &home,
        &server,
        /*overrides*/ true,
        EffortUpdates::Supported,
        &extra_config,
    )
    .await?;
    let list_id = client
        .send_hooks_list_request(HooksListParams {
            cwds: vec![home.path().to_path_buf()],
        })
        .await?;
    let hooks: HooksListResponse = client.read_response(list_id).await?;
    let trust = hooks.data[0]
        .hooks
        .iter()
        .map(|hook| (hook.key.clone(), json!({"trusted_hash": hook.current_hash})))
        .collect::<serde_json::Map<_, _>>();
    assert!(!trust.is_empty());
    let (parent, turn) = ready_parent(
        &mut client,
        ThreadStartParams {
            config: Some(std::collections::HashMap::from([(
                "hooks.state".to_string(),
                Value::Object(trust),
            )])),
            ..Default::default()
        },
    )
    .await?;
    assert_eq!(std::fs::read_to_string(&log)?, "stop\n");
    let result = sample(
        &mut client,
        ThreadSampleParams {
            thread_id: parent,
            expected_turn_id: turn,
            prompt: "Predict".to_string(),
            ..Default::default()
        },
    )
    .await?;
    assert_eq!(result.text.as_deref(), Some("sample"));
    assert_eq!(mock.requests().len(), 2);
    assert_eq!(std::fs::read_to_string(log)?, "stop\n");
    Ok(())
}

#[tokio::test]
async fn sample_returns_null_for_custom_tool_calls() -> Result<()> {
    let server = responses::start_mock_server().await;
    let mut tool = responses::ev_custom_tool_call(
        "call",
        "apply_patch",
        "*** Begin Patch\n*** Add File: blocked.txt\n+blocked\n*** End Patch",
    );
    tool["type"] = json!("response.output_item.added");
    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![responses::ev_completed("parent")]),
            responses::sse(vec![
                responses::ev_assistant_message("partial", "Discard this partial answer"),
                tool,
                responses::ev_assistant_message("after-tool", "Discard this answer too"),
                json!({"type":"response.completed","response":{"id":"blocked","end_turn":false,"usage":{"input_tokens":100,"input_tokens_details":{"cached_tokens":80},"output_tokens":5,"total_tokens":105}}}),
            ]),
        ],
    )
    .await;
    let home = TempDir::new()?;
    let mut client = client(&home, &server, /*overrides*/ true).await?;
    let (parent_id, turn_id) = ready_parent(&mut client, ThreadStartParams::default()).await?;
    let response = sample(
        &mut client,
        ThreadSampleParams {
            thread_id: parent_id.clone(),
            expected_turn_id: turn_id.clone(),
            prompt: "Call the tool".to_string(),
            ..Default::default()
        },
    )
    .await?;
    assert_eq!(
        response,
        ThreadSampleResponse {
            source_turn_id: turn_id,
            response_id: "blocked".to_string(),
            text: None,
            input_tokens: Some(100),
            cached_input_tokens: Some(80),
            output_tokens: Some(5),
        }
    );
    assert_eq!(mock.requests().len(), 2);
    let id = client
        .send_raw_request("thread/loaded/list", Some(json!({})))
        .await?;
    let loaded: ThreadLoadedListResponse = client.read_response(id).await?;
    assert_eq!(loaded.data, vec![parent_id]);
    Ok(())
}

#[test_case(false, true; "inherit MCPs by default")]
#[test_case(true, true; "disable MCPs with incremental tools")]
#[test_case(true, false; "retain MCPs without incremental tools")]
#[tokio::test]
async fn sample_blocks_mcp_execution_without_changing_parent_tools(
    disable_mcp: bool,
    incremental_tools: bool,
) -> Result<()> {
    let server = responses::start_mock_server().await;
    let mcp = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/mcp"))
        .respond_with(|request: &wiremock::Request| {
            let request: Value = serde_json::from_slice(&request.body).expect("MCP request");
            let result = match request["method"].as_str() {
                Some("initialize") => json!({
                    "protocolVersion":"2025-11-25", "capabilities":{"tools":{}},
                    "serverInfo":{"name":"sample-test","version":"1.0"}
                }),
                Some("tools/list") => json!({"tools":[{
                    "name":"probe", "description":"Record a test call", "inputSchema":{"type":"object","properties":{}},
                    "annotations":{"readOnlyHint":true}
                }]}),
                Some("tools/call") => json!({"content":[{"type":"text","text":"Recorded"}]}),
                _ => return ResponseTemplate::new(202),
            };
            ResponseTemplate::new(200).set_body_json(json!({"jsonrpc":"2.0","id":request["id"],"result":result}))
        })
        .mount(&mcp).await;
    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call_with_namespace(
                    "parent-call",
                    "mcp__sample_test",
                    "probe",
                    "{}",
                ),
                responses::ev_completed("parent-tool"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("answer", "Done"),
                responses::ev_completed("parent"),
            ]),
            responses::sse(vec![
                responses::ev_function_call_with_namespace(
                    "sample-call",
                    "mcp__sample_test",
                    "probe",
                    "{}",
                ),
                responses::ev_completed("sample-tool"),
            ]),
            responses::sse(vec![responses::ev_completed("prediction-fork")]),
        ],
    )
    .await;
    let home = TempDir::new()?;
    let extra_config = format!(
        "[mcp_servers.sample_test]\nurl = \"{}/mcp\"\ndefault_tools_approval_mode = \"approve\"",
        mcp.uri()
    );
    let mut client = client_with_effort_updates(
        &home,
        &server,
        /*overrides*/ true,
        EffortUpdates::Supported,
        &extra_config,
    )
    .await?;
    let (parent_id, turn_id) = ready_parent(
        &mut client,
        ThreadStartParams {
            config: Some(std::collections::HashMap::from([(
                "features.incremental_tools".to_string(),
                json!(incremental_tools),
            )])),
            ..Default::default()
        },
    )
    .await?;
    let response = sample(
        &mut client,
        ThreadSampleParams {
            thread_id: parent_id.clone(),
            expected_turn_id: turn_id,
            prompt: "Call the tool again".to_string(),
            disable_mcp,
            ..Default::default()
        },
    )
    .await?;
    assert_eq!(
        (response.text, response.response_id),
        (None, "sample-tool".to_string())
    );
    let fork_id = client
        .send_thread_fork_request(ThreadForkParams {
            thread_id: parent_id,
            experimental_prediction_mode: true,
            experimental_disable_prediction_mcp: disable_mcp,
            ephemeral: true,
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let fork: ThreadForkResponse = timeout(WAIT, client.read_response(fork_id)).await??;
    client
        .start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: fork.thread.id,
            input: input("Call the tool again"),
            ..Default::default()
        })
        .await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), 4);
    let parent = comparable_request(requests[1].body_json());
    let sample = comparable_request(requests[2].body_json());
    let prefix = parent["input"].as_array().expect("parent input");
    let sample_input = sample["input"].as_array().expect("sample input");
    assert_eq!(&sample_input[..prefix.len()], prefix);
    let removes_mcp = disable_mcp && incremental_tools;
    assert_eq!(
        sample_input[prefix.len()..].iter().any(|item| {
            let text = item.to_string();
            text.contains("namespaces are no longer available") && text.contains("mcp__sample_test")
        }),
        removes_mcp
    );
    assert_eq!(sample, comparable_request(requests[3].body_json()));
    assert_eq!(
        requests[2].body_json()["tools"],
        requests[1].body_json()["tools"]
    );
    let calls = mcp
        .received_requests()
        .await
        .expect("MCP requests")
        .iter()
        .filter(|request| {
            serde_json::from_slice::<Value>(&request.body)
                .is_ok_and(|body| body["method"] == "tools/call")
        })
        .count();
    // The parent executes the tool; the sample never sends a second MCP call.
    assert_eq!(calls, 1);
    Ok(())
}

#[tokio::test]
async fn sample_does_not_hide_stream_errors_after_tool_calls() -> Result<()> {
    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_sequence(&server, vec![
        responses::sse(vec![responses::ev_completed("parent")]),
        responses::sse(vec![
            responses::ev_function_call("call", "view_image", "{}"),
            json!({"type":"response.failed","response":{"id":"failed","error":{"code":"insufficient_quota","message":"quota exceeded"}}}),
        ]),
    ]).await;
    let home = TempDir::new()?;
    let mut client = client(&home, &server, /*overrides*/ true).await?;
    let (parent, turn) = ready_parent(&mut client, ThreadStartParams::default()).await?;
    let id = client
        .send_raw_request(
            "thread/sample",
            Some(json!({
                "threadId":parent, "expectedTurnId":turn, "prompt":"Call a tool"
            })),
        )
        .await?;
    let error = timeout(
        WAIT,
        client.read_stream_until_error_message(RequestId::Integer(id)),
    )
    .await??;
    assert_eq!(error.error.code, -32603);
    assert_eq!(mock.requests().len(), 2);
    Ok(())
}

enum SampleStop {
    Timeout,
    Cancel,
}

#[test_case(SampleStop::Timeout; "timeout")]
#[test_case(SampleStop::Cancel; "cancel")]
#[tokio::test]
async fn sample_wait_keeps_parent_responsive_and_reclaims_child(stop: SampleStop) -> Result<()> {
    let server = responses::start_mock_server().await;
    responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("parent")]),
    )
    .await;
    let home = TempDir::new()?;
    let mut client = client(&home, &server, /*overrides*/ true).await?;
    let (parent_id, turn_id) = ready_parent(&mut client, ThreadStartParams::default()).await?;
    let arrived = Arc::new(Notify::new());
    let notify = Arc::clone(&arrived);
    let mock = responses::mount_response_once_match(
        &server,
        move |_: &wiremock::Request| {
            notify.notify_one();
            true
        },
        ResponseTemplate::new(200)
            .insert_header("content-type", "text/event-stream")
            .set_body_string(responses::sse(vec![responses::ev_completed("delayed")]))
            .set_delay(Duration::from_secs(60)),
    )
    .await;
    let params = ThreadSampleParams {
        thread_id: parent_id.clone(),
        expected_turn_id: turn_id,
        prompt: "Predict".to_string(),
        ..Default::default()
    };
    let sample_id = client
        .send_raw_request("thread/sample", Some(serde_json::to_value(&params)?))
        .await?;
    timeout(WAIT, arrived.notified()).await?;
    responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("next-parent-turn")]),
    )
    .await;
    timeout(
        Duration::from_secs(10),
        client.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: parent_id.clone(),
            input: input("Continue the parent while sampling"),
            ..Default::default()
        }),
    )
    .await??;
    let (expected_error, wait) = match stop {
        SampleStop::Timeout => ("sample timed out", WAIT),
        SampleStop::Cancel => {
            let cancel_id = client
                .send_raw_request(
                    "thread/sample/cancel",
                    Some(json!({"requestId": sample_id})),
                )
                .await?;
            let response: ThreadSampleCancelResponse =
                timeout(Duration::from_secs(10), client.read_response(cancel_id)).await??;
            assert_eq!(response, ThreadSampleCancelResponse {});
            ("sample canceled", Duration::from_secs(10))
        }
    };
    let error = timeout(
        wait,
        client.read_stream_until_error_message(RequestId::Integer(sample_id)),
    )
    .await??;
    assert_eq!(error.error.message, expected_error);
    if matches!(stop, SampleStop::Cancel) {
        for request_id in [json!(sample_id), json!("unknown")] {
            let cancel_id = client
                .send_raw_request(
                    "thread/sample/cancel",
                    Some(json!({"requestId": request_id})),
                )
                .await?;
            let response: ThreadSampleCancelResponse =
                timeout(Duration::from_secs(10), client.read_response(cancel_id)).await??;
            assert_eq!(response, ThreadSampleCancelResponse {});
        }
    }
    assert_eq!(mock.requests().len(), 1);
    timeout(WAIT, async {
        loop {
            let list_id = client
                .send_raw_request("thread/loaded/list", Some(json!({})))
                .await?;
            let loaded: ThreadLoadedListResponse = client.read_response(list_id).await?;
            if loaded.data == vec![parent_id.clone()] {
                return Ok::<(), anyhow::Error>(());
            }
        }
    })
    .await??;
    Ok(())
}

#[tokio::test]
async fn sample_rejects_invalid_sources_and_prompts() -> Result<()> {
    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_sequence(
        &server,
        vec![responses::sse(vec![responses::ev_completed("parent")])],
    )
    .await;
    let home = TempDir::new()?;
    let mut client = client(&home, &server, /*overrides*/ true).await?;
    let fresh = client
        .start_thread(ThreadStartParams::default())
        .await?
        .thread
        .id;
    let ephemeral = client
        .start_thread(ThreadStartParams {
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?
        .thread
        .id;
    let (parent, turn) = ready_parent(&mut client, ThreadStartParams::default()).await?;
    for (id, expected_turn, prompt, expected_error) in [
        (
            codex_protocol::ThreadId::new().to_string(),
            turn.clone(),
            "Predict".to_string(),
            "loaded source",
        ),
        (fresh, turn.clone(), "Predict".to_string(), "rollout"),
        (
            ephemeral,
            turn.clone(),
            "Predict".to_string(),
            "persistent root",
        ),
        (
            parent.clone(),
            "stale".to_string(),
            "Predict".to_string(),
            "latest finished turn",
        ),
        (parent.clone(), turn.clone(), String::new(), "prompt"),
        (
            parent.clone(),
            turn.clone(),
            "x".repeat(4_001),
            "at most 4000 bytes",
        ),
    ] {
        let id = client
            .send_raw_request(
                "thread/sample",
                Some(json!({"threadId": id, "expectedTurnId": expected_turn, "prompt": prompt})),
            )
            .await?;
        let error = timeout(
            WAIT,
            client.read_stream_until_error_message(RequestId::Integer(id)),
        )
        .await??;
        assert!(
            error.error.message.contains(expected_error),
            "{}",
            error.error.message
        );
    }
    assert_eq!(mock.requests().len(), 1);
    Ok(())
}

#[test_case(false; "unsupported model")]
#[test_case(true; "unsupported provider")]
#[tokio::test]
async fn unsupported_effort_updates_keep_inherited_effort(
    unsupported_provider: bool,
) -> Result<()> {
    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_sequence(
        &server,
        ["parent", "sample"]
            .map(|id| {
                responses::sse(vec![
                    responses::ev_assistant_message(id, "Completed without usage"),
                    json!({"type":"response.completed","response":{"id":id}}),
                ])
            })
            .to_vec(),
    )
    .await;
    let home = TempDir::new()?;
    let effort_updates = if unsupported_provider {
        EffortUpdates::Supported
    } else {
        EffortUpdates::Unsupported
    };
    let mut client =
        client_with_effort_updates(&home, &server, /*overrides*/ true, effort_updates, "").await?;
    let overrides = if unsupported_provider {
        json!({"model_providers.mock_provider.name":"Another provider"})
    } else {
        json!({})
    };
    let (parent_id, turn_id) = ready_parent(
        &mut client,
        ThreadStartParams {
            config: Some(serde_json::from_value(overrides)?),
            ..Default::default()
        },
    )
    .await?;
    let response = sample(
        &mut client,
        ThreadSampleParams {
            thread_id: parent_id,
            expected_turn_id: turn_id.clone(),
            prompt: "Predict".to_string(),
            effort: Some(ReasoningEffort::Medium),
            ..Default::default()
        },
    )
    .await?;
    assert_eq!(
        response,
        ThreadSampleResponse {
            source_turn_id: turn_id,
            response_id: "sample".to_string(),
            text: Some("Completed without usage".to_string()),
            input_tokens: None,
            cached_input_tokens: None,
            output_tokens: None,
        }
    );
    let requests = mock.requests();
    assert_eq!(
        requests[1].body_json()["reasoning"],
        requests[0].body_json()["reasoning"]
    );
    assert!(
        !requests[1]
            .input()
            .iter()
            .any(|item| item["type"] == "configuration_update")
    );
    Ok(())
}

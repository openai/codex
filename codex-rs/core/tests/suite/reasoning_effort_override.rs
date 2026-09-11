//! Trusted reasoning-effort updates follow surviving history and the next turn's selected settings.

use codex_core::ForkSnapshot;
use codex_core::RecoverTurnRequest;
use codex_core::StartIfIdleSubmission;
use codex_core::SuspendTurnOutcome;
use codex_core::TurnInputRequest;
use codex_core::TurnInputSubmission;
use codex_core::config::Config;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ResponsesRequest;
use core_test_support::skip_if_no_network;
use core_test_support::submit_thread_settings;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use test_case::test_case;
use wiremock::ResponseTemplate;

fn override_builder() -> TestCodexBuilder {
    test_codex()
        .with_model_info_override("gpt-5.4", |model| {
            model.use_responses_lite = true;
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::ReasoningEffortOverride)
                .expect("enable reasoning effort overrides");
            config.model_reasoning_effort = Some(ReasoningEffort::Medium);
        })
}

fn effort_updates(request: &ResponsesRequest) -> Vec<Value> {
    request
        .input()
        .into_iter()
        .filter(|item| item["type"] == "configuration_update")
        .collect()
}

fn effort_update(effort: ReasoningEffort) -> Value {
    serde_json::json!({
        "type": "configuration_update",
        "reasoning": {"effort": effort},
    })
}

fn message(role: &str, text: &str) -> Value {
    serde_json::json!({
        "type": "message",
        "role": role,
        "content": [{"type": "input_text", "text": text}],
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reasoning_effort_override_recovery_reuses_trusted_tail_update() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    // Keep the turn active without allowing any model output after the update.
    responses::mount_response_once(
        &server,
        responses::sse_response(responses::sse(vec![responses::ev_completed("suspended")]))
            .set_delay(Duration::from_secs(/*secs*/ 60)),
    )
    .await;
    let builder = || {
        override_builder().with_config(|config| {
            config.model_reasoning_effort = Some(ReasoningEffort::High);
            // Recovery must work without a prewarm establishing a runtime pin.
            config.model_provider.supports_websockets = false;
        })
    };
    let test = builder().build_with_auto_env(&server).await?;
    let submission = test
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "recover this turn".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let TurnInputSubmission::Started { turn_id } = submission else {
        panic!("expected a new turn");
    };
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::RawResponseItem(raw)
            if matches!(&raw.item, ResponseItem::ConfigurationUpdate { .. }))
    })
    .await;
    let thread_settings = test.codex.restorable_thread_settings().await;
    assert_eq!(
        test.codex.suspend_turn_and_shutdown().await?,
        SuspendTurnOutcome::Suspended {
            turn_id: turn_id.clone(),
        },
    );
    let rollout_path = test.codex.rollout_path().expect("rollout path");
    test.thread_manager
        .remove_thread(&test.session_configured.thread_id)
        .await
        .expect("unload suspended thread");

    let recovery_server = responses::start_mock_server().await;
    let mock = responses::mount_sse_once(
        &recovery_server,
        responses::sse(vec![responses::ev_completed("recovered")]),
    )
    .await;
    let cwd = test.config.cwd.clone();
    let mut resume_builder = builder().with_config(move |config| config.cwd = cwd);
    if let Some(url) = test.executor_environment().exec_server_url() {
        resume_builder = resume_builder.with_exec_server_url(url);
    }
    let resumed = resume_builder
        .resume(&recovery_server, Arc::clone(&test.home), rollout_path)
        .await?;
    resumed
        .codex
        .restore_thread_settings(thread_settings)
        .await?;
    assert_eq!(
        resumed
            .codex
            .recover_turn_if_idle(RecoverTurnRequest {
                turn_id: turn_id.clone(),
                thread_settings: Default::default(),
                trace: None,
                cyber_access_program: None,
            })
            .await?,
        StartIfIdleSubmission::Started { turn_id },
    );
    wait_for_event(&resumed.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let request = mock.single_request();
    assert_eq!(
        effort_updates(&request),
        vec![effort_update(ReasoningEffort::High)]
    );
    assert_eq!(
        request.input().last(),
        Some(&effort_update(ReasoningEffort::High))
    );
    assert_eq!(request.body_json()["reasoning"]["effort"], "high");
    Ok(())
}

#[test_case(ReasoningEffort::High; "high to persistent and back")]
#[test_case(ReasoningEffort::Persistent; "persistent to high and back")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reasoning_effort_override_persistent_transitions(
    initial_effort: ReasoningEffort,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let mut mocks = Vec::new();
    for id in ["first", "changed", "unchanged", "restored"] {
        mocks.push(
            responses::mount_sse_once(&server, responses::sse(vec![responses::ev_completed(id)]))
                .await,
        );
    }
    let high = effort_update(ReasoningEffort::High);
    let disabled = effort_update(ReasoningEffort::Custom("disabled".to_string()));
    let (changed_effort, initial_update, changed_update) =
        if initial_effort == ReasoningEffort::Persistent {
            (ReasoningEffort::High, disabled, high)
        } else {
            (ReasoningEffort::Persistent, high, disabled)
        };
    let test = override_builder().build_with_auto_env(&server).await?;
    for effort in [
        initial_effort.clone(),
        changed_effort.clone(),
        changed_effort,
        initial_effort,
    ] {
        submit_thread_settings(
            &test.codex,
            ThreadSettingsOverrides {
                effort: Some(Some(effort)),
                ..Default::default()
            },
        )
        .await?;
        test.submit_text_turn("continue").await?;
    }
    let requests = mocks
        .iter()
        .map(responses::ResponseMock::single_request)
        .collect::<Vec<_>>();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.body_json()["reasoning"]["effort"].clone())
            .collect::<Vec<_>>(),
        vec![initial_update["reasoning"]["effort"].clone(); requests.len()],
    );
    assert_eq!(
        requests.iter().map(effort_updates).collect::<Vec<_>>(),
        vec![
            vec![initial_update.clone()],
            vec![initial_update.clone(), changed_update.clone()],
            vec![initial_update.clone(), changed_update.clone()],
            vec![initial_update.clone(), changed_update, initial_update],
        ],
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reasoning_effort_override_preserves_prefix_and_only_appends_on_change()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let mut mocks = Vec::new();
    for id in ["first", "changed", "unchanged", "lowered"] {
        mocks.push(
            responses::mount_sse_once(&server, responses::sse(vec![responses::ev_completed(id)]))
                .await,
        );
    }
    let test = override_builder().build_with_auto_env(&server).await?;
    test.submit_text_turn("first message").await?;
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            effort: Some(Some(ReasoningEffort::High)),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("second message").await?;
    test.submit_text_turn("third message").await?;
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            effort: Some(Some(ReasoningEffort::Low)),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("fourth message").await?;

    let requests = mocks
        .iter()
        .map(responses::ResponseMock::single_request)
        .collect::<Vec<_>>();
    let inputs = requests
        .iter()
        .map(|request| {
            responses::strip_response_item_ids_from_json(responses::strip_metadata_from_json(
                Value::Array(request.input()),
            ))
            .as_array()
            .expect("input array")
            .clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.body_json()["reasoning"]["effort"].clone())
            .collect::<Vec<_>>(),
        vec![Value::from("medium"); requests.len()],
    );
    let cache_keys = requests
        .iter()
        .map(|request| request.body_json()["prompt_cache_key"].clone())
        .collect::<Vec<_>>();
    assert_eq!(cache_keys, vec![cache_keys[0].clone(); requests.len()]);
    assert_eq!(
        inputs[0][inputs[0].len() - 2..],
        [
            message("user", "first message"),
            effort_update(ReasoningEffort::Medium)
        ],
    );
    let mut expected = inputs[0].clone();
    expected.extend([
        message("user", "second message"),
        effort_update(ReasoningEffort::High),
    ]);
    assert_eq!(inputs[1], expected);
    expected.push(message("user", "third message"));
    assert_eq!(inputs[2], expected);
    expected.extend([
        message("user", "fourth message"),
        effort_update(ReasoningEffort::Low),
    ]);
    assert_eq!(inputs[3], expected);
    Ok(())
}

#[test_case(ReasoningEffort::High; "model maps ultra to high")]
#[test_case(ReasoningEffort::Max; "model maps ultra to max")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reasoning_effort_override_normalizes_ultra_before_comparing_updates(
    resolved_effort: ReasoningEffort,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let first = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("first")]),
    )
    .await;
    let second = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("second")]),
    )
    .await;
    let model_effort = resolved_effort.clone();
    let test = override_builder()
        .with_model_info_override("gpt-5.4", move |model| {
            model.multi_agent_reasoning_effort = Some(model_effort.clone());
            model.supported_reasoning_levels = vec![ReasoningEffortPreset {
                effort: model_effort,
                description: "Model effort".to_string(),
            }];
        })
        .with_config(|config| config.model_reasoning_effort = Some(ReasoningEffort::Ultra))
        .build_with_auto_env(&server)
        .await?;
    test.submit_text_turn("ultra selection").await?;
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            effort: Some(Some(resolved_effort.clone())),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("equivalent explicit effort").await?;
    let requests = [first.single_request(), second.single_request()];
    assert_eq!(
        requests.each_ref().map(effort_updates),
        [
            vec![effort_update(resolved_effort.clone())],
            vec![effort_update(resolved_effort.clone())],
        ]
    );
    assert_eq!(
        requests.map(|request| request.body_json()["reasoning"]["effort"].clone()),
        [
            serde_json::to_value(&resolved_effort)?,
            serde_json::to_value(&resolved_effort)?
        ]
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum PrewarmStartup {
    New,
    Resume,
    Fork,
}

#[test_case(true, PrewarmStartup::New; "new thread feature enabled")]
#[test_case(false, PrewarmStartup::New; "new thread feature disabled")]
#[test_case(true, PrewarmStartup::Resume; "resumed thread feature enabled")]
#[test_case(false, PrewarmStartup::Resume; "resumed thread feature disabled")]
#[test_case(true, PrewarmStartup::Fork; "forked thread feature enabled")]
#[test_case(false, PrewarmStartup::Fork; "forked thread feature disabled")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reasoning_effort_override_websocket_prewarm_preserves_baseline(
    feature_enabled: bool,
    startup: PrewarmStartup,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let websocket = responses::start_websocket_server(vec![vec![
        vec![
            responses::ev_response_created("warmup"),
            responses::ev_completed("warmup"),
        ],
        vec![
            responses::ev_response_created("first"),
            responses::ev_completed("first"),
        ],
    ]])
    .await;
    let base_url = format!("{}/v1", websocket.uri());
    let configure_prewarm = move |config: &mut Config| {
        config.model_provider.base_url = Some(base_url.clone());
        config.model_provider.supports_websockets = true;
        config
            .features
            .set_enabled(Feature::ReasoningEffortOverride, feature_enabled)
            .expect("configure reasoning effort overrides");
    };
    let mut builder = override_builder().with_config(configure_prewarm.clone());
    let test = if !matches!(startup, PrewarmStartup::New) {
        let previous_mock = responses::mount_sse_once(
            &server,
            responses::sse(vec![responses::ev_completed("previous")]),
        )
        .await;
        let mut previous = override_builder()
            .with_config(|config| {
                config
                    .features
                    .disable(Feature::ReasoningEffortOverride)
                    .expect("disable reasoning effort overrides for source history");
                config.model_provider.supports_websockets = false;
            })
            .build_with_auto_env(&server)
            .await?;
        previous.submit_text_turn("previous turn").await?;
        assert_eq!(
            previous_mock
                .single_request()
                .message_input_texts("user")
                .last()
                .map(String::as_str),
            Some("previous turn"),
        );
        if matches!(startup, PrewarmStartup::Fork) {
            previous.codex.shutdown_and_wait().await?;
            let mut config = previous.config.clone();
            configure_prewarm(&mut config);
            let forked = previous
                .thread_manager
                .fork_thread(
                    ForkSnapshot::Interrupted,
                    codex_core::StartThreadOptions::new(config.clone()),
                    previous.codex.rollout_path().expect("rollout path"),
                )
                .await?;
            previous.codex = forked.thread;
            previous.session_configured = forked.session_configured;
            previous.config = config;
            previous
        } else {
            builder.restart(&server, &previous).await?
        }
    } else {
        builder.build_with_auto_env(&server).await?
    };
    let warmup = tokio::time::timeout(
        std::time::Duration::from_secs(/*secs*/ 10),
        websocket.wait_for_request(/*connection_index*/ 0, /*request_index*/ 0),
    )
    .await?;
    assert_eq!(warmup.body_json()["generate"], false);
    assert_eq!(warmup.body_json()["reasoning"]["effort"], "medium");
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            effort: Some(Some(ReasoningEffort::High)),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("first message after changing effort")
        .await?;

    let connection = websocket.single_connection();
    assert_eq!(connection.len(), 2);
    let first = connection[1].body_json();
    let updates = first["input"]
        .as_array()
        .expect("first turn input")
        .iter()
        .filter(|item| item["type"] == "configuration_update")
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        (first["reasoning"]["effort"].clone(), updates),
        if feature_enabled {
            (
                Value::from("medium"),
                vec![effort_update(ReasoningEffort::High)],
            )
        } else {
            (Value::from("high"), Vec::new())
        },
    );
    websocket.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reasoning_effort_override_websocket_appends_then_replays_after_reconnect()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let websocket = responses::start_websocket_server(vec![
        vec![
            vec![
                responses::ev_response_created("warmup"),
                responses::ev_completed("warmup"),
            ],
            vec![
                responses::ev_response_created("first"),
                responses::ev_completed("first"),
            ],
            vec![
                responses::ev_response_created("changed"),
                responses::ev_completed("changed"),
            ],
        ],
        vec![vec![
            responses::ev_response_created("replayed"),
            responses::ev_completed("replayed"),
        ]],
    ])
    .await;
    let base_url = format!("{}/v1", websocket.uri());
    let test = override_builder()
        .with_config(move |config| {
            config.model_provider.base_url = Some(base_url);
            config.model_provider.supports_websockets = true;
        })
        .build_with_auto_env(&server)
        .await?;
    let warmup = tokio::time::timeout(
        std::time::Duration::from_secs(/*secs*/ 10),
        websocket.wait_for_request(/*connection_index*/ 0, /*request_index*/ 0),
    )
    .await?;
    assert_eq!(warmup.body_json()["generate"], false);
    test.submit_text_turn("first message").await?;
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            effort: Some(Some(ReasoningEffort::High)),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("changed effort").await?;
    test.submit_text_turn("unchanged effort after reconnect")
        .await?;

    let connections = websocket.connections();
    assert_eq!(connections.iter().map(Vec::len).collect::<Vec<_>>(), [3, 1]);
    let first = connections[0][1].body_json();
    let changed = connections[0][2].body_json();
    let replayed = connections[1][0].body_json();
    assert_eq!(changed["previous_response_id"], "first");
    assert_eq!(replayed.get("previous_response_id"), None);
    assert_eq!(
        responses::strip_response_item_ids_from_json(responses::strip_metadata_from_json(
            changed["input"].clone()
        )),
        serde_json::json!([
            message("user", "changed effort"),
            effort_update(ReasoningEffort::High),
        ])
    );
    assert_eq!(
        replayed["input"]
            .as_array()
            .expect("replayed input")
            .iter()
            .filter(|item| item["type"] == "configuration_update")
            .cloned()
            .collect::<Vec<_>>(),
        [effort_update(ReasoningEffort::High)]
    );
    assert_eq!(
        [&first, &changed, &replayed].map(|body| body["reasoning"]["effort"].clone()),
        [
            Value::from("medium"),
            Value::from("medium"),
            Value::from("medium")
        ]
    );
    websocket.shutdown().await;
    Ok(())
}

#[derive(Clone, Copy)]
enum OverrideUnavailable {
    FeatureDisabled,
    NonOpenAiProvider,
    ResponsesLiteDisabled,
}

#[test_case(OverrideUnavailable::FeatureDisabled; "feature disabled")]
#[test_case(OverrideUnavailable::NonOpenAiProvider; "unsupported provider")]
#[test_case(OverrideUnavailable::ResponsesLiteDisabled; "responses lite disabled")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reasoning_effort_override_unavailable_uses_request_effort(
    unavailable: OverrideUnavailable,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let first = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("first")]),
    )
    .await;
    let second = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("second")]),
    )
    .await;
    let mut builder = match unavailable {
        OverrideUnavailable::FeatureDisabled => override_builder().with_config(|config| {
            config
                .features
                .disable(Feature::ReasoningEffortOverride)
                .expect("disable overrides");
        }),
        OverrideUnavailable::NonOpenAiProvider => override_builder().with_config(|config| {
            config.model_provider.name = "unsupported provider".into();
        }),
        OverrideUnavailable::ResponsesLiteDisabled => override_builder()
            .with_model_info_override("gpt-5.4", |model| model.use_responses_lite = false),
    };
    let test = builder.build_with_auto_env(&server).await?;
    test.submit_text_turn("first").await?;
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            effort: Some(Some(ReasoningEffort::High)),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("second").await?;
    let requests = [first.single_request(), second.single_request()];
    assert_eq!(
        requests.each_ref().map(effort_updates),
        [Vec::<Value>::new(), Vec::new()]
    );
    assert_eq!(
        requests.map(|request| request.body_json()["reasoning"]["effort"].clone()),
        [Value::from("medium"), Value::from("high")],
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reasoning_effort_override_disabled_on_resume_retires_update_at_compaction()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let reply = |id, text| {
        responses::sse(vec![
            responses::ev_assistant_message(id, text),
            responses::ev_completed(id),
        ])
    };
    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            reply("initial", "initial reply"),
            reply("resumed", "resumed reply"),
            responses::sse(vec![
                serde_json::json!({
                    "type": "response.output_item.done",
                    "item": {"type": "compaction", "encrypted_content": "compacted-history"},
                }),
                responses::ev_completed("compact"),
            ]),
            reply("after", "after compaction reply"),
        ],
    )
    .await;
    let initial = override_builder().build_with_auto_env(&server).await?;
    initial.submit_text_turn("before resume").await?;
    let chatgpt_base_url = format!("{}/backend-api", server.uri());
    let resumed = override_builder()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            config
                .features
                .disable(Feature::ReasoningEffortOverride)
                .expect("disable overrides");
            config.model_reasoning_effort = Some(ReasoningEffort::High);
            config.chatgpt_base_url = chatgpt_base_url;
        })
        .restart(&server, &initial)
        .await?;
    resumed.submit_text_turn("after resume").await?;
    resumed.codex.submit(Op::Compact).await?;
    wait_for_event(&resumed.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    resumed.submit_text_turn("after compaction").await?;

    let requests = mock.requests();
    assert_eq!(
        requests
            .iter()
            .map(|request| (
                request.body_json()["reasoning"]["effort"].clone(),
                effort_updates(request),
            ))
            .collect::<Vec<_>>(),
        [
            (
                Value::from("medium"),
                vec![effort_update(ReasoningEffort::Medium)]
            ),
            (
                Value::from("high"),
                vec![effort_update(ReasoningEffort::Medium)]
            ),
            (
                Value::from("high"),
                vec![effort_update(ReasoningEffort::Medium)]
            ),
            (Value::from("high"), Vec::new()),
        ],
    );
    assert!(
        requests[3]
            .input()
            .iter()
            .any(|item| item["type"] == "compaction")
    );
    Ok(())
}

#[test_case(ReasoningEffort::Medium; "same selection")]
#[test_case(ReasoningEffort::High; "changed selection")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reasoning_effort_override_model_switch_reestablishes_selected_effort(
    effort: ReasoningEffort,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let first = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("first")]),
    )
    .await;
    let second = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("second")]),
    )
    .await;
    let test = override_builder()
        .with_model_info_override("gpt-5.4", |model| model.comp_hash = None)
        .with_model_info_override("gpt-5.5", |model| {
            model.comp_hash = None;
            model.use_responses_lite = true;
        })
        .with_model("gpt-5.4")
        .build_with_auto_env(&server)
        .await?;
    test.submit_text_turn("first model").await?;
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some("gpt-5.5".to_string()),
            effort: Some(Some(effort.clone())),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("second model").await?;
    assert_eq!(
        effort_updates(&first.single_request()),
        [effort_update(ReasoningEffort::Medium)]
    );
    let request = second.single_request();
    assert_eq!(
        effort_updates(&request),
        [
            effort_update(ReasoningEffort::Medium),
            effort_update(effort.clone())
        ]
    );
    assert_eq!(
        serde_json::json!([
            request.body_json()["model"],
            request.body_json()["reasoning"]["effort"]
        ]),
        serde_json::json!(["gpt-5.5", effort])
    );
    Ok(())
}

#[test_case(ReasoningEffort::High; "unchanged effort")]
#[test_case(ReasoningEffort::Medium; "changed effort")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reasoning_effort_override_resume_refreshes_selected_effort(
    effort: ReasoningEffort,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let first = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("first")]),
    )
    .await;
    let second = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("second")]),
    )
    .await;
    let resumed = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("resumed")]),
    )
    .await;
    let test = override_builder().build_with_auto_env(&server).await?;
    test.submit_text_turn("medium turn").await?;
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            effort: Some(Some(ReasoningEffort::High)),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("high turn").await?;
    let selected = effort.clone();
    let restarted = override_builder()
        .with_config(move |config| {
            config.model_reasoning_effort = Some(selected);
        })
        .restart(&server, &test)
        .await?;
    restarted.submit_text_turn("after resume").await?;

    assert_eq!(
        [first.single_request(), second.single_request()]
            .map(|request| request.body_json()["reasoning"]["effort"].clone()),
        [Value::from("medium"), Value::from("medium")]
    );
    let request = resumed.single_request();
    assert_eq!(request.body_json()["reasoning"]["effort"], effort.as_str());
    // Even an unchanged selection must refresh the override when replay invalidates the pin.
    assert_eq!(
        effort_updates(&request),
        [
            effort_update(ReasoningEffort::Medium),
            effort_update(ReasoningEffort::High),
            effort_update(effort.clone())
        ]
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reasoning_effort_override_compaction_fallback_uses_each_models_effort()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let first = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("first")]),
    )
    .await;
    let second = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("second")]),
    )
    .await;
    let compaction = serde_json::json!({
        "type": "compaction", "encrypted_content": "fallback-summary"
    });
    let failure = ResponseTemplate::new(/*s*/ 400).set_body_json(serde_json::json!({
        "error": {"message": "previous model cannot compact this history"}
    }));
    let compactions = responses::mount_response_sequence(
        &server,
        vec![
            failure,
            responses::sse_response(responses::sse(vec![
                serde_json::json!({"type": "response.output_item.done", "item": compaction}),
                responses::ev_completed("fallback"),
            ])),
        ],
    )
    .await;
    let after = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("after")]),
    )
    .await;
    let chatgpt_base_url = format!("{}/backend-api", server.uri());
    let test = override_builder()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model_info_override("gpt-5.4", |model| {
            model.comp_hash = Some("original".to_string());
        })
        .with_model_info_override("gpt-5.2", |model| {
            model.comp_hash = Some("fallback".to_string());
            model.use_responses_lite = true;
        })
        .with_model("gpt-5.4")
        .with_config(move |config| {
            config.chatgpt_base_url = chatgpt_base_url;
            config.model_provider.stream_max_retries = Some(0);
        })
        .build_with_auto_env(&server)
        .await?;
    test.submit_text_turn("first").await?;
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            effort: Some(Some(ReasoningEffort::High)),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("second").await?;
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some("gpt-5.2".to_string()),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("switch model").await?;

    let compaction_requests = compactions.requests();
    assert_eq!(compaction_requests.len(), 2);
    let requests = [
        first.single_request(),
        second.single_request(),
        compaction_requests[0].clone(),
        compaction_requests[1].clone(),
        after.single_request(),
    ];
    assert_eq!(
        requests.each_ref().map(|request| {
            let body = request.body_json();
            serde_json::json!([body["model"], body["reasoning"]["effort"]])
        }),
        [
            serde_json::json!(["gpt-5.4", "medium"]),
            serde_json::json!(["gpt-5.4", "medium"]),
            serde_json::json!(["gpt-5.4", "medium"]),
            serde_json::json!(["gpt-5.2", "high"]),
            serde_json::json!(["gpt-5.2", "high"]),
        ]
    );
    assert_eq!(
        requests.each_ref().map(effort_updates),
        [
            vec![effort_update(ReasoningEffort::Medium)],
            vec![
                effort_update(ReasoningEffort::Medium),
                effort_update(ReasoningEffort::High)
            ],
            vec![
                effort_update(ReasoningEffort::Medium),
                effort_update(ReasoningEffort::High)
            ],
            vec![
                effort_update(ReasoningEffort::Medium),
                effort_update(ReasoningEffort::High)
            ],
            vec![],
        ]
    );
    Ok(())
}

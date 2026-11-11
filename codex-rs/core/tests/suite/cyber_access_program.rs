use anyhow::Result;
use codex_core::RecoverTurnRequest;
use codex_core::StartIfIdleSubmission;
use codex_core::TurnInput;
use codex_core::TurnInputRequest;
use codex_core::TurnInputSubmission;
use codex_core::TurnStartOptions;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelAccessPrograms;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnSettingsUpdate;
use codex_protocol::protocol::TurnSettingsUpdateOutcome;
use codex_protocol::turn_input::CyberAccessProgram;
use codex_protocol::user_input::UserInput;
use codex_rollout::RolloutItem;
use codex_rollout::RolloutRecorder;
use codex_thread_store::ThreadMetadataPatch;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::SEARCH_CALENDAR_CREATE_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_NAMESPACE;
use core_test_support::apps_test_server::search_capable_apps_builder;
use core_test_support::responses;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_successful_turn;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;
use tokio::sync::oneshot;
use wiremock::Mock;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[test_case(false; "user_input")]
#[test_case(true; "standalone_tool_output")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recover_turn_restores_start_options_without_making_them_sticky(
    standalone_tool_output: bool,
) -> Result<()> {
    // Keep sampling pending until interruption so restart must reload the unfinished turn.
    let (release_response, response_gate) = oneshot::channel();
    let (initial_server, _completions) = start_streaming_sse_server(vec![
        vec![StreamingSseChunk {
            gate: None,
            body: responses::sse_completed("seed"),
        }],
        vec![StreamingSseChunk {
            gate: Some(response_gate),
            body: responses::sse(vec![responses::ev_completed("resp-initial")]),
        }],
    ])
    .await;
    let server = responses::start_mock_server().await;
    let apps = AppsTestServer::mount_searchable(&server).await?;
    let builder = || {
        search_capable_apps_builder(apps.chatgpt_base_url.clone()).with_config(|config| {
            config
                .features
                .enable(Feature::CodeModeOnly)
                .expect("enable code mode only");
            config.code_mode.direct_only_tool_namespaces =
                vec![SEARCH_CALENDAR_NAMESPACE.to_owned()];
        })
    };
    let initial = builder()
        .build_with_streaming_server(&initial_server)
        .await?;
    initial.submit_text_turn("seed user turn").await?;
    let request = if standalone_tool_output {
        TurnInputRequest::new(TurnInput::ResponseItem(serde_json::from_value(json!({
            "type": "function_call_output", "name": "result", "namespace": "example",
            "output": "recover this tool output",
        }))?))
    } else {
        TurnInputRequest::user_input(vec![UserInput::Text {
            text: "recover this turn".to_owned(),
            text_elements: Vec::new(),
        }])
    };
    let TurnInputSubmission::Started { turn_id, .. } = initial
        .codex
        .start_or_steer_turn(request.on_start(TurnStartOptions {
            direct_only_tool_namespaces: Some(Vec::new()),
            cyber_access_program: Some(CyberAccessProgram::DaybreakBlue),
            ..Default::default()
        }))
        .await?
    else {
        panic!("expected a new turn");
    };
    initial_server.wait_for_request_count(/*count*/ 2).await;
    initial.codex.submit(Op::Interrupt).await?;
    wait_for_event(&initial.codex, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;
    drop(release_response);
    initial_server.shutdown().await;
    initial.codex.flush_rollout().await?;

    let test = builder().restart(&server, &initial).await?;
    let (rollout, _, _) = RolloutRecorder::load_rollout_items(
        &test
            .codex
            .rollout_path()
            .expect("recovered turn rollout path"),
    )
    .await?;
    let persisted_context = rollout
        .iter()
        .rev()
        .find_map(|item| match item {
            RolloutItem::TurnContext(context)
                if context.turn_id.as_deref() == Some(turn_id.as_str()) =>
            {
                Some(context)
            }
            _ => None,
        })
        .expect("persisted context for the interrupted turn");
    assert_eq!(
        persisted_context.cyber_access_program,
        Some(CyberAccessProgram::DaybreakBlue)
    );

    assert_eq!(
        persisted_context.direct_only_tool_namespaces,
        Some(Vec::new())
    );

    let response_mock =
        responses::mount_sse_once(&server, responses::sse_completed("resp-1")).await;
    let submission = test
        .codex
        .recover_turn_if_idle(RecoverTurnRequest {
            turn_id: turn_id.clone(),
            thread_settings: Default::default(),
            trace: None,
            prompt_cache_ttl: None,
            cyber_access_program: persisted_context.cyber_access_program,
        })
        .await?;
    assert_eq!(
        submission,
        StartIfIdleSubmission::Started {
            root_turn_id: turn_id.clone(),
            turn_id: turn_id.clone()
        }
    );
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(
        response_mock.single_request().body_json()["access_programs"],
        json!({"cyber": "daybreak_blue"})
    );
    let recovered_request = response_mock.single_request();
    let recovered_input_count = if standalone_tool_output {
        recovered_request
            .inputs_of_type("function_call_output")
            .iter()
            .filter(|item| item["name"] == "result")
            .count()
    } else {
        recovered_request
            .message_input_texts("user")
            .iter()
            .filter(|text| text.as_str() == "recover this turn")
            .count()
    };
    assert_eq!(recovered_input_count, 1);

    let declares_calendar = |body: &serde_json::Value| {
        responses::namespace_child_tool(
            body,
            SEARCH_CALENDAR_NAMESPACE,
            SEARCH_CALENDAR_CREATE_TOOL,
        )
        .is_some()
    };
    assert!(!declares_calendar(
        &response_mock.single_request().body_json()
    ));

    let next_response = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("resp-next")]),
    )
    .await;
    let TurnInputSubmission::Started {
        turn_id: next_turn_id,
        ..
    } = test
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "start a new turn".to_owned(),
            text_elements: Vec::new(),
        }]))
        .await?
    else {
        panic!("expected a new turn");
    };
    assert_ne!(next_turn_id, turn_id);
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(
        next_response
            .single_request()
            .body_json()
            .get("access_programs"),
        None
    );
    assert!(declares_calendar(
        &next_response.single_request().body_json()
    ));
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[test_case(None; "omitted")]
#[test_case(Some(false); "disabled")]
#[test_case(Some(true); "enabled")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cyber_access_program_custom_provider_requires_capability(
    capability: Option<bool>,
) -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    for auth in [
        CodexAuth::from_api_key("test-key"),
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    ] {
        let expected_authorization = if auth.is_api_key_auth() {
            "Bearer test-key"
        } else {
            "Bearer Access Token"
        };
        let server = responses::start_mock_server().await;
        let request = responses::mount_sse_once(
            &server,
            responses::sse(vec![responses::ev_completed("resp-1")]),
        )
        .await;
        let test = test_codex()
            .with_auth(auth)
            .with_config(move |config| {
                // Keep the display name "OpenAI": provider identity must not use it.
                config.model_provider_id = "custom".to_owned();
                config.model_provider.capabilities =
                    Some(codex_model_provider_info::ModelProviderCapabilities {
                        cyber_access_programs: capability,
                        ..Default::default()
                    });
                config
                    .features
                    .enable(Feature::ApiKeyCyberAccessPrograms)
                    .expect("enable API-key Cyber access programs");
            })
            .build_with_auto_env(&server)
            .await?;
        submit(&test, Some(CyberAccessProgram::DaybreakRed)).await?;
        assert_eq!(
            request.single_request().header("authorization").as_deref(),
            Some(expected_authorization)
        );
        assert_eq!(
            request
                .single_request()
                .body_json()
                .get("access_programs")
                .cloned(),
            (capability == Some(true)).then(|| json!({"cyber": "daybreak_red"}))
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cyber_access_program_custom_provider_preserves_api_key_feature_gate() -> Result<()> {
    let server = responses::start_mock_server().await;
    let test = test_codex()
        .with_auth(CodexAuth::from_api_key("test-api-key"))
        .with_config(|config| {
            config.model_provider_id = "proxy".to_owned();
            config.model_provider.capabilities =
                Some(codex_model_provider_info::ModelProviderCapabilities {
                    cyber_access_programs: Some(true),
                    ..Default::default()
                });
            config
                .features
                .disable(Feature::ApiKeyCyberAccessPrograms)
                .unwrap();
        })
        .build_with_auto_env(&server)
        .await?;
    let error = submit(&test, Some(CyberAccessProgram::DaybreakBlue))
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Cyber access programs are disabled for this API-key session.")
    );
    assert!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .all(|request| !request.url.path().ends_with("/responses"))
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[test_case(CodexAuth::create_dummy_chatgpt_auth_for_testing(), "openai"; "chatgpt")]
#[test_case(CodexAuth::from_api_key("test-api-key"), "openai"; "api_key")]
#[test_case(CodexAuth::create_dummy_chatgpt_auth_for_testing(), "proxy"; "custom_chatgpt")]
#[test_case(CodexAuth::from_api_key("test-api-key"), "proxy"; "custom_api_key")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cyber_access_program_survives_mid_turn_remote_compaction_v2(
    auth: CodexAuth,
    provider_id: &'static str,
) -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let requests = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call("call-before-compact", "test_tool", "{}"),
                responses::ev_completed_with_tokens("resp-1", /*total_tokens*/ 500),
            ]),
            responses::sse(vec![
                serde_json::json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "compaction",
                        "encrypted_content": "V2_COMPACT_SUMMARY",
                    }
                }),
                responses::ev_completed("resp-compact"),
            ]),
            responses::sse(vec![responses::ev_completed_with_tokens(
                "resp-2", /*total_tokens*/ 80,
            )]),
        ],
    )
    .await;
    let test = test_codex()
        .with_auth(auth)
        .with_config(move |config| {
            config.model_provider_id = provider_id.to_owned();
            if provider_id == "proxy" {
                config.model_provider.capabilities =
                    Some(codex_model_provider_info::ModelProviderCapabilities {
                        cyber_access_programs: Some(true),
                        ..Default::default()
                    });
            }
            config.model_auto_compact_token_limit = Some(200);
            config
                .features
                .enable(Feature::ApiKeyModelDiscovery)
                .expect("enable API-key model discovery");
            config
                .features
                .enable(Feature::ApiKeyCyberAccessPrograms)
                .expect("enable API-key Cyber access programs");
        })
        .build_with_auto_env(&server)
        .await?;

    submit(&test, Some(CyberAccessProgram::DaybreakBlue)).await?;

    let requests = requests.requests();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.body_json()["access_programs"].clone())
            .collect::<Vec<_>>(),
        vec![json!({"cyber": "daybreak_blue"}); 3]
    );
    assert_eq!(requests[1].inputs_of_type("compaction_trigger").len(), 1);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cyber_access_program_is_inherited_by_child_turns() -> Result<()> {
    // Final answers defer late child completion mail to the next parent turn.
    let final_response = |id: &str| {
        responses::sse(vec![
            responses::ev_response_created(id),
            responses::ev_assistant_message(&format!("message-{id}"), "Done."),
            responses::ev_completed(id),
        ])
    };
    for (namespace, fork_turns) in [
        ("collaboration", "none"),
        ("collaboration", "all"),
        ("multi_agent_v1", "none"),
    ] {
        let is_v2 = namespace == "collaboration";
        let spawn_arguments = if is_v2 {
            json!({"message": "inspect the repository", "task_name": "worker", "fork_turns": fork_turns})
        } else {
            json!({"message": "inspect the repository"})
        };
        let server = responses::start_mock_server().await;
        // V1 completion notifications can require another parent response.
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .and(|request: &wiremock::Request| !request.headers.contains_key("x-openai-subagent"))
            .respond_with(responses::sse_response(final_response("resp-parent-done")))
            .with_priority(/*p*/ 10)
            .mount(&server)
            .await;
        responses::mount_sse_once(
            &server,
            responses::sse(vec![
                responses::ev_function_call_with_namespace(
                    "spawn-worker",
                    namespace,
                    "spawn_agent",
                    &spawn_arguments.to_string(),
                ),
                responses::ev_completed("resp-spawn"),
            ]),
        )
        .await;
        let initial_child_request = responses::mount_sse_once_match(
            &server,
            header("x-openai-subagent", "collab_spawn"),
            final_response("resp-child-initial"),
        )
        .await;
        let test = test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_model(if is_v2 { "gpt-5.6-sol" } else { "gpt-5.1" })
            .with_config(move |config| {
                config
                    .features
                    .enable(Feature::Collab)
                    .expect("enable multi-agent tools");
                if is_v2 {
                    config
                        .features
                        .enable(Feature::MultiAgentV2)
                        .expect("enable v2 tools");
                }
            })
            .build_with_auto_env(&server)
            .await?;
        let mut created_threads = test.thread_manager.subscribe_thread_created();
        submit(&test, Some(CyberAccessProgram::DaybreakRed)).await?;
        let child_id = created_threads.recv().await?;
        let child = test.thread_manager.get_thread(child_id).await?;
        wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;

        let child_programs = |requests: &responses::ResponseMock| {
            requests
                .requests()
                .iter()
                .filter(|request| {
                    request.body_json()["client_metadata"]["thread_id"] == json!(child_id)
                })
                .map(|request| request.body_json()["access_programs"].clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            child_programs(&initial_child_request),
            vec![json!({"cyber": "daybreak_red"})],
            "namespace={namespace}, fork_turns={fork_turns}"
        );

        for (program, expected, reload) in [
            (
                Some(CyberAccessProgram::Standard),
                json!({"cyber": "standard"}),
                false,
            ),
            (
                Some(CyberAccessProgram::DaybreakBlue),
                json!({"cyber": "daybreak_blue"}),
                true,
            ),
            (None, json!(null), false),
            (
                Some(CyberAccessProgram::Standard),
                json!({"cyber": "standard"}),
                true,
            ),
        ] {
            if reload {
                let child = test.thread_manager.get_thread(child_id).await?;
                child.shutdown_and_wait().await?;
                test.thread_manager.remove_thread(&child_id).await;
            }
            let mut reply_sequence = Vec::new();
            if reload && !is_v2 {
                reply_sequence.push(responses::sse(vec![
                    responses::ev_function_call_with_namespace(
                        "resume-worker",
                        namespace,
                        "resume_agent",
                        &json!({"id": child_id}).to_string(),
                    ),
                    responses::ev_completed("resp-resume"),
                ]));
            }
            reply_sequence.push(responses::sse(vec![
                responses::ev_function_call_with_namespace(
                    "followup-worker",
                    namespace,
                    if is_v2 { "followup_task" } else { "send_input" },
                    &json!({
                        "target": if is_v2 { "worker".to_string() } else { child_id.to_string() },
                        "message": "inspect the tests too",
                    })
                    .to_string(),
                ),
                responses::ev_completed("resp-followup"),
            ]));
            responses::mount_sse_sequence(&server, reply_sequence).await;
            let followup_child_request = responses::mount_sse_once_match(
                &server,
                header("x-openai-subagent", "collab_spawn"),
                final_response("resp-child-next"),
            )
            .await;
            submit(&test, program).await?;
            let child = test.thread_manager.get_thread(child_id).await?;
            wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
            assert_eq!(
                child_programs(&followup_child_request),
                vec![expected],
                "namespace={namespace}, fork_turns={fork_turns}, reload={reload}"
            );
        }
    }
    Ok(())
}

#[test_case(CodexAuth::create_dummy_chatgpt_auth_for_testing(), "openai"; "chatgpt")]
#[test_case(CodexAuth::from_api_key("test-api-key"), "openai"; "api_key")]
#[test_case(CodexAuth::create_dummy_chatgpt_auth_for_testing(), "proxy"; "custom_chatgpt")]
#[test_case(CodexAuth::from_api_key("test-api-key"), "proxy"; "custom_api_key")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cyber_access_program_changes_on_one_websocket_with_response_reuse(
    auth: CodexAuth,
    provider_id: &'static str,
) -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let response_ids = [
        "prewarm", "first", "second", "blue", "red", "off", "omitted",
    ];
    let server = responses::start_websocket_server(vec![
        response_ids
            .iter()
            .map(|id| vec![responses::ev_completed(id)])
            .collect(),
    ])
    .await;
    let test = test_codex()
        .with_auth(auth)
        .with_config(move |config| {
            config.model_provider_id = provider_id.to_owned();
            if provider_id == "proxy" {
                config.model_provider.capabilities =
                    Some(codex_model_provider_info::ModelProviderCapabilities {
                        cyber_access_programs: Some(true),
                        ..Default::default()
                    });
            }
            config
                .features
                .enable(Feature::ApiKeyModelDiscovery)
                .expect("enable API-key model discovery");
            config
                .features
                .enable(Feature::ApiKeyCyberAccessPrograms)
                .expect("enable API-key Cyber access programs");
        })
        .build_with_websocket_server(&server)
        .await?;
    for program in [
        Some(CyberAccessProgram::Standard),
        Some(CyberAccessProgram::Standard),
        Some(CyberAccessProgram::DaybreakBlue),
        Some(CyberAccessProgram::DaybreakRed),
        Some(CyberAccessProgram::Standard),
        None,
    ] {
        submit(&test, program).await?;
    }

    let requests = server.single_connection();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.body_json().get("access_programs").cloned())
            .collect::<Vec<_>>(),
        [
            None,
            Some(json!({"cyber": "standard"})),
            Some(json!({"cyber": "standard"})),
            Some(json!({"cyber": "daybreak_blue"})),
            Some(json!({"cyber": "daybreak_red"})),
            Some(json!({"cyber": "standard"})),
            None,
        ]
    );
    // Each choice is sent alongside the previous response id, not a full replay.
    assert_eq!(
        requests
            .iter()
            .skip(1)
            .map(|request| request.body_json().get("previous_response_id").cloned())
            .collect::<Vec<_>>(),
        response_ids[..response_ids.len() - 1]
            .iter()
            .map(|id| Some(json!(id)))
            .collect::<Vec<_>>()
    );
    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}

#[test_case(None; "account changed")]
#[test_case(Some(CyberAccessProgram::DaybreakRed); "saved off without Standard")]
#[test_case(Some(CyberAccessProgram::Standard); "saved on without Daybreak")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn saved_daybreak_rejection_records_child_input(
    only_program: Option<CyberAccessProgram>,
) -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let requests = responses::mount_response_handler(&server, |request| {
        let output = if request.header("x-openai-subagent").is_none()
            && request.function_call_output_text("spawn-worker").is_none()
        {
            responses::ev_function_call_with_namespace(
                "spawn-worker",
                "collaboration",
                "spawn_agent",
                &json!({"task_name": "worker", "message": "inspect", "fork_turns": "none"})
                    .to_string(),
            )
        } else {
            responses::ev_assistant_message("done", "Done.")
        };
        responses::sse_response(responses::sse(vec![
            output,
            responses::ev_completed("done"),
        ]))
    })
    .await;
    let test = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model_info_override("gpt-5.4", move |model| {
            model.available_access_programs = Some(ModelAccessPrograms {
                cyber: only_program.map_or_else(
                    || {
                        vec![
                            CyberAccessProgram::DaybreakBlue,
                            CyberAccessProgram::Standard,
                        ]
                    },
                    |program| vec![program],
                ),
            });
            model.use_responses_lite = false;
        })
        .with_config(|config| {
            for feature in [
                Feature::Collab,
                Feature::MultiAgentV2,
                Feature::DaybreakProgramPropagation,
            ] {
                config
                    .features
                    .enable(feature)
                    .expect("enable saved choice test");
            }
        })
        .build_with_auto_env(&server)
        .await?;
    test.codex.ensure_rollout_materialized().await;
    test.codex
        .update_thread_metadata(
            ThreadMetadataPatch {
                daybreak_enabled: Some(only_program != Some(CyberAccessProgram::Standard)),
                ..Default::default()
            },
            /*include_archived*/ false,
        )
        .await?;
    let mut created_threads = test.thread_manager.subscribe_thread_created();
    submit(&test, /*program*/ None).await?;
    let child = test
        .thread_manager
        .get_thread(created_threads.recv().await?)
        .await?;
    wait_for_successful_turn(&child).await;
    let before = requests.requests();
    assert_eq!(
        before
            .iter()
            .filter(|request| request.header("x-openai-subagent").is_some())
            .map(|request| request.body_json()["access_programs"].clone())
            .collect::<Vec<_>>(),
        vec![json!({"cyber": only_program.unwrap_or(CyberAccessProgram::DaybreakBlue)})]
    );

    let accepted_text = "First accepted follow-up.";
    child
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: accepted_text.to_owned(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_successful_turn(&child).await;
    let requests_before_rejection = requests.requests().len();

    let expected_error = if let Some(program) = only_program {
        child
            .update_thread_metadata(
                ThreadMetadataPatch {
                    daybreak_enabled: Some(program == CyberAccessProgram::Standard),
                    ..Default::default()
                },
                /*include_archived*/ false,
            )
            .await?;
        if program == CyberAccessProgram::Standard {
            "does not advertise Daybreak"
        } else {
            "does not advertise Standard"
        }
    } else {
        let auth = test.thread_manager.auth_manager();
        let previous_owner = auth.auth_owner();
        codex_login::auth::login_with_chatgpt_auth_tokens(
            test.home.path(),
            "header.e30.changed",
            "other-account",
            /*chatgpt_plan_type*/ None,
        )?;
        auth.reload().await;
        assert!(!previous_owner.is_current());
        assert_eq!(
            auth.auth_cached().and_then(|auth| auth.get_account_id()),
            Some("other-account".to_owned())
        );

        "saved Daybreak choice has changed"
    };
    let rejected_text = "Follow-up with a rejected saved choice.";
    child
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: rejected_text.to_owned(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let completed =
        wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    assert!(
        matches!(&completed, EventMsg::TurnComplete(turn)
        if turn.error.as_ref().is_some_and(|error| error.message.contains(expected_error))),
        "expected {expected_error}: {completed:?}"
    );
    let assert_history = async |thread: &codex_core::CodexThread| {
        let history = thread.conversation_history_snapshot().await;
        for expected in [accepted_text, rejected_text] {
            let input = ContentItem::InputText {
                text: expected.to_owned(),
            };
            let count = history.items().filter(|item| {
                matches!(item, ResponseItem::Message { role, content, .. } if role == "user" && content.contains(&input))
            }).count();
            assert_eq!(count, 1, "{expected}");
        }
    };
    assert_history(&child).await;
    let (child_id, history) =
        super::agents_md::persisted_resume_history(&child, test.thread_store.as_ref()).await?;
    child.shutdown_and_wait().await?;
    test.thread_manager.remove_thread(&child_id).await;
    assert_eq!(requests.requests().len(), requests_before_rejection);
    let resumed = test
        .thread_manager
        .resume_thread_with_history(
            test.config.clone(),
            history,
            test.thread_manager.auth_manager(),
            /*parent_trace*/ None,
            Default::default(),
        )
        .await?;
    assert_history(&resumed.thread).await;
    resumed.thread.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn saved_daybreak_uses_existing_websocket_requests() -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = responses::start_websocket_server(vec![
        vec![
            vec![responses::ev_completed("parent-warm")],
            vec![
                responses::ev_function_call_with_namespace(
                    "spawn-worker",
                    "collaboration",
                    "spawn_agent",
                    &json!({"task_name": "worker", "message": "inspect", "fork_turns": "none"})
                        .to_string(),
                ),
                responses::ev_completed("parent-spawn"),
            ],
            vec![
                responses::ev_assistant_message("parent-done", "Done."),
                responses::ev_completed("parent-done"),
            ],
        ],
        vec![
            vec![responses::ev_completed("child-warm")],
            vec![
                responses::ev_function_call("pause", "test_tool", "{}"),
                responses::ev_completed("child-first"),
            ],
            vec![responses::ev_completed("child-on")],
            vec![responses::ev_completed("child-off")],
        ],
    ])
    .await;
    let release_child = server.gate_response_event(&responses::ev_completed("child-first"));
    let test = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model_info_override("gpt-5.4", |model| {
            model.available_access_programs = Some(ModelAccessPrograms {
                cyber: vec![
                    CyberAccessProgram::DaybreakBlue,
                    CyberAccessProgram::Standard,
                ],
            });
            model.use_responses_lite = false;
        })
        .with_config(|config| {
            for feature in [
                Feature::Collab,
                Feature::MultiAgentV2,
                Feature::DaybreakProgramPropagation,
                Feature::StepModelSwitching,
            ] {
                config
                    .features
                    .enable(feature)
                    .expect("enable saved choice test");
            }
        })
        .build_with_websocket_server(&server)
        .await?;
    test.codex.ensure_rollout_materialized().await;
    test.codex
        .update_thread_metadata(
            ThreadMetadataPatch {
                daybreak_enabled: Some(true),
                ..Default::default()
            },
            /*include_archived*/ false,
        )
        .await?;
    let mut created_threads = test.thread_manager.subscribe_thread_created();
    submit(&test, /*program*/ None).await?;
    let child = test
        .thread_manager
        .get_thread(created_threads.recv().await?)
        .await?;
    let EventMsg::TurnStarted(started) =
        wait_for_event(&child, |event| matches!(event, EventMsg::TurnStarted(_))).await
    else {
        unreachable!("waited for turn start");
    };
    server
        .wait_for_request(/*connection_index*/ 1, /*request_index*/ 1)
        .await;
    let (reply, outcome) = oneshot::channel();
    child
        .submit(Op::TurnSettings {
            turn_id: started.turn_id,
            update: TurnSettingsUpdate {
                model: Some("gpt-5.5".to_owned()),
                ..Default::default()
            },
            reply,
        })
        .await?;
    assert!(
        matches!(outcome.await?, TurnSettingsUpdateOutcome::Rejected { reason } if reason.contains("next turn"))
    );

    // A saved toggle during this turn applies to the next turn, not the next request.
    child
        .update_thread_metadata(
            ThreadMetadataPatch {
                daybreak_enabled: Some(false),
                ..Default::default()
            },
            /*include_archived*/ false,
        )
        .await?;
    release_child.send(()).expect("release child response");
    wait_for_successful_turn(&child).await;
    child
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Continue with the new choice.".to_owned(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_successful_turn(&child).await;

    let requests = server.connections();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1]
            .iter()
            .map(|request| {
                let body = request.body_json();
                json!([body["access_programs"], body["previous_response_id"]])
            })
            .collect::<Vec<_>>(),
        vec![
            json!([null, null]),
            json!([{"cyber": "daybreak_blue"}, "child-warm"]),
            json!([{"cyber": "daybreak_blue"}, "child-first"]),
            json!([{"cyber": "standard"}, "child-on"]),
        ]
    );
    child.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}

#[test_case("multi_agent_v1", true, true, Some(true), CyberAccessProgram::DaybreakBlue, CyberAccessProgram::DaybreakRed; "v1 blue to red")]
#[test_case("multi_agent_v1", true, true, Some(true), CyberAccessProgram::DaybreakRed, CyberAccessProgram::DaybreakBlue; "v1 red to blue")]
#[test_case("collaboration", true, true, Some(true), CyberAccessProgram::DaybreakBlue, CyberAccessProgram::DaybreakRed; "v2 blue to red")]
#[test_case("collaboration", true, true, Some(true), CyberAccessProgram::DaybreakRed, CyberAccessProgram::DaybreakBlue; "v2 red to blue")]
#[test_case("collaboration", true, true, Some(false), CyberAccessProgram::DaybreakBlue, CyberAccessProgram::DaybreakRed; "turn override wins over saved off")]
#[test_case("collaboration", true, true, Some(true), CyberAccessProgram::Standard, CyberAccessProgram::DaybreakRed; "standard override wins over saved on")]
#[test_case("collaboration", true, true, Some(false), CyberAccessProgram::Standard, CyberAccessProgram::DaybreakRed; "standard with saved off")]
#[test_case("collaboration", false, true, Some(true), CyberAccessProgram::DaybreakBlue, CyberAccessProgram::DaybreakRed; "rollout disabled")]
#[test_case("collaboration", true, true, None, CyberAccessProgram::DaybreakBlue, CyberAccessProgram::DaybreakRed; "choice unset")]
#[test_case("collaboration", true, false, Some(true), CyberAccessProgram::DaybreakBlue, CyberAccessProgram::DaybreakRed; "provider opt out")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn saved_daybreak_follows_the_child_model_and_automatic_compaction(
    namespace: &'static str,
    propagation_enabled: bool,
    provider_programs: bool,
    saved_choice: Option<bool>,
    parent_program: CyberAccessProgram,
    child_program: CyberAccessProgram,
) -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let parent_model = "gpt-5.6-sol";
    let child_model = "gpt-5.6-cyber";
    let parent_daybreak = match parent_program {
        CyberAccessProgram::Standard => CyberAccessProgram::DaybreakBlue,
        program => program,
    };
    let is_v2 = namespace == "collaboration";
    let mut spawn = json!({"message": "inspect the repository", "model": child_model});
    if is_v2 {
        spawn["task_name"] = json!("worker");
        spawn["fork_turns"] = json!("none");
    }
    let summary = json!({"type": "response.output_item.done", "item": {
        "type": "compaction", "encrypted_content": "CHILD_SUMMARY"
    }});
    let requests = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call("before-compact", "test_tool", "{}"),
                responses::ev_completed_with_tokens("child-first", /*total_tokens*/ 500_000),
            ]),
            responses::sse(vec![
                summary.clone(),
                responses::ev_completed_with_tokens("compact", /*total_tokens*/ 10),
            ]),
            responses::sse(vec![responses::ev_completed_with_tokens(
                "child-done",
                /*total_tokens*/ 80,
            )]),
            responses::sse(vec![
                summary.clone(),
                responses::ev_completed_with_tokens("old-model-compact", /*total_tokens*/ 10),
            ]),
            responses::sse(vec![
                responses::ev_function_call("before-derived-compact", "test_tool", "{}"),
                responses::ev_completed_with_tokens(
                    "new-model-first",
                    /*total_tokens*/ 500_000,
                ),
            ]),
            responses::sse(vec![
                summary,
                responses::ev_completed_with_tokens(
                    "derived-program-compact",
                    /*total_tokens*/ 10,
                ),
            ]),
            responses::sse(vec![responses::ev_completed_with_tokens(
                "new-model-done",
                /*total_tokens*/ 80,
            )]),
            responses::sse(vec![responses::ev_completed_with_tokens(
                "explicit-done",
                /*total_tokens*/ 80,
            )]),
        ],
    )
    .await;
    let version = if is_v2 {
        MultiAgentVersion::V2
    } else {
        MultiAgentVersion::V1
    };
    let test = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model_info_override(parent_model, move |model| {
            model.available_access_programs = Some(ModelAccessPrograms {
                cyber: vec![parent_daybreak, CyberAccessProgram::Standard],
            });
            model.comp_hash = Some("parent-window".to_owned());
            model.multi_agent_version = Some(version);
            model.use_responses_lite = false;
        })
        .with_model_info_override(child_model, move |model| {
            // An enabled choice must work even when this model only accepts Red or Blue.
            model.available_access_programs = Some(ModelAccessPrograms {
                cyber: if saved_choice == Some(false)
                    || parent_program == CyberAccessProgram::Standard
                {
                    vec![child_program, CyberAccessProgram::Standard]
                } else {
                    vec![child_program]
                },
            });
            model.comp_hash = Some("child-window".to_owned());
            model.multi_agent_version = Some(version);
            model.use_responses_lite = false;
        })
        .with_model(parent_model)
        .with_config(move |config| {
            config
                .features
                .enable(Feature::Collab)
                .expect("enable spawn");
            config
                .features
                .set_enabled(Feature::MultiAgentV2, is_v2)
                .expect("set spawn version");
            config
                .features
                .set_enabled(Feature::DaybreakProgramPropagation, propagation_enabled)
                .expect("set propagation");
            config.multi_agent_v2.expose_spawn_agent_model_overrides = true;
            config.model_auto_compact_token_limit = Some(200_000);
            if !provider_programs {
                config.model_provider.capabilities =
                    Some(codex_model_provider_info::ModelProviderCapabilities {
                        cyber_access_programs: Some(false),
                        ..Default::default()
                    });
            }
        })
        .build_with_auto_env(&server)
        .await?;
    // Parent traffic has its own responses so child compaction is deterministic.
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header(
            "thread-id",
            test.session_configured.thread_id.to_string(),
        ))
        .respond_with(responses::sse_response(responses::sse(vec![
            responses::ev_function_call_with_namespace(
                "spawn-worker",
                namespace,
                "spawn_agent",
                &spawn.to_string(),
            ),
            responses::ev_completed("parent-spawn"),
        ])))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header(
            "thread-id",
            test.session_configured.thread_id.to_string(),
        ))
        .respond_with(responses::sse_response(responses::sse(vec![
            responses::ev_assistant_message("parent-done", "Done."),
            responses::ev_completed("parent-done"),
        ])))
        .with_priority(2)
        .mount(&server)
        .await;
    if let Some(enabled) = saved_choice {
        test.codex.ensure_rollout_materialized().await;
        test.codex
            .update_thread_metadata(
                ThreadMetadataPatch {
                    daybreak_enabled: Some(enabled),
                    ..Default::default()
                },
                /*include_archived*/ false,
            )
            .await?;
    }
    let mut created_threads = test.thread_manager.subscribe_thread_created();
    submit(&test, Some(parent_program)).await?;
    let child = test
        .thread_manager
        .get_thread(created_threads.recv().await?)
        .await?;
    wait_for_successful_turn(&child).await;

    // No explicit program: both sampling and compaction must use the saved choice.
    child
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Continue with the other model.".to_owned(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                model: Some(parent_model.to_owned()),
                ..Default::default()
            }),
        )
        .await?;
    let completed =
        wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    assert!(
        matches!(&completed, EventMsg::TurnComplete(turn) if turn.error.is_none()),
        "model-switch turn failed: {completed:?}; requests: {:?}",
        requests.requests().iter().map(|request| {
            let body = request.body_json();
            json!({"model": body["model"], "program": body["access_programs"], "compaction": request.inputs_of_type("compaction_trigger").len()})
        }).collect::<Vec<_>>(),
    );

    child
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Use this explicit choice.".to_owned(),
                text_elements: Vec::new(),
            }])
            .on_start(TurnStartOptions {
                cyber_access_program: Some(CyberAccessProgram::Standard),
                ..Default::default()
            }),
        )
        .await?;
    wait_for_successful_turn(&child).await;
    child.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;

    let first_turn_program = if parent_program == CyberAccessProgram::Standard {
        CyberAccessProgram::Standard
    } else {
        child_program
    };
    let (initial, next) = match (provider_programs, propagation_enabled, saved_choice) {
        (false, _, _) => (None, None),
        (true, true, Some(true)) => (Some(first_turn_program), Some(parent_daybreak)),
        (true, true, Some(false)) => (Some(first_turn_program), Some(CyberAccessProgram::Standard)),
        (true, true, None) => (Some(first_turn_program), None),
        (true, _, _) => (Some(parent_program), None),
    };
    let program_json = |program: Option<CyberAccessProgram>| {
        program
            .map(|program| json!({"cyber": program}))
            .unwrap_or(json!(null))
    };
    let requests = requests.requests();
    assert_eq!(
        requests
            .iter()
            .map(|request| {
                let body = request.body_json();
                json!([body["model"], body["access_programs"]])
            })
            .collect::<Vec<_>>(),
        vec![
            json!([child_model, program_json(initial)]),
            json!([child_model, program_json(initial)]),
            json!([child_model, program_json(initial)]),
            json!([child_model, program_json(initial)]),
            json!([parent_model, program_json(next)]),
            json!([parent_model, program_json(next)]),
            json!([parent_model, program_json(next)]),
            json!([
                parent_model,
                program_json(provider_programs.then_some(CyberAccessProgram::Standard))
            ]),
        ]
    );
    assert_eq!(requests[1].inputs_of_type("compaction_trigger").len(), 1);
    assert_eq!(requests[3].inputs_of_type("compaction_trigger").len(), 1);
    assert_eq!(requests[5].inputs_of_type("compaction_trigger").len(), 1);
    Ok(())
}

async fn submit(test: &TestCodex, program: Option<CyberAccessProgram>) -> Result<()> {
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "hello".to_owned(),
                text_elements: Vec::new(),
            }])
            .on_start(TurnStartOptions {
                cyber_access_program: program,
                ..Default::default()
            }),
        )
        .await?;
    let event = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_) | EventMsg::Error(_))
    })
    .await;
    if let EventMsg::Error(error) = event {
        anyhow::bail!("cyber program {program:?}: {error:?}");
    }
    Ok(())
}

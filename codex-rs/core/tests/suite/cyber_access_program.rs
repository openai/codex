use anyhow::Result;
use codex_core::RecoverTurnRequest;
use codex_core::StartIfIdleSubmission;
use codex_core::TurnInputRequest;
use codex_core::TurnInputSubmission;
use codex_core::TurnStartOptions;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::turn_input::CyberAccessProgram;
use codex_protocol::user_input::UserInput;
use codex_rollout::RolloutItem;
use codex_rollout::RolloutRecorder;
use core_test_support::responses;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;
use tokio::sync::oneshot;
use wiremock::Mock;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recover_turn_restores_cyber_access_program_without_making_it_sticky() -> Result<()> {
    // Keep sampling pending until interruption so restart must reload the unfinished turn.
    let (release_response, response_gate) = oneshot::channel();
    let (initial_server, _completions) =
        start_streaming_sse_server(vec![vec![StreamingSseChunk {
            gate: Some(response_gate),
            body: responses::sse(vec![responses::ev_completed("resp-initial")]),
        }]])
        .await;
    let mut builder = test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let initial = builder.build_with_streaming_server(&initial_server).await?;
    let TurnInputSubmission::Started { turn_id } = initial
        .codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "recover this turn".to_owned(),
                text_elements: Vec::new(),
            }])
            .on_start(TurnStartOptions {
                cyber_access_program: Some(CyberAccessProgram::DaybreakBlue),
                ..Default::default()
            }),
        )
        .await?
    else {
        panic!("expected a new turn");
    };
    initial_server.wait_for_request_count(/*count*/ 1).await;
    initial.codex.submit(Op::Interrupt).await?;
    wait_for_event(&initial.codex, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;
    drop(release_response);
    initial_server.shutdown().await;
    initial.codex.flush_rollout().await?;

    let server = responses::start_mock_server().await;
    let test = builder.restart(&server, &initial).await?;
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

    let response_mock =
        responses::mount_sse_once(&server, responses::sse_completed("resp-1")).await;
    let submission = test
        .codex
        .recover_turn_if_idle(RecoverTurnRequest {
            turn_id: turn_id.clone(),
            thread_settings: Default::default(),
            trace: None,
            cyber_access_program: persisted_context.cyber_access_program,
        })
        .await?;
    assert_eq!(
        submission,
        StartIfIdleSubmission::Started {
            turn_id: turn_id.clone(),
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
    assert_eq!(
        response_mock
            .single_request()
            .message_input_texts("user")
            .into_iter()
            .filter(|text| text == "recover this turn")
            .count(),
        1
    );

    let next_response = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("resp-next")]),
    )
    .await;
    let TurnInputSubmission::Started {
        turn_id: next_turn_id,
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
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cyber_access_program_omits_spoofed_custom_providers() -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    for auth in [
        CodexAuth::from_api_key("test-key"),
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    ] {
        let server = responses::start_mock_server().await;
        let request = responses::mount_sse_once(
            &server,
            responses::sse(vec![responses::ev_completed("resp-1")]),
        )
        .await;
        let test = test_codex()
            .with_auth(auth)
            .with_config(|config| {
                // Keep the display name "OpenAI": provider identity must not use it.
                config.model_provider_id = "custom".to_owned();
            })
            .build_with_auto_env(&server)
            .await?;
        submit(&test, Some(CyberAccessProgram::DaybreakRed)).await?;
        assert_eq!(
            request.single_request().body_json().get("access_programs"),
            None
        );
    }
    Ok(())
}

#[test_case(CodexAuth::create_dummy_chatgpt_auth_for_testing(); "chatgpt")]
#[test_case(CodexAuth::from_api_key("test-api-key"); "api_key")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cyber_access_program_survives_mid_turn_remote_compaction_v2(
    auth: CodexAuth,
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
        .with_config(|config| {
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

#[test_case(CodexAuth::create_dummy_chatgpt_auth_for_testing(); "chatgpt")]
#[test_case(CodexAuth::from_api_key("test-api-key"); "api_key")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cyber_access_program_changes_on_one_websocket_with_response_reuse(
    auth: CodexAuth,
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
        .with_config(|config| {
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

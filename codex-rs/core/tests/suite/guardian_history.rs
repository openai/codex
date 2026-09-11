//! Exercises retained review history through compaction, resume, fork, eviction, and legacy rollback replay.

use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_core::ForkSnapshot;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_core::config::ThreadStoreConfig;
use codex_features::Feature;
use codex_history::InitialHistory;
use codex_history::ResumedHistory;
use codex_history::RolloutItem;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::mcp::ClientMcpExtensions;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadRolledBackEvent;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_protocol::user_input::UserInput;
use codex_thread_store::ForkBoundary;
use codex_thread_store::InMemoryThreadStore;
use codex_thread_store::LoadThreadHistoryParams;
use codex_thread_store::PrepareForkParams;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use image::DynamicImage;
use image::RgbImage;
use pretty_assertions::assert_eq;
use rand::RngCore;
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde_json::json;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use test_case::test_case;

#[test_case(ThreadHistoryMode::Paginated, ThreadStoreConfig::Local; "paginated")]
// The pathless test store only supports Legacy mode; this case tests the store interface,
// not legacy rollout compatibility.
#[test_case(ThreadHistoryMode::Legacy, ThreadStoreConfig::InMemory {
    id: uuid::Uuid::new_v4().to_string(),
}; "pathless")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_history_survives_restart_and_user_fork(
    history_mode: ThreadHistoryMode,
    store_config: ThreadStoreConfig,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );
    let server = start_mock_server().await;
    let pathless_store = match &store_config {
        ThreadStoreConfig::Local => None,
        ThreadStoreConfig::InMemory { id } => Some(InMemoryThreadStore::for_id(id)),
    };
    let mut builder = test_codex()
        .with_history_mode(history_mode)
        .with_config(move |config| {
            config.experimental_thread_store = store_config;
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("enable token budget");
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        });
    let initial = builder.build_with_auto_env(&server).await?;
    let authorization = "You may publish the reviewed release.";
    mount_sse_once(&server, sse(vec![ev_completed("authorized")])).await;
    initial.submit_text_turn(authorization).await?;
    initial.codex.submit(Op::Compact).await?;
    wait_for_event(&initial.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let restriction = "Keep the release private.";
    mount_sse_once(&server, sse(vec![ev_completed("restriction")])).await;
    initial.submit_text_turn(restriction).await?;
    initial.codex.shutdown_and_wait().await?;
    let thread_id = initial.session_configured.thread_id;
    initial.thread_manager.remove_thread(&thread_id).await;
    let model_context = initial
        .thread_store
        .load_latest_model_context(LoadThreadHistoryParams {
            thread_id,
            include_archived: false,
        })
        .await?;
    // Cross the wire boundary even for the pathless in-memory store: replay must not rely on
    // retained runtime metadata or the original live ContextManager.
    let items: Vec<RolloutItem> =
        serde_json::from_value(serde_json::to_value(model_context.items)?)?;
    let history = InitialHistory::Resumed(ResumedHistory {
        conversation_id: thread_id,
        history: Arc::new(items),
        rollout_path: None,
    });
    let fork = if history_mode == ThreadHistoryMode::Paginated {
        let prepared = initial
            .thread_store
            .prepare_fork(PrepareForkParams {
                thread_id,
                boundary: ForkBoundary::Latest,
            })
            .await?;
        initial
            .thread_manager
            .fork_prepared_thread(
                codex_core::StartThreadOptions::new(initial.config.clone()),
                prepared,
            )
            .await?
    } else {
        initial
            .thread_manager
            .fork_thread_from_history(
                ForkSnapshot::Interrupted,
                codex_core::StartThreadOptions::new(initial.config.clone()),
                history.clone(),
            )
            .await?
    };
    let resumed = initial
        .thread_manager
        .resume_thread_with_history(
            initial.config.clone(),
            history,
            initial.thread_manager.auth_manager(),
            /*parent_trace*/ None,
            ClientMcpExtensions::default(),
        )
        .await?;
    for thread in [&fork.thread, &resumed.thread] {
        if pathless_store.is_some() {
            assert_eq!(thread.rollout_path(), None);
        }
        let review = mount_sse_sequence(
            &server,
            vec![
                sse(vec![
                    ev_function_call(
                        "publish",
                        "exec_command",
                        r#"{"cmd":"echo publish","sandbox_permissions":"require_escalated"}"#,
                    ),
                    ev_completed("publish"),
                ]),
                sse(vec![
                    ev_assistant_message("review", r#"{"outcome":"deny"}"#),
                    ev_completed("review"),
                ]),
                sse(vec![ev_completed("done")]),
            ],
        )
        .await;
        thread
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Continue with the release.".into(),
                text_elements: Vec::new(),
            }]))
            .await?;
        wait_for_event(thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
        let requests = review.requests();
        let guardian = requests
            .iter()
            .find(|request| {
                request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian"
            })
            .expect("Guardian request");
        let transcript = serde_json::to_string(&guardian.input())?;
        assert!(transcript.contains(authorization));
        assert!(transcript.contains(restriction));
        assert!(!serde_json::to_string(&requests[0].input())?.contains(authorization));
        thread.shutdown_and_wait().await?;
    }
    if let Some(store) = pathless_store {
        assert_eq!(store.calls().await.read_thread_by_rollout_path, 0);
        if let ThreadStoreConfig::InMemory { id } = &initial.config.experimental_thread_store {
            InMemoryThreadStore::remove_id(id);
        }
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_history_uses_deltas_between_eviction_batches() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );
    let server = start_mock_server().await;
    let test = test_codex()
        .with_config(|config| {
            config.features.enable(Feature::TokenBudget).unwrap();
            config
                .features
                .disable(Feature::GuardianThreadContext)
                .expect("use the retained legacy history");
            config.update_plan_enabled = true;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        })
        .build_with_auto_env(&server)
        .await?;
    let plan = r#"{"plan":[{"step":"inspect the repository","status":"completed"}]}"#;
    let mut inspection: Vec<_> = (0..130)
        .map(|index| ev_function_call(&format!("initial-{index}"), "update_plan", plan))
        .collect();
    inspection.push(ev_completed("inspection"));
    mount_sse_sequence(
        &server,
        vec![sse(inspection), sse(vec![ev_completed("inspected")])],
    )
    .await;
    let restriction = "Only inspect the repository; do not publish it.";
    test.submit_text_turn(restriction).await?;
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let mut responses = Vec::new();
    for index in 0..3 {
        if index == 1 {
            // Force eviction after the first review has saved its transcript cursor.
            let mut traffic: Vec<_> = (0..130)
                .map(|index| ev_function_call(&format!("later-{index}"), "update_plan", plan))
                .collect();
            traffic.push(ev_completed("more-inspection"));
            responses.push(sse(traffic));
        }
        responses.push(sse(vec![
            ev_function_call(
                &format!("review-{index}"),
                "exec_command",
                &json!({
                    "cmd": format!("echo review-{index}"),
                    "sandbox_permissions": "require_escalated"
                })
                .to_string(),
            ),
            ev_completed(&format!("action-{index}")),
        ]));
        // Three consecutive denials would interrupt the parent turn before its final response.
        let decision = if index == 2 {
            r#"{"risk_level":"low","user_authorization":"high","outcome":"allow"}"#
        } else {
            r#"{"outcome":"deny"}"#
        };
        responses.push(sse(vec![
            ev_assistant_message(&format!("decision-{index}"), decision),
            ev_completed(&format!("reviewed-{index}")),
        ]));
    }
    responses.push(sse(vec![ev_completed("done")]));
    let reviews = mount_sse_sequence(&server, responses).await;
    test.submit_text_turn("Continue inspecting.").await?;
    let requests = reviews.requests();
    let guardian_requests = requests
        .iter()
        .filter(|request| request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian")
        .collect::<Vec<_>>();
    let prompts = guardian_requests
        .iter()
        .map(|request| {
            request
                .message_input_text_groups("user")
                .last()
                .expect("Guardian review prompt")
                .join("")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        prompts
            .iter()
            .map(|prompt| (
                prompt.contains(">>> TRANSCRIPT START\n"),
                prompt.contains(">>> TRANSCRIPT DELTA START\n"),
            ))
            .collect::<Vec<_>>(),
        vec![(true, false), (true, false), (false, true)]
    );
    assert!(prompts[1].contains(restriction));
    assert!(prompts[2].contains("review-2"));
    assert_eq!(
        guardian_requests[1].body_json()["client_metadata"]["thread_id"],
        guardian_requests[2].body_json()["client_metadata"]["thread_id"]
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_history_survives_compaction_and_eviction_but_not_legacy_rollback_replay()
-> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );
    let server = start_mock_server().await;
    let mut test = test_codex()
        .with_config(|config| {
            config.features.enable(Feature::TokenBudget).unwrap();
            config
                .features
                .enable(Feature::DefaultModeRequestUserInput)
                .unwrap();
            config.update_plan_enabled = true;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        })
        .build_with_auto_env(&server)
        .await?;
    // Enough real tool traffic to evict earlier tools when retention starts.
    let plan = r#"{"plan":[{"step":"verify repository visibility","status":"completed"}]}"#;
    let mut inspection: Vec<_> = (0..130)
        .map(|index| ev_function_call(&format!("inspect-{index}"), "update_plan", plan))
        .collect();
    inspection.push(ev_completed("inspection"));
    mount_sse_sequence(
        &server,
        vec![
            sse(inspection),
            sse(vec![
                ev_function_call("inspect-latest", "update_plan", plan),
                ev_function_call(
                    "confirm-publish",
                    "request_user_input",
                    &json!({
                        "questions": [{
                            "id": "publish", "header": "Publish", "question": "May I publish?",
                            "options": [
                                {"label": "Yes", "description": "Publish the change."},
                                {"label": "No", "description": "Keep the change local."}
                            ]
                        }]
                    })
                    .to_string(),
                ),
                ev_completed("latest-inspection"),
            ]),
            sse(vec![
                ev_assistant_message("inspected", "Inspection complete."),
                ev_completed("inspection-done"),
            ]),
        ],
    )
    .await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Only publish to a private repository.".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let question = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    test.codex
        .submit(Op::UserInputAnswer {
            id: question.turn_id,
            response: RequestUserInputResponse {
                answers: HashMap::from([(
                    "publish".to_owned(),
                    RequestUserInputAnswer {
                        answers: vec!["Do not publish anything.".to_owned()],
                    },
                )]),
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    // A valid, supported-size image whose encoded bytes exceed the retention budget.
    let (width, height) = (1200, 1200);
    let mut pixels = vec![0; (width * height * 3) as usize];
    let seed = 42;
    StdRng::seed_from_u64(seed).fill_bytes(&mut pixels);
    let image = RgbImage::from_raw(width, height, pixels).unwrap();
    let mut image_data = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image).write_to(&mut image_data, image::ImageFormat::Png)?;
    let image_url = format!(
        "data:image/png;base64,{}",
        BASE64_STANDARD.encode(image_data.into_inner())
    );
    assert!(image_url.len() > 4 * 1024 * 1024);
    let command = r#"{"cmd":"echo publish","sandbox_permissions":"require_escalated","justification":"Publish the inspected change."}"#;
    for (prompt, retained) in [
        ("Do not publish the attached image.", true),
        ("Inspect a different repository.", false),
    ] {
        let review = mount_sse_sequence(
            &server,
            vec![
                sse(vec![
                    ev_function_call("publish", "exec_command", command),
                    ev_completed("publish"),
                ]),
                sse(vec![
                    ev_assistant_message("review", r#"{"outcome":"deny"}"#),
                    ev_completed("review"),
                ]),
                sse(vec![ev_completed("publish-done")]),
            ],
        )
        .await;
        test.codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![
                UserInput::Text {
                    text: prompt.to_owned(),
                    text_elements: Vec::new(),
                },
                UserInput::Image {
                    image_url: image_url.clone(),
                    detail: None,
                },
            ]))
            .await?;
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        let requests = review.requests();
        assert!(
            requests[0]
                .input()
                .iter()
                .filter_map(|item| item["content"].as_array())
                .flatten()
                .any(|item| item["image_url"]
                    .as_str()
                    .is_some_and(|url| url.len() > 4 * 1024 * 1024))
        );
        let guardian = requests
            .iter()
            .find(|request| {
                request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian"
            })
            .expect("Guardian request");
        let transcript = serde_json::to_string(&guardian.input())?;
        assert!(transcript.contains(prompt));
        if retained {
            let trusted_answers = transcript
                .split_once(">>> TRUSTED USER ANSWERS START")
                .expect("trusted answers survive compaction")
                .1
                .split_once(">>> TRUSTED USER ANSWERS END")
                .expect("trusted answers end marker")
                .0;
            assert!(trusted_answers.contains("user: Do not publish anything."));
            let positions = [
                "Only publish to a private repository.",
                "tool update_plan call",
                "tool update_plan result",
                "Do not publish the attached image.",
            ]
            .map(|text| {
                transcript
                    .find(text)
                    .unwrap_or_else(|| panic!("missing {text}: {transcript}"))
            });
            let mut ordered = positions;
            ordered.sort();
            assert_eq!(positions, ordered);
            assert!(
                requests[0]
                    .input()
                    .iter()
                    .all(|item| item["call_id"] != "inspect-0"
                        && item["call_id"] != "confirm-publish")
            );
            test.codex.ensure_rollout_materialized().await;
            test.codex
                .append_rollout_items(&[RolloutItem::EventMsg(EventMsg::ThreadRolledBack(
                    ThreadRolledBackEvent { num_turns: 2 },
                ))])
                .await?;
            test.codex.shutdown_and_wait().await?;
            let thread_id = test.session_configured.thread_id;
            test.thread_manager.remove_thread(&thread_id).await;
            let model_context = test
                .thread_store
                .load_latest_model_context(LoadThreadHistoryParams {
                    thread_id,
                    include_archived: false,
                })
                .await?;
            test.codex = test
                .thread_manager
                .resume_thread_with_history(
                    test.config.clone(),
                    InitialHistory::Resumed(ResumedHistory {
                        conversation_id: thread_id,
                        history: Arc::new(model_context.items),
                        rollout_path: None,
                    }),
                    test.thread_manager.auth_manager(),
                    /*parent_trace*/ None,
                    ClientMcpExtensions::default(),
                )
                .await?
                .thread;
        } else {
            assert!(!transcript.contains(">>> TRUSTED USER ANSWERS START"));
            assert!(!transcript.contains("Do not publish anything."));
            assert!(!transcript.contains("Only publish to a private repository."));
            assert!(!transcript.contains("tool update_plan call"));
            assert!(!transcript.contains("tool update_plan result"));
        }
    }
    Ok(())
}

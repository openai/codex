use anyhow::Result;
use codex_core::config::RolloutBudgetConfig;
use codex_features::Feature;
use codex_model_provider_info::built_in_model_providers;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use test_case::test_case;
use tokio::time::timeout;

const MULTI_AGENT_V2_NAMESPACE: &str = "collaboration";

fn rollout_budget() -> RolloutBudgetConfig {
    RolloutBudgetConfig {
        limit_tokens: 100,
        reminder_at_remaining_tokens: vec![75, 50, 25],
        sampling_token_weight: 1.0,
        prefill_token_weight: 1.0,
    }
}

fn rollout_budget_texts(request: &ResponsesRequest) -> Vec<String> {
    request
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.starts_with("<rollout_budget>"))
        .collect()
}

fn rollout_budget_message(remaining_tokens: i64) -> String {
    format!(
        "<rollout_budget>\nYou have {remaining_tokens} weighted tokens left in the shared session token budget.\n</rollout_budget>"
    )
}

fn wire_request_contains(request: &wiremock::Request, text: &str) -> bool {
    std::str::from_utf8(&request.body).is_ok_and(|body| body.contains(text))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adds_weighted_initial_and_threshold_reminders() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                json!({
                    "type": "response.completed",
                    "response": {
                        "id": "resp-1",
                        "usage": {
                            "input_tokens": 60,
                            "input_tokens_details": { "cached_tokens": 40 },
                            "output_tokens": 15,
                            "output_tokens_details": null,
                            "total_tokens": 75
                        }
                    }
                }),
            ]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            config.rollout_budget = Some(RolloutBudgetConfig {
                sampling_token_weight: 2.0,
                prefill_token_weight: 0.5,
                ..rollout_budget()
            });
        })
        .build(&server)
        .await?;

    test.submit_turn("first turn").await?;
    test.submit_turn("second turn").await?;

    let requests = responses.requests();
    assert_eq!(
        rollout_budget_texts(&requests[0]),
        vec![rollout_budget_message(/*remaining_tokens*/ 100)]
    );
    assert_eq!(
        rollout_budget_texts(&requests[1]),
        vec![
            rollout_budget_message(/*remaining_tokens*/ 100),
            rollout_budget_message(/*remaining_tokens*/ 60),
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subagent_usage_draws_from_the_shared_budget() -> Result<()> {
    skip_if_no_network!(Ok(()));

    const ROOT_PROMPT: &str = "spawn a budget worker";
    const CHILD_PROMPT: &str = "consume child budget";
    const FOLLOW_UP_PROMPT: &str = "report the shared budget";
    const SPAWN_CALL_ID: &str = "spawn-budget-worker";

    let server = start_mock_server().await;
    let spawn_args = json!({
        "message": CHILD_PROMPT,
        "task_name": "budget_worker",
    })
    .to_string();
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| wire_request_contains(request, ROOT_PROMPT),
        sse(vec![
            ev_response_created("root-1"),
            ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                MULTI_AGENT_V2_NAMESPACE,
                "spawn_agent",
                &spawn_args,
            ),
            ev_completed_with_tokens("root-1", /*total_tokens*/ 10),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| wire_request_contains(request, "\"type\":\"agent_message\""),
        sse(vec![
            ev_response_created("child-1"),
            ev_completed_with_tokens("child-1", /*total_tokens*/ 30),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            wire_request_contains(request, SPAWN_CALL_ID)
                && !wire_request_contains(request, "\"type\":\"agent_message\"")
        },
        sse(vec![
            ev_response_created("root-2"),
            ev_completed_with_tokens("root-2", /*total_tokens*/ 10),
        ]),
    )
    .await;
    let follow_up = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| wire_request_contains(request, FOLLOW_UP_PROMPT),
        sse(vec![ev_response_created("root-3"), ev_completed("root-3")]),
    )
    .await;

    let test = test_codex()
        .with_config(|config| {
            config
                .features
                .enable(Feature::Collab)
                .expect("test config should allow multi-agent tools");
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("test config should allow multi-agent v2");
            config.rollout_budget = Some(rollout_budget());
        })
        .build(&server)
        .await?;

    let mut created_threads = test.thread_manager.subscribe_thread_created();
    test.submit_turn(ROOT_PROMPT).await?;
    let child_thread_id = timeout(Duration::from_secs(10), created_threads.recv()).await??;
    let child_thread = test.thread_manager.get_thread(child_thread_id).await?;
    wait_for_event(child_thread.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_turn(FOLLOW_UP_PROMPT).await?;

    let requests = follow_up
        .requests()
        .into_iter()
        .filter(|request| {
            request
                .message_input_texts("user")
                .iter()
                .any(|text| text == FOLLOW_UP_PROMPT)
        })
        .collect::<Vec<_>>();
    let [request] = requests.as_slice() else {
        anyhow::bail!("expected 1 follow-up request, got {}", requests.len());
    };
    assert_eq!(
        rollout_budget_texts(request).last(),
        Some(&rollout_budget_message(/*remaining_tokens*/ 50))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exhausted_budget_fails_current_and_later_turns() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("exhaust-budget"),
                ev_completed_with_tokens("exhaust-budget", /*total_tokens*/ 30),
            ]),
            sse(vec![
                ev_response_created("already-exhausted"),
                ev_completed_with_tokens("already-exhausted", /*total_tokens*/ 1),
            ]),
        ],
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            config.rollout_budget = Some(RolloutBudgetConfig {
                limit_tokens: 30,
                reminder_at_remaining_tokens: vec![20, 10],
                ..rollout_budget()
            });
        })
        .build(&server)
        .await?;

    for prompt in ["exhaust the budget", "try another turn"] {
        test.codex
            .submit(Op::UserInput {
                items: vec![UserInput::Text {
                    text: prompt.to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
                responsesapi_client_metadata: None,
                additional_context: Default::default(),
                thread_settings: Default::default(),
            })
            .await?;

        wait_for_event(&test.codex, |event| {
            matches!(
                event,
                EventMsg::Error(error)
                    if error.codex_error_info == Some(CodexErrorInfo::SessionBudgetExceeded)
            )
        })
        .await;
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(false ; "local")]
#[test_case(true ; "remote_v2")]
async fn compaction_budget_exhaustion_fails_without_retry(remote_v2: bool) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let compact_response = if remote_v2 {
        sse(vec![
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "compaction",
                    "encrypted_content": "encrypted-summary",
                }
            }),
            ev_completed_with_tokens("compact", /*total_tokens*/ 10),
        ])
    } else {
        sse(vec![
            ev_response_created("compact"),
            ev_assistant_message("compact-summary", "compact summary"),
            ev_completed_with_tokens("compact", /*total_tokens*/ 10),
        ])
    };
    let responses = mount_sse_sequence(&server, vec![compact_response]).await;
    let test = test_codex()
        .with_config(move |config| {
            config.rollout_budget = Some(RolloutBudgetConfig {
                limit_tokens: 10,
                reminder_at_remaining_tokens: vec![5],
                ..rollout_budget()
            });
            if remote_v2 {
                config
                    .features
                    .enable(Feature::RemoteCompactionV2)
                    .expect("test config should allow remote compaction v2");
            } else {
                config.model_provider.name = "OpenAI-compatible test provider".to_string();
            }
        })
        .build(&server)
        .await?;

    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::Error(error)
                if error.codex_error_info == Some(CodexErrorInfo::SessionBudgetExceeded)
        )
    })
    .await;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(responses.requests().len(), 1, "compaction should not retry");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restates_the_current_remainder_after_compaction() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_completed_with_tokens("resp-1", /*total_tokens*/ 20),
            ]),
            sse(vec![
                ev_response_created("resp-compact"),
                ev_assistant_message("msg-compact", "compact summary"),
                ev_completed_with_tokens("resp-compact", /*total_tokens*/ 10),
            ]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;
    let mut model_provider = built_in_model_providers(/*openai_base_url*/ None)["openai"].clone();
    model_provider.name = "OpenAI-compatible test provider".to_string();
    model_provider.base_url = Some(format!("{}/v1", server.uri()));
    model_provider.supports_websockets = false;
    let test = test_codex()
        .with_config(move |config| {
            config.model_provider = model_provider;
            config.rollout_budget = Some(RolloutBudgetConfig {
                reminder_at_remaining_tokens: vec![50],
                ..rollout_budget()
            });
        })
        .build(&server)
        .await?;

    test.submit_turn("first turn").await?;
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_turn("second turn").await?;

    let requests = responses.requests();
    assert_eq!(
        rollout_budget_texts(&requests[2]),
        vec![rollout_budget_message(/*remaining_tokens*/ 70)],
        "a new context window should restate the current remainder"
    );
    let request_body = requests[2].body_json().to_string();
    let summary_position = request_body
        .find("compact summary")
        .expect("post-compaction request should contain the summary");
    let reminder_position = request_body
        .find("You have 70 weighted tokens left in the shared session token budget.")
        .expect("post-compaction request should contain the current remainder");
    assert!(
        summary_position < reminder_position,
        "the current remainder should follow the compaction summary"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restates_the_current_remainder_after_rollback() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_completed_with_tokens("resp-1", /*total_tokens*/ 30),
            ]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            config.rollout_budget = Some(RolloutBudgetConfig {
                reminder_at_remaining_tokens: vec![50],
                ..rollout_budget()
            });
        })
        .build(&server)
        .await?;

    test.submit_turn("rolled-back turn").await?;
    test.codex
        .submit(Op::ThreadRollback { num_turns: 1 })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ThreadRolledBack(_))
    })
    .await;
    test.submit_turn("turn after rollback").await?;

    let requests = responses.requests();
    assert_eq!(
        rollout_budget_texts(&requests[1]),
        vec![rollout_budget_message(/*remaining_tokens*/ 70)],
        "rollback should rearm the current budget reminder without refunding usage"
    );

    Ok(())
}

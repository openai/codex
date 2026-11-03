use super::agent_control::HostRuntime;
use super::agent_control::install_host_runtime;
use anyhow::Result;
use codex_core::AgentRuntimeOwnership;
use codex_core::AgentUsageAccounting;
use codex_core::AgentUsageReport;
use codex_core::TurnInputRequest;
use codex_core::TurnInputSubmission;
use codex_core::UsageAccountingDecision;
use codex_core::config::Config;
use codex_core::config::RolloutBudgetConfig;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_features::Feature;
use codex_model_provider_info::built_in_model_providers;
use codex_protocol::error::CodexErr;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::received_responses_requests;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use test_case::test_case;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::timeout;

const MULTI_AGENT_V2_NAMESPACE: &str = "collaboration";

#[derive(Clone, Copy, Default)]
enum HostAccountingBehavior {
    #[default]
    Continue,
    Exhausted,
    Fail,
}

#[derive(Clone, Default)]
pub(super) struct HostAccounting {
    pub(super) reports: Arc<Mutex<Vec<AgentUsageReport>>>,
    behavior: HostAccountingBehavior,
    controller_owned: bool,
    pause_report: Option<mpsc::UnboundedSender<oneshot::Sender<()>>>,
}

impl AgentUsageAccounting for HostAccounting {
    fn report_usage(
        &self,
        report: AgentUsageReport,
    ) -> BoxFuture<'_, codex_protocol::error::Result<UsageAccountingDecision>> {
        Box::pin(async move {
            self.reports.lock().expect("usage reports").push(report);
            if let Some(pause) = &self.pause_report {
                let (resume, resumed) = oneshot::channel();
                let _ = pause.send(resume);
                let _ = resumed.await;
            }
            match self.behavior {
                HostAccountingBehavior::Continue => Ok(UsageAccountingDecision::Continue),
                HostAccountingBehavior::Exhausted => Ok(UsageAccountingDecision::Exhausted),
                HostAccountingBehavior::Fail => {
                    Err(CodexErr::Stream("accounting unavailable".into()))
                }
            }
        })
    }
}

impl ThreadLifecycleContributor<Config> for HostAccounting {
    fn on_thread_start<'a>(
        &'a self,
        input: ThreadStartInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if !self.controller_owned {
                install_host_runtime(input.thread_store);
            }
            input
                .thread_store
                .insert(Arc::new(self.clone()) as Arc<dyn AgentUsageAccounting>);
        })
    }
}

pub(super) fn rollout_budget() -> RolloutBudgetConfig {
    RolloutBudgetConfig {
        enforce_limit: true,
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
#[test_case(None ; "weighted token usage")]
#[test_case(Some(40.5) ; "provider budget units")]
async fn adds_weighted_initial_and_threshold_reminders(
    rollout_budget_units: Option<f64>,
) -> Result<()> {
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
                            "total_tokens": 75,
                            "codex_rollout_budget_units": rollout_budget_units
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
    assert!(requests[0].has_content_kinds(&["rollout_budget.remaining_tokens"]));
    assert_eq!(
        rollout_budget_texts(&requests[0]),
        vec![rollout_budget_message(/*remaining_tokens*/ 100)]
    );
    assert_eq!(
        rollout_budget_texts(&requests[1]),
        vec![
            rollout_budget_message(/*remaining_tokens*/ 100),
            rollout_budget_message(if rollout_budget_units.is_some() {
                59
            } else {
                60
            }),
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_provider_rollout_budget_units_fail_without_retry() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut completed = ev_completed_with_tokens("invalid-units", /*total_tokens*/ 11);
    completed["response"]["usage"]["codex_rollout_budget_units"] = json!(-1.0);
    let responses = mount_sse_sequence(
        &server,
        vec![sse(vec![ev_response_created("invalid-units"), completed])],
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            config.rollout_budget = Some(rollout_budget());
        })
        .build(&server)
        .await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "reject invalid provider budget units".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;

    let EventMsg::Error(error) =
        wait_for_event(&test.codex, |event| matches!(event, EventMsg::Error(_))).await
    else {
        unreachable!();
    };
    assert_eq!(
        error.message,
        "Fatal error: response.completed usage.codex_rollout_budget_units must be finite and non-negative"
    );
    assert_eq!(error.codex_error_info, Some(CodexErrorInfo::Other));
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(
        responses.requests().len(),
        1,
        "invalid units should not retry"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(false ; "local accounting")]
#[test_case(true ; "host accounting")]
async fn subagent_usage_draws_from_the_shared_budget(host_accounting: bool) -> Result<()> {
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

    let accounting = Arc::new(HostAccounting {
        controller_owned: !host_accounting,
        ..Default::default()
    });
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.thread_lifecycle_contributor(accounting.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(move |config| {
            config
                .features
                .enable(Feature::Collab)
                .expect("test config should allow multi-agent tools");
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("test config should allow multi-agent v2");
            config.rollout_budget = Some(RolloutBudgetConfig {
                // Host success must not also charge the Local budget.
                limit_tokens: if host_accounting { 1 } else { 100 },
                ..rollout_budget()
            });
        })
        .build(&server)
        .await?;

    let mut created_threads = test.thread_manager.subscribe_thread_created();
    test.submit_turn(ROOT_PROMPT).await?;
    let child_thread_id = timeout(Duration::from_secs(10), created_threads.recv()).await??;
    let child_thread = test.thread_manager.get_thread(child_thread_id).await?;
    let child_turn_id = wait_for_event_match(child_thread.as_ref(), |event| match event {
        EventMsg::TurnComplete(completed) => Some(completed.turn_id.clone()),
        _ => None,
    })
    .await;
    if host_accounting {
        let mut reports = accounting.reports.lock().expect("usage reports").clone();
        reports.sort_by_key(|report| report.usage.total_tokens);
        let root_thread_id = test.session_configured.thread_id;
        assert_eq!(
            reports
                .iter()
                .map(|report| (report.thread_id, report.usage.total_tokens))
                .collect::<Vec<_>>(),
            [
                (root_thread_id, 10),
                (root_thread_id, 10),
                (child_thread_id, 30)
            ]
        );
        assert!(!reports[0].turn_id.is_empty());
        assert_eq!(reports[0].turn_id, reports[1].turn_id);
        assert_eq!(reports[2].turn_id, child_turn_id);
    }
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
    if host_accounting {
        assert!(rollout_budget_texts(request).is_empty());
    } else {
        assert_eq!(
            rollout_budget_texts(request).last(),
            Some(&rollout_budget_message(/*remaining_tokens*/ 50))
        );
        assert!(accounting.reports.lock().expect("usage reports").is_empty());
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(AgentRuntimeOwnership::Controller, true ; "enforced")]
#[test_case(AgentRuntimeOwnership::Controller, false ; "advisory")]
#[test_case(AgentRuntimeOwnership::Host, true ; "host without accounting")]
async fn exhausted_budget_respects_enforcement_on_current_and_later_turns(
    owner: AgentRuntimeOwnership,
    enforce_limit: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
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
    let mut extensions = ExtensionRegistryBuilder::new();
    if owner == AgentRuntimeOwnership::Host {
        extensions.thread_lifecycle_contributor(Arc::new(HostRuntime));
    }
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(move |config| {
            config.rollout_budget = Some(RolloutBudgetConfig {
                enforce_limit,
                limit_tokens: 30,
                reminder_at_remaining_tokens: vec![20, 10],
                ..rollout_budget()
            });
        })
        .build_with_auto_env(&server)
        .await?;
    let late = Arc::new(HostAccounting::default());
    if owner == AgentRuntimeOwnership::Host {
        // Neither the owner nor the accounting provider can change after startup.
        let data = test.codex.thread_extension_data();
        data.insert(AgentRuntimeOwnership::Controller);
        data.insert(late.clone() as Arc<dyn AgentUsageAccounting>);
    }

    for prompt in ["exhaust the budget", "try another turn"] {
        test.codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: prompt.to_string(),
                text_elements: Vec::new(),
            }]))
            .await?;

        let event = wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::Error(_) | EventMsg::TurnComplete(_))
        })
        .await;
        if owner == AgentRuntimeOwnership::Host || !enforce_limit {
            assert!(matches!(event, EventMsg::TurnComplete(done) if done.error.is_none()));
            continue;
        }
        assert!(matches!(
            event,
            EventMsg::Error(error)
                if error.codex_error_info == Some(CodexErrorInfo::SessionBudgetExceeded)
        ));
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    }

    if owner == AgentRuntimeOwnership::Host {
        let requests = responses.requests();
        assert_eq!(requests.len(), 2);
        for request in &requests {
            assert!(rollout_budget_texts(request).is_empty());
        }
        assert!(late.reports.lock().expect("usage reports").is_empty());
    } else if !enforce_limit {
        assert_eq!(
            rollout_budget_texts(&responses.requests()[1]).last(),
            Some(&rollout_budget_message(/*remaining_tokens*/ 0))
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(false, false, false ; "local token usage")]
#[test_case(false, true, false ; "local provider units")]
#[test_case(true, false, false ; "remote v2 token usage")]
#[test_case(true, true, false ; "remote v2 provider units")]
#[test_case(false, false, true ; "local compaction host accounting failure")]
async fn compaction_accounting_errors_fail_without_retry(
    remote_v2: bool,
    provider_units: bool,
    host_failure: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut completed = ev_completed_with_tokens(
        "compact",
        /*total_tokens*/ if provider_units { 1 } else { 10 },
    );
    if provider_units {
        completed["response"]["usage"]["codex_rollout_budget_units"] = json!(10.0);
    }
    let compact_response = if remote_v2 {
        sse(vec![
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "compaction",
                    "encrypted_content": "encrypted-summary",
                }
            }),
            completed,
        ])
    } else {
        sse(vec![
            ev_response_created("compact"),
            ev_assistant_message("compact-summary", "compact summary"),
            completed,
        ])
    };
    let responses = mount_sse_sequence(&server, vec![compact_response]).await;
    let accounting = Arc::new(HostAccounting {
        behavior: HostAccountingBehavior::Fail,
        ..Default::default()
    });
    let mut extensions = ExtensionRegistryBuilder::new();
    if host_failure {
        extensions.thread_lifecycle_contributor(accounting.clone());
    }
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(move |config| {
            config.model_provider.stream_max_retries = Some(2);
            config.rollout_budget = Some(RolloutBudgetConfig {
                limit_tokens: 10,
                reminder_at_remaining_tokens: vec![5],
                ..rollout_budget()
            });
            if !remote_v2 {
                config.model_provider.name = "OpenAI-compatible test provider".to_string();
            }
        })
        .build(&server)
        .await?;

    let expected_error = if host_failure {
        CodexErrorInfo::Other
    } else {
        CodexErrorInfo::SessionBudgetExceeded
    };
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        assert!(
            !matches!(event, EventMsg::StreamError(_)),
            "compaction must not retry accounting failures"
        );
        matches!(
            event,
            EventMsg::Error(error)
                if error.codex_error_info == Some(expected_error.clone())
        )
    })
    .await;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(responses.requests().len(), 1, "compaction should not retry");
    if host_failure {
        let reports = accounting.reports.lock().expect("usage reports");
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].thread_id, test.session_configured.thread_id);
        assert_eq!(reports[0].usage.total_tokens, 10);
    }

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

#[tokio::test]
#[test_case(HostAccountingBehavior::Exhausted, false ; "host exhausts budget")]
#[test_case(HostAccountingBehavior::Fail, false ; "accounting failure never retries inference")]
#[test_case(HostAccountingBehavior::Fail, true ; "incomplete response keeps original error")]
async fn host_accounting_reports_raw_usage_and_preserves_error_semantics(
    behavior: HostAccountingBehavior,
    incomplete: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mut response = json!({
        "usage": {
            "input_tokens": 60,
            "input_tokens_details": {"cached_tokens": 40},
            "output_tokens": 15,
            "output_tokens_details": {"reasoning_tokens": 5},
            "total_tokens": 75,
            "codex_rollout_budget_units": 40.5
        }
    });
    let response_type = if incomplete {
        response["incomplete_details"] = json!({"reason": "max_output_tokens"});
        "response.incomplete"
    } else {
        response["id"] = json!("host-accounted");
        "response.completed"
    };
    mount_sse_once(
        &server,
        sse(vec![json!({"type": response_type, "response": response})]),
    )
    .await;
    let (pause_report, mut paused) = mpsc::unbounded_channel();
    let accounting = Arc::new(HostAccounting {
        behavior,
        pause_report: (!incomplete).then_some(pause_report),
        ..Default::default()
    });
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.thread_lifecycle_contributor(accounting.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config
                .features
                .enable(Feature::InstantInterrupt)
                .expect("enable instant interrupt");
            config.model_provider.stream_max_retries = Some(2);
            config.model_provider.request_max_retries = Some(0);
            config.rollout_budget = None;
        })
        .build_with_auto_env(&server)
        .await?;
    let user_input = |text: &str| {
        TurnInputRequest::user_input(vec![UserInput::Text {
            text: text.into(),
            text_elements: Vec::new(),
        }])
    };
    test.codex
        .start_or_steer_turn(user_input("report spending"))
        .await?;
    if !incomplete {
        let resume = timeout(Duration::from_secs(10), paused.recv())
            .await?
            .expect("accounting report");
        let submission = test
            .codex
            .start_or_steer_turn(user_input("continue"))
            .await?;
        assert!(matches!(submission, TurnInputSubmission::Steered { .. }));
        // Virtual time advances once the runnable input watcher has processed the steer.
        tokio::time::pause();
        tokio::time::sleep(Duration::from_millis(1)).await;
        tokio::time::resume();
        resume.send(()).expect("resume accounting");
    }
    drop(paused);
    let mut errors = Vec::new();
    let completed = wait_for_event(&test.codex, |event| {
        if let EventMsg::Error(error) = event {
            errors.push(error.clone());
        }
        assert!(!matches!(event, EventMsg::StreamError(_)), "{event:?}");
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let EventMsg::TurnComplete(completed) = completed else {
        unreachable!();
    };
    let [error] = errors.as_slice() else {
        anyhow::bail!("expected one turn error, got {errors:?}");
    };
    if incomplete {
        assert_eq!(
            error.message,
            "Incomplete response returned, reason: max_output_tokens"
        );
    } else if matches!(behavior, HostAccountingBehavior::Exhausted) {
        assert_eq!(
            error.codex_error_info,
            Some(CodexErrorInfo::SessionBudgetExceeded)
        );
    } else {
        assert!(error.message.contains("Host usage accounting failed"));
        assert_eq!(error.codex_error_info, Some(CodexErrorInfo::Other));
    }
    assert_eq!(received_responses_requests(&server).await.len(), 1);
    assert_eq!(
        *accounting.reports.lock().expect("usage reports"),
        vec![AgentUsageReport {
            thread_id: test.session_configured.thread_id,
            turn_id: completed.turn_id,
            usage: TokenUsage {
                input_tokens: 60,
                cached_input_tokens: 40,
                output_tokens: 15,
                reasoning_output_tokens: 5,
                total_tokens: 75,
                codex_rollout_budget_units: Some(
                    serde_json::Number::from_f64(40.5).expect("finite budget units"),
                ),
                ..Default::default()
            },
        }]
    );
    Ok(())
}

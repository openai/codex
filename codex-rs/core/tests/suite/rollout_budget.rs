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
use codex_history::RolloutItem;
use codex_model_provider_info::built_in_model_providers;
use codex_protocol::error::CodexErr;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
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
use core_test_support::wait_for_successful_turn;
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
    ExhaustedForTurnOnce,
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
            let report_count = {
                let mut reports = self.reports.lock().expect("usage reports");
                reports.push(report);
                reports.len()
            };
            if let Some(pause) = &self.pause_report {
                let (resume, resumed) = oneshot::channel();
                let _ = pause.send(resume);
                let _ = resumed.await;
            }
            match self.behavior {
                HostAccountingBehavior::Continue => Ok(UsageAccountingDecision::Continue),
                HostAccountingBehavior::Exhausted => Ok(UsageAccountingDecision::Exhausted),
                HostAccountingBehavior::ExhaustedForTurnOnce => Ok(if report_count == 1 {
                    UsageAccountingDecision::ExhaustedForTurn
                } else {
                    UsageAccountingDecision::Continue
                }),
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
        limit_budget: 100.0,
        reminder_at_context_start: true,
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

fn rollout_budget_message(remaining_tokens: impl std::fmt::Display) -> String {
    format!(
        "<rollout_budget>\nYou have {remaining_tokens} weighted tokens left in the shared session token budget.\n</rollout_budget>"
    )
}

fn wire_request_contains(request: &wiremock::Request, text: &str) -> bool {
    std::str::from_utf8(&request.body).is_ok_and(|body| body.contains(text))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(None, true ; "weighted tokens and initial reminder")]
#[test_case(Some(40.5), false ; "opaque units and threshold reminders only")]
async fn adds_weighted_initial_and_threshold_reminders(
    rollout_budget_units: Option<f64>,
    initial_reminder: bool,
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
        .with_config(move |config| {
            let mut budget = rollout_budget();
            if !initial_reminder {
                // Starting at 100.75 has not crossed the threshold at 100.
                budget.limit_budget = 100.75;
                budget.reminder_at_remaining_tokens = vec![100];
            }
            config.rollout_budget = Some(RolloutBudgetConfig {
                reminder_at_context_start: initial_reminder,
                sampling_token_weight: 2.0,
                prefill_token_weight: 0.5,
                ..budget
            });
        })
        .build(&server)
        .await?;

    test.submit_turn("first turn").await?;
    test.submit_turn("second turn").await?;

    let requests = responses.requests();
    assert_eq!(
        requests[0].has_content_kinds(&["rollout_budget.remaining_tokens"]),
        initial_reminder
    );
    let mut expected: Vec<_> = initial_reminder
        .then(|| rollout_budget_message(/*remaining_tokens*/ 100))
        .into_iter()
        .collect();
    assert_eq!(rollout_budget_texts(&requests[0]), expected);
    expected.push(rollout_budget_message(if rollout_budget_units.is_some() {
        60.25
    } else {
        60.0
    }));
    assert_eq!(rollout_budget_texts(&requests[1]), expected);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_provider_rollout_budget_units_fail_without_retry() -> Result<()> {
    use codex_history::RolloutItem;
    use codex_protocol::models::ResponseItem;
    use core_test_support::responses::ev_apply_patch_custom_tool_call;

    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut completed = ev_completed_with_tokens("invalid-units", /*total_tokens*/ 11);
    completed["response"]["usage"]["codex_rollout_budget_units"] = json!(-1.0);
    let responses = mount_sse_sequence(
        &server,
        vec![sse(vec![
            ev_response_created("invalid-units"),
            ev_apply_patch_custom_tool_call(
                "unbilled-patch",
                "*** Begin Patch\n*** Add File: unbilled-tool-ran\n+unexpected\n*** End Patch",
            ),
            completed,
        ])],
    )
    .await;
    let test = test_codex()
        .with_pre_build_hook(|home| {
            std::fs::write(
                home.join("config.toml"),
                "[responses]\nprocess_mode = \"after_complete_response\"\n",
            )
            .expect("configure buffered response processing");
        })
        .with_config(|config| config.rollout_budget = Some(rollout_budget()))
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
    assert_eq!(error.codex_error_info, Some(CodexErrorInfo::Other));
    assert!(error.message.contains("codex_rollout_budget_units"));
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(
        responses.requests().len(),
        1,
        "invalid units should not retry"
    );
    // Invalid accounting rejects the buffered response before any of its items are published.
    assert!(!test.workspace_path("unbilled-tool-ran").exists());
    test.codex.ensure_rollout_materialized().await;
    let history = test.codex.load_history(/*include_archived*/ false).await?;
    assert!(!history.items.iter().any(|item| matches!(item,
        RolloutItem::ResponseItem(envelope) if matches!(&envelope.item,
            ResponseItem::CustomToolCall { call_id, .. }
            | ResponseItem::CustomToolCallOutput { call_id, .. } if call_id == "unbilled-patch"
        )
    )));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn host_turn_exhaustion_posts_response_state_and_allows_a_later_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("exhausted"),
                ev_assistant_message("exhausted-message", "accepted output"),
                ev_completed_with_tokens("exhausted", /*total_tokens*/ 11),
            ]),
            sse(vec![
                ev_response_created("fresh"),
                ev_assistant_message("fresh-message", "fresh output"),
                ev_completed_with_tokens("fresh", /*total_tokens*/ 7),
            ]),
        ],
    )
    .await;
    let accounting = Arc::new(HostAccounting {
        behavior: HostAccountingBehavior::ExhaustedForTurnOnce,
        ..Default::default()
    });
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.thread_lifecycle_contributor(accounting.clone());
    let test = test_codex()
        .with_config(|config| {
            config.response_process_mode =
                codex_config::config_toml::ResponseProcessMode::AfterCompleteResponse;
        })
        .with_extensions(Arc::new(extensions.build()))
        .build_with_auto_env(&server)
        .await?;
    let input = |text: &str| {
        TurnInputRequest::user_input(vec![UserInput::Text {
            text: text.into(),
            text_elements: Vec::new(),
        }])
    };
    test.codex
        .start_or_steer_turn(input("spend the current turn allowance"))
        .await?;
    let mut saw_output = false;
    let mut saw_usage = false;
    let error = wait_for_event(&test.codex, |event| {
        saw_output |= matches!(event, EventMsg::AgentMessage(message) if message.message == "accepted output");
        saw_usage |= matches!(event, EventMsg::TokenCount(count)
            if count.info.as_ref().is_some_and(|info| info.total_token_usage.total_tokens == 11));
        matches!(event, EventMsg::Error(_) | EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!((saw_output, saw_usage), (true, true));
    assert!(matches!(error, EventMsg::Error(error)
        if error.codex_error_info == Some(CodexErrorInfo::SessionBudgetExceeded)));
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    test.codex
        .start_or_steer_turn(input("use a fresh turn allowance"))
        .await?;
    wait_for_successful_turn(&test.codex).await;
    let reports = accounting.reports.lock().expect("usage reports");
    assert_eq!(
        reports
            .iter()
            .map(|report| report.usage.as_ref().map(|usage| usage.total_tokens))
            .collect::<Vec<_>>(),
        vec![Some(11), Some(7)],
    );
    assert_ne!(reports[0].turn_id, reports[1].turn_id);

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
                limit_budget: if host_accounting { 1.0 } else { 100.0 },
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
        reports.sort_by_key(|report| report.usage.as_ref().expect("provider usage").total_tokens);
        let root_thread_id = test.session_configured.thread_id;
        assert_eq!(
            reports
                .iter()
                .map(|report| (
                    report.thread_id,
                    report.usage.as_ref().expect("provider usage").total_tokens
                ))
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
    const ACCEPTED_REVIEW: &str = r#"{"outcome":"allow"}"#;
    // Enforced: decimal equality is allowed; the next material charge exceeds it.
    // Advisory: a real overspend still permits a later request with a zero-balance reminder.
    let (limit_budget, reminder_at_remaining_tokens, charges): (f64, Vec<i64>, &[f64]) =
        if enforce_limit {
            (0.3, vec![], &[0.1, 0.2, 0.000001, 0.0])
        } else {
            (30.0, vec![20, 10], &[31.0, 1.0])
        };
    let crossing_turn = if enforce_limit { 2 } else { 0 };
    // The later user turn still runs, but an enforced Core budget refuses its model send.
    let model_charges = if enforce_limit && owner == AgentRuntimeOwnership::Controller {
        &charges[..=crossing_turn]
    } else {
        charges
    };
    let sequence = model_charges
        .iter()
        .enumerate()
        .map(|(turn, units)| {
            let id = format!("budget-{turn}");
            let mut completed = ev_completed_with_tokens(&id, /*total_tokens*/ 1);
            completed["response"]["usage"]["codex_rollout_budget_units"] = json!(units);
            let mut events = vec![ev_response_created(&id)];
            if turn == crossing_turn {
                events.push(ev_assistant_message("accepted-review", ACCEPTED_REVIEW));
            }
            events.push(completed);
            sse(events)
        })
        .collect();
    let responses = mount_sse_sequence(&server, sequence).await;
    let mut extensions = ExtensionRegistryBuilder::new();
    if owner == AgentRuntimeOwnership::Host {
        extensions.thread_lifecycle_contributor(Arc::new(HostRuntime));
    }
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(move |config| {
            config.rollout_budget = Some(RolloutBudgetConfig {
                enforce_limit,
                limit_budget,
                reminder_at_remaining_tokens,
                ..rollout_budget()
            });
        })
        .build_with_auto_env(&server)
        .await?;
    let late = Arc::new(HostAccounting::default());
    // This real Guardian policy replays accepted text only after Completed.
    test.codex
        .thread_extension_data()
        .insert(codex_guardian_reviewer::DenialResample::default());
    if owner == AgentRuntimeOwnership::Host {
        // Neither the owner nor the accounting provider can change after startup.
        let data = test.codex.thread_extension_data();
        data.insert(AgentRuntimeOwnership::Controller);
        data.insert(late.clone() as Arc<dyn AgentUsageAccounting>);
    }

    for turn in 0..charges.len() {
        test.codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: format!("Budget turn {turn}"),
                text_elements: Vec::new(),
            }]))
            .await?;

        let event = wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::Error(_) | EventMsg::TurnComplete(_))
        })
        .await;
        if turn < 2 || owner == AgentRuntimeOwnership::Host || !enforce_limit {
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

    test.codex.ensure_rollout_materialized().await;
    let history = test.codex.load_history(/*include_archived*/ false).await?;
    assert!(history.items.iter().any(|item| matches!(item, codex_history::RolloutItem::ResponseItem(envelope)
        if matches!(&envelope.item, codex_protocol::models::ResponseItem::Message { role, content, .. }
            if role == "assistant" && content.iter().any(|part| matches!(part,
                codex_protocol::models::ContentItem::OutputText { text, .. } if text == ACCEPTED_REVIEW))))),
        "a shared stop must not discard Guardian's already-selected final text");
    if owner == AgentRuntimeOwnership::Controller && enforce_limit {
        let warning_roles = history
            .items
            .iter()
            .filter_map(|item| {
                let RolloutItem::ResponseItem(item) = item else {
                    return None;
                };
                let ResponseItem::Message { role, content, .. } = &item.item else {
                    return None;
                };
                content
                    .iter()
                    .any(|part| {
                        matches!(part,
                            ContentItem::InputText { text }
                                if text.contains("<turn_aborted>")
                        )
                    })
                    .then_some(role.as_str())
            })
            .collect::<Vec<_>>();
        assert_eq!(warning_roles, ["developer", "developer"]);
    }

    if owner == AgentRuntimeOwnership::Host {
        let requests = responses.requests();
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
                limit_budget: 9.0,
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
        assert_eq!(
            reports[0]
                .usage
                .as_ref()
                .expect("provider usage")
                .total_tokens,
            10
        );
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
#[test_case(HostAccountingBehavior::ExhaustedForTurnOnce, false ; "host exhausts steered turn")]
#[test_case(HostAccountingBehavior::Fail, false ; "accounting failure never retries inference")]
#[test_case(HostAccountingBehavior::Fail, true ; "incomplete response keeps original error")]
async fn host_accounting_reports_raw_usage_and_preserves_error_semantics(
    behavior: HostAccountingBehavior,
    incomplete: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let exhausts_budget = matches!(
        behavior,
        HostAccountingBehavior::Exhausted | HostAccountingBehavior::ExhaustedForTurnOnce
    );
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
        .with_config(move |config| {
            for feature in [
                Feature::InstantInterrupt,
                Feature::DefaultModeRequestUserInput,
            ] {
                config
                    .features
                    .enable(feature)
                    .expect("enable test feature");
            }
            config.model_provider.stream_max_retries = Some(2);
            config.model_provider.request_max_retries = Some(0);
            // This allowance would admit the response. The selected host can still stop it.
            config.rollout_budget = exhausts_budget.then(rollout_budget);
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
    } else if exhausts_budget {
        assert_eq!(
            error.codex_error_info,
            Some(CodexErrorInfo::SessionBudgetExceeded)
        );
        assert_eq!(completed.error.as_ref(), Some(error));
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
            usage: Some(TokenUsage {
                input_tokens: 60,
                cached_input_tokens: 40,
                output_tokens: 15,
                reasoning_output_tokens: 5,
                total_tokens: 75,
                codex_rollout_budget_units: Some(
                    serde_json::Number::from_f64(40.5).expect("finite budget units"),
                ),
                ..Default::default()
            }),
            additional_budget_units: 0.0,
        }]
    );
    Ok(())
}

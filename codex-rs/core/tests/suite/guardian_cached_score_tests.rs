//! Exercises async and serving assessments through real tool calls, hooks, and approval results.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_features::Feature;
use codex_history::InitialHistory;
use codex_history::ResumedHistory;
use codex_history::RetainedContextEvent;
use codex_history::RolloutItem;
use codex_history::VerifiedAnswer;
use codex_history::VerifiedQuestionAnswer;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_protocol::approvals::GuardianAssessmentStatus;
use codex_protocol::approvals::GuardianReviewReason;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_target_windows;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::sync::oneshot;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delayed_score_only_covers_the_tool_call_it_classified() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let permission_args = json!({
        "reason": "request network access",
        "permissions": { "network": { "enabled": true } },
    })
    .to_string();
    let mut parent_gates = Vec::new();
    let mut classifier_gates = Vec::new();
    let mut parent_responses = Vec::new();
    let mut classifier_responses = Vec::new();
    for (call_id, tool, args) in [
        (
            "before-publication",
            "request_permissions",
            permission_args.clone(),
        ),
        (
            "intervening-call",
            "update_plan",
            json!({ "plan": [{ "step": "Inspect permissions", "status": "in_progress" }] })
                .to_string(),
        ),
        (
            "stale-publication",
            "request_permissions",
            permission_args.clone(),
        ),
        ("fresh-publication", "request_permissions", permission_args),
    ] {
        let (parent_tx, parent_rx) = oneshot::channel();
        parent_gates.push(parent_tx);
        parent_responses.push(vec![StreamingSseChunk {
            gate: Some(parent_rx),
            body: sse(vec![
                ev_response_created(call_id),
                ev_function_call(call_id, tool, &args),
                ev_completed(call_id),
            ]),
        }]);
        let (classifier_tx, classifier_rx) = oneshot::channel();
        classifier_gates.push(classifier_tx);
        classifier_responses.push(vec![StreamingSseChunk {
            gate: Some(classifier_rx),
            body: sse(vec![
                ev_response_created(call_id),
                ev_assistant_message(call_id, "low"),
                ev_completed(call_id),
            ]),
        }]);
    }
    let (parent_server, _) = start_streaming_sse_server(parent_responses).await;
    let (classifier_server, _) = start_streaming_sse_server(classifier_responses).await;

    // Route the concurrent parent and classifier requests to independent gated streams.
    for (model, destination) in [
        ("guardian-publication-parent", parent_server.uri()),
        ("gpt-5.6-luna", classifier_server.uri()),
    ] {
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .and(body_partial_json(json!({ "model": model })))
            .respond_with(
                ResponseTemplate::new(/*s*/ 307)
                    .insert_header("location", format!("{destination}/v1/responses")),
            )
            .with_priority(/*p*/ 1)
            .up_to_n_times(/*n*/ 4)
            .mount(&server)
            .await;
    }
    let final_response = responses::mount_sse_once_match(
        &server,
        body_partial_json(json!({ "model": "guardian-publication-parent" })),
        sse(vec![
            ev_response_created("done"),
            ev_assistant_message("done", "done"),
            ev_completed("done"),
        ]),
    )
    .await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({ "model": "gpt-5.5" })))
        .respond_with(responses::sse_response(sse(vec![
            ev_assistant_message("invalid", "invalid assessment"),
            ev_completed("invalid"),
        ])))
        .with_priority(1)
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(body_partial_json(json!({
            "model": "gpt-5.5",
            "client_metadata": { "x-openai-subagent": "guardian" },
        })))
        .respond_with(responses::sse_response(sse(vec![
            ev_response_created("review"),
            ev_assistant_message(
                "review",
                &json!({
                    "risk_level": "high",
                    "user_authorization": "low",
                    "outcome": "deny",
                    "rationale": "Network access is not authorized.",
                })
                .to_string(),
            ),
            ev_completed("review"),
        ])))
        .with_priority(/*p*/ 2)
        .expect(/*r*/ 2)
        .mount(&server)
        .await;

    let mut test = test_codex()
        .with_model("guardian-initial-model")
        .with_pre_build_hook(|home| {
            std::fs::write(
                home.join("config.toml"),
                "[features.guardianv2]\nenabled = true\npersist_scores = true\nmax_tool_call_lag = 1\n\n[features.guardianv2.review_scope]\ncomputer_use_only = false\n",
            )
            .expect("write Guardian configuration");
        })
        .with_model_info_override("guardian-publication-parent", |model| {
            model.guardian = None;
            model.auto_review_model_override = Some("gpt-5.5".to_owned());
        })
        .with_config(|config| {
            config.update_plan_enabled = true;
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("set read-only permissions");
            config
                .features
                .enable(Feature::RequestPermissionsTool)
                .expect("enable permission requests");
        })
        .build_with_auto_env(&server)
        .await?;
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory("test", "test", "1", Default::default()).with_runtime_reader(),
    )?;
    let mut options = test.start_thread_options().await;
    options.history_mode = Some(codex_protocol::protocol::ThreadHistoryMode::Legacy);
    options.thread_extension_init.insert(metrics.clone());
    test.codex.shutdown_and_wait().await?;
    test.codex = test.thread_manager.start_thread(options).await?.thread;
    test.codex.ensure_rollout_materialized().await;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Inspect the available permissions.".to_owned(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                model: Some("guardian-publication-parent".into()),
                ..Default::default()
            }),
        )
        .await?;

    let mut parent_gates = parent_gates.into_iter();
    let mut classifier_gates = classifier_gates.into_iter();
    parent_gates.next().unwrap().send(()).unwrap();
    timeout(Duration::from_secs(30), async {
        classifier_server.wait_for_request_count(/*count*/ 1).await;
        parent_server.wait_for_request_count(/*count*/ 2).await;
    })
    .await?;
    parent_gates.next().unwrap().send(()).unwrap();
    timeout(Duration::from_secs(30), async {
        classifier_server.wait_for_request_count(/*count*/ 2).await;
        parent_server.wait_for_request_count(/*count*/ 3).await;
    })
    .await?;

    // Publishing call 1 after call 2 starts must not claim coverage through call 2.
    // At call 3 it is two calls behind, exceeding the configured lag of one.
    let first_score = classifier_gates.next().unwrap();
    let _held_intervening_score = classifier_gates.next().unwrap();
    let current_score = classifier_gates.next().unwrap();
    let _held_final_score = classifier_gates.next().unwrap();
    let publish_score = async |call_id: &str, release_score: oneshot::Sender<()>| -> Result<()> {
        release_score.send(()).unwrap();
        // Persisted rollout scores are emitted only after publication. Observe that
        // public result instead of reading or modifying the private score state.
        timeout(Duration::from_secs(30), async {
            loop {
                let history = test.codex.load_history(/*include_archived*/ false).await?;
                if history.items.into_iter().any(
                    |item| matches!(item, RolloutItem::SecurityRiskScore(score) if score.call_id.as_deref() == Some(call_id)),
                ) {
                    return Ok::<_, anyhow::Error>(());
                }
                tokio::task::yield_now().await;
            }
        })
        .await??;
        Ok(())
    };
    publish_score("before-publication", first_score).await?;
    parent_gates.next().unwrap().send(()).unwrap();
    timeout(Duration::from_secs(30), async {
        classifier_server.wait_for_request_count(/*count*/ 3).await;
        parent_server.wait_for_request_count(/*count*/ 4).await;
    })
    .await?;

    // Call 3's result is now fresh enough for call 4, whose own score is still held.
    publish_score("stale-publication", current_score).await?;
    parent_gates.next().unwrap().send(()).unwrap();

    let mut review_reasons = Vec::new();
    loop {
        match wait_for_event(&test.codex, |_| true).await {
            EventMsg::GuardianAssessment(event)
                if event.status == GuardianAssessmentStatus::Denied =>
            {
                review_reasons.push(event.review_reason);
            }
            EventMsg::RequestPermissions(event) => panic!("unexpected user prompt: {event:?}"),
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }
    assert_eq!(
        review_reasons,
        vec![
            Some(GuardianReviewReason::MissingScore),
            Some(GuardianReviewReason::StaleScore)
        ],
    );
    let request = final_response.single_request();
    for (call_id, network) in [
        ("before-publication", None),
        ("stale-publication", None),
        (
            "fresh-publication",
            Some(NetworkPermissions {
                enabled: Some(true),
            }),
        ),
    ] {
        let output = request
            .function_call_output_text(call_id)
            .expect("permission result");
        assert_eq!(
            serde_json::from_str::<RequestPermissionsResponse>(&output)?,
            RequestPermissionsResponse {
                permissions: RequestPermissionProfile {
                    network,
                    file_system: None
                },
                scope: PermissionGrantScope::Turn,
                strict_auto_review: false,
            },
            "{call_id}",
        );
    }

    test.codex.shutdown_and_wait().await?;
    let snapshot = metrics.snapshot()?;
    let route = snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        .find(|metric| metric.name() == "codex.guardian.review.route")
        .unwrap();
    let AggregatedMetrics::U64(MetricData::Sum(routes)) = route.data() else {
        panic!("expected route counter");
    };
    for point in routes.data_points() {
        let model = point
            .attributes()
            .find(|tag| tag.key.as_str() == "model")
            .unwrap();
        assert_eq!(model.value.as_str(), "guardian-publication-parent");
    }
    let metric = snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        .find(|metric| metric.name() == "codex.guardian.review.retry_wait_ms")
        .unwrap();
    let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = metric.data() else {
        panic!("expected retry-wait histogram");
    };
    let points = histogram.data_points().collect::<Vec<_>>();
    assert_eq!(points.len(), 1);
    let point = points[0];
    let tags = point
        .attributes()
        .map(|tag| (tag.key.as_str(), tag.value.as_str().to_string()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(tags["model"], "guardian-publication-parent");
    assert_eq!(tags["failure_reason"], "parse_error");
    assert_eq!(tags["outcome"], "retry");
    assert_eq!(point.count(), 1);
    assert!(point.sum() > 0.0);
    parent_server.shutdown().await;
    classifier_server.shutdown().await;
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProbeCase {
    Standard,
    Escalated,
    Rewrite,
    Interrupted,
    CatalogPolicy,
    IncompleteAuthorization,
}

#[test_case::test_case(Some(false), false, "probe-item", ProbeCase::Standard, 0; "direct_low")]
#[test_case::test_case(Some(true), false, "probe-item", ProbeCase::Standard, 1; "direct_review")]
#[test_case::test_case(Some(false), true, "probe-item", ProbeCase::Standard, 0; "code_cell_low")]
#[test_case::test_case(Some(true), true, "probe-item", ProbeCase::Standard, 2; "code_cell_review")]
#[test_case::test_case(None, false, "probe-item", ProbeCase::Standard, 1; "missing_uses_async")]
#[test_case::test_case(Some(false), false, "other-item", ProbeCase::Standard, 1; "mismatched_uses_async")]
#[test_case::test_case(Some(false), false, "probe-item", ProbeCase::Escalated, 1; "explicit_escalation_still_reviews")]
#[test_case::test_case(Some(false), false, "probe-item", ProbeCase::Rewrite, 1; "direct_hook_rewrite")]
#[test_case::test_case(Some(false), true, "probe-item", ProbeCase::Rewrite, 2; "nested_hook_rewrite")]
#[test_case::test_case(Some(true), false, "probe-item", ProbeCase::Interrupted, 1; "interrupted_direct_review")]
#[test_case::test_case(Some(true), true, "probe-item", ProbeCase::Interrupted, 2; "interrupted_cell_review")]
#[test_case::test_case(Some(false), false, "probe-item", ProbeCase::CatalogPolicy, 0; "catalog_low_without_legacy_flag")]
#[test_case::test_case(Some(true), false, "probe-item", ProbeCase::CatalogPolicy, 1; "catalog_review_without_legacy_flag")]
#[test_case::test_case(Some(false), false, "probe-item", ProbeCase::IncompleteAuthorization, 1; "incomplete_authorization_still_reviews")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn serving_assessment_replaces_async_scoring(
    requires_review: Option<bool>,
    code_mode: bool,
    assessed_item: &str,
    scenario: ProbeCase,
    expected_reviews: u64,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_target_windows!(Ok(()), "exec_command requires a POSIX executor");
    core_test_support::skip_if_test_condition!(
        Ok(()),
        cfg!(target_os = "windows")
            && matches!(scenario, ProbeCase::Rewrite | ProbeCase::Interrupted),
        "a Windows host",
        "hook fixtures require a POSIX host",
    );
    let server = responses::start_mock_server().await;
    let mut arguments = json!({
        "cmd": "echo probe-policy", "login": false
    });
    if scenario == ProbeCase::Escalated {
        arguments["sandbox_permissions"] = json!("require_escalated");
        arguments["justification"] = json!("Run the requested command outside the sandbox.");
    }
    let mut tool = if code_mode {
        responses::ev_custom_tool_call(
            "call",
            "exec",
            &format!(
                "const first = await tools.exec_command({arguments}); \
             const second = await tools.exec_command({second_arguments}); \
             if (first.exit_code !== 0 || second.exit_code !== 0) throw new Error('command failed'); \
             text(first.output); text(second.output);",
                second_arguments = json!({"cmd":"echo probe-policy-second", "login":false})
            ),
        )
    } else {
        ev_function_call("call", "exec_command", &arguments.to_string())
    };
    tool["item"]["id"] = json!("probe-item");
    let mut events = vec![ev_response_created("probe-response")];
    if let Some(requires_review) = requires_review {
        events.push(json!({
            "type": "response.metadata", "response_id": "probe-response",
            "metadata": {"guardian": {"item_id": assessed_item, "requires_review": requires_review}}
        }));
    }
    events.push(tool);
    if scenario != ProbeCase::Interrupted {
        events.push(ev_completed("probe-response"));
    }
    let mut response = sse(events);
    if scenario == ProbeCase::Interrupted {
        response.push_str(&responses::sse_failed(
            "probe-response",
            "server_error",
            "response interrupted",
        ));
    }
    let parent = responses::mount_sse_sequence(
        &server,
        vec![
            response,
            sse(vec![
                ev_assistant_message("done", "done"),
                ev_completed("done"),
            ]),
        ],
    )
    .await;
    let uses_async = requires_review.is_none()
        || assessed_item != "probe-item"
        || scenario == ProbeCase::Rewrite;
    let expected_scores = if uses_async {
        1..=if code_mode { 2 } else { 1 }
    } else {
        0..=0
    };
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"model": "gpt-5.6-luna"})))
        .respond_with(responses::sse_response(sse(vec![
            ev_assistant_message(
                "score",
                if scenario == ProbeCase::Interrupted {
                    "low"
                } else {
                    "high"
                },
            ),
            ev_completed("score"),
        ])))
        .with_priority(/*p*/ 1)
        .expect(expected_scores)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"client_metadata": {"x-openai-subagent": "guardian"}})))
        .and(body_partial_json(json!({"model": "gpt-5.5"})))
        .respond_with(responses::sse_response(sse(vec![
            ev_assistant_message("review", r#"{"risk_level":"low","user_authorization":"high","outcome":"allow","rationale":"requested command"}"#),
            ev_completed("review"),
        ])))
        .with_priority(/*p*/ 1)
        .expect(expected_reviews)
        .mount(&server).await;
    let mut builder = test_codex()
        .with_model("guardian-probe-parent")
        .with_pre_build_hook(move |home| {
            #[cfg(not(target_os = "windows"))]
            match scenario {
                ProbeCase::Rewrite => super::hooks::write_updating_pre_tool_use_hook(
                    home, "^Bash$", &json!({"command": "echo probe-policy-second-rewritten"}),
                ).expect("write input rewrite hook"),
                ProbeCase::Interrupted => super::hooks::write_lifecycle_hooks(
                    home, &["PostModelRequest"],
                ).expect("write synchronous post-request hook"),
                ProbeCase::Standard | ProbeCase::Escalated
                | ProbeCase::CatalogPolicy | ProbeCase::IncompleteAuthorization => {}
            }
            let rules = home.join("rules");
            std::fs::create_dir_all(&rules).expect("create rules directory");
            std::fs::write(rules.join("default.rules"), r#"prefix_rule(pattern=["echo"], decision="prompt")"#)
                .expect("write prompt rule");
            std::fs::write(home.join("config.toml"),
                "[features.guardianv2]\nenabled = true\npersist_scores = true\n[features.guardianv2.review_scope]\ncomputer_use_only = false\nsandboxed_exec_commands = true\n"
            ).expect("write Guardian configuration");
        })
        .with_model_info_override("guardian-probe-parent", move |model| {
            model.guardian = (scenario == ProbeCase::CatalogPolicy).then(|| {
                serde_json::from_value(json!({
                    "shell": "adaptive", "sandboxed_exec_commands": true
                })).expect("catalog Guardian policy")
            });
            model.auto_review_model_override = Some("gpt-5.5".into());
        })
        .with_config(move |config| {
            if matches!(scenario, ProbeCase::Rewrite | ProbeCase::Interrupted) {
                trust_discovered_hooks(config);
            }
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.permissions.set_permission_profile(PermissionProfile::read_only()).expect("read-only permissions");
            config.features.set_enabled(Feature::CodeMode, code_mode).expect("configure Code Mode");
        });
    let mut test = builder.build_with_auto_env(&server).await?;
    if scenario == ProbeCase::CatalogPolicy {
        // The fixture installs the extension under the legacy flag. Production always installs it.
        let mut options = test.start_thread_options().await;
        options
            .config
            .features
            .disable(Feature::GuardianV2)
            .expect("disable legacy scorer flag");
        test.codex.shutdown_and_wait().await?;
        test.codex = test.thread_manager.start_thread(options).await?.thread;
    }
    if scenario == ProbeCase::IncompleteAuthorization {
        // Restore enough verified answers to evict an original before the serving request.
        let mut options = test.start_thread_options().await;
        options.history_mode = Some(codex_protocol::protocol::ThreadHistoryMode::Legacy);
        options.initial_history = InitialHistory::Resumed(ResumedHistory {
            history_revision: None,
            conversation_id: test.codex.startup_metadata().thread_id,
            history: Arc::new(
                (0..9)
                    .map(|index| {
                        RolloutItem::RetainedContext(RetainedContextEvent::VerifiedAnswer {
                            answer: VerifiedAnswer {
                                turn_id: "previous-turn".into(),
                                call_id: format!("answer-{index}"),
                                questions: vec![VerifiedQuestionAnswer {
                                    question: "Run the command?".into(),
                                    answer: "Yes".into(),
                                }],
                            },
                            acceptance_order: Some(index),
                        })
                    })
                    .collect(),
            ),
            rollout_path: None,
        });
        test.codex.ensure_rollout_materialized().await;
        test.codex.shutdown_and_wait().await?;
        test.thread_manager
            .remove_thread(&test.codex.startup_metadata().thread_id)
            .await;
        test.codex = test.thread_manager.start_thread(options).await?.thread;
        assert!(
            !test
                .codex
                .conversation_history_snapshot()
                .await
                .retained_context()
                .expect("restored authorization")
                .verified_answers_complete()
        );
    }
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Run the requested echo commands.".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    if uses_async {
        timeout(Duration::from_secs(30), async {
            loop {
                if server
                    .received_requests()
                    .await
                    .unwrap_or_default()
                    .iter()
                    .any(|request| {
                        request
                            .body_json::<serde_json::Value>()
                            .is_ok_and(|body| body["model"] == "gpt-5.6-luna")
                    })
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await?;
    }
    test.codex.shutdown_and_wait().await?;
    let requests = parent.requests();
    assert_eq!(requests.len(), 2);
    let output = requests[1].call_output(
        "call",
        if code_mode {
            "custom_tool_call_output"
        } else {
            "function_call_output"
        },
    );
    let output = output["output"].to_string();
    if scenario == ProbeCase::Rewrite {
        assert!(
            output.contains("probe-policy-second-rewritten"),
            "rewritten command ran: {output}"
        );
    }
    if code_mode {
        assert!(
            output.contains("probe-policy-second"),
            "both nested commands ran: {output}"
        );
    } else {
        assert!(
            output.contains("Process exited with code 0"),
            "the direct command succeeded: {output}"
        );
    }
    assert!(
        output.contains("probe-policy"),
        "the assessed command ran: {output}"
    );
    Ok(())
}

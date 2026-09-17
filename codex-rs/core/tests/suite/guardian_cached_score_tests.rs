//! Exercises asynchronous score publication through real tool calls and approval results.

use std::time::Duration;

use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_protocol::approvals::GuardianAssessmentStatus;
use codex_protocol::approvals::GuardianReviewReason;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
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

    let test = test_codex()
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
    test.codex.ensure_rollout_materialized().await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Inspect the available permissions.".to_owned(),
            text_elements: Vec::new(),
        }]))
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
    parent_server.shutdown().await;
    classifier_server.shutdown().await;
    Ok(())
}

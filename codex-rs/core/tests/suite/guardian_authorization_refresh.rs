//! Reproduces parent input racing a subagent's in-flight Guardian approval.

use anyhow::Context;
use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::*;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::skip_if_wine_exec;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::VecDeque;
use std::time::Duration;
use test_case::test_case;
use tokio::sync::oneshot;

#[derive(Clone, Copy)]
enum Change {
    None,
    Status,
    Revoke,
    Repeated,
    Cancel,
    RootReset,
}

#[test_case(Change::None; "unchanged authorization")]
#[test_case(Change::Status; "parent status message")]
#[test_case(Change::Revoke; "parent revokes authorization")]
#[test_case(Change::Repeated; "repeated parent updates exhaust budget")]
#[test_case(Change::Cancel; "explicit worker cancellation")]
#[test_case(Change::RootReset; "root history reset")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_refreshes_subagent_authorization(change: Change) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );

    let (attempts, updates, expected_status) = match change {
        Change::None => (1, 0, GuardianAssessmentStatus::Approved),
        Change::Status => (2, 1, GuardianAssessmentStatus::Approved),
        Change::Revoke => (2, 1, GuardianAssessmentStatus::Denied),
        Change::Repeated => (3, 3, GuardianAssessmentStatus::Denied),
        Change::Cancel => (1, 0, GuardianAssessmentStatus::Aborted),
        Change::RootReset => (1, 1, GuardianAssessmentStatus::Aborted),
    };
    let (root_tx, root_rx) = oneshot::channel();
    let mut gates = VecDeque::new();
    let mut streams = vec![vec![
        StreamingSseChunk {
            gate: None,
            body: sse(vec![
                ev_response_created("root-spawn"),
                ev_function_call_with_namespace(
                    "spawn-worker", "collaboration", "spawn_agent",
                    &json!({"task_name": "worker", "message": "Write the requested marker once."}).to_string(),
                ),
            ]),
        },
        StreamingSseChunk { gate: Some(root_rx), body: sse(vec![ev_completed("root-spawn")]) },
    ], vec![StreamingSseChunk {
        gate: None,
        body: sse(vec![
            ev_function_call("write-marker", "exec_command", &json!({
                "cmd": "echo executed >> guardian-parent-refresh.txt", "login": false,
                "sandbox_permissions": "require_escalated", "justification": "Write the requested marker once.",
            }).to_string()),
            ev_completed("worker-call"),
        ]),
    }]];
    for attempt in 0..attempts {
        let (tx, rx) = oneshot::channel();
        gates.push_back(tx);
        let outcome = if matches!(change, Change::Revoke) && attempt > 0 {
            "deny"
        } else {
            "allow"
        };
        streams.push(vec![StreamingSseChunk {
            gate: Some(rx),
            body: sse(vec![
                ev_assistant_message(
                    "assessment",
                    &json!({
                        "risk_level": "low", "user_authorization": "high", "outcome": outcome,
                        "rationale": "Decision for the current authorization snapshot.",
                    })
                    .to_string(),
                ),
                ev_completed(&format!("review-{attempt}")),
            ]),
        }]);
        if attempt == 0 {
            streams.push(vec![StreamingSseChunk {
                gate: None,
                body: sse(vec![ev_completed("root-done")]),
            }]);
        }
        if attempt < updates {
            streams.push(vec![StreamingSseChunk { gate: None, body: if matches!(change, Change::RootReset) {
                sse(vec![json!({"type": "response.output_item.done", "item": {
                    "type": "compaction", "id": "compacted", "encrypted_content": "known producer"
                }}), ev_completed("root-compacted")])
            } else { sse(vec![ev_completed(&format!("root-update-{attempt}"))]) } }]);
        }
    }
    streams.push(vec![StreamingSseChunk {
        gate: None,
        body: sse(vec![ev_completed("worker-done")]),
    }]);
    let (streaming, _) = start_streaming_sse_server(streams).await;
    let base_url = format!("{}/v1", streaming.uri());
    let server = start_mock_server().await;
    let mut test = test_codex()
        .with_model_info_override("test-gpt-5.1-codex", |model| {
            model.comp_hash = Some("compatible".to_owned());
            model.auto_review_model_override = Some(model.slug.clone());
        })
        .with_config(move |config| {
            config.model_provider.base_url = Some(base_url);
            for feature in [Feature::Collab, Feature::MultiAgentV2] {
                config
                    .features
                    .enable(feature)
                    .expect("enable test feature");
            }
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .permissions
                .set_permission_profile(PermissionProfile::workspace_write())
                .expect("set workspace-write permissions");
        })
        .build_with_auto_env(&server)
        .await?;
    if matches!(change, Change::RootReset) {
        test.codex.ensure_rollout_materialized().await;
        test.codex = super::guardian_checkpoint_migration::resume(
            &test,
            &test.codex,
            vec![RolloutItem::Compacted(serde_json::from_value(json!({
                "message": "old checkpoint",
                "replacement_history": [{
                    "type": "compaction", "id": "old", "encrypted_content": "unknown producer"
                }]
            }))?)],
        )
        .await?;
    }
    let mut created = test.thread_manager.subscribe_thread_created();
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Spawn a worker to write the requested marker once.".into(),
            text_elements: vec![],
        }]))
        .await?;
    tokio::time::timeout(
        Duration::from_secs(/*secs*/ 10),
        streaming.wait_for_request_count(/*count*/ 3),
    )
    .await?;
    let worker = test
        .thread_manager
        .get_thread(created.recv().await?)
        .await?;
    root_tx.send(()).expect("release root response");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let mut review_index = 2;
    let mut latest_message: Option<String> = None;
    let marker = test.workspace_path_uri("guardian-parent-refresh.txt")?;
    for attempt in 0..attempts {
        let requests = streaming.requests().await;
        let review: Value = serde_json::from_slice(&requests[review_index])?;
        assert_eq!(
            review["client_metadata"]["x-openai-subagent"], "guardian",
            "attempt {attempt}: expected a fresh review rather than worker continuation"
        );
        if let Some(message) = &latest_message {
            assert!(
                review.to_string().contains(message),
                "refreshed review omitted latest parent input"
            );
        }
        // Each retry reviews the same action. The executor has not run it yet.
        assert!(review.to_string().contains("guardian-parent-refresh.txt"));
        assert_eq!(
            test.fs()
                .get_metadata(&marker, Default::default(), /*sandbox*/ None)
                .await
                .expect_err("unapproved action must not create its marker")
                .kind(),
            std::io::ErrorKind::NotFound
        );
        if matches!(change, Change::Cancel) {
            worker.submit(Op::Interrupt).await?;
            break;
        }
        if attempt < updates {
            let message = if matches!(change, Change::Revoke) {
                "Do not write the marker after all.".to_owned()
            } else {
                format!("Status update {attempt}: how is the worker doing?")
            };
            if matches!(change, Change::RootReset) {
                test.codex.submit(Op::Compact).await?;
                wait_for_event(&test.codex, |event| {
                    matches!(event, EventMsg::TurnComplete(_))
                })
                .await;
            } else {
                test.submit_text_turn(&message).await?;
                latest_message = Some(message);
            }
        }
        review_index = streaming.requests().await.len();
        gates
            .pop_front()
            .expect("review completion gate")
            .send(())
            .expect("release review response");
        if attempt + 1 < attempts {
            // The next request is another review, never another worker tool invocation.
            tokio::time::timeout(
                Duration::from_secs(/*secs*/ 10),
                streaming.wait_for_request_count(review_index + 1),
            )
            .await
            .context("stale authorization did not trigger a fresh review")?;
        }
    }
    let mut statuses = Vec::new();
    let mut worker_finished = false;
    while !worker_finished || statuses.len() < 2 {
        let event =
            tokio::time::timeout(Duration::from_secs(/*secs*/ 10), worker.next_event()).await??;
        match event.msg {
            EventMsg::GuardianAssessment(assessment) => statuses.push(assessment.status),
            EventMsg::TurnComplete(_) | EventMsg::TurnAborted(_) => worker_finished = true,
            _ => {}
        }
    }
    assert_eq!(
        statuses,
        vec![GuardianAssessmentStatus::InProgress, expected_status]
    );
    if expected_status == GuardianAssessmentStatus::Approved {
        let contents = test
            .fs()
            .read_file_text(&marker, Default::default(), /*sandbox*/ None)
            .await?;
        assert_eq!(contents.lines().collect::<Vec<_>>(), vec!["executed"]);
    } else {
        assert_eq!(
            test.fs()
                .get_metadata(&marker, Default::default(), /*sandbox*/ None)
                .await
                .expect_err("unapproved action must not create its marker")
                .kind(),
            std::io::ErrorKind::NotFound
        );
    }
    let requests = streaming.requests().await;
    let guardian_requests = requests
        .iter()
        .map(|request| serde_json::from_slice::<Value>(request).expect("valid Responses request"))
        .filter(|request| request["client_metadata"]["x-openai-subagent"] == "guardian")
        .count();
    assert_eq!(guardian_requests, attempts);
    let shutdown = test
        .thread_manager
        .shutdown_all_threads_bounded(Duration::from_secs(/*secs*/ 10))
        .await;
    assert!(shutdown.timed_out.is_empty());
    streaming.shutdown().await;
    Ok(())
}

//! Guardian reviews a hosted worker using evidence from its root session.
use super::*;
use crate::suite::guardian_host::HostGuardianContext;
use crate::suite::guardian_host::HostRead;
use codex_core::GuardianScoreAuthorization;
use codex_core::config::Constrained;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GuardianAssessmentStatus;
use core_test_support::ThreadIdle;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::sse_response;
use pretty_assertions::assert_eq;
use test_case::test_case;

const HOST_ROOT_INSTRUCTION: &str = "Never mutate the production database.";

#[test_case(HostRead::Unavailable, HostRead::Reloaded; "allow then unavailable and reloaded")]
#[test_case(HostRead::Stalled, HostRead::Stalled; "recovered denial then deadline and cancellation")]
#[tokio::test]
async fn host_guardian_context_is_required_for_review(
    preparation: HostRead,
    validation: HostRead,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let reconsider = matches!(preparation, HostRead::Stalled);
    let server = start_mock_server().await;
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.thread_lifecycle_contributor(Arc::new(ThreadIdle));
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_model_info_override("gpt-5.5", |model| {
            model.multi_agent_version = Some(MultiAgentVersion::V2);
            model.auto_review_model_override = Some("gpt-5.6-luna".into());
        })
        .with_config(move |config| {
            config.multi_agent_v2.disable_native_tools = true;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            if reconsider {
                config
                    .features
                    .enable(Feature::GuardianDenialReconsideration)
                    .expect("enable denial reconsideration");
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let mut changes = test.codex.subscribe_guardian_evidence_changes();
    let initial_sequence = *changes.borrow_and_update();
    core_test_support::responses::mount_sse_once(&server, sse(vec![ev_completed("root")])).await;
    test.submit_text_turn(HOST_ROOT_INSTRUCTION).await?;
    assert!(changes.has_changed()?);
    let observed_sequence = *changes.borrow_and_update();
    assert!(observed_sequence > initial_sequence);
    let evidence = test.codex.capture_guardian_root_evidence().await?;
    assert!(evidence.sequence >= observed_sequence);
    let mut options = test.start_thread_options().await;
    let context = HostGuardianContext::install(&mut options, evidence);
    let worker = test.thread_manager.start_thread(options).await?.thread;
    let inspect = |text: &str| {
        worker.start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: text.into(),
            text_elements: vec![],
        }]))
    };
    let action = || {
        sse(vec![
            ev_function_call(
                "inspect",
                "exec_command",
                r#"{"cmd":"echo hosted-inspection-complete","justification":"Inspect the database.","sandbox_permissions":"require_escalated"}"#,
            ),
            ev_completed("action"),
        ])
    };
    let done = || sse(vec![ev_completed("done")]);
    let expire_review = || async {
        tokio::time::pause();
        let timeout = test.config.guardian_review_timeout.expect("review timeout");
        tokio::time::advance(timeout + Duration::from_secs(1)).await;
        tokio::time::resume();
    };
    const ALLOW: &str = r#"{"risk_level":"low","user_authorization":"high","outcome":"allow","rationale":"Read-only inspection."}"#;
    const DENY: &str =
        r#"{"outcome":"deny","rationale":"The root did not authorize this action."}"#;
    let mut responses = vec![
        sse_response(action()),
        sse_response(sse(vec![
            ev_assistant_message("decision", if reconsider { DENY } else { ALLOW }),
            ev_completed("review"),
        ])),
    ];
    let second_sample = if reconsider {
        let (hold, gate) = oneshot::channel();
        let (stream, _) = start_streaming_sse_server(vec![vec![StreamingSseChunk {
            gate: Some(gate),
            body: done(),
        }]])
        .await;
        responses.push(
            wiremock::ResponseTemplate::new(/*s*/ 307)
                .insert_header("location", format!("{}/v1/responses", stream.uri())),
        );
        Some((stream, hold))
    } else {
        None
    };
    responses.push(sse_response(done()));
    let initial = core_test_support::responses::mount_response_sequence(&server, responses).await;
    inspect("Inspect the database.").await?;
    if let Some((stream, _)) = &second_sample {
        tokio::time::timeout(
            Duration::from_secs(10),
            stream.wait_for_request_count(/*count*/ 1),
        )
        .await?;
        expire_review().await;
    }
    wait_for_event(&worker, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    let requests = initial.requests();
    assert!(!requests[0].body_contains_text(HOST_ROOT_INSTRUCTION));
    assert!(requests[1].body_contains_text(HOST_ROOT_INSTRUCTION));
    let output = requests
        .last()
        .expect("worker continuation")
        .function_call_output_text("inspect")
        .expect("inspection result");
    if let Some((stream, hold)) = second_sample {
        drop(hold);
        stream.shutdown().await;
        assert_eq!(requests.len(), 4);
        assert!(
            output.contains("The root did not authorize this action."),
            "{output}"
        );
    } else {
        assert_eq!(requests.len(), 3);
        assert!(!requests[2].body_contains_text(HOST_ROOT_INSTRUCTION));
        assert!(
            output
                .lines()
                .any(|line| line.trim() == "hosted-inspection-complete"),
            "{output}"
        );
        let history = core_test_support::responses::received_responses_requests(&server).await;
        assert_eq!(history.len(), 4);
        let snapshot = super::guardian_reconsideration::format_guardian_exec_history(
            "The root sets a restriction. A hosted worker requests an inspection. Guardian reviews it with root evidence, then the worker receives the command output.",
            &history,
            &test,
        )?;
        insta::assert_snapshot!("hosted_guardian_allow", snapshot);
    }

    ThreadIdle::wait(&worker).await;
    *context.read.lock().expect("host read mode") = preparation;
    let failed =
        core_test_support::responses::mount_sse_sequence(&server, vec![action(), done()]).await;
    inspect("Inspect again.").await?;
    if matches!(preparation, HostRead::Stalled) {
        tokio::time::timeout(Duration::from_secs(10), context.stalled.notified()).await?;
        expire_review().await;
    }
    wait_for_event(&worker, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    let requests = failed.requests();
    assert_eq!(
        requests.len(),
        2,
        "no Guardian inference without root evidence"
    );
    let expected = if matches!(preparation, HostRead::Stalled) {
        "did not finish before its deadline"
    } else {
        "Guardian root evidence unavailable"
    };
    assert!(requests[1].body_contains_text(expected));
    assert!(!requests[1].body_contains_text("root context unavailable"));

    ThreadIdle::wait(&worker).await;
    *context.read.lock().expect("host read mode") = HostRead::Current;
    let permissions = codex_guardian_context::PermissionContext::default();
    let prior_score = GuardianScoreAuthorization::current(&worker, &permissions).await?;
    core_test_support::responses::mount_sse_once(&server, action()).await;
    let is_guardian = |request: &ResponsesRequest| {
        request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian"
    };
    let provider = Arc::clone(&context);
    let review = core_test_support::responses::mount_response_handler(&server, move |request| {
        if !is_guardian(&request) {
            return sse_response(done());
        }
        *provider.read.lock().expect("host read mode") = validation;
        sse_response(sse(vec![
            ev_assistant_message("stale-approval", ALLOW),
            ev_completed("stale-review"),
        ]))
    })
    .await;
    inspect("Retry the inspection.").await?;
    if matches!(validation, HostRead::Stalled) {
        tokio::time::timeout(Duration::from_secs(10), context.stalled.notified()).await?;
        worker.submit(Op::Interrupt).await?;
    }
    let status = wait_for_event_match(&worker, |event| match event {
        EventMsg::GuardianAssessment(assessment)
            if assessment.status != GuardianAssessmentStatus::InProgress =>
        {
            Some(assessment.status)
        }
        _ => None,
    })
    .await;
    assert_eq!(status, GuardianAssessmentStatus::Aborted);
    assert_eq!(review.requests().into_iter().filter(is_guardian).count(), 1);
    if matches!(validation, HostRead::Stalled) {
        return Ok(());
    }

    // Keep worker-local revisions current so only the previous root can invalidate the score.
    ThreadIdle::wait(&worker).await;
    let mut score = GuardianScoreAuthorization::current(&worker, &permissions).await?;
    assert!(score.is_current(&worker).await?);
    score.root = prior_score.root;
    assert!(!score.is_current(&worker).await?);
    Ok(())
}

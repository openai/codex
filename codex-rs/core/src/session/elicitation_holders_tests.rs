use codex_direct_tool_timing::ToolBatchTiming;
use codex_direct_tool_timing::ToolTimingGuard;
use codex_features::Feature;
use codex_utils_critical_path::CriticalPath;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use crate::tools::handlers::PreparedPermissionRequest;
use codex_core_plugins::PluginCommandAttribution;
use codex_plugin::PluginId;
use codex_protocol::approvals::ExecApprovalKind;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::request_user_input::RequestUserInputArgs;
use codex_protocol::request_user_input::RequestUserInputResponse;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use super::tests::make_session_and_context_with_rx;
use crate::session::step_context::StepContext;
use crate::state::ActiveTurn;

fn start_timing(
    turn: &mut Arc<super::turn_context::TurnContext>,
    enabled: bool,
) -> (Arc<ToolTimingGuard>, Instant) {
    Arc::make_mut(&mut Arc::get_mut(turn).expect("unique test turn").config)
        .features
        .set_enabled(Feature::CriticalPathTiming, enabled)
        .unwrap();
    let start = Instant::now();
    let owner = turn
        .turn_timing_state
        .direct_tool_timings
        .start_turn_direct_call(
            "call-1",
            &turn.sub_id,
            ToolBatchTiming::default(),
            start,
            Some(CriticalPath::default()),
        );
    (owner, start)
}

fn assert_user_wait(owner: &ToolTimingGuard, start: Instant, enabled: bool) {
    let completed = owner.finish().unwrap();
    let totals = completed.path.unwrap().durations(start, completed.ended_at);
    assert_eq!(totals.user_wait > Duration::ZERO, enabled);
}

async fn wait_until_held(pause_state: &mut watch::Receiver<bool>) {
    pause_state
        .wait_for(|paused| *paused)
        .await
        .expect("elicitation service should remain available");
}

async fn wait_until_released(pause_state: &mut watch::Receiver<bool>) {
    pause_state
        .wait_for(|paused| !*paused)
        .await
        .expect("elicitation service should remain available");
}

#[test_case::test_case(true, "call-1"; "capture_on")]
#[test_case::test_case(false, "call-1"; "capture_off")]
#[test_case::test_case(true, "network-prompt"; "distinct_network_prompt_identity")]
#[tokio::test]
async fn command_approval_holds_an_elicitation_until_response(capture: bool, prompt_id: &str) {
    let (session, mut turn_context, events) = make_session_and_context_with_rx().await;
    let (timing, timing_start) = start_timing(&mut turn_context, capture);
    *session.active_turn.lock().await = Some(ActiveTurn::default());
    let mut pause_state = session.subscribe_elicitation_pause_state();
    #[allow(deprecated)]
    let cwd = turn_context.cwd.clone();
    let plugin_attribution = PluginCommandAttribution {
        plugin_id: PluginId::parse("sample@openai-curated").expect("valid plugin id"),
        normalized_relative_path: "scripts/run.py".to_string(),
    };

    let request = tokio::spawn({
        let prompt_id = prompt_id.to_owned();
        let session = session.clone();
        let turn_context = turn_context.clone();
        async move {
            session
                .request_command_approval(
                    turn_context.as_ref(),
                    ExecApprovalKind::Command,
                    crate::guardian::GuardianReviewContext::from(turn_context.clone())
                        .model_context(),
                    prompt_id,
                    Some("call-1"),
                    /*approval_id*/ None,
                    /*environment_id*/ None,
                    vec!["echo".to_string()],
                    cwd.into(),
                    /*reason*/ None,
                    /*network_approval_context*/ None,
                    /*proposed_execpolicy_amendment*/ None,
                    /*additional_permissions*/ None,
                    /*available_decisions*/ None,
                    /*plugin_attribution_override*/ Some(plugin_attribution),
                )
                .await
        }
    });

    let event = events.recv().await.expect("approval event");
    let codex_protocol::protocol::EventMsg::ExecApprovalRequest(event) = event.msg else {
        panic!("expected command approval event");
    };
    assert_eq!(event.plugin_id.as_deref(), Some("sample@openai-curated"));
    assert_eq!(event.script_path.as_deref(), Some("scripts/run.py"));
    wait_until_held(&mut pause_state).await;
    session
        .notify_approval(prompt_id, ReviewDecision::Approved)
        .await;
    request.await.expect("approval task");
    wait_until_released(&mut pause_state).await;
    assert_user_wait(&timing, timing_start, capture);
}

#[test_case::test_case(true; "capture_on")]
#[test_case::test_case(false; "capture_off")]
#[tokio::test]
async fn patch_approval_holds_an_elicitation_until_response(capture: bool) {
    let (session, mut turn_context, events) = make_session_and_context_with_rx().await;
    let (timing, timing_start) = start_timing(&mut turn_context, capture);
    *session.active_turn.lock().await = Some(ActiveTurn::default());
    let mut pause_state = session.subscribe_elicitation_pause_state();

    let request = tokio::spawn({
        let session = session.clone();
        let turn_context = turn_context.clone();
        async move {
            session
                .request_patch_approval(
                    turn_context.as_ref(),
                    "call-1".to_string(),
                    HashMap::new(),
                    /*reason*/ None,
                    /*grant_root*/ None,
                )
                .await
        }
    });

    events.recv().await.expect("approval event");
    wait_until_held(&mut pause_state).await;
    session
        .notify_approval("call-1", ReviewDecision::Approved)
        .await;
    request.await.expect("approval task");
    wait_until_released(&mut pause_state).await;
    assert_user_wait(&timing, timing_start, capture);
}

#[test_case::test_case(true; "capture_on")]
#[test_case::test_case(false; "capture_off")]
#[tokio::test]
async fn permission_request_holds_an_elicitation_until_response(capture: bool) {
    let (session, mut turn_context, events) = make_session_and_context_with_rx().await;
    let (timing, timing_start) = start_timing(&mut turn_context, capture);
    *session.active_turn.lock().await = Some(ActiveTurn::default());
    let mut pause_state = session.subscribe_elicitation_pause_state();

    let request = tokio::spawn({
        let session = session.clone();
        let turn_context = turn_context.clone();
        async move {
            let environment = turn_context
                .initial_environments
                .primary()
                .expect("primary environment")
                .selection();
            session
                .request_permissions_for_environment(
                    &StepContext::for_test(Arc::clone(&turn_context)),
                    "call-1".to_string(),
                    PreparedPermissionRequest {
                        environment_id: environment.environment_id,
                        reason: None,
                        permissions: RequestPermissionProfile::default(),
                    },
                    CancellationToken::new(),
                )
                .await
        }
    });

    events.recv().await.expect("permission request event");
    wait_until_held(&mut pause_state).await;
    session
        .notify_request_permissions_response(
            "call-1",
            RequestPermissionsResponse {
                permissions: RequestPermissionProfile::default(),
                scope: PermissionGrantScope::Turn,
                strict_auto_review: false,
            },
        )
        .await;
    request.await.expect("permission request task");
    wait_until_released(&mut pause_state).await;
    assert_user_wait(&timing, timing_start, capture);
}

#[test_case::test_case(true, true, false; "blocking")]
#[test_case::test_case(true, false, false; "nonblocking_ui_label")]
#[test_case::test_case(true, true, true; "cancelled")]
#[test_case::test_case(false, true, false; "capture_off")]
#[tokio::test]
async fn request_user_input_holds_an_elicitation_until_response(
    capture: bool,
    blocking: bool,
    cancel: bool,
) {
    let (session, mut turn_context, events) = make_session_and_context_with_rx().await;
    let (timing, timing_start) = start_timing(&mut turn_context, capture);
    *session.active_turn.lock().await = Some(ActiveTurn::default());
    let mut pause_state = session.subscribe_elicitation_pause_state();

    let request = tokio::spawn({
        let session = session.clone();
        let turn_context = turn_context.clone();
        async move {
            session
                .request_user_input(
                    turn_context.as_ref(),
                    "call-1".to_string(),
                    RequestUserInputArgs {
                        questions: Vec::new(),
                        is_blocking: blocking,
                        auto_resolution_ms: None,
                    },
                )
                .await
        }
    });

    events.recv().await.expect("request user input event");
    wait_until_held(&mut pause_state).await;

    if cancel {
        request.abort();
        assert!(matches!(request.await, Err(error) if error.is_cancelled()));
        wait_until_released(&mut pause_state).await;
        assert_user_wait(&timing, timing_start, capture);
        return;
    }
    let response = RequestUserInputResponse {
        answers: HashMap::new(),
    };
    session
        .notify_user_input_response(&turn_context.sub_id, response)
        .await;

    request.await.expect("request user input task");
    wait_until_released(&mut pause_state).await;
    assert_user_wait(&timing, timing_start, capture);
}

#[test_case::test_case(true; "enabled_unowned_wait_is_incomplete")]
#[test_case::test_case(false; "disabled_unowned_wait_is_unchanged")]
#[tokio::test]
async fn unowned_human_wait_does_not_claim_a_complete_path(capture: bool) {
    let (session, mut turn_context, _rx) = make_session_and_context_with_rx().await;
    let (_tool_timing, _) = start_timing(&mut turn_context, capture);
    turn_context.turn_timing_state.enable_critical_path();
    turn_context
        .turn_timing_state
        .mark_turn_started(Instant::now())
        .await;
    assert!(
        turn_context
            .turn_timing_state
            .snapshot_path(Instant::now())
            .is_some()
    );
    assert!(
        session
            .user_wait_timing_owner(&turn_context, Some("unknown-call"))
            .is_none()
    );
    assert!(
        session
            .start_user_wait_timing(&turn_context, /*owner*/ None)
            .is_none()
    );
    assert_eq!(
        turn_context
            .turn_timing_state
            .snapshot_path(Instant::now())
            .is_none(),
        capture
    );
}

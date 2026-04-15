#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_config::RequirementSource;
use codex_core::config::Constrained;
use codex_core::config::ConstraintError;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::request_permission_preset::PermissionPresetId;
use codex_protocol::request_permission_preset::RequestPermissionPresetDecision;
use codex_protocol::request_permission_preset::RequestPermissionPresetEvent;
use codex_protocol::request_permission_preset::RequestPermissionPresetResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

fn request_permission_preset_tool_event(
    call_id: &str,
    preset: PermissionPresetId,
    reason: &str,
) -> Result<Value> {
    let args = json!({
        "preset": preset,
        "reason": reason,
    });
    let args_str = serde_json::to_string(&args)?;
    Ok(ev_function_call(
        call_id,
        "request_permission_preset",
        &args_str,
    ))
}

fn workspace_write_without_network() -> SandboxPolicy {
    SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        read_only_access: Default::default(),
        network_access: false,
        exclude_tmpdir_env_var: false,
        exclude_slash_tmp: false,
    }
}

async fn submit_turn(
    test: &core_test_support::test_codex::TestCodex,
    prompt: &str,
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
) -> Result<()> {
    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: prompt.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd.path().to_path_buf(),
            approval_policy,
            approvals_reviewer: None,
            sandbox_policy,
            model: test.session_configured.model.clone(),
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;
    Ok(())
}

async fn expect_request_permission_preset_event(
    test: &core_test_support::test_codex::TestCodex,
    expected_call_id: &str,
) -> RequestPermissionPresetEvent {
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::RequestPermissionPreset(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;

    match event {
        EventMsg::RequestPermissionPreset(request) => {
            assert_eq!(request.call_id, expected_call_id);
            request
        }
        EventMsg::TurnComplete(_) => panic!("expected request_permission_preset before completion"),
        other => panic!("unexpected event: {other:?}"),
    }
}

fn invalid_approval_policy(candidate: AskForApproval) -> ConstraintError {
    ConstraintError::InvalidValue {
        field_name: "approval_policy",
        candidate: format!("{candidate:?}"),
        allowed: "[OnRequest]".to_string(),
        requirement_source: RequirementSource::Unknown,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn accepted_permission_preset_request_returns_model_output() -> Result<()> {
    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = workspace_write_without_network();
    let sandbox_policy_for_config = sandbox_policy.clone();
    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config
            .features
            .enable(Feature::RequestPermissionPresetTool)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-request-preset-1"),
                request_permission_preset_tool_event(
                    "preset-call",
                    PermissionPresetId::FullAccess,
                    "User asked for full access",
                )?,
                ev_completed("resp-request-preset-1"),
            ]),
            sse(vec![
                ev_response_created("resp-request-preset-2"),
                ev_assistant_message("msg-request-preset-1", "done"),
                ev_completed("resp-request-preset-2"),
            ]),
        ],
    )
    .await;

    submit_turn(
        &test,
        "switch to full access",
        approval_policy,
        sandbox_policy,
    )
    .await?;

    let request = expect_request_permission_preset_event(&test, "preset-call").await;
    assert_eq!(request.call_id, "preset-call");
    assert_eq!(request.preset, PermissionPresetId::FullAccess);

    test.codex
        .submit(Op::RequestPermissionPresetResponse {
            id: "preset-call".to_string(),
            response: RequestPermissionPresetResponse {
                decision: RequestPermissionPresetDecision::Accepted,
                preset: PermissionPresetId::FullAccess,
                message: "Permissions updated to Full access.".to_string(),
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let output_text = responses
        .function_call_output_text("preset-call")
        .expect("expected request_permission_preset output");
    let output: RequestPermissionPresetResponse = serde_json::from_str(&output_text)?;
    assert_eq!(
        output,
        RequestPermissionPresetResponse {
            decision: RequestPermissionPresetDecision::Accepted,
            preset: PermissionPresetId::FullAccess,
            message: "Permissions updated to Full access.".to_string(),
        }
    );

    let snapshot = test.codex.config_snapshot().await;
    assert_eq!(
        (
            snapshot.approval_policy,
            snapshot.approvals_reviewer,
            snapshot.sandbox_policy,
        ),
        (
            AskForApproval::Never,
            ApprovalsReviewer::User,
            SandboxPolicy::DangerFullAccess,
        )
    );

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn unknown_permission_preset_response_does_not_mutate_settings() -> Result<()> {
    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = workspace_write_without_network();
    let sandbox_policy_for_config = sandbox_policy.clone();
    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config
            .features
            .enable(Feature::RequestPermissionPresetTool)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-request-preset-1"),
                request_permission_preset_tool_event(
                    "preset-call",
                    PermissionPresetId::FullAccess,
                    "User asked for full access",
                )?,
                ev_completed("resp-request-preset-1"),
            ]),
            sse(vec![
                ev_response_created("resp-request-preset-2"),
                ev_assistant_message("msg-request-preset-1", "done"),
                ev_completed("resp-request-preset-2"),
            ]),
        ],
    )
    .await;

    submit_turn(
        &test,
        "switch to full access",
        approval_policy,
        sandbox_policy.clone(),
    )
    .await?;

    let request = expect_request_permission_preset_event(&test, "preset-call").await;
    assert_eq!(request.preset, PermissionPresetId::FullAccess);

    test.codex
        .submit(Op::RequestPermissionPresetResponse {
            id: "stale-call".to_string(),
            response: RequestPermissionPresetResponse {
                decision: RequestPermissionPresetDecision::Accepted,
                preset: PermissionPresetId::FullAccess,
                message: "stale response".to_string(),
            },
        })
        .await?;

    let snapshot = test.codex.config_snapshot().await;
    assert_eq!(
        (
            snapshot.approval_policy,
            snapshot.approvals_reviewer,
            snapshot.sandbox_policy,
        ),
        (
            AskForApproval::OnRequest,
            ApprovalsReviewer::User,
            sandbox_policy.clone(),
        )
    );

    test.codex
        .submit(Op::RequestPermissionPresetResponse {
            id: "preset-call".to_string(),
            response: RequestPermissionPresetResponse {
                decision: RequestPermissionPresetDecision::Accepted,
                preset: PermissionPresetId::FullAccess,
                message: "Permissions updated to Full access.".to_string(),
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let output_text = responses
        .function_call_output_text("preset-call")
        .expect("expected request_permission_preset output");
    let output: RequestPermissionPresetResponse = serde_json::from_str(&output_text)?;
    assert_eq!(
        output,
        RequestPermissionPresetResponse {
            decision: RequestPermissionPresetDecision::Accepted,
            preset: PermissionPresetId::FullAccess,
            message: "Permissions updated to Full access.".to_string(),
        }
    );

    let snapshot = test.codex.config_snapshot().await;
    assert_eq!(
        (
            snapshot.approval_policy,
            snapshot.approvals_reviewer,
            snapshot.sandbox_policy,
        ),
        (
            AskForApproval::Never,
            ApprovalsReviewer::User,
            SandboxPolicy::DangerFullAccess,
        )
    );

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn failed_permission_preset_apply_returns_declined_and_keeps_settings() -> Result<()> {
    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = workspace_write_without_network();
    let sandbox_policy_for_config = sandbox_policy.clone();
    let allow_full_access = Arc::new(AtomicBool::new(true));
    let allow_full_access_for_config = allow_full_access.clone();
    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::new(approval_policy, move |candidate| {
            if *candidate == AskForApproval::OnRequest
                || (*candidate == AskForApproval::Never
                    && allow_full_access_for_config.load(Ordering::SeqCst))
            {
                Ok(())
            } else {
                Err(invalid_approval_policy(*candidate))
            }
        })
        .expect("initial approval policy should satisfy the validator");
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config
            .features
            .enable(Feature::RequestPermissionPresetTool)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-request-preset-1"),
                request_permission_preset_tool_event(
                    "preset-call",
                    PermissionPresetId::FullAccess,
                    "User asked for full access",
                )?,
                ev_completed("resp-request-preset-1"),
            ]),
            sse(vec![
                ev_response_created("resp-request-preset-2"),
                ev_assistant_message("msg-request-preset-1", "done"),
                ev_completed("resp-request-preset-2"),
            ]),
        ],
    )
    .await;

    submit_turn(
        &test,
        "switch to full access",
        approval_policy,
        sandbox_policy.clone(),
    )
    .await?;

    let request = expect_request_permission_preset_event(&test, "preset-call").await;
    assert_eq!(request.preset, PermissionPresetId::FullAccess);

    allow_full_access.store(false, Ordering::SeqCst);

    test.codex
        .submit(Op::RequestPermissionPresetResponse {
            id: "preset-call".to_string(),
            response: RequestPermissionPresetResponse {
                decision: RequestPermissionPresetDecision::Accepted,
                preset: PermissionPresetId::FullAccess,
                message: "Permissions updated to Full access.".to_string(),
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let output_text = responses
        .function_call_output_text("preset-call")
        .expect("expected request_permission_preset output");
    let output: RequestPermissionPresetResponse = serde_json::from_str(&output_text)?;
    assert_eq!(output.decision, RequestPermissionPresetDecision::Declined);
    assert_eq!(output.preset, PermissionPresetId::FullAccess);
    assert!(
        output
            .message
            .contains("requested permission preset could not be applied"),
        "unexpected output message: {}",
        output.message
    );

    let snapshot = test.codex.config_snapshot().await;
    assert_eq!(
        (
            snapshot.approval_policy,
            snapshot.approvals_reviewer,
            snapshot.sandbox_policy,
        ),
        (
            AskForApproval::OnRequest,
            ApprovalsReviewer::User,
            sandbox_policy,
        )
    );

    Ok(())
}

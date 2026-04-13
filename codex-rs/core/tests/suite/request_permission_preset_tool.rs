#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_core::config::Constrained;
use codex_features::Feature;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::request_permission_preset::PermissionPresetId;
use codex_protocol::request_permission_preset::RequestPermissionPresetDecision;
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

#[tokio::test(flavor = "current_thread")]
async fn accepted_permission_preset_request_returns_model_output() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        config.permissions.sandbox_policy = Constrained::allow_any(SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            read_only_access: Default::default(),
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        });
        config
            .features
            .enable(Feature::RequestPermissionsTool)
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

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "switch to full access".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::OnRequest,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![],
                read_only_access: Default::default(),
                network_access: false,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            },
            model: test.session_configured.model.clone(),
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::RequestPermissionPreset(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    let EventMsg::RequestPermissionPreset(request) = event else {
        panic!("expected request_permission_preset before completion");
    };
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

    Ok(())
}

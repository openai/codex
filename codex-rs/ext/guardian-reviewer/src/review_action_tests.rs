#![cfg(test)]

//! Exercises approval action serialization and originating tool-call provenance.

use super::ReviewAction;
use crate::GuardianApprovalRequest;
use crate::guardian_approval_request_to_json;
use crate::guardian_assessment_action;
use crate::guardian_request_target_item_id;
use crate::guardian_request_turn_id;
use codex_protocol::approvals::GuardianAssessmentAction;
use codex_protocol::models::SandboxPermissions;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;

#[test_case::test_case(SandboxPermissions::UseDefault)]
#[test_case::test_case(SandboxPermissions::RequireEscalated)]
#[test_case::test_case(SandboxPermissions::WithAdditionalPermissions)]
fn guardian_write_stdin_preserves_original_command_input_and_foreign_cwd(
    sandbox_permissions: SandboxPermissions,
) -> serde_json::Result<()> {
    let cwd = PathUri::parse("file:///C:/workspace").expect("valid executor cwd");
    let input = "confirm\n";
    let additional_permissions =
        if sandbox_permissions == SandboxPermissions::WithAdditionalPermissions {
            Some(serde_json::from_value(
                serde_json::json!({"network":{"enabled":true}}),
            )?)
        } else {
            None
        };
    let action = GuardianApprovalRequest::WriteStdin {
        id: "terminal-open".to_string(),
        approval_id: "terminal-write".to_string(),
        environment_id: "windows-executor".to_string(),
        process_id: 1000,
        original_command: "python helper.py --project 'example site'".to_string(),
        input: input.to_string(),
        cwd: cwd.clone(),
        tty: true,
        sandbox_permissions,
        additional_permissions: additional_permissions.clone(),
    };

    let mut expected = serde_json::json!({
        "tool": "write_stdin",
        "environment_id": "windows-executor",
        "session_id": 1000,
        "original_command": "python helper.py --project 'example site'",
        "chars": input,
        "cwd": r"C:\workspace",
        "sandbox_permissions": sandbox_permissions,
        "tty": true,
    });
    if let Some(permissions) = additional_permissions {
        expected["additional_permissions"] = serde_json::to_value(permissions)?;
    }
    assert_eq!(guardian_approval_request_to_json(&action)?, expected);
    assert_eq!(
        guardian_assessment_action(&action).expect("reported approval action"),
        GuardianAssessmentAction::WriteStdin {
            approval_id: "terminal-write".to_string(),
            process_id: "1000".to_string(),
            stdin: input.to_string(),
            cwd,
        },
    );
    assert_eq!(
        (
            guardian_request_target_item_id(&action),
            guardian_request_turn_id(&action, "current-turn"),
        ),
        (Some("terminal-open"), "current-turn"),
    );
    assert_eq!(
        ReviewAction::from(action).tool_call_id.as_deref(),
        Some("terminal-write")
    );
    Ok(())
}

#![cfg(test)]

use super::*;
use crate::guardian_approval_request_to_json;
use pretty_assertions::assert_eq;

#[test]
fn guardian_apply_patch_preserves_foreign_paths_and_redacts_patch_text() -> serde_json::Result<()> {
    let cwd = PathUri::parse("file:///C:/workspace").expect("valid executor cwd");
    let file =
        PathUri::parse("file:///C:/workspace/guardian.txt").expect("valid executor file path");
    let patch = "*** Begin Patch\n*** Update File: guardian.txt\n@@\n+secret\n*** End Patch";
    let action = GuardianApprovalRequest::ApplyPatch {
        environment_id: "local".to_string(),
        id: "patch-1".to_string(),
        cwd,
        files: vec![file],
        patch: patch.to_string(),
    };

    assert_eq!(
        guardian_approval_request_to_json(&action)?,
        serde_json::json!({
            "tool": "apply_patch",
            "environment_id": "local",
            "cwd": r"C:\workspace",
            "files": [r"C:\workspace\guardian.txt"],
            "patch": patch,
        }),
    );
    assert_eq!(
        serde_json::to_value(guardian_assessment_action(&action))?,
        serde_json::json!({
            "type": "apply_patch",
            "cwd": r"C:\workspace",
            "files": [r"C:\workspace\guardian.txt"],
        }),
    );
    Ok(())
}

#[test]
fn guardian_request_turn_id_prefers_network_access_owner_turn() {
    let network_access = GuardianApprovalRequest::NetworkAccess {
        id: "network-1".to_string(),
        turn_id: "owner-turn".to_string(),
        environment_id: "local".to_string(),
        target: "https://example.com:443".to_string(),
        host: "example.com".to_string(),
        protocol: NetworkApprovalProtocol::Https,
        port: 443,
        trigger: None,
    };
    let apply_patch = GuardianApprovalRequest::ApplyPatch {
        environment_id: "local".to_string(),
        id: "patch-1".to_string(),
        cwd: PathUri::parse("file:///tmp").expect("valid cwd"),
        files: vec![PathUri::parse("file:///tmp/guardian.txt").expect("valid file path")],
        patch: "*** Begin Patch\n*** Update File: guardian.txt\n@@\n+hello\n*** End Patch"
            .to_string(),
    };

    assert_eq!(
        guardian_request_turn_id(&network_access, "fallback-turn"),
        "owner-turn"
    );
    assert_eq!(
        guardian_request_turn_id(&apply_patch, "fallback-turn"),
        "fallback-turn"
    );
}

#[test]
fn guardian_request_target_item_id_omits_network_access_trigger_call_id() {
    let network_access = GuardianApprovalRequest::NetworkAccess {
        id: "network-1".to_string(),
        turn_id: "owner-turn".to_string(),
        environment_id: "local".to_string(),
        target: "https://example.com:443".to_string(),
        host: "example.com".to_string(),
        protocol: NetworkApprovalProtocol::Https,
        port: 443,
        trigger: Some(GuardianNetworkAccessTrigger {
            call_id: "call-1".to_string(),
            tool_name: "shell".to_string(),
            command: vec!["curl".to_string(), "https://example.com".to_string()],
            cwd: PathUri::parse("file:///repo").expect("valid cwd"),
            sandbox_permissions: SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: None,
            tty: None,
        }),
    };

    assert_eq!(guardian_request_target_item_id(&network_access), None);
}

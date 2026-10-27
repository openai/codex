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

use super::*;

use anyhow::Result;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::PathBuf;

#[test]
fn extract_conversation_summary_prefers_plain_user_messages() -> Result<()> {
    let conversation_id = ThreadId::from_string("3f941c35-29b3-493b-b0a4-e25800d9aeb0")?;
    let timestamp = Some("2025-09-05T16:53:11.850Z".to_string());
    let path = PathBuf::from("rollout.jsonl");

    let head = vec![
        json!({
            "id": conversation_id.to_string(),
            "timestamp": timestamp,
            "cwd": "/",
            "originator": "codex",
            "cli_version": "0.0.0",
            "model_provider": "test-provider"
        }),
        json!({
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "# AGENTS.md instructions for project\n\n<INSTRUCTIONS>\n<AGENTS.md contents>\n</INSTRUCTIONS>".to_string(),
            }],
        }),
        json!({
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": format!("<prior context> {USER_MESSAGE_BEGIN}Count to 5"),
            }],
        }),
    ];

    let session_meta = serde_json::from_value::<SessionMeta>(head[0].clone())?;

    let summary = extract_conversation_summary(
        path.clone(),
        &head,
        &session_meta,
        /*git*/ None,
        "test-provider",
        timestamp.clone(),
    )
    .expect("summary");

    let expected = ConversationSummary {
        conversation_id,
        timestamp: timestamp.clone(),
        updated_at: timestamp,
        path,
        preview: "Count to 5".to_string(),
        model_provider: "test-provider".to_string(),
        cwd: PathBuf::from("/"),
        cli_version: "0.0.0".to_string(),
        source: codex_protocol::protocol::SessionSource::VSCode,
        git_info: None,
    };

    assert_eq!(summary, expected);
    Ok(())
}

#[test]
fn legacy_permission_profile_modifications_extend_runtime_roots() -> Result<()> {
    let root = if cfg!(windows) {
        AbsolutePathBuf::try_from("C:\\workspace-extra")?
    } else {
        AbsolutePathBuf::try_from("/workspace-extra")?
    };
    let selection = serde_json::from_value::<PermissionProfileSelectionParams>(json!({
        "type": "profile",
        "id": ":workspace",
        "modifications": [
            {
                "type": "additionalWritableRoot",
                "path": root,
            }
        ],
    }))?;

    let mut overrides = ConfigOverrides::default();
    apply_permission_profile_selection_to_config_overrides(&mut overrides, Some(selection.clone()));
    assert_eq!(
        overrides.default_permissions,
        Some(":workspace".to_string())
    );
    assert_eq!(
        overrides.additional_writable_roots,
        vec![root.to_path_buf()]
    );

    let mut overrides = ConfigOverrides {
        workspace_roots: Some(Vec::new()),
        ..ConfigOverrides::default()
    };
    apply_permission_profile_selection_to_config_overrides(&mut overrides, Some(selection));
    assert_eq!(overrides.additional_writable_roots, Vec::<PathBuf>::new());
    assert_eq!(overrides.workspace_roots, Some(vec![root.to_path_buf()]));

    Ok(())
}

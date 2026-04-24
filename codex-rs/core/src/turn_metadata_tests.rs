use super::*;

use crate::sandbox_tags::sandbox_tag;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use core_test_support::PathBufExt;
use core_test_support::PathExt;
use serde_json::Value;
use std::collections::HashMap;
use tempfile::TempDir;
use tokio::process::Command;

#[tokio::test]
async fn build_turn_metadata_header_includes_has_changes_for_clean_repo() {
    let temp_dir = TempDir::new().expect("temp dir");
    let repo_path = temp_dir.path().join("repo").abs();
    std::fs::create_dir_all(&repo_path).expect("create repo");

    Command::new("git")
        .args(["init"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("git init");
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("git config user.name");
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("git config user.email");

    std::fs::write(repo_path.join("README.md"), "hello").expect("write file");
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("git add");
    Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("git commit");

    let header = build_turn_metadata_header(&repo_path, Some("none"))
        .await
        .expect("header");
    let parsed: Value = serde_json::from_str(&header).expect("valid json");
    let workspace = parsed
        .get("workspaces")
        .and_then(Value::as_object)
        .and_then(|workspaces| workspaces.values().next())
        .cloned()
        .expect("workspace");

    assert_eq!(
        workspace.get("has_changes").and_then(Value::as_bool),
        Some(false)
    );
}

#[test]
fn turn_metadata_state_uses_platform_sandbox_tag() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let permission_profile = PermissionProfile::from_legacy_sandbox_policy(&sandbox_policy);

    let state = TurnMetadataState::new(
        "session-a".to_string(),
        &SessionSource::Exec,
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
    );

    let header = state.current_header_value().expect("header");
    let json: Value = serde_json::from_str(&header).expect("json");
    let sandbox_name = json.get("sandbox").and_then(Value::as_str);
    let session_id = json.get("session_id").and_then(Value::as_str);
    let thread_source = json.get("thread_source").and_then(Value::as_str);

    let expected_sandbox = sandbox_tag(&sandbox_policy, WindowsSandboxLevel::Disabled);
    assert_eq!(sandbox_name, Some(expected_sandbox));
    assert_eq!(session_id, Some("session-a"));
    assert_eq!(thread_source, Some("user"));
    assert!(json.get("session_source").is_none());
}

#[test]
fn turn_metadata_state_classifies_subagent_thread_source() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile =
        PermissionProfile::from_legacy_sandbox_policy(&SandboxPolicy::new_read_only_policy());
    let session_source = SessionSource::SubAgent(SubAgentSource::Review);

    let state = TurnMetadataState::new(
        "session-a".to_string(),
        &session_source,
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
    );

    let header = state.current_header_value().expect("header");
    let json: Value = serde_json::from_str(&header).expect("json");

    assert_eq!(json["thread_source"].as_str(), Some("subagent"));
    assert!(json.get("session_source").is_none());
}

#[test]
fn turn_metadata_state_merges_client_metadata_without_replacing_reserved_fields() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile =
        PermissionProfile::from_legacy_sandbox_policy(&SandboxPolicy::new_read_only_policy());

    let state = TurnMetadataState::new(
        "session-a".to_string(),
        &SessionSource::Exec,
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
    );
    state.set_responsesapi_client_metadata(HashMap::from([
        ("fiber_run_id".to_string(), "fiber-123".to_string()),
        ("session_id".to_string(), "client-supplied".to_string()),
        ("thread_source".to_string(), "client-supplied".to_string()),
    ]));

    let header = state.current_header_value().expect("header");
    let json: Value = serde_json::from_str(&header).expect("json");

    assert_eq!(json["fiber_run_id"].as_str(), Some("fiber-123"));
    assert_eq!(json["session_id"].as_str(), Some("session-a"));
    assert_eq!(json["thread_source"].as_str(), Some("user"));
    assert_eq!(json["turn_id"].as_str(), Some("turn-a"));
}

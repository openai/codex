use super::*;

use crate::sandbox_tags::permission_profile_sandbox_tag;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::PathBufExt;
use core_test_support::PathExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tempfile::TempDir;
use tokio::process::Command;

fn test_mcp_turn_metadata_context() -> McpTurnMetadataContext<'static> {
    McpTurnMetadataContext {
        model: "gpt-5.4",
        reasoning_effort: Some(ReasoningEffortConfig::High),
    }
}

async fn create_clean_git_repo(repo_name: &str) -> (TempDir, AbsolutePathBuf) {
    let temp_dir = TempDir::new().expect("temp dir");
    let repo_path = temp_dir.path().join(repo_name).abs();
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

    (temp_dir, repo_path)
}

#[tokio::test]
async fn build_turn_metadata_header_marks_detached_memory_without_turn_identity() {
    let (_temp_dir, repo_path) = create_clean_git_repo("repo-東京").await;

    let header = build_turn_metadata_header(&repo_path, Some("none"))
        .await
        .expect("header");
    assert!(header.is_ascii());
    assert!(!header.contains("東京"));
    let parsed: Value = serde_json::from_str(&header).expect("valid json");
    assert_eq!(parsed["request_kind"].as_str(), Some("memory"));
    assert!(parsed.get("session_id").is_none());
    assert!(parsed.get("thread_id").is_none());
    assert!(parsed.get("forked_from_thread_id").is_none());
    assert!(parsed.get("turn_id").is_none());
    assert!(parsed.get(WINDOW_ID_KEY).is_none());

    let expected_repo_path = repo_path.to_string_lossy().into_owned();
    let actual_repo_path = parsed
        .get("workspaces")
        .and_then(Value::as_object)
        .and_then(|workspaces| workspaces.keys().next())
        .expect("workspace path");
    assert_eq!(actual_repo_path, &expected_repo_path);
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

#[tokio::test]
async fn build_turn_metadata_header_marks_memory_without_workspace_metadata() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();

    let header = build_turn_metadata_header(&cwd, /*sandbox*/ None)
        .await
        .expect("detached memory should emit its request kind");
    let parsed: Value = serde_json::from_str(&header).expect("valid json");

    assert_eq!(parsed, serde_json::json!({"request_kind": "memory"}));
}

#[test]
fn turn_metadata_state_uses_platform_sandbox_tag() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile = PermissionProfile::read_only();

    let state = TurnMetadataState::new(
        "session-a".to_string(),
        "thread-a".to_string(),
        /*forked_from_thread_id*/ None,
        /*parent_thread_id*/ None,
        &SessionSource::Exec,
        Some(ThreadSource::User),
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );

    let header = state.current_header_value().expect("header");
    let json: Value = serde_json::from_str(&header).expect("json");
    let sandbox_name = json.get("sandbox").and_then(Value::as_str);
    let session_id = json.get("session_id").and_then(Value::as_str);
    let thread_id = json.get("thread_id").and_then(Value::as_str);
    let thread_source = json.get("thread_source").and_then(Value::as_str);

    assert!(json.get("request_kind").is_none());
    let expected_sandbox = permission_profile_sandbox_tag(
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );
    assert_eq!(sandbox_name, Some(expected_sandbox));
    assert_eq!(session_id, Some("session-a"));
    assert_eq!(thread_id, Some("thread-a"));
    assert_eq!(thread_source, Some("user"));
    assert!(json.get("forked_from_thread_id").is_none());
    assert!(json.get("parent_thread_id").is_none());
    assert!(json.get("subagent_kind").is_none());
    assert!(json.get("session_source").is_none());
}

#[test]
fn turn_metadata_state_uses_explicit_subagent_thread_source() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile = PermissionProfile::read_only();
    let state = TurnMetadataState::new(
        "session-a".to_string(),
        "thread-a".to_string(),
        /*forked_from_thread_id*/ None,
        /*parent_thread_id*/ None,
        &SessionSource::Exec,
        Some(ThreadSource::Subagent),
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );

    let header = state.current_header_value().expect("header");
    let json: Value = serde_json::from_str(&header).expect("json");

    assert_eq!(json["thread_source"].as_str(), Some("subagent"));
    assert!(json.get("session_source").is_none());
}

#[test]
fn turn_metadata_state_includes_root_fork_lineage() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile = PermissionProfile::read_only();
    let source_thread_id =
        ThreadId::from_string("11111111-1111-4111-8111-111111111111").expect("thread id");

    let state = TurnMetadataState::new(
        "session-a".to_string(),
        "thread-a".to_string(),
        Some(source_thread_id),
        /*parent_thread_id*/ None,
        &SessionSource::Exec,
        Some(ThreadSource::User),
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );

    let header = state.current_header_value().expect("header");
    let json: Value = serde_json::from_str(&header).expect("json");

    assert_eq!(
        json["forked_from_thread_id"].as_str(),
        Some("11111111-1111-4111-8111-111111111111")
    );
    assert!(json.get("parent_thread_id").is_none());
    assert!(json.get("subagent_kind").is_none());
}

#[test]
fn turn_metadata_state_includes_thread_spawn_subagent_parent_without_fork() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile = PermissionProfile::read_only();
    let parent_thread_id =
        ThreadId::from_string("22222222-2222-4222-8222-222222222222").expect("thread id");

    let state = TurnMetadataState::new(
        "session-a".to_string(),
        "thread-a".to_string(),
        /*forked_from_thread_id*/ None,
        Some(parent_thread_id),
        &SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            depth: 1,
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
        }),
        Some(ThreadSource::Subagent),
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );

    let header = state.current_header_value().expect("header");
    let json: Value = serde_json::from_str(&header).expect("json");

    assert!(json.get("forked_from_thread_id").is_none());
    assert_eq!(
        json["parent_thread_id"].as_str(),
        Some("22222222-2222-4222-8222-222222222222")
    );
    assert_eq!(json["subagent_kind"].as_str(), Some("thread_spawn"));
}

#[test]
fn turn_metadata_state_includes_forked_thread_spawn_subagent_lineage() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile = PermissionProfile::read_only();
    let parent_thread_id =
        ThreadId::from_string("33333333-3333-4333-8333-333333333333").expect("thread id");

    let state = TurnMetadataState::new(
        "session-a".to_string(),
        "thread-a".to_string(),
        Some(parent_thread_id),
        Some(parent_thread_id),
        &SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            depth: 1,
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
        }),
        Some(ThreadSource::Subagent),
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );

    let header = state.current_header_value().expect("header");
    let json: Value = serde_json::from_str(&header).expect("json");

    assert_eq!(
        json["forked_from_thread_id"].as_str(),
        Some("33333333-3333-4333-8333-333333333333")
    );
    assert_eq!(
        json["parent_thread_id"].as_str(),
        Some("33333333-3333-4333-8333-333333333333")
    );
    assert_eq!(json["subagent_kind"].as_str(), Some("thread_spawn"));
}

#[test]
fn turn_metadata_state_includes_known_parent_for_non_thread_spawn_subagents_without_fork() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile = PermissionProfile::read_only();
    let parent_thread_id =
        ThreadId::from_string("44444444-4444-4444-8444-444444444444").expect("thread id");
    let sources = [
        (SubAgentSource::Review, "review"),
        (SubAgentSource::Other("guardian".to_string()), "guardian"),
        (
            SubAgentSource::Other("custom_worker:job-1".to_string()),
            "custom_worker:job-1",
        ),
    ];

    for (subagent_source, subagent_kind) in sources {
        let state = TurnMetadataState::new(
            "session-a".to_string(),
            "thread-a".to_string(),
            /*forked_from_thread_id*/ None,
            Some(parent_thread_id),
            &SessionSource::SubAgent(subagent_source),
            Some(ThreadSource::Subagent),
            "turn-a".to_string(),
            cwd.clone(),
            &permission_profile,
            WindowsSandboxLevel::Disabled,
            /*enforce_managed_network*/ false,
        );

        let header = state.current_header_value().expect("header");
        let json: Value = serde_json::from_str(&header).expect("json");

        assert!(json.get("forked_from_thread_id").is_none());
        assert_eq!(
            json["parent_thread_id"].as_str(),
            Some("44444444-4444-4444-8444-444444444444")
        );
        assert_eq!(json["subagent_kind"].as_str(), Some(subagent_kind));
    }
}

#[test]
fn turn_metadata_state_includes_turn_started_at_unix_ms_after_start() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile = PermissionProfile::read_only();

    let state = TurnMetadataState::new(
        "session-a".to_string(),
        "thread-a".to_string(),
        /*forked_from_thread_id*/ None,
        /*parent_thread_id*/ None,
        &SessionSource::Exec,
        Some(ThreadSource::User),
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );
    state.set_turn_started_at_unix_ms(/*turn_started_at_unix_ms*/ 1_700_000_000_123);

    let header = state.current_header_value().expect("header");
    let json: Value = serde_json::from_str(&header).expect("json");

    assert_eq!(
        json["turn_started_at_unix_ms"].as_i64(),
        Some(1_700_000_000_123)
    );
}

#[test]
fn turn_metadata_state_includes_model_and_reasoning_effort_only_in_request_meta() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile = PermissionProfile::read_only();

    let state = TurnMetadataState::new(
        "session-a".to_string(),
        "thread-a".to_string(),
        /*forked_from_thread_id*/ None,
        /*parent_thread_id*/ None,
        &SessionSource::Exec,
        /*thread_source*/ None,
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );

    let header = state.current_header_value().expect("header");
    let header_json: Value = serde_json::from_str(&header).expect("json");
    assert!(header_json.get("model").is_none());
    assert!(header_json.get("reasoning_effort").is_none());

    let meta = state
        .current_meta_value_for_mcp_request(test_mcp_turn_metadata_context())
        .expect("turn metadata should be present");
    assert!(meta.get("request_kind").is_none());
    assert_eq!(meta["model"].as_str(), Some("gpt-5.4"));
    assert_eq!(meta["reasoning_effort"].as_str(), Some("high"));

    let meta_without_reasoning_effort = state
        .current_meta_value_for_mcp_request(McpTurnMetadataContext {
            model: "gpt-5.4",
            reasoning_effort: None,
        })
        .expect("turn metadata should be present");
    assert_eq!(
        meta_without_reasoning_effort["model"].as_str(),
        Some("gpt-5.4")
    );
    assert!(
        meta_without_reasoning_effort
            .get("reasoning_effort")
            .is_none()
    );
}

#[test]
fn turn_metadata_state_marks_user_input_requested_during_turn_only_for_mcp_request_meta() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile = PermissionProfile::read_only();

    let state = TurnMetadataState::new(
        "session-a".to_string(),
        "thread-a".to_string(),
        /*forked_from_thread_id*/ None,
        /*parent_thread_id*/ None,
        &SessionSource::Exec,
        /*thread_source*/ None,
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );

    let header = state.current_header_value().expect("header");
    let header_json: Value = serde_json::from_str(&header).expect("json");
    assert!(
        header_json
            .get(USER_INPUT_REQUESTED_DURING_TURN_KEY)
            .is_none()
    );

    let meta = state
        .current_meta_value_for_mcp_request(test_mcp_turn_metadata_context())
        .expect("turn metadata should be present");
    assert!(meta.get(USER_INPUT_REQUESTED_DURING_TURN_KEY).is_none());

    state.mark_user_input_requested_during_turn();

    let header = state.current_header_value().expect("header");
    let header_json: Value = serde_json::from_str(&header).expect("json");
    assert!(
        header_json
            .get(USER_INPUT_REQUESTED_DURING_TURN_KEY)
            .is_none()
    );

    let meta = state
        .current_meta_value_for_mcp_request(test_mcp_turn_metadata_context())
        .expect("turn metadata should be present");
    assert_eq!(
        meta.get(USER_INPUT_REQUESTED_DURING_TURN_KEY)
            .and_then(Value::as_bool),
        Some(true)
    );
}

#[test]
fn turn_metadata_state_ignores_client_reserved_metadata_before_start() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile = PermissionProfile::read_only();

    let state = TurnMetadataState::new(
        "session-a".to_string(),
        "thread-a".to_string(),
        /*forked_from_thread_id*/ None,
        /*parent_thread_id*/ None,
        &SessionSource::Exec,
        Some(ThreadSource::User),
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );
    state.set_responsesapi_client_metadata(HashMap::from([
        (
            "turn_started_at_unix_ms".to_string(),
            "client-supplied".to_string(),
        ),
        (
            "forked_from_thread_id".to_string(),
            "client-supplied".to_string(),
        ),
        (
            "parent_thread_id".to_string(),
            "client-supplied".to_string(),
        ),
        ("subagent_kind".to_string(), "client-supplied".to_string()),
    ]));

    let header = state.current_header_value().expect("header");
    let json: Value = serde_json::from_str(&header).expect("json");

    assert!(json.get("turn_started_at_unix_ms").is_none());
    assert!(json.get("forked_from_thread_id").is_none());
    assert!(json.get("parent_thread_id").is_none());
    assert!(json.get("subagent_kind").is_none());
}

#[test]
fn turn_metadata_state_merges_client_metadata_without_replacing_reserved_fields() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile = PermissionProfile::read_only();
    let source_thread_id =
        ThreadId::from_string("44444444-4444-4444-8444-444444444444").expect("thread id");
    let parent_thread_id =
        ThreadId::from_string("55555555-5555-4555-8555-555555555555").expect("thread id");

    let state = TurnMetadataState::new(
        "session-a".to_string(),
        "thread-a".to_string(),
        Some(source_thread_id),
        Some(parent_thread_id),
        &SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            depth: 1,
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
        }),
        Some(ThreadSource::User),
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );
    state.set_responsesapi_client_metadata(HashMap::from([
        ("fiber_run_id".to_string(), "fiber-123".to_string()),
        ("origin".to_string(), "東京".to_string()),
        ("workspace_kind".to_string(), "projectless".to_string()),
        ("model".to_string(), "client-supplied".to_string()),
        (
            "reasoning_effort".to_string(),
            "client-supplied".to_string(),
        ),
        ("session_id".to_string(), "client-supplied".to_string()),
        ("thread_id".to_string(), "client-supplied".to_string()),
        (
            "forked_from_thread_id".to_string(),
            "client-supplied".to_string(),
        ),
        (
            "parent_thread_id".to_string(),
            "client-supplied".to_string(),
        ),
        ("subagent_kind".to_string(), "client-supplied".to_string()),
        ("turn_id".to_string(), "client-supplied".to_string()),
        (WINDOW_ID_KEY.to_string(), "client-supplied".to_string()),
        ("thread_source".to_string(), "client-supplied".to_string()),
        ("request_kind".to_string(), "client-supplied".to_string()),
        (
            "turn_started_at_unix_ms".to_string(),
            "client-supplied".to_string(),
        ),
    ]));
    state.set_turn_started_at_unix_ms(/*turn_started_at_unix_ms*/ 1_700_000_000_123);

    let header = state.current_header_value().expect("header");
    assert!(header.is_ascii());
    assert!(!header.contains("東京"));
    let json: Value = serde_json::from_str(&header).expect("json");

    assert_eq!(json["fiber_run_id"].as_str(), Some("fiber-123"));
    assert_eq!(json["origin"].as_str(), Some("東京"));
    assert_eq!(json["workspace_kind"].as_str(), Some("projectless"));
    assert_eq!(json["model"].as_str(), Some("client-supplied"));
    assert_eq!(json["reasoning_effort"].as_str(), Some("client-supplied"));
    assert_eq!(json["session_id"].as_str(), Some("session-a"));
    assert_eq!(json["thread_id"].as_str(), Some("thread-a"));
    assert_eq!(
        json["forked_from_thread_id"].as_str(),
        Some("44444444-4444-4444-8444-444444444444")
    );
    assert_eq!(
        json["parent_thread_id"].as_str(),
        Some("55555555-5555-4555-8555-555555555555")
    );
    assert_eq!(json["subagent_kind"].as_str(), Some("thread_spawn"));
    assert_eq!(json["thread_source"].as_str(), Some("user"));
    assert_eq!(json["turn_id"].as_str(), Some("turn-a"));
    assert!(json.get("request_kind").is_none());
    assert!(json.get(WINDOW_ID_KEY).is_none());
    assert_eq!(
        json["turn_started_at_unix_ms"].as_i64(),
        Some(1_700_000_000_123)
    );

    let model_request_header = state
        .current_header_value_for_model_request("thread-a:1")
        .expect("model request header");
    let model_request_json: Value =
        serde_json::from_str(&model_request_header).expect("model request json");
    assert_eq!(model_request_json["request_kind"].as_str(), Some("turn"));
    assert_eq!(
        model_request_json[WINDOW_ID_KEY].as_str(),
        Some("thread-a:1")
    );

    let meta = state
        .current_meta_value_for_mcp_request(test_mcp_turn_metadata_context())
        .expect("turn metadata should be present");
    assert_eq!(meta["model"].as_str(), Some("gpt-5.4"));
    assert_eq!(meta["reasoning_effort"].as_str(), Some("high"));
    assert!(meta.get(WINDOW_ID_KEY).is_none());
    assert_eq!(state.workspace_kind().as_deref(), Some("projectless"));
}

#[test]
fn turn_metadata_state_overlays_compaction_only_on_compaction_requests() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile = PermissionProfile::read_only();
    let state = TurnMetadataState::new(
        "session-a".to_string(),
        "thread-a".to_string(),
        /*forked_from_thread_id*/ None,
        /*parent_thread_id*/ None,
        &SessionSource::Exec,
        Some(ThreadSource::User),
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );
    state.set_responsesapi_client_metadata(HashMap::from([(
        "compaction".to_string(),
        "client-supplied".to_string(),
    )]));

    let compact_header = state
        .current_header_value_for_compaction(
            "thread-a:2",
            CompactionTurnMetadata::new(
                CompactionTrigger::Auto,
                CompactionReason::ContextLimit,
                CompactionImplementation::ResponsesCompactionV2,
                CompactionPhase::MidTurn,
            ),
        )
        .expect("compact header");
    let compact_json: Value = serde_json::from_str(&compact_header).expect("json");
    assert_eq!(compact_json["request_kind"].as_str(), Some("compaction"));
    assert_eq!(compact_json["turn_id"].as_str(), Some("turn-a"));
    assert_eq!(compact_json[WINDOW_ID_KEY].as_str(), Some("thread-a:2"));
    assert_eq!(
        compact_json["compaction"],
        serde_json::json!({
            "trigger": "auto",
            "reason": "context_limit",
            "implementation": "responses_compaction_v2",
            "phase": "mid_turn",
            "strategy": "memento",
        })
    );

    let regular_header = state
        .current_header_value_for_model_request("thread-a:3")
        .expect("regular header");
    let regular_json: Value = serde_json::from_str(&regular_header).expect("json");
    assert_eq!(regular_json["request_kind"].as_str(), Some("turn"));
    assert_eq!(regular_json[WINDOW_ID_KEY].as_str(), Some("thread-a:3"));
    assert!(regular_json.get("compaction").is_none());
}

#[tokio::test]
async fn turn_metadata_state_preserves_lineage_after_git_enrichment() {
    let (_temp_dir, repo_path) = create_clean_git_repo("repo").await;

    let permission_profile = PermissionProfile::read_only();
    let parent_thread_id =
        ThreadId::from_string("66666666-6666-4666-8666-666666666666").expect("thread id");
    let state = TurnMetadataState::new(
        "session-a".to_string(),
        "thread-a".to_string(),
        Some(parent_thread_id),
        Some(parent_thread_id),
        &SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            depth: 1,
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
        }),
        Some(ThreadSource::Subagent),
        "turn-a".to_string(),
        repo_path,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );

    state.spawn_git_enrichment_task();

    let json = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let header = state.current_header_value().expect("header");
            let json: Value = serde_json::from_str(&header).expect("json");
            if json
                .get("workspaces")
                .and_then(Value::as_object)
                .is_some_and(|workspaces| !workspaces.is_empty())
            {
                return json;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("git enrichment should complete");

    assert_eq!(
        json["forked_from_thread_id"].as_str(),
        Some("66666666-6666-4666-8666-666666666666")
    );
    assert_eq!(
        json["parent_thread_id"].as_str(),
        Some("66666666-6666-4666-8666-666666666666")
    );
    assert_eq!(json["subagent_kind"].as_str(), Some("thread_spawn"));
}

//! Skill invocation events and privacy rules for local skill identifiers.

use crate::facts::AnalyticsFact;
use crate::facts::CustomAnalyticsFact;
use crate::facts::InvocationType;
use crate::facts::SkillInvocation;
use crate::facts::SkillInvocationLocation;
use crate::facts::SkillInvokedInput;
use crate::reducer::AnalyticsReducer;
use crate::reducer::normalize_path_for_skill_id;
use crate::reducer::skill_id_for_local_skill;
use crate::tests::support::TEST_PRODUCT_CLIENT_ID;
use crate::tests::support::test_tracking_context;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::PathBuf;

fn expected_absolute_path(path: &PathBuf) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
}

#[test]
fn normalize_path_for_skill_id_repo_scoped_uses_relative_path() {
    let repo_root = PathBuf::from("/repo/root");
    let skill_path = PathBuf::from("/repo/root/.codex/skills/doc/SKILL.md");

    let path = normalize_path_for_skill_id(
        Some("https://example.com/repo.git"),
        Some(repo_root.as_path()),
        skill_path.as_path(),
    );

    assert_eq!(path, ".codex/skills/doc/SKILL.md");
}

#[test]
fn normalize_path_for_skill_id_user_scoped_uses_absolute_path() {
    let skill_path = PathBuf::from("/Users/abc/.codex/skills/doc/SKILL.md");

    let path = normalize_path_for_skill_id(
        /*repo_url*/ None,
        /*repo_root*/ None,
        skill_path.as_path(),
    );
    let expected = expected_absolute_path(&skill_path);

    assert_eq!(path, expected);
}

#[test]
fn normalize_path_for_skill_id_admin_scoped_uses_absolute_path() {
    let skill_path = PathBuf::from("/etc/codex/skills/doc/SKILL.md");

    let path = normalize_path_for_skill_id(
        /*repo_url*/ None,
        /*repo_root*/ None,
        skill_path.as_path(),
    );
    let expected = expected_absolute_path(&skill_path);

    assert_eq!(path, expected);
}

#[test]
fn normalize_path_for_skill_id_repo_root_not_in_skill_path_uses_absolute_path() {
    let repo_root = PathBuf::from("/repo/root");
    let skill_path = PathBuf::from("/other/path/.codex/skills/doc/SKILL.md");

    let path = normalize_path_for_skill_id(
        Some("https://example.com/repo.git"),
        Some(repo_root.as_path()),
        skill_path.as_path(),
    );
    let expected = expected_absolute_path(&skill_path);

    assert_eq!(path, expected);
}

#[tokio::test]
async fn reducer_ingests_skill_invoked_fact() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    let tracking = test_tracking_context("thread-1", "turn-1");
    let skill_path = PathBuf::from("/Users/abc/.codex/skills/doc/SKILL.md");
    let expected_skill_id = skill_id_for_local_skill(
        /*repo_url*/ None,
        /*repo_root*/ None,
        skill_path.as_path(),
        "doc",
    );

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::SkillInvoked(SkillInvokedInput {
                tracking,
                invocations: vec![SkillInvocation {
                    skill_name: "doc".to_string(),
                    location: SkillInvocationLocation::Host {
                        path: skill_path,
                        scope: codex_protocol::protocol::SkillScope::User,
                    },
                    plugin_id: None,
                    remote_plugin_id: None,
                    invocation_type: InvocationType::Explicit,
                }],
            })),
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(
        payload,
        json!([{
            "event_type": "skill_invocation",
            "skill_id": expected_skill_id,
            "skill_name": "doc",
            "event_params": {
                "product_client_id": TEST_PRODUCT_CLIENT_ID,
                "skill_scope": "user",
                "plugin_id": null,
                "remote_plugin_id": null,
                "thread_id": "thread-1",
                "turn_id": "turn-1",
                "voice_session_id": null,
                "invoke_type": "explicit",
                "model_slug": "gpt-5"
            }
        }])
    );
}

#[tokio::test]
async fn reducer_includes_plugin_ids_for_plugin_skill_invocations() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    let tracking = test_tracking_context("thread-1", "turn-1");
    let skill_path =
        PathBuf::from("/Users/abc/.codex/plugins/cache/test/sample/skills/doc/SKILL.md");

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::SkillInvoked(SkillInvokedInput {
                tracking,
                invocations: vec![SkillInvocation {
                    skill_name: "sample:doc".to_string(),
                    location: SkillInvocationLocation::Host {
                        path: skill_path,
                        scope: codex_protocol::protocol::SkillScope::User,
                    },
                    plugin_id: Some("sample@test".to_string()),
                    remote_plugin_id: Some("plugins~Plugin_sample".to_string()),
                    invocation_type: InvocationType::Explicit,
                }],
            })),
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(
        (
            &payload[0]["event_params"]["plugin_id"],
            &payload[0]["event_params"]["remote_plugin_id"],
        ),
        (&json!("sample@test"), &json!("plugins~Plugin_sample"))
    );
}

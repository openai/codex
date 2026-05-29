use super::SkillLoadOutcome;
use super::SkillMetadata;
use super::canonicalize_if_exists;
use super::detect_skill_doc_read;
use super::detect_skill_script_run;
use super::script_run_token;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::sync::Arc;

fn test_skill_metadata(skill_doc_path: AbsolutePathBuf) -> SkillMetadata {
    SkillMetadata {
        name: "test-skill".to_string(),
        description: "test".to_string(),
        short_description: None,
        interface: None,
        dependencies: None,
        policy: None,
        source_path: codex_exec_server::EnvironmentPathRef::local((skill_doc_path).clone()),
        path_to_skills_md: skill_doc_path,
        scope: codex_protocol::protocol::SkillScope::User,
        plugin_id: None,
    }
}

fn test_path_display(unix_path: &str) -> String {
    test_path_buf(unix_path).display().to_string()
}

fn local_path_ref(path: AbsolutePathBuf) -> codex_exec_server::EnvironmentPathRef {
    codex_exec_server::EnvironmentPathRef::local(path)
}

#[test]
fn script_run_detection_matches_runner_plus_extension() {
    let tokens = vec![
        "python3".to_string(),
        "-u".to_string(),
        "scripts/fetch_comments.py".to_string(),
    ];

    assert_eq!(script_run_token(&tokens).is_some(), true);
}

#[test]
fn script_run_detection_excludes_python_c() {
    let tokens = vec![
        "python3".to_string(),
        "-c".to_string(),
        "print(1)".to_string(),
    ];

    assert_eq!(script_run_token(&tokens).is_some(), false);
}

#[tokio::test]
async fn skill_doc_read_detection_matches_absolute_path() {
    let skill_doc_path = test_path_buf("/tmp/skill-test/SKILL.md").abs();
    let normalized_skill_doc_path =
        canonicalize_if_exists(&local_path_ref(skill_doc_path.clone())).await;
    let skill = test_skill_metadata(skill_doc_path);
    let outcome = SkillLoadOutcome {
        implicit_skills_by_scripts_dir: Arc::new(HashMap::new()),
        implicit_skills_by_doc_path: Arc::new(HashMap::from([(normalized_skill_doc_path, skill)])),
        ..Default::default()
    };

    let tokens = vec![
        "cat".to_string(),
        test_path_display("/tmp/skill-test/SKILL.md"),
        "|".to_string(),
        "head".to_string(),
    ];
    let found = detect_skill_doc_read(
        &outcome,
        &tokens,
        &local_path_ref(test_path_buf("/tmp").abs()),
    )
    .await;

    assert_eq!(
        found.map(|value| value.name),
        Some("test-skill".to_string())
    );
}

#[tokio::test]
async fn skill_doc_read_detection_matches_parent_relative_path() {
    let skill_doc_path = test_path_buf("/tmp/project/.agents/skills/test-skill/SKILL.md").abs();
    let normalized_skill_doc_path =
        canonicalize_if_exists(&local_path_ref(skill_doc_path.clone())).await;
    let skill = test_skill_metadata(skill_doc_path);
    let outcome = SkillLoadOutcome {
        implicit_skills_by_scripts_dir: Arc::new(HashMap::new()),
        implicit_skills_by_doc_path: Arc::new(HashMap::from([(normalized_skill_doc_path, skill)])),
        ..Default::default()
    };
    let tokens = vec![
        "cat".to_string(),
        "../.agents/skills/test-skill/SKILL.md".to_string(),
    ];

    let found = detect_skill_doc_read(
        &outcome,
        &tokens,
        &local_path_ref(test_path_buf("/tmp/project/nested").abs()),
    )
    .await;

    assert_eq!(
        found.map(|value| value.name),
        Some("test-skill".to_string())
    );
}

#[tokio::test]
async fn skill_script_run_detection_matches_relative_path_from_skill_root() {
    let skill_doc_path = test_path_buf("/tmp/skill-test/SKILL.md").abs();
    let scripts_dir = canonicalize_if_exists(&local_path_ref(
        test_path_buf("/tmp/skill-test/scripts").abs(),
    ))
    .await;
    let skill = test_skill_metadata(skill_doc_path);
    let outcome = SkillLoadOutcome {
        implicit_skills_by_scripts_dir: Arc::new(HashMap::from([(scripts_dir, skill)])),
        implicit_skills_by_doc_path: Arc::new(HashMap::new()),
        ..Default::default()
    };
    let tokens = vec![
        "python3".to_string(),
        "scripts/fetch_comments.py".to_string(),
    ];

    let found = detect_skill_script_run(
        &outcome,
        &tokens,
        &local_path_ref(test_path_buf("/tmp/skill-test").abs()),
    )
    .await;

    assert_eq!(
        found.map(|value| value.name),
        Some("test-skill".to_string())
    );
}

#[tokio::test]
async fn skill_script_run_detection_matches_absolute_path_from_any_workdir() {
    let skill_doc_path = test_path_buf("/tmp/skill-test/SKILL.md").abs();
    let scripts_dir = canonicalize_if_exists(&local_path_ref(
        test_path_buf("/tmp/skill-test/scripts").abs(),
    ))
    .await;
    let skill = test_skill_metadata(skill_doc_path);
    let outcome = SkillLoadOutcome {
        implicit_skills_by_scripts_dir: Arc::new(HashMap::from([(scripts_dir, skill)])),
        implicit_skills_by_doc_path: Arc::new(HashMap::new()),
        ..Default::default()
    };
    let tokens = vec![
        "python3".to_string(),
        test_path_display("/tmp/skill-test/scripts/fetch_comments.py"),
    ];

    let found = detect_skill_script_run(
        &outcome,
        &tokens,
        &local_path_ref(test_path_buf("/tmp/other").abs()),
    )
    .await;

    assert_eq!(
        found.map(|value| value.name),
        Some("test-skill".to_string())
    );
}

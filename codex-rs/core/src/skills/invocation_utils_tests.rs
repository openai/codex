use super::SkillLoadOutcome;
use super::SkillMetadata;
use super::detect_skill_doc_read;
use super::detect_skill_script_run;
use super::normalize_path;
use super::script_run_token;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

fn test_skill_metadata(skill_doc_path: PathBuf) -> SkillMetadata {
    SkillMetadata {
        name: "test-skill".to_string(),
        description: "test".to_string(),
        short_description: None,
        interface: None,
        dependencies: None,
        policy: None,
        permission_profile: None,
        path_to_skills_md: skill_doc_path,
        scope: codex_protocol::protocol::SkillScope::User,
    }
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

#[test]
fn skill_doc_read_detection_matches_absolute_path() {
    let skill_doc_path = PathBuf::from("/tmp/skill-test/SKILL.md");
    let normalized_skill_doc_path = normalize_path(skill_doc_path.as_path());
    let skill = test_skill_metadata(skill_doc_path);
    let outcome = SkillLoadOutcome {
        implicit_skills_by_scripts_dir: Arc::new(HashMap::new()),
        implicit_skills_by_doc_path: Arc::new(HashMap::from([(normalized_skill_doc_path, skill)])),
        ..Default::default()
    };

    let tokens = vec![
        "cat".to_string(),
        "/tmp/skill-test/SKILL.md".to_string(),
        "|".to_string(),
        "head".to_string(),
    ];
    let found = detect_skill_doc_read(&outcome, &tokens, Path::new("/tmp"));

    assert_eq!(
        found.map(|value| value.name),
        Some("test-skill".to_string())
    );
}

#[test]
fn skill_script_run_detection_matches_relative_path_from_skill_root() {
    let skill_doc_path = PathBuf::from("/tmp/skill-test/SKILL.md");
    let scripts_dir = normalize_path(Path::new("/tmp/skill-test/scripts"));
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

    let found = detect_skill_script_run(&outcome, &tokens, Path::new("/tmp/skill-test"));

    assert_eq!(
        found.map(|value| value.name),
        Some("test-skill".to_string())
    );
}

#[test]
fn skill_script_run_detection_matches_absolute_path_from_any_workdir() {
    let skill_doc_path = PathBuf::from("/tmp/skill-test/SKILL.md");
    let scripts_dir = normalize_path(Path::new("/tmp/skill-test/scripts"));
    let skill = test_skill_metadata(skill_doc_path);
    let outcome = SkillLoadOutcome {
        implicit_skills_by_scripts_dir: Arc::new(HashMap::from([(scripts_dir, skill)])),
        implicit_skills_by_doc_path: Arc::new(HashMap::new()),
        ..Default::default()
    };
    let tokens = vec![
        "python3".to_string(),
        "/tmp/skill-test/scripts/fetch_comments.py".to_string(),
    ];

    let found = detect_skill_script_run(&outcome, &tokens, Path::new("/tmp/other"));

    assert_eq!(
        found.map(|value| value.name),
        Some("test-skill".to_string())
    );
}

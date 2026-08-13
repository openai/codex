use std::collections::HashMap;

use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;

use super::*;

#[derive(Default)]
struct TestLookup {
    by_scripts_dir: HashMap<AbsolutePathBuf, SkillMetadata>,
    by_doc_path: HashMap<AbsolutePathBuf, SkillMetadata>,
}

impl ImplicitSkillLookup for TestLookup {
    fn implicit_skill_for_scripts_dir(&self, path: &AbsolutePathBuf) -> Option<&SkillMetadata> {
        self.by_scripts_dir.get(path)
    }

    fn implicit_skill_for_doc_path(&self, path: &AbsolutePathBuf) -> Option<&SkillMetadata> {
        self.by_doc_path.get(path)
    }
}

fn test_skill_metadata(skill_doc_path: AbsolutePathBuf) -> SkillMetadata {
    SkillMetadata {
        name: "test-skill".to_string(),
        description: "test".to_string(),
        short_description: None,
        interface: None,
        dependencies: None,
        policy: None,
        path_to_skills_md: skill_doc_path,
        scope: codex_protocol::protocol::SkillScope::User,
        plugin_id: None,
        remote_plugin_id: None,
    }
}

fn test_path_display(unix_path: &str) -> String {
    test_path_buf(unix_path).display().to_string()
}

#[test]
fn script_run_detection_matches_runner_plus_extension() {
    let tokens = vec![
        "python3".to_string(),
        "-u".to_string(),
        "scripts/fetch_comments.py".to_string(),
    ];

    assert!(script_run_token(&tokens).is_some());
}

#[test]
fn script_run_detection_excludes_python_c() {
    let tokens = vec![
        "python3".to_string(),
        "-c".to_string(),
        "print(1)".to_string(),
    ];

    assert!(script_run_token(&tokens).is_none());
}

#[test]
fn powershell_skill_doc_read_matches_common_forms() {
    let skill_doc_path = test_path_buf("/tmp/skill-test/SKILL.md").abs();
    let spaced_skill_doc_path = test_path_buf("/tmp/skill test/SKILL.md").abs();
    let skill = test_skill_metadata(skill_doc_path.clone());
    let spaced_skill = test_skill_metadata(spaced_skill_doc_path.clone());
    let outcome = TestLookup {
        by_doc_path: HashMap::from([
            (canonicalize_if_exists(&skill_doc_path), skill),
            (canonicalize_if_exists(&spaced_skill_doc_path), spaced_skill),
        ]),
        ..Default::default()
    };
    let path = skill_doc_path.display();
    let spaced_path = spaced_skill_doc_path.display();

    for command in [
        format!("Get-Content {path}"),
        format!("Get-Content -Raw {path}"),
        format!("Get-Content \"{spaced_path}\""),
        format!("Get-Content -Raw \"{spaced_path}\""),
        format!("get-content   -raw '{spaced_path}'"),
    ] {
        let found = detect_implicit_skill_invocation_for_command(
            &outcome,
            &command,
            &test_path_buf("/tmp").abs(),
        );

        assert_eq!(
            found.map(|value| value.name),
            Some("test-skill".to_string()),
            "command: {command}"
        );
    }
}

#[test]
fn windows_executor_skill_reads_share_powershell_classification() {
    let workdir = PathUri::parse("file:///C:/skills").expect("Windows workdir URI");
    let document = PathUri::parse("file:///C:/skills/demo/SKILL.md").expect("skill URI");

    for command in [
        r"Get-Content C:\skills\demo\SKILL.md",
        r"get-content -Raw C:\skills\demo\SKILL.md",
        r"Get-Content -Path C:\skills\demo\SKILL.md",
        r"Get-Content -LiteralPath C:\skills\demo\SKILL.md",
        r"Get-Content C:\skills\demo\SKILL.md -Raw",
        r"Get-Content -Raw -LiteralPath C:\skills\demo\SKILL.md",
        r"gc C:\skills\demo\SKILL.md",
        r"type C:\skills\demo\SKILL.md",
    ] {
        assert_eq!(
            implicit_skill_accesses_for_command(command, &workdir),
            vec![ImplicitSkillAccess::Document(document.clone())],
            "command: {command}"
        );
    }
}

#[test]
fn skill_doc_read_detection_matches_absolute_path() {
    let skill_doc_path = test_path_buf("/tmp/skill-test/SKILL.md").abs();
    let normalized_skill_doc_path = canonicalize_if_exists(&skill_doc_path);
    let skill = test_skill_metadata(skill_doc_path);
    let outcome = TestLookup {
        by_doc_path: HashMap::from([(normalized_skill_doc_path, skill)]),
        ..Default::default()
    };
    let tokens = vec![
        "cat".to_string(),
        test_path_display("/tmp/skill-test/SKILL.md"),
        "|".to_string(),
        "head".to_string(),
    ];

    let found = detect_skill_doc_read(&outcome, &tokens, &test_path_buf("/tmp").abs());

    assert_eq!(
        found.map(|value| value.name),
        Some("test-skill".to_string())
    );
}

#[test]
fn skill_doc_read_detection_matches_shared_read_parser() {
    let skill_doc_path = test_path_buf("/tmp/skill-test/SKILL.md").abs();
    let normalized_skill_doc_path = canonicalize_if_exists(&skill_doc_path);
    let skill = test_skill_metadata(skill_doc_path);
    let outcome = TestLookup {
        by_doc_path: HashMap::from([(normalized_skill_doc_path, skill)]),
        ..Default::default()
    };
    let tokens = vec![
        "nl".to_string(),
        "-ba".to_string(),
        test_path_display("/tmp/skill-test/SKILL.md"),
    ];

    let found = detect_skill_doc_read(&outcome, &tokens, &test_path_buf("/tmp").abs());

    assert_eq!(
        found.map(|value| value.name),
        Some("test-skill".to_string())
    );
}

#[test]
fn skill_script_run_detection_matches_relative_path_from_skill_root() {
    let skill_doc_path = test_path_buf("/tmp/skill-test/SKILL.md").abs();
    let scripts_dir = canonicalize_if_exists(&test_path_buf("/tmp/skill-test/scripts").abs());
    let skill = test_skill_metadata(skill_doc_path);
    let outcome = TestLookup {
        by_scripts_dir: HashMap::from([(scripts_dir, skill)]),
        ..Default::default()
    };
    let tokens = vec![
        "python3".to_string(),
        "scripts/fetch_comments.py".to_string(),
    ];

    let found = detect_skill_script_run(&outcome, &tokens, &test_path_buf("/tmp/skill-test").abs());

    assert_eq!(
        found.map(|value| value.name),
        Some("test-skill".to_string())
    );
}

#[test]
fn skill_script_run_detection_matches_absolute_path_from_any_workdir() {
    let skill_doc_path = test_path_buf("/tmp/skill-test/SKILL.md").abs();
    let scripts_dir = canonicalize_if_exists(&test_path_buf("/tmp/skill-test/scripts").abs());
    let skill = test_skill_metadata(skill_doc_path);
    let outcome = TestLookup {
        by_scripts_dir: HashMap::from([(scripts_dir, skill)]),
        ..Default::default()
    };
    let tokens = vec![
        "python3".to_string(),
        test_path_display("/tmp/skill-test/scripts/fetch_comments.py"),
    ];

    let found = detect_skill_script_run(&outcome, &tokens, &test_path_buf("/tmp/other").abs());

    assert_eq!(
        found.map(|value| value.name),
        Some("test-skill".to_string())
    );
}

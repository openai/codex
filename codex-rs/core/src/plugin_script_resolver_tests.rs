use codex_core_skills::SkillMetadata;
use codex_protocol::protocol::SkillScope;
use pretty_assertions::assert_eq;
use std::fs;
use tempfile::TempDir;

use super::*;

fn fixture() -> (TempDir, AbsolutePathBuf, Vec<FirstPartyPluginRoot>) {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = AbsolutePathBuf::try_from(temp.path().join("plugin")).expect("absolute path");
    fs::create_dir_all(root.join("skills/demo/scripts")).expect("create scripts directory");
    fs::write(root.join("skills/demo/SKILL.md"), "# Demo").expect("write skill");
    fs::write(root.join("skills/demo/scripts/run.py"), "print('ok')").expect("write script");
    let roots = vec![FirstPartyPluginRoot {
        plugin_id: "openai/demo".to_string(),
        plugin_root: root.clone(),
    }];
    (temp, root, roots)
}

fn skill(root: &AbsolutePathBuf, name: &str, path: &str) -> SkillMetadata {
    SkillMetadata {
        name: name.to_string(),
        description: String::new(),
        short_description: None,
        interface: None,
        dependencies: None,
        policy: None,
        path_to_skills_md: root.join(path),
        scope: SkillScope::User,
        plugin_id: Some("openai/demo".to_string()),
    }
}

fn skill_outcome(root: &AbsolutePathBuf) -> SkillLoadOutcome {
    let mut outcome = SkillLoadOutcome::default();
    outcome.skills = vec![skill(root, "demo", "skills/demo/SKILL.md")];
    outcome
}

fn resolve(
    roots: &[FirstPartyPluginRoot],
    skills: &SkillLoadOutcome,
    command: &str,
    cwd: &AbsolutePathBuf,
) -> Option<ResolvedPluginScript> {
    resolve_plugin_script(roots, skills, command, cwd, ShellType::Bash)
}

#[test]
fn resolves_interpreter_script_to_plugin_relative_path_and_skill() {
    let (_temp, root, roots) = fixture();
    let resolved = resolve(
        &roots,
        &skill_outcome(&root),
        "python skills/demo/scripts/run.py --secret argument",
        &root,
    )
    .expect("plugin script");

    assert_eq!(resolved.plugin_id, "openai/demo");
    assert_eq!(resolved.script_path, "skills/demo/scripts/run.py");
    assert_eq!(resolved.skill.expect("skill").skill_name, "demo");

    let absolute = resolve(
        &roots,
        &SkillLoadOutcome::default(),
        root.join("skills/demo/scripts/run.py")
            .to_string_lossy()
            .replace('\\', "/")
            .as_ref(),
        &root,
    )
    .expect("absolute plugin script");
    assert_eq!(absolute.script_path, "skills/demo/scripts/run.py");
}

#[test]
fn rejects_non_plugin_and_symlink_escape_paths() {
    let (temp, root, roots) = fixture();
    let outside = AbsolutePathBuf::try_from(temp.path().join("outside.py")).expect("absolute path");
    fs::write(&outside, "print('outside')").expect("write outside script");

    for command in [
        outside.to_string_lossy().into_owned(),
        "skills/demo/scripts".to_string(),
    ] {
        assert!(
            resolve(&roots, &SkillLoadOutcome::default(), &command, &root).is_none(),
            "unexpected lifecycle attribution for {command}"
        );
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside, root.join("escape.py")).expect("create symlink");
        assert!(
            resolve(
                &roots,
                &SkillLoadOutcome::default(),
                "python escape.py",
                &root,
            )
            .is_none()
        );
    }
}

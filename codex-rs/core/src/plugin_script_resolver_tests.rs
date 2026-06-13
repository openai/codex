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
fn resolves_direct_executable_without_a_known_extension() {
    let (_temp, root, roots) = fixture();
    fs::create_dir_all(root.join("bin")).expect("create bin");
    fs::write(root.join("bin/run"), "#!/bin/sh\n").expect("write executable");

    let resolved =
        resolve(&roots, &SkillLoadOutcome::default(), "./bin/run", &root).expect("plugin script");

    assert_eq!(resolved.script_path, "bin/run");
    assert!(resolved.skill.is_none());
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

#[test]
fn nested_skill_owns_its_deepest_matching_scripts_directory() {
    let (_temp, root, roots) = fixture();
    fs::create_dir_all(root.join("skills/demo/scripts/nested/scripts"))
        .expect("create nested scripts");
    fs::write(root.join("skills/demo/scripts/nested/SKILL.md"), "# Nested")
        .expect("write nested skill");
    fs::write(
        root.join("skills/demo/scripts/nested/scripts/run.py"),
        "print('ok')",
    )
    .expect("write nested script");
    let mut skills = skill_outcome(&root);
    skills.skills.push(skill(
        &root,
        "nested",
        "skills/demo/scripts/nested/SKILL.md",
    ));

    let resolved = resolve(
        &roots,
        &skills,
        "python skills/demo/scripts/nested/scripts/run.py",
        &root,
    )
    .expect("nested plugin script");
    assert_eq!(resolved.skill.expect("skill").skill_name, "nested");
}

#[test]
fn resolves_node_and_shell_scripts() {
    let (_temp, root, roots) = fixture();
    fs::write(root.join("skills/demo/scripts/run.js"), "console.log('ok')")
        .expect("write node script");
    fs::write(root.join("skills/demo/scripts/run.sh"), "echo ok").expect("write shell script");

    for (command, expected) in [
        (
            "node skills/demo/scripts/run.js",
            "skills/demo/scripts/run.js",
        ),
        (
            "sh skills/demo/scripts/run.sh",
            "skills/demo/scripts/run.sh",
        ),
        (
            "python -u skills/demo/scripts/run.py",
            "skills/demo/scripts/run.py",
        ),
        (
            "sh -e skills/demo/scripts/run.sh",
            "skills/demo/scripts/run.sh",
        ),
    ] {
        let resolved =
            resolve(&roots, &SkillLoadOutcome::default(), command, &root).expect("plugin script");
        assert_eq!(resolved.script_path, expected);
    }
}

#[test]
fn rejects_shell_path_expansion_candidates() {
    for shell_type in [ShellType::Bash, ShellType::Sh, ShellType::Zsh] {
        for command in [
            "python ~/run.py",
            "~/run",
            "python scripts/z?.py",
            "python scripts/*.py",
            "scripts/*.sh",
            "python scripts/[rz]un.py",
            "python scripts/{run,skip}.py",
            r#"python scripts/run\ script.py"#,
            r#"python "scripts/run\\script.py""#,
        ] {
            assert!(
                script_token(command, shell_type).is_none(),
                "unexpected shell expansion attribution for {shell_type:?}: {command}"
            );
        }
    }
}

#[test]
fn rejects_zsh_equals_expansion_candidates() {
    for command in ["python =foo", "node =foo", "=foo"] {
        assert!(
            script_token(command, ShellType::Zsh).is_none(),
            "unexpected Zsh EQUALS attribution for {command}"
        );
    }

    for shell_type in [ShellType::Bash, ShellType::Sh] {
        for command in ["python =foo", "node =foo"] {
            assert_eq!(
                script_token(command, shell_type),
                Some("=foo".to_string()),
                "unexpected literal equals rejection for {shell_type:?}: {command}"
            );
        }
    }
}

#[test]
fn rejects_compound_commands_and_runner_options() {
    let (_temp, root, roots) = fixture();
    for command in [
        "python skills/demo/scripts/run.py && python skills/demo/scripts/run.py",
        "python -c skills/demo/scripts/run.py",
        "node --loader skills/demo/scripts/loader.js skills/demo/scripts/run.js",
        "env -C skills/demo python scripts/run.py",
        "timeout 1 python skills/demo/scripts/run.py",
        "uv run skills/demo/scripts/run.py",
        "cd skills/demo && python scripts/run.py",
        r#"C:\tmp\python.exe skills/demo/scripts/run.py"#,
    ] {
        assert!(
            resolve(&roots, &SkillLoadOutcome::default(), command, &root).is_none(),
            "unexpected lifecycle attribution for {command}"
        );
    }
}

#[test]
#[cfg(not(windows))]
fn direct_executable_matching_interpreter_name_is_case_sensitive() {
    let (_temp, root, roots) = fixture();
    fs::write(root.join("Python"), "#!/bin/sh\n").expect("write case-sensitive executable");

    let resolved = resolve(&roots, &SkillLoadOutcome::default(), "./Python", &root)
        .expect("case-sensitive direct executable");
    assert_eq!(resolved.script_path, "Python");
}

#[test]
fn powershell_split_preserves_paths_and_rejects_compounds() {
    assert_eq!(
        command_tokens(
            r#"pwsh.exe -File C:\Users\me\plugin\scripts\run.ps1"#,
            ShellType::PowerShell,
        ),
        Some(vec![
            "pwsh.exe".to_string(),
            "-File".to_string(),
            r#"C:\Users\me\plugin\scripts\run.ps1"#.to_string(),
        ])
    );
    assert_eq!(
        command_tokens(
            r#"& 'C:\Program Files\plugin\scripts\run.ps1'"#,
            ShellType::PowerShell,
        ),
        Some(vec![
            r#"C:\Program Files\plugin\scripts\run.ps1"#.to_string()
        ])
    );
    assert!(
        command_tokens(
            r#"'C:\Program Files\plugin\scripts\run.ps1'"#,
            ShellType::PowerShell
        )
        .is_none()
    );
    for command in [
        r#"& 'C:\plugin`name\scripts\run.ps1'"#,
        r#"& "C:\plugin` name\scripts\run.ps1""#,
        r#"& "C:\plugin`u{2e}\scripts\run.ps1""#,
    ] {
        assert!(
            command_tokens(command, ShellType::PowerShell).is_none(),
            "unexpected PowerShell backtick attribution for {command}"
        );
    }
    for command in [
        r#"python "$HOME\run.py""#,
        r#"python $HOME\run.py"#,
        r#"& "$HOME\run.ps1""#,
        r#"& ~\run.ps1"#,
        "python (Get-Command outside-script).Source",
        "python {outside-script}",
        "python @(outside-script)",
        "python scripts/run.py > outside.txt",
        "python scripts/run.py # outside",
    ] {
        assert!(
            command_tokens(command, ShellType::PowerShell).is_none(),
            "unexpected PowerShell expansion attribution for {command}"
        );
    }
    assert!(command_tokens("python a.py; python b.py", ShellType::PowerShell).is_none());
    assert_eq!(
        command_tokens(r#"C:\plugin\scripts\run.cmd"#, ShellType::Cmd),
        Some(vec![r#"C:\plugin\scripts\run.cmd"#.to_string()])
    );
    assert_eq!(
        command_tokens(
            r#""C:\Program Files\plugin\scripts\run.bat""#,
            ShellType::Cmd
        ),
        Some(vec![
            r#"C:\Program Files\plugin\scripts\run.bat"#.to_string()
        ])
    );
    assert!(command_tokens("python run.py`&whoami", ShellType::Cmd).is_none());
    assert!(command_tokens(r#"'C:\plugin\scripts\run.cmd'"#, ShellType::Cmd).is_none());
    assert!(script_token(r#"C:\tmp\python.exe run.py"#, ShellType::Cmd).is_none());
}

#[test]
#[cfg(unix)]
fn rejects_non_utf8_normalized_paths() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let path = OsString::from_vec(b"scripts/run\xff.py".to_vec());
    assert_eq!(normalized_relative_path(Path::new(&path)), None);
}

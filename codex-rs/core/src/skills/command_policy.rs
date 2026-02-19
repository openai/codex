use std::collections::HashSet;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::parse_command::ParsedCommand;
use dunce::canonicalize as canonicalize_path;

use crate::config::Permissions;
use crate::path_utils::normalize_for_path_comparison;
use crate::skills::SkillLoadOutcome;

/// Derives transient in-memory execpolicy prefix rules for commands executed
/// from skill `scripts/` directories.
///
/// Assumptions:
/// 1. Skill prefix derivation is enabled only when `shell_zsh_fork` is enabled.
/// 2. `command` contains the executable and arguments for the invocation.
/// 3. `command_cwd` reflects the effective command target location.
/// 4. Only commands executed via `zsh` are eligible.
/// 5. Only executable paths under `<skill>/scripts` are eligible.
///
/// Return shape:
/// - Outer `Vec`: all derived prefix rules.
/// - Inner `Vec<String>`: one prefix rule pattern as command tokens.
///
/// Returns an empty list when `shell_zsh_fork` is disabled, command shell is
/// not `zsh`, or when no enabled skill with permissions matches an eligible
/// command action executable.
pub(crate) fn derive_skill_execpolicy_overlay_prefixes_for_command(
    skills_outcome: &SkillLoadOutcome,
    shell_zsh_fork_enabled: bool,
    command: &[String],
    command_cwd: &Path,
    command_actions: &[ParsedCommand],
) -> Vec<Vec<String>> {
    if !shell_zsh_fork_enabled {
        return Vec::new();
    }

    let command_executable_name = command
        .first()
        .and_then(|program| Path::new(program).file_name())
        .and_then(|name| name.to_str());
    if command_executable_name != Some("zsh") {
        return Vec::new();
    }

    let mut prefixes = Vec::new();
    let mut seen = HashSet::new();
    for command_action in command_actions {
        let Some((action_candidate, executable_prefix_token)) =
            command_action_candidate(command_action, command_cwd)
        else {
            continue;
        };
        if match_skill_for_candidate(skills_outcome, &action_candidate).is_none() {
            continue;
        }
        let prefix = vec![executable_prefix_token];
        if seen.insert(prefix.clone()) {
            prefixes.push(prefix);
        }
    }

    prefixes
}

fn command_action_candidate(
    command_action: &ParsedCommand,
    command_cwd: &Path,
) -> Option<(PathBuf, String)> {
    let (action_path, executable_prefix_token) = match command_action {
        ParsedCommand::Unknown { cmd } => {
            let tokens = shlex::split(cmd)?;
            let executable = tokens.first()?;
            let executable_path = PathBuf::from(executable);
            if !executable_path.is_absolute() && !executable.contains('/') {
                return None;
            }
            Some((executable_path, executable.clone()))
        }
        ParsedCommand::Read { .. }
        | ParsedCommand::ListFiles { .. }
        | ParsedCommand::Search { .. } => None,
    }?;
    let action_path = if action_path.is_absolute() {
        action_path
    } else {
        command_cwd.join(action_path)
    };
    let normalized_action_path = normalize_candidate_path(action_path.as_path())?;
    Some((normalized_action_path, executable_prefix_token))
}

fn normalize_candidate_path(path: &Path) -> Option<PathBuf> {
    let normalized = normalize_lexically(path);
    let canonicalized = canonicalize_path(&normalized).unwrap_or(normalized);
    let comparison_path =
        normalize_for_path_comparison(canonicalized.as_path()).unwrap_or(canonicalized);
    if comparison_path.is_absolute() {
        Some(comparison_path)
    } else {
        None
    }
}

fn match_skill_for_candidate<'a>(
    skills_outcome: &'a SkillLoadOutcome,
    candidate: &Path,
) -> Option<&'a Permissions> {
    for skill in &skills_outcome.skills {
        // Disabled skills must not contribute sandbox policy extensions.
        if skills_outcome.disabled_paths.contains(&skill.path) {
            continue;
        }
        // Skills without a permissions block cannot supply sandbox policy.
        let Some(permissions) = skill.permissions.as_ref() else {
            continue;
        };
        // Match against the containing skill directory, not the SKILL.md file.
        let Some(skill_dir) = skill.path.parent() else {
            continue;
        };
        let skill_scripts_dir = skill_dir.join("scripts");
        // Normalize before comparing so path containment checks are stable.
        let Some(skill_scripts_dir) = normalize_candidate_path(&skill_scripts_dir) else {
            continue;
        };
        // The executable must live inside the skill's scripts directory.
        if !candidate.starts_with(&skill_scripts_dir) {
            continue;
        }
        return Some(permissions);
    }

    None
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::derive_skill_execpolicy_overlay_prefixes_for_command;
    use crate::config::Constrained;
    use crate::config::Permissions;
    use crate::config::types::ShellEnvironmentPolicy;
    use crate::protocol::AskForApproval;
    use crate::protocol::SandboxPolicy;
    use crate::skills::SkillLoadOutcome;
    use crate::skills::model::SkillMetadata;
    use codex_protocol::parse_command::ParsedCommand;
    use codex_protocol::protocol::SkillScope;
    use pretty_assertions::assert_eq;
    use std::collections::HashSet;
    use std::path::Path;
    use std::path::PathBuf;

    fn skill_with_permissions(skill_path: PathBuf) -> SkillMetadata {
        SkillMetadata {
            name: "skill".to_string(),
            description: "skill".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permissions: Some(Permissions {
                approval_policy: Constrained::allow_any(AskForApproval::Never),
                sandbox_policy: Constrained::allow_any(SandboxPolicy::new_workspace_write_policy()),
                network: None,
                shell_environment_policy: ShellEnvironmentPolicy::default(),
                windows_sandbox_mode: None,
                macos_seatbelt_profile_extensions: None,
            }),
            path: skill_path,
            scope: SkillScope::User,
        }
    }

    fn outcome_with_skills(skills: Vec<SkillMetadata>) -> SkillLoadOutcome {
        SkillLoadOutcome {
            skills,
            errors: Vec::new(),
            disabled_paths: HashSet::new(),
        }
    }

    fn canonical(path: &Path) -> PathBuf {
        dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }

    #[test]
    fn derives_prefix_for_zsh_executable_inside_skill_scripts_directory() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skills/demo");
        let scripts_dir = skill_dir.join("scripts");
        std::fs::create_dir_all(&scripts_dir).expect("create scripts");
        std::fs::write(scripts_dir.join("run.sh"), "#!/bin/sh\necho ok\n").expect("write script");
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_path, "skill").expect("write SKILL.md");
        let cwd = tempdir.path().to_path_buf();

        let outcome = outcome_with_skills(vec![skill_with_permissions(canonical(&skill_path))]);
        let command = vec![
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            "skills/demo/scripts/run.sh".to_string(),
        ];
        let command_actions = vec![ParsedCommand::Unknown {
            cmd: "skills/demo/scripts/run.sh".to_string(),
        }];

        let resolved = derive_skill_execpolicy_overlay_prefixes_for_command(
            &outcome,
            true,
            &command,
            &cwd,
            &command_actions,
        );

        assert_eq!(
            resolved,
            vec![vec!["skills/demo/scripts/run.sh".to_string()]]
        );
    }

    #[test]
    fn returns_empty_prefixes_when_command_is_not_zsh() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skills/demo");
        let scripts_dir = skill_dir.join("scripts");
        std::fs::create_dir_all(&scripts_dir).expect("create scripts");
        std::fs::write(scripts_dir.join("run.sh"), "#!/bin/sh\necho ok\n").expect("write script");
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_path, "skill").expect("write SKILL.md");

        let outcome = outcome_with_skills(vec![skill_with_permissions(canonical(&skill_path))]);
        let command = vec![
            "/bin/bash".to_string(),
            "-lc".to_string(),
            "skills/demo/scripts/run.sh".to_string(),
        ];
        let command_actions = vec![ParsedCommand::Unknown {
            cmd: "skills/demo/scripts/run.sh".to_string(),
        }];

        let resolved = derive_skill_execpolicy_overlay_prefixes_for_command(
            &outcome,
            true,
            &command,
            tempdir.path(),
            &command_actions,
        );

        assert!(resolved.is_empty());
    }

    #[test]
    fn returns_empty_prefixes_for_paths_outside_skill_scripts_directory() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skills/demo");
        std::fs::create_dir_all(&skill_dir).expect("create skill");
        std::fs::write(skill_dir.join("run.sh"), "#!/bin/sh\necho ok\n").expect("write script");
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_path, "skill").expect("write SKILL.md");

        let outcome = outcome_with_skills(vec![skill_with_permissions(canonical(&skill_path))]);
        let command = vec![
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            "skills/demo/run.sh".to_string(),
        ];
        let command_actions = vec![ParsedCommand::Unknown {
            cmd: "skills/demo/run.sh".to_string(),
        }];
        let resolved = derive_skill_execpolicy_overlay_prefixes_for_command(
            &outcome,
            true,
            &command,
            tempdir.path(),
            &command_actions,
        );

        assert!(resolved.is_empty());
    }

    #[test]
    fn ignores_disabled_skill_when_deriving_prefix_rules() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skills/demo");
        let scripts_dir = skill_dir.join("scripts");
        std::fs::create_dir_all(&scripts_dir).expect("create skill dir");
        std::fs::write(scripts_dir.join("tool.sh"), "#!/bin/sh\necho ok\n").expect("write script");
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_path, "skill").expect("write SKILL.md");
        let skill_path = canonical(&skill_path);

        let mut outcome = outcome_with_skills(vec![skill_with_permissions(skill_path.clone())]);
        outcome.disabled_paths.insert(skill_path);
        let command = vec![
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            "skills/demo/scripts/tool.sh".to_string(),
        ];
        let command_actions = vec![ParsedCommand::Unknown {
            cmd: "skills/demo/scripts/tool.sh".to_string(),
        }];

        let resolved = derive_skill_execpolicy_overlay_prefixes_for_command(
            &outcome,
            true,
            &command,
            tempdir.path(),
            &command_actions,
        );

        assert!(resolved.is_empty());
    }

    #[test]
    fn derives_prefix_for_nested_skill_scripts_path() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let parent_skill_dir = tempdir.path().join("skills/parent");
        let nested_skill_dir = parent_skill_dir.join("nested");
        std::fs::create_dir_all(parent_skill_dir.join("scripts")).expect("create parent scripts");
        std::fs::create_dir_all(nested_skill_dir.join("scripts")).expect("create nested scripts");
        std::fs::write(
            parent_skill_dir.join("scripts/run.sh"),
            "#!/bin/sh\necho parent\n",
        )
        .expect("write script");

        std::fs::write(
            nested_skill_dir.join("scripts/run.sh"),
            "#!/bin/sh\necho nested\n",
        )
        .expect("write script");

        let parent_skill_path = parent_skill_dir.join("SKILL.md");
        let nested_skill_path = nested_skill_dir.join("SKILL.md");
        std::fs::write(&parent_skill_path, "parent").expect("write parent skill");
        std::fs::write(&nested_skill_path, "nested").expect("write nested skill");

        let outcome = outcome_with_skills(vec![
            skill_with_permissions(canonical(&parent_skill_path)),
            skill_with_permissions(canonical(&nested_skill_path)),
        ]);
        let command = vec![
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            "skills/parent/nested/scripts/run.sh".to_string(),
        ];
        let command_actions = vec![ParsedCommand::Unknown {
            cmd: "skills/parent/nested/scripts/run.sh".to_string(),
        }];

        let resolved = derive_skill_execpolicy_overlay_prefixes_for_command(
            &outcome,
            true,
            &command,
            tempdir.path(),
            &command_actions,
        );

        assert_eq!(
            resolved,
            vec![vec!["skills/parent/nested/scripts/run.sh".to_string()]]
        );
    }

    #[test]
    fn ignores_non_path_unknown_command_actions() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skills/demo");
        std::fs::create_dir_all(skill_dir.join("scripts")).expect("create scripts");
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_path, "skill").expect("write SKILL.md");

        let outcome = outcome_with_skills(vec![skill_with_permissions(canonical(&skill_path))]);
        let command = vec![
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            "echo hi".to_string(),
        ];
        let command_actions = vec![ParsedCommand::Unknown {
            cmd: "echo hi".to_string(),
        }];

        let resolved = derive_skill_execpolicy_overlay_prefixes_for_command(
            &outcome,
            true,
            &command,
            skill_dir.join("scripts").as_path(),
            &command_actions,
        );

        assert!(resolved.is_empty());
    }

    #[test]
    fn returns_empty_prefixes_when_shell_zsh_fork_is_disabled() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skills/demo");
        let scripts_dir = skill_dir.join("scripts");
        std::fs::create_dir_all(&scripts_dir).expect("create scripts");
        std::fs::write(scripts_dir.join("run.sh"), "#!/bin/sh\necho ok\n").expect("write script");
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_path, "skill").expect("write SKILL.md");
        let cwd = tempdir.path().to_path_buf();

        let outcome = outcome_with_skills(vec![skill_with_permissions(canonical(&skill_path))]);
        let command = vec![
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            "skills/demo/scripts/run.sh".to_string(),
        ];
        let command_actions = vec![ParsedCommand::Unknown {
            cmd: "skills/demo/scripts/run.sh".to_string(),
        }];

        let resolved = derive_skill_execpolicy_overlay_prefixes_for_command(
            &outcome,
            false,
            &command,
            &cwd,
            &command_actions,
        );

        assert!(resolved.is_empty());
    }
}

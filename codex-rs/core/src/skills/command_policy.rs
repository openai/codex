use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::parse_command::ParsedCommand;
use dunce::canonicalize as canonicalize_path;

use crate::config::Permissions;
use crate::path_utils::normalize_for_path_comparison;
use crate::skills::SkillLoadOutcome;

/// Resolves the full permissions extension contributed by the first matching
/// skill for a command invocation.
///
/// Assumptions:
/// 1. Skill policy extension is enabled only when `shell_zsh_fork` is enabled.
/// 2. `command` contains the executable and arguments for the invocation.
/// 3. `command_cwd` reflects the effective command target location.
/// 4. Only commands executed via `zsh` are eligible.
/// 5. Only executable paths under `<skill>/scripts` are eligible.
///
/// Returns `None` when `shell_zsh_fork` is disabled, command shell is not
/// `zsh`, or when no enabled skill with permissions matches an eligible
/// command action executable.
pub(crate) fn resolve_skill_permissions_for_command(
    skills_outcome: &SkillLoadOutcome,
    shell_zsh_fork_enabled: bool,
    command: &[String],
    command_cwd: &Path,
    command_actions: &[ParsedCommand],
) -> Option<Permissions> {
    resolve_skill_permissions_ref(
        skills_outcome,
        shell_zsh_fork_enabled,
        command,
        command_cwd,
        command_actions,
    )
    .cloned()
}

fn resolve_skill_permissions_ref<'a>(
    skills_outcome: &'a SkillLoadOutcome,
    shell_zsh_fork_enabled: bool,
    command: &[String],
    command_cwd: &Path,
    command_actions: &[ParsedCommand],
) -> Option<&'a Permissions> {
    if !shell_zsh_fork_enabled {
        return None;
    }

    let command_executable_name = command
        .first()
        .and_then(|program| Path::new(program).file_name())
        .and_then(|name| name.to_str());
    if command_executable_name != Some("zsh") {
        return None;
    }

    for command_action in command_actions {
        let Some(action_candidate) = command_action_candidate_path(command_action, command_cwd)
        else {
            continue;
        };
        if let Some(permissions) = match_skill_for_candidate(skills_outcome, &action_candidate) {
            return Some(permissions);
        }
    }

    None
}

fn command_action_candidate_path(
    command_action: &ParsedCommand,
    command_cwd: &Path,
) -> Option<PathBuf> {
    let action_path = match command_action {
        ParsedCommand::Unknown { cmd } => {
            let tokens = shlex::split(cmd)?;
            let executable = tokens.first()?;
            let executable_path = PathBuf::from(executable);
            if !executable_path.is_absolute() && !executable.contains('/') {
                return None;
            }
            Some(executable_path)
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
    normalize_candidate_path(action_path.as_path())
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
    use super::resolve_skill_permissions_for_command;
    use crate::config::Constrained;
    use crate::config::Permissions;
    use crate::config::types::ShellEnvironmentPolicy;
    use crate::protocol::AskForApproval;
    use crate::protocol::ReadOnlyAccess;
    use crate::protocol::SandboxPolicy;
    use crate::skills::SkillLoadOutcome;
    use crate::skills::model::SkillMetadata;
    use codex_protocol::parse_command::ParsedCommand;
    use codex_protocol::protocol::SkillScope;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::collections::HashSet;
    use std::path::Path;
    use std::path::PathBuf;

    fn skill_with_policy(skill_path: PathBuf, sandbox_policy: SandboxPolicy) -> SkillMetadata {
        SkillMetadata {
            name: "skill".to_string(),
            description: "skill".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            permissions: Some(Permissions {
                approval_policy: Constrained::allow_any(AskForApproval::Never),
                sandbox_policy: Constrained::allow_any(sandbox_policy),
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

    fn resolve_skill_sandbox_extension_for_command(
        skills_outcome: &SkillLoadOutcome,
        shell_zsh_fork_enabled: bool,
        command: &[String],
        command_cwd: &Path,
        command_actions: &[ParsedCommand],
    ) -> Option<SandboxPolicy> {
        resolve_skill_permissions_for_command(
            skills_outcome,
            shell_zsh_fork_enabled,
            command,
            command_cwd,
            command_actions,
        )
        .map(|permissions| permissions.sandbox_policy.get().clone())
    }

    #[test]
    fn resolves_policy_for_zsh_executable_inside_skill_scripts_directory() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skills/demo");
        let scripts_dir = skill_dir.join("scripts");
        std::fs::create_dir_all(&scripts_dir).expect("create scripts");
        std::fs::write(scripts_dir.join("run.sh"), "#!/bin/sh\necho ok\n").expect("write script");
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_path, "skill").expect("write SKILL.md");
        let cwd = tempdir.path().to_path_buf();

        let write_root = AbsolutePathBuf::try_from(skill_dir.join("output")).expect("absolute");
        let skill_policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![write_root],
            read_only_access: ReadOnlyAccess::FullAccess,
            network_access: true,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        };
        let outcome = outcome_with_skills(vec![skill_with_policy(
            canonical(&skill_path),
            skill_policy.clone(),
        )]);
        let command = vec![
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            "skills/demo/scripts/run.sh".to_string(),
        ];
        let command_actions = vec![ParsedCommand::Unknown {
            cmd: "skills/demo/scripts/run.sh".to_string(),
        }];

        let resolved = resolve_skill_sandbox_extension_for_command(
            &outcome,
            true,
            &command,
            &cwd,
            &command_actions,
        );

        assert_eq!(resolved, Some(skill_policy));
    }

    #[test]
    fn does_not_resolve_policy_when_command_is_not_zsh() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skills/demo");
        let scripts_dir = skill_dir.join("scripts");
        std::fs::create_dir_all(&scripts_dir).expect("create scripts");
        std::fs::write(scripts_dir.join("run.sh"), "#!/bin/sh\necho ok\n").expect("write script");
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_path, "skill").expect("write SKILL.md");

        let skill_policy = SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: true,
                readable_roots: vec![
                    AbsolutePathBuf::try_from(skill_dir.join("data")).expect("absolute"),
                ],
            },
        };
        let outcome = outcome_with_skills(vec![skill_with_policy(
            canonical(&skill_path),
            skill_policy,
        )]);
        let command = vec![
            "/bin/bash".to_string(),
            "-lc".to_string(),
            "skills/demo/scripts/run.sh".to_string(),
        ];
        let command_actions = vec![ParsedCommand::Unknown {
            cmd: "skills/demo/scripts/run.sh".to_string(),
        }];

        let resolved = resolve_skill_sandbox_extension_for_command(
            &outcome,
            true,
            &command,
            tempdir.path(),
            &command_actions,
        );

        assert_eq!(resolved, None);
    }

    #[test]
    fn does_not_resolve_policy_for_paths_outside_skill_scripts_directory() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skills/demo");
        std::fs::create_dir_all(&skill_dir).expect("create skill");
        std::fs::write(skill_dir.join("run.sh"), "#!/bin/sh\necho ok\n").expect("write script");
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_path, "skill").expect("write SKILL.md");

        let skill_policy = SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: true,
                readable_roots: vec![
                    AbsolutePathBuf::try_from(skill_dir.join("data")).expect("absolute"),
                ],
            },
        };
        let outcome = outcome_with_skills(vec![skill_with_policy(
            canonical(&skill_path),
            skill_policy,
        )]);
        let command = vec![
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            "skills/demo/run.sh".to_string(),
        ];
        let command_actions = vec![ParsedCommand::Unknown {
            cmd: "skills/demo/run.sh".to_string(),
        }];
        let resolved = resolve_skill_sandbox_extension_for_command(
            &outcome,
            true,
            &command,
            tempdir.path(),
            &command_actions,
        );

        assert_eq!(resolved, None);
    }

    #[test]
    fn ignores_disabled_skill_when_resolving_command_policy() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skills/demo");
        let scripts_dir = skill_dir.join("scripts");
        std::fs::create_dir_all(&scripts_dir).expect("create skill dir");
        std::fs::write(scripts_dir.join("tool.sh"), "#!/bin/sh\necho ok\n").expect("write script");
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_path, "skill").expect("write SKILL.md");
        let skill_path = canonical(&skill_path);

        let mut outcome = outcome_with_skills(vec![skill_with_policy(
            skill_path.clone(),
            SandboxPolicy::new_workspace_write_policy(),
        )]);
        outcome.disabled_paths.insert(skill_path);
        let command = vec![
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            "skills/demo/scripts/tool.sh".to_string(),
        ];
        let command_actions = vec![ParsedCommand::Unknown {
            cmd: "skills/demo/scripts/tool.sh".to_string(),
        }];

        let resolved = resolve_skill_sandbox_extension_for_command(
            &outcome,
            true,
            &command,
            tempdir.path(),
            &command_actions,
        );

        assert_eq!(resolved, None);
    }

    #[test]
    fn resolves_nested_skill_scripts_path() {
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

        let parent_policy = SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: false,
                readable_roots: vec![
                    AbsolutePathBuf::try_from(parent_skill_dir.join("data")).expect("absolute"),
                ],
            },
        };
        let nested_policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![
                AbsolutePathBuf::try_from(nested_skill_dir.join("output")).expect("absolute"),
            ],
            read_only_access: ReadOnlyAccess::FullAccess,
            network_access: true,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        };
        let outcome = outcome_with_skills(vec![
            skill_with_policy(canonical(&parent_skill_path), parent_policy),
            skill_with_policy(canonical(&nested_skill_path), nested_policy.clone()),
        ]);
        let command = vec![
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            "skills/parent/nested/scripts/run.sh".to_string(),
        ];
        let command_actions = vec![ParsedCommand::Unknown {
            cmd: "skills/parent/nested/scripts/run.sh".to_string(),
        }];

        let resolved = resolve_skill_sandbox_extension_for_command(
            &outcome,
            true,
            &command,
            tempdir.path(),
            &command_actions,
        );

        assert_eq!(resolved, Some(nested_policy));
    }

    #[test]
    fn ignores_non_path_unknown_command_actions() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skills/demo");
        std::fs::create_dir_all(skill_dir.join("scripts")).expect("create scripts");
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_path, "skill").expect("write SKILL.md");

        let outcome = outcome_with_skills(vec![skill_with_policy(
            canonical(&skill_path),
            SandboxPolicy::new_workspace_write_policy(),
        )]);
        let command = vec![
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            "echo hi".to_string(),
        ];
        let command_actions = vec![ParsedCommand::Unknown {
            cmd: "echo hi".to_string(),
        }];

        let resolved = resolve_skill_sandbox_extension_for_command(
            &outcome,
            true,
            &command,
            skill_dir.join("scripts").as_path(),
            &command_actions,
        );

        assert_eq!(resolved, None);
    }

    #[test]
    fn does_not_resolve_policy_when_shell_zsh_fork_is_disabled() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skills/demo");
        let scripts_dir = skill_dir.join("scripts");
        std::fs::create_dir_all(&scripts_dir).expect("create scripts");
        std::fs::write(scripts_dir.join("run.sh"), "#!/bin/sh\necho ok\n").expect("write script");
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_path, "skill").expect("write SKILL.md");
        let cwd = tempdir.path().to_path_buf();

        let outcome = outcome_with_skills(vec![skill_with_policy(
            canonical(&skill_path),
            SandboxPolicy::new_workspace_write_policy(),
        )]);
        let command = vec![
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            "skills/demo/scripts/run.sh".to_string(),
        ];
        let command_actions = vec![ParsedCommand::Unknown {
            cmd: "skills/demo/scripts/run.sh".to_string(),
        }];

        let resolved = resolve_skill_sandbox_extension_for_command(
            &outcome,
            false,
            &command,
            &cwd,
            &command_actions,
        );

        assert_eq!(resolved, None);
    }
}

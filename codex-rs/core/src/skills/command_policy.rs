use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::parse_command::ParsedCommand;
use codex_protocol::protocol::SandboxPolicy;
use dunce::canonicalize as canonicalize_path;

use crate::config::Permissions;
use crate::path_utils::normalize_for_path_comparison;
use crate::skills::SkillLoadOutcome;

/// Resolves the sandbox policy extension contributed by the first matching
/// skill for a command invocation.
///
/// Assumptions:
/// 1. `command_cwd` reflects the effective command target location.
/// 2. If `command_cwd` is contained by multiple skill directories, the first
///    enabled skill in `skills_outcome.skills` wins.
/// 3. If `command_cwd` does not match, each command action path is checked.
///
/// Returns `None` when no enabled skill with permissions matches
/// `command_cwd` or command action paths.
pub(crate) fn resolve_skill_sandbox_extension_for_command(
    skills_outcome: &SkillLoadOutcome,
    command_cwd: &Path,
    command_actions: &[ParsedCommand],
) -> Option<SandboxPolicy> {
    let command_cwd = normalize_candidate_path(command_cwd)?;
    if let Some(permissions) = match_skill_for_candidate(skills_outcome, command_cwd.as_path()) {
        return Some(permissions.sandbox_policy.get().clone());
    }

    for command_action in command_actions {
        let Some(action_candidate) =
            command_action_candidate_path(command_action, command_cwd.as_path())
        else {
            continue;
        };
        if let Some(permissions) = match_skill_for_candidate(skills_outcome, &action_candidate) {
            return Some(permissions.sandbox_policy.get().clone());
        }
    }

    None
}

fn command_action_candidate_path(
    command_action: &ParsedCommand,
    command_cwd: &Path,
) -> Option<PathBuf> {
    let action_path = match command_action {
        ParsedCommand::Read { path, .. } => Some(path.as_path()),
        ParsedCommand::ListFiles { path, .. } | ParsedCommand::Search { path, .. } => {
            path.as_deref().map(Path::new)
        }
        ParsedCommand::Unknown { .. } => None,
    }?;
    let action_path = if action_path.is_absolute() {
        action_path.to_path_buf()
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
        // Normalize before comparing so path containment checks are stable.
        let Some(skill_dir) = normalize_candidate_path(skill_dir) else {
            continue;
        };
        // The command cwd must be inside the skill directory.
        if !candidate.starts_with(&skill_dir) {
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
    use super::resolve_skill_sandbox_extension_for_command;
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

    #[test]
    fn resolves_skill_policy_when_cwd_is_inside_skill_directory() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skills/demo");
        let scripts_dir = skill_dir.join("scripts");
        std::fs::create_dir_all(&scripts_dir).expect("create scripts");
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_path, "skill").expect("write SKILL.md");

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

        let resolved = resolve_skill_sandbox_extension_for_command(&outcome, &scripts_dir, &[]);

        assert_eq!(resolved, Some(skill_policy));
    }

    #[test]
    fn does_not_resolve_policy_when_neither_cwd_nor_command_action_paths_match() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skills/demo");
        let outside_dir = tempdir.path().join("outside");
        std::fs::create_dir_all(&outside_dir).expect("create outside");
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
            skill_policy.clone(),
        )]);

        let resolved = resolve_skill_sandbox_extension_for_command(&outcome, &outside_dir, &[]);

        assert_eq!(resolved, None);
    }

    #[test]
    fn resolves_policy_when_command_action_path_is_inside_skill_directory() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skills/demo");
        let outside_dir = tempdir.path().join("outside");
        std::fs::create_dir_all(&outside_dir).expect("create outside");
        let scripts_dir = skill_dir.join("scripts");
        std::fs::create_dir_all(&scripts_dir).expect("create scripts");
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_path, "skill").expect("write SKILL.md");
        let skill_path = canonical(&skill_path);

        let skill_policy = SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: true,
                readable_roots: vec![
                    AbsolutePathBuf::try_from(skill_dir.join("data")).expect("absolute"),
                ],
            },
        };
        let outcome = outcome_with_skills(vec![skill_with_policy(
            skill_path.clone(),
            skill_policy.clone(),
        )]);

        let command_actions = vec![ParsedCommand::Read {
            cmd: format!("cat {}", skill_path.display()),
            name: "SKILL.md".to_string(),
            path: skill_path,
        }];
        let resolved =
            resolve_skill_sandbox_extension_for_command(&outcome, &outside_dir, &command_actions);

        assert_eq!(resolved, Some(skill_policy));
    }

    #[test]
    fn ignores_disabled_skill_when_resolving_command_policy() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skills/demo");
        std::fs::create_dir_all(&skill_dir).expect("create skill dir");
        std::fs::write(skill_dir.join("tool.sh"), "#!/bin/sh\necho ok\n").expect("write script");
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_path, "skill").expect("write SKILL.md");
        let skill_path = canonical(&skill_path);

        let mut outcome = outcome_with_skills(vec![skill_with_policy(
            skill_path.clone(),
            SandboxPolicy::new_workspace_write_policy(),
        )]);
        outcome.disabled_paths.insert(skill_path);

        let resolved = resolve_skill_sandbox_extension_for_command(&outcome, &skill_dir, &[]);

        assert_eq!(resolved, None);
    }

    #[test]
    fn resolves_first_matching_skill_directory_for_nested_match() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let parent_skill_dir = tempdir.path().join("skills/parent");
        let nested_skill_dir = parent_skill_dir.join("nested");
        std::fs::create_dir_all(nested_skill_dir.join("scripts")).expect("create scripts");

        std::fs::write(
            nested_skill_dir.join("scripts/run.sh"),
            "#!/bin/sh\necho ok\n",
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
            skill_with_policy(canonical(&parent_skill_path), parent_policy.clone()),
            skill_with_policy(canonical(&nested_skill_path), nested_policy.clone()),
        ]);

        let resolved =
            resolve_skill_sandbox_extension_for_command(&outcome, &nested_skill_dir, &[]);

        assert_eq!(resolved, Some(parent_policy));
    }
}

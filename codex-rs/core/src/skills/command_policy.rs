use std::collections::HashSet;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::protocol::SandboxPolicy;
use dirs::home_dir;
use dunce::canonicalize as canonicalize_path;

use crate::bash::parse_shell_lc_plain_commands;
use crate::bash::parse_shell_lc_single_command_prefix;
use crate::config::Permissions;
use crate::path_utils::normalize_for_path_comparison;
use crate::skills::SkillLoadOutcome;
use crate::skills::SkillMetadata;

pub(crate) fn resolve_skill_sandbox_extension_for_command(
    skills_outcome: &SkillLoadOutcome,
    command: &[String],
    command_cwd: &Path,
) -> Option<SandboxPolicy> {
    let segments = command_segments_for_matching(command);
    let mut best_match: Option<(usize, SandboxPolicy)> = None;

    for segment in segments {
        for candidate in candidate_paths_for_segment(&segment, command_cwd) {
            let Some((depth, _, permissions)) =
                match_skill_for_candidate(skills_outcome, candidate.as_path())
            else {
                continue;
            };

            let should_replace = match &best_match {
                Some((best_depth, _)) => depth > *best_depth,
                None => true,
            };
            if should_replace {
                best_match = Some((depth, permissions.sandbox_policy.get().clone()));
            }
        }
    }

    best_match.map(|(_, sandbox_policy)| sandbox_policy)
}

fn command_segments_for_matching(command: &[String]) -> Vec<Vec<String>> {
    if let Some(commands) = parse_shell_lc_plain_commands(command)
        && !commands.is_empty()
    {
        return commands;
    }

    if let Some(command) = parse_shell_lc_single_command_prefix(command) {
        return vec![command];
    }

    vec![command.to_vec()]
}

fn candidate_paths_for_segment(segment: &[String], command_cwd: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    if let Some(path) = normalize_candidate_path(command_cwd)
        && seen.insert(path.clone())
    {
        candidates.push(path);
    }

    for token in segment {
        let Some(path) = candidate_path_from_token(token, command_cwd) else {
            continue;
        };
        if seen.insert(path.clone()) {
            candidates.push(path);
        }
    }

    candidates
}

fn candidate_path_from_token(token: &str, command_cwd: &Path) -> Option<PathBuf> {
    if token.is_empty() || token.contains("://") || token.starts_with('-') {
        return None;
    }

    let is_path_like = token == "~"
        || token.starts_with("~/")
        || token.starts_with("./")
        || token.starts_with("../")
        || token.contains('/')
        || token.contains('\\')
        || Path::new(token).is_absolute()
        || command_cwd.join(token).exists();
    if !is_path_like {
        return None;
    }

    let expanded = expand_home(token);
    let path = PathBuf::from(expanded);
    let absolute = if path.is_absolute() {
        path
    } else {
        command_cwd.join(path)
    };
    normalize_candidate_path(&absolute)
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
) -> Option<(usize, &'a SkillMetadata, &'a Permissions)> {
    let mut best_match: Option<(usize, &SkillMetadata, &Permissions)> = None;

    for skill in &skills_outcome.skills {
        if skills_outcome.disabled_paths.contains(&skill.path) {
            continue;
        }
        let Some(permissions) = skill.permissions.as_ref() else {
            continue;
        };
        let Some(skill_dir) = skill.path.parent() else {
            continue;
        };
        let Some(skill_dir) = normalize_candidate_path(skill_dir) else {
            continue;
        };
        if !candidate.starts_with(&skill_dir) {
            continue;
        }

        let depth = skill_dir.components().count();
        let should_replace = match &best_match {
            Some((best_depth, _, _)) => depth > *best_depth,
            None => true,
        };
        if should_replace {
            best_match = Some((depth, skill, permissions));
        }
    }

    best_match
}

fn expand_home(path: &str) -> String {
    if path == "~" {
        if let Some(home) = home_dir() {
            return home.to_string_lossy().to_string();
        }
        return path.to_string();
    }
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = home_dir()
    {
        return home.join(rest).to_string_lossy().to_string();
    }
    path.to_string()
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
    fn resolves_skill_policy_for_executable_inside_skill_directory() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skills/demo");
        let scripts_dir = skill_dir.join("scripts");
        std::fs::create_dir_all(&scripts_dir).expect("create scripts");
        let executable = scripts_dir.join("run.sh");
        std::fs::write(&executable, "#!/bin/sh\necho ok\n").expect("write script");
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

        let resolved = resolve_skill_sandbox_extension_for_command(
            &outcome,
            &[canonical(&executable).to_string_lossy().to_string()],
            tempdir.path(),
        );

        assert_eq!(resolved, Some(skill_policy));
    }

    #[test]
    fn resolves_skill_policy_for_shell_wrapped_relative_script_command() {
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
            skill_policy.clone(),
        )]);

        let resolved = resolve_skill_sandbox_extension_for_command(
            &outcome,
            &[
                "bash".to_string(),
                "-lc".to_string(),
                "./scripts/run.sh --flag".to_string(),
            ],
            &skill_dir,
        );

        assert_eq!(resolved, Some(skill_policy));
    }

    #[test]
    fn ignores_disabled_skill_when_resolving_command_policy() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let skill_dir = tempdir.path().join("skills/demo");
        std::fs::create_dir_all(&skill_dir).expect("create skill dir");
        let executable = skill_dir.join("tool.sh");
        std::fs::write(&executable, "#!/bin/sh\necho ok\n").expect("write script");
        let skill_path = skill_dir.join("SKILL.md");
        std::fs::write(&skill_path, "skill").expect("write SKILL.md");
        let skill_path = canonical(&skill_path);

        let mut outcome = outcome_with_skills(vec![skill_with_policy(
            skill_path.clone(),
            SandboxPolicy::new_workspace_write_policy(),
        )]);
        outcome.disabled_paths.insert(skill_path);

        let resolved = resolve_skill_sandbox_extension_for_command(
            &outcome,
            &[canonical(&executable).to_string_lossy().to_string()],
            tempdir.path(),
        );

        assert_eq!(resolved, None);
    }

    #[test]
    fn prefers_most_specific_skill_directory_for_nested_match() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let parent_skill_dir = tempdir.path().join("skills/parent");
        let nested_skill_dir = parent_skill_dir.join("nested");
        std::fs::create_dir_all(nested_skill_dir.join("scripts")).expect("create scripts");

        let executable = nested_skill_dir.join("scripts/run.sh");
        std::fs::write(&executable, "#!/bin/sh\necho ok\n").expect("write script");

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

        let resolved = resolve_skill_sandbox_extension_for_command(
            &outcome,
            &[canonical(&executable).to_string_lossy().to_string()],
            tempdir.path(),
        );

        assert_eq!(resolved, Some(nested_policy));
    }
}

use codex_analytics::InvocationType;
use codex_analytics::SkillInvocation;
use codex_analytics::SkillInvocationLocation;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_extension_api::ExtensionData;
use codex_skills::ImplicitSkillAccess;
use codex_skills::detect_implicit_skill_invocation_for_command;
use codex_skills::implicit_skill_accesses_for_command;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;

use crate::HostSkillsSnapshot;
use crate::catalog::SkillSourceKind;
use crate::state::ExecutorSkillsStepState;

/// Identifies the executor-owned or host-owned skill referenced by a command.
pub fn detect_implicit_skill_invocation(
    turn_store: &ExtensionData,
    environment_id: &str,
    command: &str,
    workdir: &PathUri,
    native_workdir: Option<&AbsolutePathBuf>,
) -> Option<SkillInvocation> {
    if environment_id == LOCAL_ENVIRONMENT_ID
        && let Some(native_workdir) = native_workdir
        && let Some(host_snapshot) = turn_store.get::<HostSkillsSnapshot>()
        && let Some(skill) = detect_implicit_skill_invocation_for_command(
            host_snapshot.outcome(),
            command,
            native_workdir,
        )
    {
        return Some(SkillInvocation {
            skill_name: skill.name,
            location: SkillInvocationLocation::Host {
                path: skill.path_to_skills_md.to_path_buf(),
                scope: skill.scope,
            },
            plugin_id: skill.plugin_id,
            remote_plugin_id: skill.remote_plugin_id,
            invocation_type: InvocationType::Implicit,
        });
    }

    let catalog = turn_store.get::<ExecutorSkillsStepState>()?;
    for access in implicit_skill_accesses_for_command(command, workdir) {
        for entry in &catalog.0.entries {
            let Some((skill_environment_id, skill_path)) = entry.main_prompt.environment_path()
            else {
                continue;
            };
            if !entry.enabled
                || entry.authority.kind != SkillSourceKind::Executor
                || skill_environment_id != environment_id
            {
                continue;
            }

            let matches = match &access {
                ImplicitSkillAccess::Document(path) => path == skill_path,
                ImplicitSkillAccess::Script(path) => skill_path
                    .parent()
                    .and_then(|skill_dir| skill_dir.join("scripts").ok())
                    .is_some_and(|scripts_dir| path.starts_with(&scripts_dir)),
            };
            if matches {
                return Some(SkillInvocation {
                    skill_name: entry.name.clone(),
                    location: SkillInvocationLocation::Resource {
                        id: entry.main_prompt.as_str().to_owned(),
                        skill_id: entry.canonical_skill_id.clone(),
                        scope: entry.analytics_scope,
                    },
                    plugin_id: entry.plugin_id.clone(),
                    remote_plugin_id: None,
                    invocation_type: InvocationType::Implicit,
                });
            }
        }
    }

    None
}

#[cfg(test)]
#[path = "invocation_tests.rs"]
mod tests;

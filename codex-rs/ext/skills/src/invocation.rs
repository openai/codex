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
use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillResourceId;
use crate::catalog::SkillSourceKind;
use crate::catalog::SkillSourceLocation;

impl SkillCatalogEntry {
    /// Uses merged identity and scope, with plugin attribution from the route actually used.
    pub(crate) fn invocation(
        &self,
        source: &SkillSourceLocation,
        invocation_type: InvocationType,
    ) -> SkillInvocation {
        SkillInvocation {
            skill_name: self.name.clone(),
            location: SkillInvocationLocation::Resource {
                id: self.main_prompt.as_str().to_owned(),
                skill_id: self.canonical_skill_id.clone(),
                scope: self.analytics_scope,
            },
            plugin_id: source.plugin_id.clone(),
            remote_plugin_id: self.remote_plugin_id.clone(),
            invocation_type,
        }
    }
}

/// Identifies the executor-owned or host-owned skill referenced by a command.
pub fn detect_implicit_skill_invocation(
    turn_store: &ExtensionData,
    step_store: &ExtensionData,
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
                path: skill.path_to_skills_md,
                scope: skill.scope,
            },
            plugin_id: skill.plugin_id,
            remote_plugin_id: skill.remote_plugin_id,
            invocation_type: InvocationType::Implicit,
        });
    }

    let accesses = implicit_skill_accesses_for_command(command, workdir);
    if accesses.is_empty() {
        return None;
    }
    let step_state = step_store.get::<crate::step_state::SkillsStepState>()?;
    let step_catalog = &step_state.catalog;
    for access in accesses {
        let matches_route = |authority: &SkillAuthority, main_prompt: &SkillResourceId| {
            if authority.kind != SkillSourceKind::Executor
                || main_prompt.environment_id() != Some(environment_id)
            {
                return false;
            }
            let Some((_, skill_path)) = main_prompt.environment_path() else {
                return false;
            };
            match &access {
                ImplicitSkillAccess::Document(path) => path == skill_path,
                ImplicitSkillAccess::Script(path) => skill_path
                    .parent()
                    .and_then(|skill_dir| skill_dir.join("scripts").ok())
                    .is_some_and(|scripts_dir| path.starts_with(&scripts_dir)),
            }
        };
        for entry in step_catalog.merged.entries.iter().filter(|entry| {
            matches!(
                entry.authority.kind,
                SkillSourceKind::Cloud | SkillSourceKind::Executor
            )
        }) {
            if entry.enabled && matches_route(&entry.authority, &entry.main_prompt) {
                return Some(entry.invocation(&entry.primary_source(), InvocationType::Implicit));
            }
            if let Some(source) = entry.sources.iter().find(|source| {
                source.enabled && matches_route(&source.authority, &source.main_prompt)
            }) {
                return Some(entry.invocation(source, InvocationType::Implicit));
            }
        }
    }

    None
}

#[cfg(test)]
#[path = "invocation_tests.rs"]
mod tests;

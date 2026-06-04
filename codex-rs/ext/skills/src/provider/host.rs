use codex_core_skills::HostLoadedSkills;
use codex_core_skills::SkillLoadOutcome;
use codex_core_skills::SkillMetadata;

use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillPackageId;
use crate::catalog::SkillProviderError;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillResourceId;
use crate::catalog::SkillSourceKind;
use crate::provider::SkillReadRequest;

const HOST_AUTHORITY_ID: &str = "host";

/// Host-owned skill provider backed by the already-loaded turn skills.
///
/// The provider intentionally does not reload or cache host skills. Core owns
/// skill loading, including plugin roots, runtime extra roots, and the primary
/// environment filesystem. This adapter only maps that loaded outcome into the
/// skills-extension catalog/read contract.
#[derive(Clone, Debug, Default)]
pub struct HostSkillProvider;

impl HostSkillProvider {
    pub fn new() -> Self {
        Self
    }

    pub fn list(&self, host_loaded_skills: &HostLoadedSkills) -> SkillCatalog {
        catalog_from_outcome(host_loaded_skills.outcome())
    }

    pub async fn read(
        &self,
        request: SkillReadRequest,
        host_loaded_skills: &HostLoadedSkills,
    ) -> Result<SkillReadResult, SkillProviderError> {
        let Some(skill) = host_loaded_skills.outcome().skills.iter().find(|skill| {
            let skill_path = skill.path_to_skills_md.to_string_lossy();
            skill_path == request.resource.0.as_str()
                || skill_path.replace('\\', "/") == request.resource.0
        }) else {
            return Err(SkillProviderError::new(format!(
                "host skill resource is not loaded: {}",
                request.resource.0
            )));
        };

        let contents = host_loaded_skills
            .read_skill_text(skill)
            .await
            .map_err(|err| {
                SkillProviderError::new(format!(
                    "failed to read host skill resource {}: {err}",
                    request.resource.0
                ))
            })?;

        Ok(SkillReadResult {
            resource: request.resource,
            contents,
        })
    }
}

fn catalog_from_outcome(outcome: &SkillLoadOutcome) -> SkillCatalog {
    let mut catalog = SkillCatalog {
        entries: Vec::new(),
        warnings: outcome
            .errors
            .iter()
            .map(|err| {
                format!(
                    "Failed to load skill at {}: {}",
                    err.path.display(),
                    err.message
                )
            })
            .collect(),
    };

    for (skill, enabled) in outcome.skills_with_enabled() {
        catalog.push_entry(catalog_entry_from_skill(skill, enabled));
    }

    catalog
}

fn catalog_entry_from_skill(skill: &SkillMetadata, enabled: bool) -> SkillCatalogEntry {
    let skill_path = skill.path_to_skills_md.to_string_lossy().into_owned();
    let display_path = skill_path.replace('\\', "/");
    let mut entry = SkillCatalogEntry::new(
        SkillPackageId(skill_path.clone()),
        SkillAuthority::new(SkillSourceKind::Host, HOST_AUTHORITY_ID),
        skill.name.clone(),
        skill.description.clone(),
        SkillResourceId(skill_path),
    )
    .with_short_description(skill.short_description.clone())
    .with_display_path(display_path)
    .with_dependencies(skill.dependencies.clone());

    if !enabled {
        entry = entry.disabled();
    }
    if !skill.allows_implicit_invocation() {
        entry = entry.hidden_from_prompt();
    }

    entry
}

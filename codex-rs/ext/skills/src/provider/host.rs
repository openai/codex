use std::path::PathBuf;
use std::sync::Arc;

use codex_core_skills::SkillLoadOutcome;
use codex_core_skills::SkillMetadata;
use codex_core_skills::SkillsManager;

use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillPackageId;
use crate::catalog::SkillProviderError;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillResourceId;
use crate::catalog::SkillSearchResult;
use crate::catalog::SkillSourceKind;
use crate::provider::SkillListQuery;
use crate::provider::SkillProvider;
use crate::provider::SkillProviderFuture;
use crate::provider::SkillReadRequest;
use crate::provider::SkillSearchRequest;

const HOST_AUTHORITY_ID: &str = "host";

/// Host-owned skill provider backed by the legacy skill loader.
///
/// The provider lists local host skills through [`SkillsManager`] and reads
/// `SKILL.md` bodies for explicit invocation. Other skill files remain ordinary
/// filesystem paths for the model to inspect through shell/unified exec.
#[derive(Clone, Default)]
pub struct HostSkillProvider {
    manager: Option<Arc<SkillsManager>>,
}

impl HostSkillProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_manager(manager: Arc<SkillsManager>) -> Self {
        Self {
            manager: Some(manager),
        }
    }
}

impl SkillProvider for HostSkillProvider {
    fn list(&self, query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        Box::pin(async move {
            let Some(host_query) = query.host else {
                return Err(SkillProviderError::new(
                    "host skill provider requires host skill configuration",
                ));
            };

            let local_manager;
            let manager = match self.manager.as_deref() {
                Some(manager) => manager,
                None => {
                    local_manager = SkillsManager::new(
                        host_query.codex_home.clone(),
                        host_query.input.bundled_skills_enabled,
                    );
                    &local_manager
                }
            };

            let outcome = manager
                .skills_for_config(&host_query.input, /*fs*/ None)
                .await;
            Ok(catalog_from_outcome(outcome))
        })
    }

    fn read(&self, request: SkillReadRequest) -> SkillProviderFuture<'_, SkillReadResult> {
        Box::pin(async move {
            let path = PathBuf::from(&request.resource.0);
            if !path.is_absolute() {
                return Err(SkillProviderError::new(format!(
                    "host skill resource is not an absolute path: {}",
                    request.resource.0
                )));
            }

            let contents = std::fs::read_to_string(&path).map_err(|err| {
                SkillProviderError::new(format!(
                    "failed to read host skill resource {}: {err}",
                    path.display()
                ))
            })?;

            Ok(SkillReadResult {
                resource: request.resource,
                contents,
            })
        })
    }

    fn search(&self, _request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult> {
        Box::pin(async { Ok(SkillSearchResult::default()) })
    }
}

fn catalog_from_outcome(outcome: SkillLoadOutcome) -> SkillCatalog {
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
    let mut entry = SkillCatalogEntry::new(
        SkillPackageId(skill_path.clone()),
        SkillAuthority::new(SkillSourceKind::Host, HOST_AUTHORITY_ID),
        skill.name.clone(),
        skill.description.clone(),
        SkillResourceId(skill_path.clone()),
    )
    .with_short_description(skill.short_description.clone())
    .with_display_path(skill_path)
    .with_dependencies(skill.dependencies.clone());

    if !enabled {
        entry = entry.disabled();
    }
    if !allows_prompt_invocation(skill) {
        entry = entry.hidden_from_prompt();
    }

    entry
}

fn allows_prompt_invocation(skill: &SkillMetadata) -> bool {
    skill
        .policy
        .as_ref()
        .and_then(|policy| policy.allow_implicit_invocation)
        .unwrap_or(true)
}

use std::collections::HashMap;

use crate::SkillLoadOutcome;
use codex_skills::SkillMetadata;
use codex_utils_path_uri::PathUri;

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
use crate::provider::SkillReadContext;
use crate::provider::SkillReadRequest;
use crate::provider::SkillSearchRequest;

const HOST_AUTHORITY_ID: &str = "host";

/// Host-owned skill provider backed by an immutable service snapshot.
///
/// Discovery and caching belong to `HostSkillsService`; this provider only maps a
/// snapshot into the authority-aware catalog/read contract.
#[derive(Clone, Default)]
pub struct HostSkillProvider;

impl HostSkillProvider {
    pub fn new() -> Self {
        Self
    }
}

impl SkillProvider for HostSkillProvider {
    fn list(&self, query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        Box::pin(async move {
            let Some(host_snapshot) = query.host_snapshot else {
                return Err(SkillProviderError::new(
                    "host skill provider requires a host skills snapshot",
                ));
            };

            Ok(catalog_from_outcome(host_snapshot.outcome()))
        })
    }

    fn read<'a>(
        &'a self,
        request: SkillReadRequest<'a>,
    ) -> SkillProviderFuture<'a, SkillReadResult> {
        Box::pin(async move {
            let SkillReadContext::Host {
                host_snapshot: Some(host_snapshot),
            } = request.context
            else {
                return Err(SkillProviderError::new(
                    "host skill provider requires a host skills snapshot",
                ));
            };
            // TODO(anp): Infer resource path conventions once foreign repository paths can reach this
            // provider. Despite its name, it also serves repository skills from the caller's environment;
            // the loader currently rejects foreign paths. Host-owned skills use the app-server's local
            // filesystem.
            let resource_path = PathUri::from_host_native_path(request.resource.as_str()).ok();
            let Some(skill) = host_snapshot
                .outcome()
                .skills
                .iter()
                .find(|skill| Some(&skill.path_to_skills_md) == resource_path.as_ref())
            else {
                return Err(SkillProviderError::new(format!(
                    "host skill resource is not loaded: {}",
                    request.resource.as_str()
                )));
            };

            let contents = host_snapshot.read_skill_text(skill).await.map_err(|err| {
                SkillProviderError::new(format!(
                    "failed to read host skill resource {}: {err}",
                    request.resource.as_str()
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

fn catalog_from_outcome(outcome: &SkillLoadOutcome) -> SkillCatalog {
    let root_order_by_path = outcome
        .skill_roots_in_discovery_order()
        .enumerate()
        .map(|(index, root)| (root.as_path(), index))
        .collect::<HashMap<_, _>>();
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
        let mut entry = catalog_entry_from_skill(skill, enabled);
        if let Some(discovery_path) =
            outcome.skill_discovery_path_for_path(&skill.path_to_skills_md)
        {
            // Keep the catalog's existing forward-slash display on Windows; lookups use PathUri.
            entry = entry.with_display_path(
                discovery_path
                    .inferred_native_path_string()
                    .replace('\\', "/"),
            );
        }
        if let Some(root) = outcome.skill_root_for_path(&skill.path_to_skills_md) {
            // Alias labels share the display path's separators so catalog rendering stays stable.
            entry = entry.with_alias_root(root.to_string_lossy().replace('\\', "/"));
            if let Some(root_order) = root_order_by_path.get(root.as_path()) {
                entry = entry.with_alias_root_order(*root_order);
            }
        }
        catalog.push_entry(entry);
    }

    catalog
}

fn catalog_entry_from_skill(skill: &SkillMetadata, enabled: bool) -> SkillCatalogEntry {
    let skill_path = skill.path_to_skills_md.inferred_native_path_string();
    // Preserve model-visible paths while resource IDs retain the native spelling used by clients.
    let display_path = skill_path.replace('\\', "/");
    let mut entry = SkillCatalogEntry::new(
        SkillPackageId(skill_path.clone()),
        SkillAuthority::new(SkillSourceKind::Host, HOST_AUTHORITY_ID),
        skill.name.clone(),
        skill.description.clone(),
        SkillResourceId::new(skill_path),
    )
    .with_short_description(skill.short_description.clone())
    .with_display_path(display_path)
    .with_prompt_scope(skill.scope)
    .with_dependencies(skill.dependencies.clone());

    if !enabled {
        entry = entry.disabled();
    }
    if !skill.allows_implicit_invocation() {
        entry = entry.hidden_from_prompt();
    }

    entry
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;

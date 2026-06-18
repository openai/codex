use std::sync::Arc;
use std::sync::Mutex;

use codex_core_plugins::PluginLoadOutcome;
use codex_core_skills::HostSkillsSnapshot;
use codex_core_skills::SkillLoadOutcome;
use codex_core_skills::SkillMetadata;
use codex_core_skills::SkillsService;
use codex_exec_server::ExecutorFileSystem;
use codex_protocol::protocol::Product;

use crate::HostSkillsConfig;
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

/// Host-owned skill provider backed by `SkillsService` snapshots.
#[derive(Clone, Default)]
pub struct HostSkillProvider {
    service: Option<Arc<SkillsService>>,
    managed_service: Arc<Mutex<Option<ManagedSkillsService>>>,
}

struct ManagedSkillsService {
    key: HostServiceKey,
    service: Arc<SkillsService>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct HostServiceKey {
    codex_home: codex_utils_absolute_path::AbsolutePathBuf,
    restriction_product: Option<Product>,
    bundled_skills_enabled: bool,
}

impl HostSkillProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_service(service: Arc<SkillsService>) -> Self {
        Self {
            service: Some(service),
            managed_service: Arc::default(),
        }
    }

    pub(crate) async fn snapshot_for_turn(
        &self,
        config: &HostSkillsConfig,
        restriction_product: Option<Product>,
        plugins: Option<&PluginLoadOutcome>,
        fs: Option<Arc<dyn ExecutorFileSystem>>,
    ) -> HostSkillsSnapshot {
        let service = self.service.as_ref().cloned().unwrap_or_else(|| {
            let key = HostServiceKey {
                codex_home: config.codex_home.clone(),
                restriction_product,
                bundled_skills_enabled: config.load_input.bundled_skills_enabled,
            };
            let mut managed_service = self
                .managed_service
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(current) = managed_service.as_ref()
                && current.key == key
            {
                return Arc::clone(&current.service);
            }
            let service = Arc::new(SkillsService::new_with_restriction_product(
                config.codex_home.clone(),
                config.load_input.bundled_skills_enabled,
                restriction_product,
            ));
            *managed_service = Some(ManagedSkillsService {
                key,
                service: Arc::clone(&service),
            });
            service
        });
        let mut load_input = config.load_input.clone();
        load_input.effective_skill_roots = plugins
            .map(PluginLoadOutcome::effective_plugin_skill_roots)
            .unwrap_or_default();
        service.snapshot_for_config(&load_input, fs).await
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

    fn read(&self, request: SkillReadRequest) -> SkillProviderFuture<'_, SkillReadResult> {
        Box::pin(async move {
            let Some(host_snapshot) = request.host_snapshot else {
                return Err(SkillProviderError::new(
                    "host skill provider requires a host skills snapshot",
                ));
            };
            let Some(skill) = host_snapshot.outcome().skills.iter().find(|skill| {
                let skill_path = skill.path_to_skills_md.to_string_lossy();
                skill_path == request.resource.as_str()
                    || skill_path.replace('\\', "/") == request.resource.as_str()
            }) else {
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
        SkillResourceId::new(skill_path),
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

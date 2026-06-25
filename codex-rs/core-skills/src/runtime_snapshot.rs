use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_protocol::protocol::Product;
use codex_protocol::user_input::UserInput;

use crate::AvailableSkills;
use crate::ExecutorSkillCatalogCache;
use crate::HostSkillsSnapshot;
use crate::SkillMetadata;
use crate::collect_runtime_skill_mentions;
use crate::default_skill_metadata_budget;
use crate::executor_runtime::ExecutorSkillSource;
use crate::render::SkillRenderSideEffects;
use crate::render::build_available_skills_from_catalog;
use crate::runtime::SkillAuthority;
use crate::runtime::SkillCatalog;
use crate::runtime::SkillCatalogEntry;
use crate::runtime::SkillPackageId;
use crate::runtime::SkillReadRequest;
use crate::runtime::SkillReadResult;
use crate::runtime::SkillResourceId;
use crate::runtime::SkillSource;
use crate::runtime::SkillSourceError;
use crate::runtime::SkillSourceFuture;
use crate::runtime::SkillSourceIdentity;
use crate::runtime::SkillSourceKind;
use crate::runtime::SkillSources;

const HOST_AUTHORITY_ID: &str = "host";

type EntryKey = (SkillAuthority, SkillPackageId);

/// Identity of one injected package and the runtime owner that supplied it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SkillInjectionIdentity {
    source: SkillSourceIdentity,
    authority: SkillAuthority,
    package: SkillPackageId,
}

#[derive(Debug)]
pub struct RuntimeSkillInjection {
    pub identity: SkillInjectionIdentity,
    pub entry: SkillCatalogEntry,
    pub contents: String,
}

#[derive(Debug, Default)]
pub struct RuntimeSkillInjections {
    pub items: Vec<RuntimeSkillInjection>,
    pub warnings: Vec<String>,
}

/// One immutable, authority-aware skill view used by a model sampling step.
#[derive(Clone)]
pub struct SkillsSnapshot {
    host: HostSkillsSnapshot,
    catalog: Arc<SkillCatalog>,
    sources: Arc<HashMap<EntryKey, Arc<dyn SkillSource>>>,
    available: Arc<Option<AvailableSkills>>,
    warnings: Arc<Vec<String>>,
}

impl SkillsSnapshot {
    pub fn from_host(host: HostSkillsSnapshot, context_window: Option<i64>) -> Self {
        let source: Arc<dyn SkillSource> = Arc::new(HostSkillSource::new(host.clone()));
        let catalog = host_catalog(&host);
        let source_by_entry = catalog
            .entries
            .iter()
            .map(|entry| {
                (
                    (entry.authority.clone(), entry.id.clone()),
                    Arc::clone(&source),
                )
            })
            .collect();
        Self::new(host, catalog, source_by_entry, context_window)
    }

    pub async fn load(
        host: HostSkillsSnapshot,
        executor_catalog_cache: &ExecutorSkillCatalogCache,
        executor_roots: &[ResolvedSelectedCapabilityRoot],
        extra_sources: Option<&SkillSources>,
        restriction_product: Option<Product>,
        context_window: Option<i64>,
    ) -> Self {
        let host_source: Arc<dyn SkillSource> = Arc::new(HostSkillSource::new(host.clone()));
        let mut catalog = SkillCatalog::default();
        let mut source_by_entry = HashMap::new();
        merge_bound_catalog(
            &mut catalog,
            &mut source_by_entry,
            &host_catalog(&host),
            &host_source,
        );
        for root in executor_roots {
            let cached = executor_catalog_cache
                .catalog_for_stable_root(root, restriction_product)
                .await;
            let executor_source = Arc::new(ExecutorSkillSource::new(
                root.clone(),
                restriction_product,
                cached.identity(),
            ));
            let source: Arc<dyn SkillSource> = executor_source;
            merge_bound_catalog(
                &mut catalog,
                &mut source_by_entry,
                cached.catalog(),
                &source,
            );
        }
        if let Some(extra_sources) = extra_sources {
            let (extra_catalog, extra_source_by_entry) = extra_sources.list_with_sources().await;
            catalog.warnings.extend(extra_catalog.warnings);
            for entry in extra_catalog.entries {
                let key = (entry.authority.clone(), entry.id.clone());
                if let std::collections::hash_map::Entry::Vacant(route) =
                    source_by_entry.entry(key.clone())
                    && let Some(source) = extra_source_by_entry.get(&key)
                {
                    route.insert(Arc::clone(source));
                    catalog.entries.push(entry);
                }
            }
        }

        Self::new(host, catalog, source_by_entry, context_window)
    }

    fn new(
        host: HostSkillsSnapshot,
        catalog: SkillCatalog,
        source_by_entry: HashMap<EntryKey, Arc<dyn SkillSource>>,
        context_window: Option<i64>,
    ) -> Self {
        let available = build_available_skills_from_catalog(
            &catalog,
            Some(host.outcome()),
            default_skill_metadata_budget(context_window),
            SkillRenderSideEffects::None,
        );
        let mut warnings = catalog.warnings.clone();
        if let Some(warning) = available
            .as_ref()
            .and_then(|available| available.warning_message.clone())
        {
            warnings.push(warning);
        }
        Self {
            host,
            catalog: Arc::new(catalog),
            sources: Arc::new(source_by_entry),
            available: Arc::new(available),
            warnings: Arc::new(warnings),
        }
    }

    pub fn available(&self) -> Option<&AvailableSkills> {
        self.available.as_ref().as_ref()
    }

    pub fn warnings(&self) -> &[String] {
        self.warnings.as_ref()
    }

    pub fn skill_name_counts_lower(&self) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for entry in self.catalog.entries.iter().filter(|entry| entry.enabled) {
            *counts.entry(entry.name.to_ascii_lowercase()).or_default() += 1;
        }
        counts
    }

    pub fn host_skill(&self, entry: &SkillCatalogEntry) -> Option<&SkillMetadata> {
        if entry.authority.kind != SkillSourceKind::Host {
            return None;
        }
        self.host
            .outcome()
            .skills
            .iter()
            .find(|skill| skill.path_to_skills_md.to_string_lossy() == entry.main_prompt.as_str())
    }

    pub async fn injections(
        &self,
        input: &[UserInput],
        plain_name_conflicts: &HashSet<String>,
        active_skills: &HashMap<String, SkillInjectionIdentity>,
        remaining_items: usize,
    ) -> RuntimeSkillInjections {
        let selected = collect_runtime_skill_mentions(input, &self.catalog, plain_name_conflicts);
        let mut result = RuntimeSkillInjections::default();
        for entry in &selected {
            let key = (entry.authority.clone(), entry.id.clone());
            let Some(source) = self.sources.get(&key) else {
                result.warnings.push(format!(
                    "Failed to load skill `{}`: its runtime source is unavailable.",
                    entry.name
                ));
                continue;
            };
            let identity = SkillInjectionIdentity {
                source: source.identity(),
                authority: entry.authority.clone(),
                package: entry.id.clone(),
            };
            if active_skills.get(&entry.name) == Some(&identity) {
                continue;
            }
            if result.items.len() == remaining_items {
                result.warnings.push(format!(
                    "Only the first {remaining_items} newly selected skills were loaded because this turn reached its skill instruction limit."
                ));
                break;
            }
            let request = SkillReadRequest {
                authority: entry.authority.clone(),
                package: entry.id.clone(),
                resource: entry.main_prompt.clone(),
            };
            match source.read(request).await {
                Ok(read) => result.items.push(RuntimeSkillInjection {
                    identity,
                    entry: entry.clone(),
                    contents: read.contents,
                }),
                Err(err) => result
                    .warnings
                    .push(format!("Failed to load skill `{}`: {err}", entry.name)),
            }
        }
        result
    }

    pub fn contains_injection(&self, identity: &SkillInjectionIdentity) -> bool {
        let key = (identity.authority.clone(), identity.package.clone());
        self.catalog
            .entries
            .iter()
            .any(|entry| entry.enabled && entry.authority == key.0 && entry.id == key.1)
            && self
                .sources
                .get(&key)
                .is_some_and(|source| source.identity() == identity.source)
    }
}

fn merge_bound_catalog(
    catalog: &mut SkillCatalog,
    source_by_entry: &mut HashMap<EntryKey, Arc<dyn SkillSource>>,
    source_catalog: &SkillCatalog,
    source: &Arc<dyn SkillSource>,
) {
    catalog
        .warnings
        .extend(source_catalog.warnings.iter().cloned());
    for entry in &source_catalog.entries {
        let key = (entry.authority.clone(), entry.id.clone());
        if let std::collections::hash_map::Entry::Vacant(route) = source_by_entry.entry(key) {
            route.insert(Arc::clone(source));
            catalog.entries.push(entry.clone());
        }
    }
}

impl fmt::Debug for SkillsSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SkillsSnapshot")
            .field("catalog", &self.catalog)
            .field("available", &self.available)
            .finish_non_exhaustive()
    }
}

struct HostSkillSource {
    snapshot: HostSkillsSnapshot,
    identity: SkillSourceIdentity,
}

impl HostSkillSource {
    fn new(snapshot: HostSkillsSnapshot) -> Self {
        Self {
            identity: SkillSourceIdentity::from_owner(snapshot.outcome_arc()),
            snapshot,
        }
    }
}

impl SkillSource for HostSkillSource {
    fn identity(&self) -> SkillSourceIdentity {
        self.identity.clone()
    }

    fn list(&self) -> SkillSourceFuture<'_, SkillCatalog> {
        Box::pin(async move { Ok(host_catalog(&self.snapshot)) })
    }

    fn read(&self, request: SkillReadRequest) -> SkillSourceFuture<'_, SkillReadResult> {
        Box::pin(async move {
            if request.authority != SkillAuthority::new(SkillSourceKind::Host, HOST_AUTHORITY_ID) {
                return Err(SkillSourceError::new("host skill authority does not match"));
            }
            let Some(skill) = self.snapshot.outcome().skills.iter().find(|skill| {
                skill.path_to_skills_md.to_string_lossy() == request.resource.as_str()
            }) else {
                return Err(SkillSourceError::new(format!(
                    "host skill resource is not loaded: {}",
                    request.resource.as_str()
                )));
            };
            let contents = self.snapshot.read_skill_text(skill).await.map_err(|err| {
                SkillSourceError::new(format!(
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
}

fn host_catalog(snapshot: &HostSkillsSnapshot) -> SkillCatalog {
    let outcome = snapshot.outcome();
    let mut catalog = SkillCatalog {
        warnings: outcome
            .errors
            .iter()
            .map(|error| {
                format!(
                    "Failed to load skill at {}: {}",
                    error.path.display(),
                    error.message
                )
            })
            .collect(),
        ..Default::default()
    };
    for (skill, enabled) in outcome.skills_with_enabled() {
        let path = skill.path_to_skills_md.to_string_lossy().into_owned();
        let mut entry = SkillCatalogEntry::new(
            SkillPackageId(path.clone()),
            SkillAuthority::new(SkillSourceKind::Host, HOST_AUTHORITY_ID),
            skill.name.clone(),
            skill.description.clone(),
            SkillResourceId::new(path.clone()),
        )
        .with_short_description(skill.short_description.clone())
        .with_display_path(path.replace('\\', "/"))
        .with_dependencies(skill.dependencies.clone());
        if !enabled {
            entry = entry.disabled();
        }
        if !skill.allows_implicit_invocation() {
            entry = entry.hidden_from_prompt();
        }
        catalog.push_entry(entry);
    }
    catalog
}

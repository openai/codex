use std::sync::Arc;
use std::sync::Mutex;

use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::protocol::Product;

use crate::loader::EnvironmentSkillMetadata;
use crate::loader::load_environment_skills_from_root;
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

/// Session-lifetime cache for catalogs discovered from stable executor capability roots.
///
/// Cache entries are keyed by the complete selected root and product restriction, not by the
/// process-local [`ResolvedSelectedCapabilityRoot`]. Environment availability therefore controls
/// whether a cached catalog is projected into a model step, but temporarily losing an environment
/// does not invalidate its catalog. A different root identity, environment ID, path, or product
/// restriction produces a cache miss and a new discovery.
/// The entry also owns the source identity used for injection deduplication, so reconnecting the
/// same stable environment does not make an unchanged skill look like a different instruction.
///
/// There is intentionally no filesystem-based invalidation. Selected environment contents are
/// treated as stable for the lifetime of a session. Dropping this cache at session shutdown is the
/// only operation that invalidates successful entries. Catalogs containing warnings are not
/// cached, so transient discovery failures can recover on a later model step.
#[derive(Default)]
pub struct ExecutorSkillCatalogCache {
    entries: Mutex<Vec<CachedExecutorSkillCatalog>>,
}

#[derive(Clone)]
pub(crate) struct CachedExecutorSkillCatalog {
    key: ExecutorSkillCatalogCacheKey,
    catalog: Arc<SkillCatalog>,
    identity: SkillSourceIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExecutorSkillCatalogCacheKey {
    selected_root: SelectedCapabilityRoot,
    restriction_product: Option<Product>,
}

impl ExecutorSkillCatalogCache {
    pub(crate) async fn catalog_for_stable_root(
        &self,
        root: &ResolvedSelectedCapabilityRoot,
        restriction_product: Option<Product>,
    ) -> CachedExecutorSkillCatalog {
        let key = ExecutorSkillCatalogCacheKey {
            selected_root: root.selected_root().clone(),
            restriction_product,
        };
        if let Some(cached) = self.catalog(&key) {
            return cached;
        }

        // Do not hold the cache lock across executor I/O. Concurrent first loads may duplicate
        // discovery, but the second check below ensures only one result becomes authoritative.
        let identity = SkillSourceIdentity::from_owner(Arc::new(key.clone()));
        let source = ExecutorSkillSource::new(root.clone(), restriction_product, identity.clone());
        let discovered = Arc::new(source.load_catalog().await);
        let discovered = CachedExecutorSkillCatalog {
            key: key.clone(),
            catalog: discovered,
            identity,
        };
        if !discovered.warnings().is_empty() {
            return discovered;
        }
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(entry) = entries.iter().find(|entry| entry.key == key) {
            return entry.clone();
        }
        entries.push(discovered.clone());
        discovered
    }

    fn catalog(&self, key: &ExecutorSkillCatalogCacheKey) -> Option<CachedExecutorSkillCatalog> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .find(|entry| &entry.key == key)
            .cloned()
    }
}

impl CachedExecutorSkillCatalog {
    pub(crate) fn catalog(&self) -> &SkillCatalog {
        self.catalog.as_ref()
    }

    pub(crate) fn identity(&self) -> SkillSourceIdentity {
        self.identity.clone()
    }

    fn warnings(&self) -> &[String] {
        &self.catalog.warnings
    }
}

pub(crate) struct ExecutorSkillSource {
    root: ResolvedSelectedCapabilityRoot,
    authority: SkillAuthority,
    restriction_product: Option<Product>,
    identity: SkillSourceIdentity,
}

impl ExecutorSkillSource {
    pub(crate) fn new(
        root: ResolvedSelectedCapabilityRoot,
        restriction_product: Option<Product>,
        identity: SkillSourceIdentity,
    ) -> Self {
        Self {
            authority: SkillAuthority::new(
                SkillSourceKind::Executor,
                root.selected_root().id.clone(),
            ),
            identity,
            root,
            restriction_product,
        }
    }

    async fn load_catalog(&self) -> SkillCatalog {
        let CapabilityRootLocation::Environment {
            environment_id,
            path,
        } = &self.root.selected_root().location;
        let outcome = load_environment_skills_from_root(
            self.root.file_system().as_ref(),
            path,
            self.restriction_product,
        )
        .await;
        let mut catalog = SkillCatalog {
            warnings: outcome.warnings,
            ..Default::default()
        };
        for skill in outcome.skills {
            catalog.push_entry(executor_catalog_entry(
                skill,
                self.authority.clone(),
                &self.root.selected_root().id,
                environment_id,
            ));
        }
        catalog
    }
}

impl SkillSource for ExecutorSkillSource {
    fn identity(&self) -> SkillSourceIdentity {
        self.identity.clone()
    }

    fn list(&self) -> SkillSourceFuture<'_, SkillCatalog> {
        Box::pin(async move { Ok(self.load_catalog().await) })
    }

    fn read(&self, request: SkillReadRequest) -> SkillSourceFuture<'_, SkillReadResult> {
        Box::pin(async move {
            if request.authority != self.authority || request.package.0 != request.resource.as_str()
            {
                return Err(SkillSourceError::new(
                    "executor skill resource does not match its captured source",
                ));
            }
            let CapabilityRootLocation::Environment { environment_id, .. } =
                &self.root.selected_root().location;
            let Some((resource_environment, path)) = request.resource.environment_path() else {
                return Err(SkillSourceError::new(
                    "executor skill resource has no environment path",
                ));
            };
            if resource_environment != environment_id {
                return Err(SkillSourceError::new(
                    "executor skill resource belongs to a different environment",
                ));
            }
            let contents = self
                .root
                .file_system()
                .read_file_text(path, /*sandbox*/ None)
                .await
                .map_err(|err| {
                    SkillSourceError::new(format!(
                        "failed to read executor skill resource {}: {err}",
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

fn executor_catalog_entry(
    skill: EnvironmentSkillMetadata,
    authority: SkillAuthority,
    root_id: &str,
    environment_id: &str,
) -> SkillCatalogEntry {
    let prompt_visible = skill.allows_implicit_invocation();
    let path = skill.path_to_skills_md.inferred_native_path_string();
    let display_path = format!(
        "skill://{root_id}/{}",
        path.replace('\\', "/").trim_start_matches('/')
    );
    let entry = SkillCatalogEntry::new(
        SkillPackageId(display_path.clone()),
        authority,
        skill.name,
        skill.description,
        SkillResourceId::environment(
            display_path.clone(),
            environment_id,
            skill.path_to_skills_md,
        ),
    )
    .with_short_description(skill.short_description)
    .with_display_path(display_path)
    .with_dependencies(skill.dependencies);
    if prompt_visible {
        entry
    } else {
        entry.hidden_from_prompt()
    }
}

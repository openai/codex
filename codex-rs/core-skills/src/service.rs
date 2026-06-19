use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;
use std::sync::RwLock;

use codex_app_server_protocol::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigLayerStackOrdering;
use codex_exec_server::ExecutorFileSystem;
use codex_protocol::protocol::Product;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_plugins::PluginSkillRoot;
use tracing::info;
use tracing::instrument;
use tracing::warn;

use crate::HostSkillsSnapshot;
use crate::PluginSkillSnapshots;
use crate::SkillLoadOutcome;
use crate::build_implicit_skill_path_indexes;
use crate::config_rules::SkillConfigRules;
use crate::config_rules::resolve_disabled_skill_paths;
use crate::config_rules::skill_config_rules_from_stack;
use crate::loader::SkillRoot;
use crate::loader::load_skills_from_roots;
use crate::loader::skill_roots;
use crate::system::install_system_skills;
use crate::system::uninstall_system_skills;
use codex_config::SkillsConfig;

#[derive(Debug, Clone)]
pub struct SkillsLoadInput {
    pub cwd: AbsolutePathBuf,
    pub effective_skill_roots: Vec<PluginSkillRoot>,
    pub config_layer_stack: ConfigLayerStack,
    pub bundled_skills_enabled: bool,
    plugin_skill_snapshots: Option<PluginSkillSnapshots>,
}

impl SkillsLoadInput {
    pub fn new(
        cwd: AbsolutePathBuf,
        effective_skill_roots: Vec<PluginSkillRoot>,
        config_layer_stack: ConfigLayerStack,
        bundled_skills_enabled: bool,
    ) -> Self {
        Self {
            cwd,
            effective_skill_roots,
            config_layer_stack,
            bundled_skills_enabled,
            plugin_skill_snapshots: None,
        }
    }

    /// Attaches plugin skill snapshots parsed during plugin loading, when available.
    pub fn with_plugin_skill_snapshots(
        mut self,
        plugin_skill_snapshots: Option<PluginSkillSnapshots>,
    ) -> Self {
        self.plugin_skill_snapshots = plugin_skill_snapshots;
        self
    }
}

/// Owns host skill discovery, immutable snapshots, cache invalidation, and extra roots.
///
/// Source-specific model exposure remains the responsibility of the skills extension.
pub struct SkillsService {
    codex_home: AbsolutePathBuf,
    restriction_product: Option<Product>,
    extra_roots: RwLock<Vec<AbsolutePathBuf>>,
    cache_by_cwd: RwLock<HashMap<AbsolutePathBuf, HostSkillsSnapshot>>,
    cache_by_config: RwLock<HashMap<ConfigSkillsCacheKey, CachedConfigSkillsSnapshot>>,
}

impl SkillsService {
    pub fn new(codex_home: AbsolutePathBuf, bundled_skills_enabled: bool) -> Self {
        Self::new_with_restriction_product(codex_home, bundled_skills_enabled, Some(Product::Codex))
    }

    pub fn new_with_restriction_product(
        codex_home: AbsolutePathBuf,
        bundled_skills_enabled: bool,
        restriction_product: Option<Product>,
    ) -> Self {
        let service = Self {
            codex_home,
            restriction_product,
            extra_roots: RwLock::new(Vec::new()),
            cache_by_cwd: RwLock::new(HashMap::new()),
            cache_by_config: RwLock::new(HashMap::new()),
        };
        if !bundled_skills_enabled {
            // The loader caches bundled skills under `skills/.system`. Clearing that directory is
            // best-effort cleanup; root selection still enforces the config even if removal fails.
            uninstall_system_skills(&service.codex_home);
        } else if let Err(err) = install_system_skills(&service.codex_home) {
            tracing::error!("failed to install system skills: {err}");
        }
        service
    }

    pub fn set_extra_roots(&self, extra_roots: Vec<AbsolutePathBuf>) {
        {
            let mut roots = self
                .extra_roots
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *roots = extra_roots;
        }
        self.clear_cache();
    }

    /// Load skills for an already-constructed [`Config`], avoiding any additional config-layer
    /// loading.
    ///
    /// This path uses a cache keyed by the effective skill-relevant config state rather than just
    /// cwd so role-local and session-local skill overrides cannot bleed across sessions that happen
    /// to share a directory. When `refresh_filesystem` is true, filesystem-derived roots and skill
    /// configuration rules are revalidated before reusing the cached snapshot.
    #[instrument(
        name = "skills_for_config",
        level = "info",
        skip_all,
        fields(otel.name = "skills_for_config")
    )]
    pub async fn snapshot_for_config(
        &self,
        input: &SkillsLoadInput,
        fs: Option<Arc<dyn ExecutorFileSystem>>,
        refresh_filesystem: bool,
    ) -> HostSkillsSnapshot {
        let extra_roots = self.extra_roots();
        let cache_key = config_skills_cache_key(input, &extra_roots, fs.as_ref());
        let cached = self.cached_snapshot_for_config(&cache_key);
        if !refresh_filesystem && let Some(cached) = &cached {
            return cached.snapshot.clone();
        }

        let roots = self
            .skill_roots_for_config_with_extra_roots(input, fs, extra_roots)
            .await;
        let skill_config_rules = skill_config_rules_from_stack(&input.config_layer_stack);
        let fingerprint = config_skills_filesystem_fingerprint(&roots, &skill_config_rules);
        if let Some(cached) = cached
            && cached.fingerprint == fingerprint
        {
            return cached.snapshot;
        }

        let snapshot = HostSkillsSnapshot::new(Arc::new(
            self.build_skill_outcome(input, roots, &skill_config_rules)
                .await,
        ));
        let mut cache = self
            .cache_by_config
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.insert(
            cache_key,
            CachedConfigSkillsSnapshot {
                fingerprint,
                snapshot: snapshot.clone(),
            },
        );
        snapshot
    }

    pub async fn skill_roots_for_config(
        &self,
        input: &SkillsLoadInput,
        fs: Option<Arc<dyn ExecutorFileSystem>>,
    ) -> Vec<SkillRoot> {
        self.skill_roots_for_config_with_extra_roots(input, fs, self.extra_roots())
            .await
    }

    async fn skill_roots_for_config_with_extra_roots(
        &self,
        input: &SkillsLoadInput,
        fs: Option<Arc<dyn ExecutorFileSystem>>,
        extra_roots: Vec<AbsolutePathBuf>,
    ) -> Vec<SkillRoot> {
        let mut roots = skill_roots(
            fs,
            &input.config_layer_stack,
            &input.cwd,
            input.effective_skill_roots.clone(),
            extra_roots,
        )
        .await;
        if !input.bundled_skills_enabled {
            roots.retain(|root| root.scope != SkillScope::System);
        }
        roots
    }

    pub async fn snapshot_for_cwd(
        &self,
        input: &SkillsLoadInput,
        force_reload: bool,
        fs: Option<Arc<dyn ExecutorFileSystem>>,
    ) -> HostSkillsSnapshot {
        let use_cwd_cache = fs.is_some();
        if use_cwd_cache
            && !force_reload
            && let Some(snapshot) = self.cached_snapshot_for_cwd(&input.cwd)
        {
            return snapshot;
        }

        let mut roots = skill_roots(
            fs.clone(),
            &input.config_layer_stack,
            &input.cwd,
            input.effective_skill_roots.clone(),
            self.extra_roots(),
        )
        .await;
        if !bundled_skills_enabled_from_stack(&input.config_layer_stack) {
            roots.retain(|root| root.scope != SkillScope::System);
        }
        let skill_config_rules = skill_config_rules_from_stack(&input.config_layer_stack);
        let snapshot = HostSkillsSnapshot::new(Arc::new(
            self.build_skill_outcome(input, roots, &skill_config_rules)
                .await,
        ));
        if use_cwd_cache {
            let mut cache = self
                .cache_by_cwd
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            cache.insert(input.cwd.clone(), snapshot.clone());
        }
        snapshot
    }

    #[instrument(level = "trace", skip_all)]
    async fn build_skill_outcome(
        &self,
        input: &SkillsLoadInput,
        roots: Vec<SkillRoot>,
        skill_config_rules: &SkillConfigRules,
    ) -> SkillLoadOutcome {
        let outcome = load_skills_from_roots(roots, input.plugin_skill_snapshots.as_ref()).await;
        let outcome =
            crate::filter_skill_load_outcome_for_product(outcome, self.restriction_product);
        let disabled_paths = resolve_disabled_skill_paths(&outcome.skills, skill_config_rules);
        finalize_skill_outcome(outcome, disabled_paths)
    }

    pub fn clear_cache(&self) {
        let cleared_cwd = {
            let mut cache = self
                .cache_by_cwd
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let cleared = cache.len();
            cache.clear();
            cleared
        };
        let cleared_config = {
            let mut cache = self
                .cache_by_config
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let cleared = cache.len();
            cache.clear();
            cleared
        };
        let cleared = cleared_cwd + cleared_config;
        info!("skills cache cleared ({cleared} entries)");
    }

    fn cached_snapshot_for_cwd(&self, cwd: &AbsolutePathBuf) -> Option<HostSkillsSnapshot> {
        match self.cache_by_cwd.read() {
            Ok(cache) => cache.get(cwd).cloned(),
            Err(err) => err.into_inner().get(cwd).cloned(),
        }
    }

    fn cached_snapshot_for_config(
        &self,
        cache_key: &ConfigSkillsCacheKey,
    ) -> Option<CachedConfigSkillsSnapshot> {
        match self.cache_by_config.read() {
            Ok(cache) => cache.get(cache_key).cloned(),
            Err(err) => err.into_inner().get(cache_key).cloned(),
        }
    }

    fn extra_roots(&self) -> Vec<AbsolutePathBuf> {
        match self.extra_roots.read() {
            Ok(roots) => roots.clone(),
            Err(err) => err.into_inner().clone(),
        }
    }
}

#[derive(Clone)]
struct CachedConfigSkillsSnapshot {
    fingerprint: ConfigSkillsFilesystemFingerprint,
    snapshot: HostSkillsSnapshot,
}

/// Skill-relevant inputs that can be compared before root discovery touches the filesystem.
#[derive(Clone, PartialEq, Eq, Hash)]
struct ConfigSkillsCacheKey {
    cwd: AbsolutePathBuf,
    config_layers: Vec<ConfigLayerSkillsCacheKey>,
    effective_skill_roots: Vec<PluginSkillRoot>,
    bundled_skills_enabled: bool,
    extra_roots: Vec<AbsolutePathBuf>,
    file_system: Option<ExecutorFileSystemCacheKey>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct ConfigLayerSkillsCacheKey {
    source: std::mem::Discriminant<ConfigLayerSource>,
    config_folder: Option<AbsolutePathBuf>,
    disabled: bool,
    skills_config: Option<String>,
    project_root_markers: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
struct ConfigSkillsFilesystemFingerprint {
    roots: Vec<(AbsolutePathBuf, u8, Option<String>, Option<String>)>,
    skill_config_rules: SkillConfigRules,
}

// Snapshots retain filesystem-bound skill paths, so cache entries must distinguish instances.
#[derive(Clone)]
struct ExecutorFileSystemCacheKey(Arc<dyn ExecutorFileSystem>);

impl PartialEq for ExecutorFileSystemCacheKey {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ExecutorFileSystemCacheKey {}

impl Hash for ExecutorFileSystemCacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(Arc::as_ptr(&self.0), state);
    }
}

pub fn bundled_skills_enabled_from_stack(
    config_layer_stack: &codex_config::ConfigLayerStack,
) -> bool {
    let effective_config = config_layer_stack.effective_config();
    let Some(skills_value) = effective_config
        .as_table()
        .and_then(|table| table.get("skills"))
    else {
        return true;
    };

    let skills: SkillsConfig = match skills_value.clone().try_into() {
        Ok(skills) => skills,
        Err(err) => {
            warn!("invalid skills config: {err}");
            return true;
        }
    };

    skills.bundled.unwrap_or_default().enabled
}

fn config_skills_cache_key(
    input: &SkillsLoadInput,
    extra_roots: &[AbsolutePathBuf],
    fs: Option<&Arc<dyn ExecutorFileSystem>>,
) -> ConfigSkillsCacheKey {
    ConfigSkillsCacheKey {
        cwd: input.cwd.clone(),
        config_layers: input
            .config_layer_stack
            .get_layers(
                ConfigLayerStackOrdering::LowestPrecedenceFirst,
                /*include_disabled*/ true,
            )
            .into_iter()
            .filter_map(|layer| {
                let config_folder = layer.config_folder();
                let skills_config = if matches!(
                    layer.name,
                    ConfigLayerSource::User { .. } | ConfigLayerSource::SessionFlags
                ) {
                    layer.config.get("skills").map(ToString::to_string)
                } else {
                    None
                };
                let project_root_markers = if !layer.is_disabled()
                    && !matches!(layer.name, ConfigLayerSource::Project { .. })
                {
                    layer
                        .config
                        .get("project_root_markers")
                        .map(ToString::to_string)
                } else {
                    None
                };
                (config_folder.is_some()
                    || skills_config.is_some()
                    || project_root_markers.is_some())
                .then(|| ConfigLayerSkillsCacheKey {
                    source: std::mem::discriminant(&layer.name),
                    config_folder,
                    disabled: layer.is_disabled(),
                    skills_config,
                    project_root_markers,
                })
            })
            .collect(),
        effective_skill_roots: input.effective_skill_roots.clone(),
        bundled_skills_enabled: input.bundled_skills_enabled,
        extra_roots: extra_roots.to_vec(),
        file_system: fs.cloned().map(ExecutorFileSystemCacheKey),
    }
}

fn config_skills_filesystem_fingerprint(
    roots: &[SkillRoot],
    skill_config_rules: &SkillConfigRules,
) -> ConfigSkillsFilesystemFingerprint {
    ConfigSkillsFilesystemFingerprint {
        roots: roots
            .iter()
            .map(|root| {
                let scope_rank = match root.scope {
                    SkillScope::Repo => 0,
                    SkillScope::User => 1,
                    SkillScope::System => 2,
                    SkillScope::Admin => 3,
                };
                (
                    root.path.clone(),
                    scope_rank,
                    root.plugin_id.clone(),
                    root.plugin_namespace.clone(),
                )
            })
            .collect(),
        skill_config_rules: skill_config_rules.clone(),
    }
}

fn finalize_skill_outcome(
    mut outcome: SkillLoadOutcome,
    disabled_paths: HashSet<AbsolutePathBuf>,
) -> SkillLoadOutcome {
    outcome.disabled_paths = disabled_paths;
    let (by_scripts_dir, by_doc_path) =
        build_implicit_skill_path_indexes(outcome.allowed_skills_for_implicit_invocation());
    outcome.implicit_skills_by_scripts_dir = Arc::new(by_scripts_dir);
    outcome.implicit_skills_by_doc_path = Arc::new(by_doc_path);
    outcome
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;

use codex_config::ConfigLayerStack;
use codex_config::SkillConfigRules;
use codex_config::bundled_skills_enabled_from_stack;
use codex_config::skill_config_rules_from_stack;
use codex_exec_server::EnvironmentAccess;
use codex_exec_server::EnvironmentAccessKey;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::FileSystemEnvironmentAccessor;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_exec_server::LOCAL_FS;
use codex_protocol::protocol::Product;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_plugins::PluginIdentity;
use codex_utils_plugins::PluginSkillRoot;
use tokio::sync::OnceCell;
use tokio::sync::Semaphore;
use tracing::info;
use tracing::instrument;

use codex_skills::LoadedSkills;
use codex_skills::SkillLoadFuture;
use codex_skills::SkillRootLoadRequest;
use codex_skills::SkillRootLoader;
use codex_skills::SkillRootSnapshots;
use codex_skills::install_system_skills;

use crate::HostSkillsSnapshot;
use crate::SkillLoadOutcome;
use crate::host_roots::EmptyRemoteRootsCache;
use crate::host_roots::resolve_skill_roots;
use crate::loader::HostSkillRoot;
use crate::loader::HostSkillRootSnapshot;
use crate::loader::MAX_CONCURRENT_ROOT_SCANS;
use crate::loader::load_and_merge_host_skill_roots;
use crate::loader::load_and_merge_host_skill_roots_with_request_snapshots;

const CONFIG_SKILLS_CACHE_CAPACITY: usize = 32;

struct ConfigSkillsCacheEntry {
    key: ConfigSkillsCacheKey,
    snapshot: Arc<OnceCell<HostSkillsSnapshot>>,
}

#[derive(Debug, Clone)]
pub struct HostSkillsLoadInput {
    cwd: AbsolutePathBuf,
    implicit_repository_skills: bool,
    effective_skill_roots: Vec<PluginSkillRoot>,
    config_layer_stack: ConfigLayerStack,
    plugin_skill_snapshots: Option<SkillRootSnapshots<PluginSkillRoot>>,
}

impl HostSkillsLoadInput {
    pub fn new(
        cwd: AbsolutePathBuf,
        effective_skill_roots: Vec<PluginSkillRoot>,
        config_layer_stack: ConfigLayerStack,
    ) -> Self {
        Self {
            cwd,
            implicit_repository_skills: true,
            effective_skill_roots,
            config_layer_stack,
            plugin_skill_snapshots: None,
        }
    }

    /// Retains the turn's configuration and plugin selection for a new repository location.
    pub fn for_cwd(&self, cwd: AbsolutePathBuf) -> Self {
        Self {
            cwd,
            ..self.clone()
        }
    }

    /// Host implicit command attribution only runs for the local environment.
    pub fn with_environment_id(mut self, environment_id: &str) -> Self {
        self.implicit_repository_skills = environment_id == LOCAL_ENVIRONMENT_ID;
        self
    }

    /// Attaches plugin skill snapshots parsed during plugin loading, when available.
    pub fn with_plugin_skill_snapshots(
        mut self,
        plugin_skill_snapshots: Option<SkillRootSnapshots<PluginSkillRoot>>,
    ) -> Self {
        self.plugin_skill_snapshots = plugin_skill_snapshots;
        self
    }
}

/// Owns host skill discovery, immutable snapshots, cache invalidation, and extra roots.
///
/// Source-specific model exposure remains the responsibility of the skills extension.
pub struct HostSkillsService {
    codex_home: AbsolutePathBuf,
    restriction_product: Option<Product>,
    extra_roots: RwLock<Vec<AbsolutePathBuf>>,
    cache_by_cwd: RwLock<HashMap<AbsolutePathBuf, HostSkillsSnapshot>>,
    cache_by_config: RwLock<VecDeque<ConfigSkillsCacheEntry>>,
    empty_remote_roots: Mutex<EmptyRemoteRootsCache>,
    // Shared across cwds so root scheduling cannot multiply per-root I/O fanout.
    root_scan_slots: Arc<Semaphore>,
}

pub(crate) type RequestSkillRootSnapshots =
    RwLock<HashMap<ConfigSkillRootCacheKey, Arc<OnceCell<HostSkillRootSnapshot>>>>;

/// Shares host-root scans between workspaces for the lifetime of one request.
pub struct HostSkillsRequest<'a> {
    service: &'a HostSkillsService,
    root_snapshots: RequestSkillRootSnapshots,
}

impl HostSkillsRequest<'_> {
    pub async fn snapshot_for_cwd(
        &self,
        input: &HostSkillsLoadInput,
        force_reload: bool,
        fs: Option<Arc<dyn ExecutorFileSystem>>,
    ) -> HostSkillsSnapshot {
        let repository = fs.as_ref().map(FileSystemEnvironmentAccessor::unrestricted);
        self.service
            .snapshot_for_cwd_with_root_snapshots(
                input,
                force_reload,
                repository
                    .as_ref()
                    .map(|access| access as &dyn EnvironmentAccess),
                Some(&self.root_snapshots),
            )
            .await
    }
}

impl HostSkillsService {
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
            cache_by_config: RwLock::new(VecDeque::new()),
            empty_remote_roots: Mutex::default(),
            root_scan_slots: Arc::new(Semaphore::new(MAX_CONCURRENT_ROOT_SCANS)),
        };
        // The cache is shared by every process using this CODEX_HOME. Disabled services filter
        // system roots when loading rather than mutating shared state.
        if bundled_skills_enabled {
            service.ensure_system_skills_installed();
        }
        service
    }

    /// Creates a request-local view without changing persistent plugin snapshots.
    pub fn for_request(&self) -> HostSkillsRequest<'_> {
        HostSkillsRequest {
            service: self,
            root_snapshots: RwLock::new(HashMap::new()),
        }
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
    /// to share a directory.
    pub async fn snapshot_for_config(
        &self,
        input: &HostSkillsLoadInput,
        repository: Option<&dyn EnvironmentAccess>,
    ) -> HostSkillsSnapshot {
        self.snapshot_for_config_with_project_markers(input, repository, repository)
            .await
    }

    /// Load repository skills with current access and marker metadata from the same executor.
    /// Unrestricted marker checks preserve hidden repository boundaries and run only on watch misses.
    pub async fn snapshot_for_config_on_executor(
        &self,
        input: &HostSkillsLoadInput,
        repository: Option<(&Arc<dyn ExecutorFileSystem>, &dyn EnvironmentAccess)>,
    ) -> HostSkillsSnapshot {
        let project_markers = repository
            .map(|(file_system, _)| FileSystemEnvironmentAccessor::unrestricted(file_system));
        self.snapshot_for_config_with_project_markers(
            input,
            repository.map(|(_, access)| access),
            project_markers
                .as_ref()
                .map(|access| access as &dyn EnvironmentAccess),
        )
        .await
    }

    #[instrument(
        name = "skills_for_config",
        level = "info",
        skip_all,
        fields(otel.name = "skills_for_config")
    )]
    async fn snapshot_for_config_with_project_markers(
        &self,
        input: &HostSkillsLoadInput,
        repository: Option<&dyn EnvironmentAccess>,
        project_markers: Option<&dyn EnvironmentAccess>,
    ) -> HostSkillsSnapshot {
        // User and plugin roots retain host authority independently of the selected executor.
        let host = FileSystemEnvironmentAccessor::unrestricted(&LOCAL_FS);
        let roots = self
            .skill_roots_for_config(
                input,
                repository,
                project_markers,
                &host,
                Some(&self.empty_remote_roots),
            )
            .await;
        let skill_config_rules = skill_config_rules_from_stack(&input.config_layer_stack);
        let cache_key = config_skills_cache_key(&roots, &skill_config_rules, input);
        self.snapshot_for_skill_roots(
            input,
            roots,
            &skill_config_rules,
            cache_key,
            /*force_reload*/ false,
            /*request_root_snapshots*/ None,
        )
        .await
    }

    /// Returns filesystem roots whose changes should invalidate discovered host skills.
    ///
    /// Plugin roots have their own lifecycle invalidation, and bundled roots are installed before
    /// filesystem watching begins.
    pub async fn watchable_skill_root_paths(
        &self,
        input: &HostSkillsLoadInput,
        fs: Arc<dyn ExecutorFileSystem>,
    ) -> Vec<AbsolutePathBuf> {
        let repository = FileSystemEnvironmentAccessor::unrestricted(&fs);
        let host = FileSystemEnvironmentAccessor::unrestricted(&LOCAL_FS);
        self.skill_roots_for_config(
            input,
            Some(&repository),
            Some(&repository),
            &host,
            /*empty_remote_roots*/ None,
        )
        .await
        .into_iter()
        .filter(|root| root.plugin_identity().is_none() && root.scope != SkillScope::System)
        .map(|root| root.path)
        .collect()
    }

    async fn skill_roots_for_config<'a>(
        &self,
        input: &HostSkillsLoadInput,
        repository: Option<&'a dyn EnvironmentAccess>,
        project_markers: Option<&dyn EnvironmentAccess>,
        host: &'a dyn EnvironmentAccess,
        empty_remote_roots: Option<&Mutex<EmptyRemoteRootsCache>>,
    ) -> Vec<HostSkillRoot<'a>> {
        let bundled_skills_enabled = bundled_skills_enabled_from_stack(&input.config_layer_stack);
        if bundled_skills_enabled {
            self.ensure_system_skills_installed();
        }
        let mut roots = resolve_skill_roots(
            repository,
            host,
            &input.config_layer_stack,
            &input.cwd,
            project_markers,
            input.effective_skill_roots.clone(),
            self.extra_roots(),
            empty_remote_roots,
        )
        .await;
        if !bundled_skills_enabled {
            roots.retain(|root| root.scope != SkillScope::System);
        }
        roots
    }

    async fn snapshot_for_cwd_with_root_snapshots(
        &self,
        input: &HostSkillsLoadInput,
        force_reload: bool,
        repository: Option<&dyn EnvironmentAccess>,
        request_root_snapshots: Option<&RequestSkillRootSnapshots>,
    ) -> HostSkillsSnapshot {
        let bundled_skills_enabled = bundled_skills_enabled_from_stack(&input.config_layer_stack);
        if bundled_skills_enabled {
            self.ensure_system_skills_installed();
        }
        let host = FileSystemEnvironmentAccessor::unrestricted(&LOCAL_FS);
        let use_cwd_cache = repository.is_some();
        let cache_snapshot_by_cwd = use_cwd_cache && input.effective_skill_roots.is_empty();
        if cache_snapshot_by_cwd
            && !force_reload
            && let Some(snapshot) = self.cached_snapshot_for_cwd(&input.cwd)
        {
            return snapshot;
        }

        if force_reload {
            self.empty_remote_roots
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clear();
        }
        let mut roots = resolve_skill_roots(
            repository,
            &host,
            &input.config_layer_stack,
            &input.cwd,
            repository,
            input.effective_skill_roots.clone(),
            self.extra_roots(),
            Some(&self.empty_remote_roots),
        )
        .await;
        if !bundled_skills_enabled {
            roots.retain(|root| root.scope != SkillScope::System);
        }
        let skill_config_rules = skill_config_rules_from_stack(&input.config_layer_stack);
        let snapshot = if use_cwd_cache {
            let cache_key = config_skills_cache_key(&roots, &skill_config_rules, input);
            self.snapshot_for_skill_roots(
                input,
                roots,
                &skill_config_rules,
                cache_key,
                force_reload,
                request_root_snapshots,
            )
            .await
        } else {
            HostSkillsSnapshot::new(Arc::new(
                self.build_skill_outcome(input, roots, &skill_config_rules, request_root_snapshots)
                    .await,
            ))
        };
        if cache_snapshot_by_cwd {
            let mut cache = self
                .cache_by_cwd
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            cache.insert(input.cwd.clone(), snapshot.clone());
        }
        snapshot
    }

    async fn snapshot_for_skill_roots(
        &self,
        input: &HostSkillsLoadInput,
        roots: Vec<HostSkillRoot<'_>>,
        skill_config_rules: &SkillConfigRules,
        cache_key: ConfigSkillsCacheKey,
        force_reload: bool,
        request_root_snapshots: Option<&RequestSkillRootSnapshots>,
    ) -> HostSkillsSnapshot {
        let snapshot_cell = {
            let mut cache = self
                .cache_by_config
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let snapshot_cell = cache
                .iter()
                .position(|entry| entry.key == cache_key)
                .and_then(|index| cache.remove(index))
                .filter(|_| !force_reload)
                .map(|entry| entry.snapshot)
                .unwrap_or_else(|| Arc::new(OnceCell::new()));
            cache.push_front(ConfigSkillsCacheEntry {
                key: cache_key,
                snapshot: Arc::clone(&snapshot_cell),
            });
            // Keys retain parsed plugin generations; eviction releases obsolete catalogs while
            // callers and in-flight loads keep their own snapshots alive.
            cache.truncate(CONFIG_SKILLS_CACHE_CAPACITY);
            snapshot_cell
        };

        snapshot_cell
            .get_or_init(|| async {
                HostSkillsSnapshot::new(Arc::new(
                    self.build_skill_outcome(
                        input,
                        roots,
                        skill_config_rules,
                        request_root_snapshots,
                    )
                    .await,
                ))
            })
            .await
            .clone()
    }

    #[instrument(level = "trace", skip_all)]
    async fn build_skill_outcome(
        &self,
        input: &HostSkillsLoadInput,
        roots: Vec<HostSkillRoot<'_>>,
        skill_config_rules: &SkillConfigRules,
        request_root_snapshots: Option<&RequestSkillRootSnapshots>,
    ) -> SkillLoadOutcome {
        let accessors = roots
            .iter()
            .filter(|root| input.implicit_repository_skills || root.scope != SkillScope::Repo)
            .map(|root| root.file_system)
            .collect::<Vec<_>>();
        let outcome = load_and_merge_host_skill_roots_with_request_snapshots(
            roots,
            &self.root_scan_slots,
            self.restriction_product,
            input.plugin_skill_snapshots.as_ref(),
            request_root_snapshots,
        )
        .await;
        let disabled_paths = skill_config_rules.resolve_disabled_paths(
            outcome
                .skills
                .iter()
                .map(|skill| (skill.name.as_str(), &skill.path_to_skills_md)),
        );
        outcome
            .with_disabled_paths(disabled_paths, &accessors)
            .await
    }

    pub fn clear_cache(&self) {
        self.empty_remote_roots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
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

    fn extra_roots(&self) -> Vec<AbsolutePathBuf> {
        match self.extra_roots.read() {
            Ok(roots) => roots.clone(),
            Err(err) => err.into_inner().clone(),
        }
    }

    fn ensure_system_skills_installed(&self) {
        if let Err(err) = install_system_skills(&self.codex_home) {
            tracing::error!("failed to install system skills: {err}");
        }
    }
}

impl SkillRootLoader<PluginSkillRoot> for HostSkillsService {
    fn load_roots(
        &self,
        request: SkillRootLoadRequest<PluginSkillRoot>,
    ) -> SkillLoadFuture<'_, LoadedSkills> {
        Box::pin(async move {
            let host = FileSystemEnvironmentAccessor::unrestricted(&LOCAL_FS);
            let roots = request
                .roots
                .into_iter()
                .map(|root| HostSkillRoot::plugin(root, &host))
                .collect();
            let outcome = load_and_merge_host_skill_roots(
                roots,
                &self.root_scan_slots,
                request.restriction_product,
                request.snapshots.as_ref(),
            )
            .await;
            LoadedSkills {
                skills: outcome.skills,
                errors: outcome.errors,
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ConfigSkillsCacheKey {
    roots: Vec<ConfigSkillRootCacheKey>,
    implicit_repository_skills: bool,
    skill_config_rules: SkillConfigRules,
    plugin_skill_snapshots: Option<SkillRootSnapshots<PluginSkillRoot>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ConfigSkillRootCacheKey {
    path: AbsolutePathBuf,
    scope_rank: u8,
    plugin_identity: Option<PluginIdentity>,
    plugin_namespace: Option<String>,
    access: EnvironmentAccessKey,
}

fn config_skills_cache_key(
    roots: &[HostSkillRoot<'_>],
    skill_config_rules: &SkillConfigRules,
    input: &HostSkillsLoadInput,
) -> ConfigSkillsCacheKey {
    ConfigSkillsCacheKey {
        roots: roots.iter().map(config_skill_root_cache_key).collect(),
        implicit_repository_skills: input.implicit_repository_skills,
        skill_config_rules: skill_config_rules.clone(),
        plugin_skill_snapshots: input
            .plugin_skill_snapshots
            .as_ref()
            .filter(|_| roots.iter().any(|root| root.plugin_identity().is_some()))
            .cloned(),
    }
}

pub(crate) fn config_skill_root_cache_key(root: &HostSkillRoot<'_>) -> ConfigSkillRootCacheKey {
    let scope_rank = match root.scope {
        SkillScope::Repo => 0,
        SkillScope::User => 1,
        SkillScope::System => 2,
        SkillScope::Admin => 3,
    };
    ConfigSkillRootCacheKey {
        path: root.path.clone(),
        scope_rank,
        plugin_identity: root.plugin_identity().cloned(),
        plugin_namespace: root.plugin_namespace().map(str::to_string),
        access: root.file_system.cache_key(),
    }
}

#[cfg(test)]
#[path = "host_service_tests.rs"]
mod tests;

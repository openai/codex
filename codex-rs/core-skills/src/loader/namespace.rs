use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::FindUpErrorPolicy;
use codex_exec_server::FindUpMatchKind;
use codex_exec_server::FindUpOptions;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::DISCOVERABLE_PLUGIN_MANIFEST_PATHS;
use codex_utils_plugins::plugin_namespace_for_manifest_uri;
use codex_utils_plugins::plugin_namespace_for_root_uri;
use futures::StreamExt;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::OnceCell;
use tokio::sync::Semaphore;

use super::discovery::MAX_CONCURRENT_SKILL_LOADS;

#[path = "namespace_batch.rs"]
mod batch;
use batch::resolve_namespace_lookups;

const MAX_CONCURRENT_NAMESPACE_LOOKUPS: usize = 8;

struct NamespaceProbeCache<'a> {
    fs: &'a dyn ExecutorFileSystem,
    roots: Mutex<HashMap<PathUri, Arc<OnceCell<Option<String>>>>>,
    manifests: Mutex<HashMap<PathUri, Arc<OnceCell<Option<String>>>>>,
    permits: Semaphore,
}

impl<'a> NamespaceProbeCache<'a> {
    fn new(fs: &'a dyn ExecutorFileSystem) -> Self {
        Self {
            fs,
            roots: Mutex::new(HashMap::new()),
            manifests: Mutex::new(HashMap::new()),
            permits: Semaphore::new(MAX_CONCURRENT_SKILL_LOADS),
        }
    }

    async fn resolve_root(&self, root: &PathUri) -> Option<String> {
        let cell = cached_cell(&self.roots, root);
        cell.get_or_init(|| async {
            let Ok(_permit) = self.permits.acquire().await else {
                return None;
            };
            plugin_namespace_for_root_uri(self.fs, root).await
        })
        .await
        .clone()
    }

    async fn resolve_manifest(&self, root: &PathUri, manifest: &PathUri) -> Option<String> {
        let cell = cached_cell(&self.manifests, manifest);
        cell.get_or_init(|| plugin_namespace_for_manifest_uri(self.fs, root, manifest))
            .await
            .clone()
    }
}

fn cached_cell(
    cells: &Mutex<HashMap<PathUri, Arc<OnceCell<Option<String>>>>>,
    path: &PathUri,
) -> Arc<OnceCell<Option<String>>> {
    let mut cells = cells
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Arc::clone(
        cells
            .entry(path.clone())
            .or_insert_with(|| Arc::new(OnceCell::new())),
    )
}

struct ResolvedNamespaceLookup {
    root: PathUri,
    namespace: Option<String>,
}

async fn resolve_namespace_lookup(
    cache: &NamespaceProbeCache<'_>,
    root: PathUri,
    probe_root_alone: bool,
) -> ResolvedNamespaceLookup {
    let mut next_ancestor = if probe_root_alone {
        if let Some(namespace) = cache.resolve_root(&root).await {
            return ResolvedNamespaceLookup {
                root,
                namespace: Some(namespace),
            };
        }
        root.parent()
    } else {
        Some(root.clone())
    };
    let options = namespace_find_up_options();

    while let Some(search_start) = next_ancestor {
        let Ok(outcome) = cache
            .fs
            .find_up(&search_start, &options, /*sandbox*/ None)
            .await
        else {
            break;
        };
        let Some(matched) = outcome.matched else {
            break;
        };
        if let Some(namespace) = cache
            .resolve_manifest(&matched.ancestor, &matched.path)
            .await
        {
            return ResolvedNamespaceLookup {
                root,
                namespace: Some(namespace),
            };
        }
        next_ancestor = matched.ancestor.parent();
    }

    ResolvedNamespaceLookup {
        root,
        namespace: None,
    }
}

fn namespace_find_up_options() -> FindUpOptions {
    FindUpOptions {
        candidate_relative_paths: DISCOVERABLE_PLUGIN_MANIFEST_PATHS
            .iter()
            .map(ToString::to_string)
            .collect(),
        match_kind: FindUpMatchKind::File,
        non_not_found_error_policy: FindUpErrorPolicy::Ignore,
    }
}

/// Resolves the namespace prefix applied to skill names during one skills scan.
pub(crate) struct SkillNamespaceResolver {
    inherited_namespace: ResolvedSkillNamespace,
    nested_namespaces: Vec<(PathUri, ResolvedSkillNamespace)>,
}

impl SkillNamespaceResolver {
    pub(crate) fn with_provided_namespace(namespace: &str) -> Self {
        Self {
            inherited_namespace: ResolvedSkillNamespace::Plugin(namespace.to_string()),
            nested_namespaces: Vec::new(),
        }
    }

    pub(crate) async fn discover(
        fs: &dyn ExecutorFileSystem,
        root: &PathUri,
        skill_paths: &[PathUri],
        plugin_roots: HashSet<PathUri>,
        namespace_roots: HashSet<PathUri>,
    ) -> Self {
        let mut skill_ancestors = HashSet::new();
        for skill_path in skill_paths {
            let mut ancestor = skill_path.parent();
            while let Some(path) = ancestor {
                skill_ancestors.insert(path.clone());
                ancestor = path.parent();
            }
        }
        let plugin_roots = plugin_roots
            .into_iter()
            .filter(|plugin_root| skill_ancestors.contains(plugin_root))
            .collect::<HashSet<_>>();
        let discovered_manifest_roots = plugin_roots.clone();
        let namespace_roots = namespace_roots
            .into_iter()
            .filter(|namespace_root| namespace_root != root)
            .collect::<Vec<_>>();
        let namespace_root_set = namespace_roots.iter().cloned().collect::<HashSet<_>>();
        let plugin_roots = plugin_roots
            .into_iter()
            .filter(|plugin_root| plugin_root != root && !namespace_root_set.contains(plugin_root))
            .collect::<Vec<_>>();

        let lookup_requests = std::iter::once(root.clone())
            .chain(namespace_roots.iter().cloned())
            .map(|lookup_root| {
                let probe_root_alone = discovered_manifest_roots.contains(&lookup_root);
                (lookup_root, probe_root_alone)
            })
            .collect::<Vec<_>>();
        let probe_cache = NamespaceProbeCache::new(fs);
        let lookup_resolutions = resolve_namespace_lookups(&probe_cache, &lookup_requests);
        let plugin_resolutions = futures::stream::iter(plugin_roots.iter().cloned())
            .map(|plugin_root| {
                let probe_cache = &probe_cache;
                async move {
                    let namespace = probe_cache.resolve_root(&plugin_root).await;
                    (plugin_root, namespace)
                }
            })
            .buffer_unordered(MAX_CONCURRENT_SKILL_LOADS)
            .collect::<Vec<_>>();
        let (lookup_resolutions, plugin_resolutions) =
            futures::join!(lookup_resolutions, plugin_resolutions);
        let namespaces_by_lookup_root = lookup_resolutions
            .into_iter()
            .map(|lookup| (lookup.root, lookup.namespace))
            .collect::<HashMap<_, _>>();
        let namespaces_by_plugin_root = plugin_resolutions.into_iter().collect::<HashMap<_, _>>();

        let inherited_namespace = namespaces_by_lookup_root
            .get(root)
            .and_then(Option::as_ref)
            .cloned()
            .map(ResolvedSkillNamespace::Plugin)
            .unwrap_or(ResolvedSkillNamespace::Plain);
        let namespace_lookups = namespace_roots.into_iter().map(|namespace_root| {
            let namespace = namespaces_by_lookup_root
                .get(&namespace_root)
                .and_then(Option::as_ref)
                .cloned()
                .map(ResolvedSkillNamespace::Plugin)
                .unwrap_or(ResolvedSkillNamespace::Plain);
            (namespace_root, namespace)
        });
        let plugin_lookups = plugin_roots.into_iter().filter_map(|plugin_root| {
            namespaces_by_plugin_root
                .get(&plugin_root)
                .and_then(Option::as_ref)
                .cloned()
                .map(|namespace| (plugin_root, ResolvedSkillNamespace::Plugin(namespace)))
        });
        Self {
            inherited_namespace,
            nested_namespaces: namespace_lookups.chain(plugin_lookups).collect(),
        }
    }

    pub(crate) fn for_skill(&self, root: &PathUri, path: &PathUri) -> &ResolvedSkillNamespace {
        let path_is_under_root = path.starts_with(root);
        self.nested_namespaces
            .iter()
            .filter(|(namespace_root, _)| {
                path.starts_with(namespace_root)
                    && (!path_is_under_root || !root.starts_with(namespace_root))
            })
            .max_by_key(|(namespace_root, _)| namespace_root.ancestors().count())
            .map(|(_, namespace)| namespace)
            .unwrap_or(&self.inherited_namespace)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResolvedSkillNamespace {
    Plain,
    Plugin(String),
}

impl ResolvedSkillNamespace {
    pub(crate) fn qualify(&self, base_name: &str) -> String {
        match self {
            Self::Plain => base_name.to_string(),
            Self::Plugin(namespace) => format!("{namespace}:{base_name}"),
        }
    }
}

#[cfg(test)]
#[path = "namespace_tests.rs"]
mod tests;

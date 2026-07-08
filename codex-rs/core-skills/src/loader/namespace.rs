use codex_exec_server::ExecutorFileSystem;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::plugin_namespace_for_root_uri;
use futures::StreamExt;
use std::collections::HashMap;
use std::collections::HashSet;

use super::discovery::MAX_CONCURRENT_SKILL_LOADS;

/// Resolves the namespace prefix applied to skill names during one skills scan.
///
/// A plugin namespace is the plugin name from the nearest valid plugin manifest
/// above a skill path. For example, a skill named `search` beneath a plugin named
/// `sample` is exposed as `sample:search`.
///
/// Resolving the namespace separately for every `SKILL.md` repeats the same
/// ancestor manifest probes for sibling skills. This resolver resolves relevant
/// roots once per scan, then selects the nearest matching root for each skill.
///
/// Namespace precedence is:
///
/// 1. an explicitly provided plugin namespace;
/// 2. the deepest matching canonical symlink root or nested plugin root;
/// 3. the namespace inherited from the scanned skills root.
pub(crate) struct SkillNamespaceResolver {
    inherited_namespace: ResolvedSkillNamespace,
    nested_namespaces: Vec<(PathUri, ResolvedSkillNamespace)>,
}

impl SkillNamespaceResolver {
    /// Builds a resolver whose explicit plugin-owned namespace overrides discovery.
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
        // Only probe plugin roots above loaded skills; unused siblings cannot affect names.
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

        // The scan root is already the fallback above if nothing else matches, exclude from the search.
        let namespace_roots = namespace_roots
            .into_iter()
            .filter(|namespace_root| namespace_root != root)
            .collect::<Vec<_>>();
        let namespace_root_set = namespace_roots.iter().cloned().collect::<HashSet<_>>();
        let plugin_roots = plugin_roots
            .into_iter()
            .filter(|plugin_root| plugin_root != root && !namespace_root_set.contains(plugin_root))
            .collect::<Vec<_>>();

        let lookup_roots = std::iter::once(root.clone())
            .chain(namespace_roots.iter().cloned())
            .collect::<Vec<_>>();
        let mut pending_lookups = lookup_roots
            .iter()
            .cloned()
            .map(|lookup_root| (lookup_root.clone(), lookup_root))
            .collect::<Vec<_>>();
        let mut direct_plugin_roots = plugin_roots.iter().cloned().collect::<HashSet<_>>();
        let mut namespaces_by_root = HashMap::new();
        let mut namespaces_by_lookup_root = HashMap::new();
        while !pending_lookups.is_empty() {
            let probe_roots = pending_lookups
                .iter()
                .map(|(_, ancestor)| ancestor.clone())
                .chain(direct_plugin_roots.drain())
                .filter(|ancestor| !namespaces_by_root.contains_key(ancestor))
                .collect::<HashSet<_>>();
            namespaces_by_root.extend(
                futures::stream::iter(probe_roots)
                    .map(|manifest_root| async move {
                        let namespace = plugin_namespace_for_root_uri(fs, &manifest_root).await;
                        (manifest_root, namespace)
                    })
                    .buffered(MAX_CONCURRENT_SKILL_LOADS)
                    .collect::<HashMap<_, _>>()
                    .await,
            );

            let mut next_lookups = Vec::new();
            for (lookup_root, ancestor) in pending_lookups {
                match namespaces_by_root.get(&ancestor) {
                    Some(Some(namespace)) => {
                        namespaces_by_lookup_root.insert(lookup_root, Some(namespace.clone()));
                    }
                    Some(None) => match ancestor.parent() {
                        Some(parent) => next_lookups.push((lookup_root, parent)),
                        None => {
                            namespaces_by_lookup_root.insert(lookup_root, None);
                        }
                    },
                    None => unreachable!("pending namespace ancestor was not probed"),
                }
            }
            pending_lookups = next_lookups;
        }

        // Ordinary descendants fall back to the nearest valid manifest at or above the scan root.
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
        // Invalid nested manifests are omitted, so the deepest remaining match wins.
        let plugin_lookups = plugin_roots.into_iter().filter_map(|plugin_root| {
            namespaces_by_root
                .get(&plugin_root)
                .and_then(Option::as_ref)
                .cloned()
                .map(|namespace| (plugin_root, ResolvedSkillNamespace::Plugin(namespace)))
        });
        let nested_namespaces = namespace_lookups.chain(plugin_lookups).collect();

        Self {
            inherited_namespace,
            nested_namespaces,
        }
    }

    pub(crate) fn for_skill(&self, root: &PathUri, path: &PathUri) -> &ResolvedSkillNamespace {
        // Ancestor symlink targets cannot override skills still owned by the scan root.
        let path_is_under_root = path.starts_with(root);
        // The deepest matching path prefix is the nearest applicable namespace.
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

/// The completed namespace resolution for a skill root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResolvedSkillNamespace {
    /// No plugin namespace applies to matching skills.
    Plain,
    /// Qualify matching skill names with this plugin namespace.
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

use codex_exec_server::ExecutorFileSystem;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::plugin_namespace_for_root_uri;
use codex_utils_plugins::plugin_namespace_for_skill_uri;
use futures::future::join_all;
use std::collections::HashSet;

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

        // Ordinary descendants fall back to the nearest valid manifest at or above the scan root.
        let inherited_namespace = plugin_namespace_for_skill_uri(fs, root)
            .await
            .map(ResolvedSkillNamespace::Plugin)
            .unwrap_or(ResolvedSkillNamespace::Plain);
        // The scan root is already the fallback above if nothing else matches, exclude from the search.
        let namespace_roots = namespace_roots
            .into_iter()
            .filter(|namespace_root| namespace_root != root)
            .collect::<Vec<_>>();
        let namespace_root_set = namespace_roots.iter().cloned().collect::<HashSet<_>>();
        // Keep independent probes concurrent for remote executor latency.
        let namespace_lookups = join_all(namespace_roots.into_iter().map(|namespace_root| async {
            let namespace = plugin_namespace_for_skill_uri(fs, &namespace_root)
                .await
                .map(ResolvedSkillNamespace::Plugin)
                .unwrap_or(ResolvedSkillNamespace::Plain);
            (namespace_root, namespace)
        }));
        // Invalid nested manifests are omitted, so the deepest remaining match wins.
        let plugin_lookups = join_all(
            plugin_roots
                .into_iter()
                .filter(|plugin_root| {
                    plugin_root != root && !namespace_root_set.contains(plugin_root)
                })
                .map(|plugin_root| async move {
                    plugin_namespace_for_root_uri(fs, &plugin_root)
                        .await
                        .map(|namespace| (plugin_root, ResolvedSkillNamespace::Plugin(namespace)))
                }),
        );
        let (namespace_lookups, plugin_lookups) = tokio::join!(namespace_lookups, plugin_lookups);
        let nested_namespaces = namespace_lookups
            .into_iter()
            .chain(plugin_lookups.into_iter().flatten())
            .collect();

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

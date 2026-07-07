use codex_exec_server::ExecutorFileSystem;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::plugin_namespace_for_root_uri;
use codex_utils_plugins::plugin_namespace_for_skill_uri;
use futures::future::join_all;
use std::collections::HashSet;

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

pub(crate) struct SkillNamespaceResolver {
    inherited_namespace: ResolvedSkillNamespace,
    nested_namespaces: Vec<(PathUri, ResolvedSkillNamespace)>,
}

impl SkillNamespaceResolver {
    pub(crate) async fn new(
        fs: &dyn ExecutorFileSystem,
        root: &PathUri,
        provided_namespace: Option<&str>,
        plugin_roots: HashSet<PathUri>,
        namespace_roots: HashSet<PathUri>,
    ) -> Self {
        // Plugin-owned skill roots already know their namespace. Keep that explicit source above
        // all manifest discovery so a nested or symlinked manifest cannot rename those skills.
        if let Some(namespace) = provided_namespace {
            return Self {
                inherited_namespace: ResolvedSkillNamespace::Plugin(namespace.to_string()),
                nested_namespaces: Vec::new(),
            };
        }

        // Ordinary descendants inherit the nearest valid manifest at or above the scanned root.
        // This remains the fallback when no more specific discovered root matches a skill path.
        let inherited_namespace = plugin_namespace_for_skill_uri(fs, root)
            .await
            .map(ResolvedSkillNamespace::Plugin)
            .unwrap_or(ResolvedSkillNamespace::Plain);
        // Symlink traversal records canonical roots that can leave the scanned tree. Resolve each
        // one independently, including Plain, so a symlink into a plain tree does not accidentally
        // inherit the scanned root's plugin namespace.
        let namespace_roots = namespace_roots
            .into_iter()
            .filter(|namespace_root| namespace_root != root)
            .collect::<Vec<_>>();
        let namespace_root_set = namespace_roots.iter().cloned().collect::<HashSet<_>>();
        // Keep independent root probes concurrent: remote executors pay RPC latency for each
        // filesystem request, so awaiting these serially would scale startup with plugin count.
        let namespace_lookups = join_all(namespace_roots.into_iter().map(|namespace_root| async {
            let namespace = plugin_namespace_for_skill_uri(fs, &namespace_root)
                .await
                .map(ResolvedSkillNamespace::Plugin)
                .unwrap_or(ResolvedSkillNamespace::Plain);
            (namespace_root, namespace)
        }));
        // Valid nested plugin manifests are more specific candidates than their enclosing roots.
        // Invalid manifests are omitted, allowing selection to fall back to the next nearest root.
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

    pub(crate) fn for_skill(&self, path: &PathUri) -> &ResolvedSkillNamespace {
        // Candidate roots are path prefixes; the deepest match is the nearest applicable
        // namespace, whether it came from a canonical symlink root or a nested plugin manifest.
        self.nested_namespaces
            .iter()
            .filter(|(namespace_root, _)| path.starts_with(namespace_root))
            .max_by_key(|(namespace_root, _)| namespace_root.ancestors().count())
            .map(|(_, namespace)| namespace)
            .unwrap_or(&self.inherited_namespace)
    }
}

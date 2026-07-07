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
    ) -> Self {
        if let Some(namespace) = provided_namespace {
            return Self {
                inherited_namespace: ResolvedSkillNamespace::Plugin(namespace.to_string()),
                nested_namespaces: Vec::new(),
            };
        }

        let inherited_namespace = plugin_namespace_for_skill_uri(fs, root)
            .await
            .map(ResolvedSkillNamespace::Plugin)
            .unwrap_or(ResolvedSkillNamespace::Plain);
        // Keep independent root probes concurrent: remote executors pay RPC latency for each
        // filesystem request, so awaiting these serially would scale startup with plugin count.
        let nested_namespaces = join_all(
            plugin_roots
                .into_iter()
                .filter(|plugin_root| plugin_root != root)
                .map(|plugin_root| async move {
                    plugin_namespace_for_root_uri(fs, &plugin_root)
                        .await
                        .map(|namespace| (plugin_root, ResolvedSkillNamespace::Plugin(namespace)))
                }),
        )
        .await
        .into_iter()
        .flatten()
        .collect();

        Self {
            inherited_namespace,
            nested_namespaces,
        }
    }

    pub(crate) fn for_skill(&self, path: &PathUri) -> &ResolvedSkillNamespace {
        self.nested_namespaces
            .iter()
            .filter(|(plugin_root, _)| path.starts_with(plugin_root))
            .max_by_key(|(plugin_root, _)| plugin_root.ancestors().count())
            .map(|(_, namespace)| namespace)
            .unwrap_or(&self.inherited_namespace)
    }
}

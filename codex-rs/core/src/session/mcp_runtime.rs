use std::sync::Arc;

use codex_core_plugins::ExecutorPluginRuntime;
use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_mcp::McpRuntimeSnapshot;
use codex_protocol::capabilities::SelectedCapabilityRoot;

/// One live selected-plugin MCP runtime retained between model steps.
///
/// Selected environment identity and contents are stable. Plugin manifests, MCP declarations, and
/// app declarations are therefore cached by the complete selected root for the session lifetime.
/// Successful projections and stable non-plugin roots are cached. Failed reads are not cached, so
/// a transient executor error can recover later.
///
/// A live runtime is a separate cache: it is reusable only for the same ordered selected roots and
/// the same process-local environment handles. The caller additionally compares the effective MCP
/// config and runtime context. Replacing a connection handle may rebuild live processes, but it
/// reuses the stable plugin projection and does not reread capability files.
///
/// Within a live session, the selected runtime is invalidated in exactly two ways:
///
/// 1. [`Self::replace_base_and_invalidate_selected`] installs a new base MCP runtime.
/// 2. [`Self::replace_selected_runtime`] stores a newly projected runtime. This happens when the
///    active bindings or effective runtime configuration change.
///
/// In-flight [`McpRuntimeSnapshot`] values retain their manager until their model step finishes.
#[derive(Default)]
pub(crate) struct SelectedMcpRuntimeCache {
    base_runtime: Option<Arc<McpRuntimeSnapshot>>,
    plugin_projections: Vec<CachedExecutorPluginProjection>,
    runtime: Option<CachedSelectedRuntime>,
}

struct CachedSelectedRuntime {
    bindings: Vec<(usize, ResolvedSelectedCapabilityRoot)>,
    runtime: Arc<McpRuntimeSnapshot>,
}

struct CachedExecutorPluginProjection {
    selected_root: SelectedCapabilityRoot,
    plugin: Option<ExecutorPluginRuntime>,
}

impl SelectedMcpRuntimeCache {
    pub(crate) fn replace_base_and_invalidate_selected(
        &mut self,
        runtime: Arc<McpRuntimeSnapshot>,
    ) {
        self.base_runtime = Some(runtime);
        self.runtime = None;
    }

    pub(crate) fn base_runtime(&self) -> Arc<McpRuntimeSnapshot> {
        self.base_runtime
            .as_ref()
            .map(Arc::clone)
            .expect("base MCP runtime must be installed before capturing a step")
    }

    pub(crate) fn runtime_for_bindings(
        &self,
        bindings: &[(usize, ResolvedSelectedCapabilityRoot)],
    ) -> Option<Arc<McpRuntimeSnapshot>> {
        self.runtime
            .as_ref()
            .filter(|cached| same_bindings(&cached.bindings, bindings))
            .map(|cached| Arc::clone(&cached.runtime))
    }

    pub(crate) async fn project_plugins(
        &mut self,
        bindings: &[(usize, ResolvedSelectedCapabilityRoot)],
    ) -> Vec<(usize, ExecutorPluginRuntime)> {
        let mut plugins = Vec::new();
        for (selection_order, root) in bindings {
            let selected_root = root.selected_root();
            if let Some(cached) = self
                .plugin_projections
                .iter()
                .find(|cached| &cached.selected_root == selected_root)
            {
                if let Some(plugin) = &cached.plugin {
                    plugins.push((*selection_order, plugin.clone()));
                }
                continue;
            }

            match ExecutorPluginRuntime::project(root).await {
                Ok(plugin) => {
                    self.plugin_projections
                        .push(CachedExecutorPluginProjection {
                            selected_root: selected_root.clone(),
                            plugin: plugin.clone(),
                        });
                    if let Some(plugin) = plugin {
                        plugins.push((*selection_order, plugin));
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        selected_root = selected_root.id,
                        error = %err,
                        "failed to project selected executor plugin runtime"
                    );
                }
            }
        }
        plugins
    }

    pub(crate) fn replace_selected_runtime(
        &mut self,
        bindings: Vec<(usize, ResolvedSelectedCapabilityRoot)>,
        runtime: Arc<McpRuntimeSnapshot>,
    ) {
        self.runtime = Some(CachedSelectedRuntime { bindings, runtime });
    }
}

fn same_bindings(
    left: &[(usize, ResolvedSelectedCapabilityRoot)],
    right: &[(usize, ResolvedSelectedCapabilityRoot)],
) -> bool {
    // Order is part of the key because later selected roots can be renamed when MCP server names
    // collide. Arc identity is only a live-connection key: stable plugin metadata is cached above
    // by selected root and survives connection-handle replacement.
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|((left_order, left), (right_order, right))| {
                left_order == right_order && same_binding(left, right)
            })
}

fn same_binding(
    left: &ResolvedSelectedCapabilityRoot,
    right: &ResolvedSelectedCapabilityRoot,
) -> bool {
    left.selected_root() == right.selected_root()
        && Arc::ptr_eq(left.environment(), right.environment())
}

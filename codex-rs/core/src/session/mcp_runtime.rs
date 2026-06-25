use std::sync::Arc;

use codex_core_plugins::ExecutorPluginRuntime;
use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_mcp::McpRuntimeSnapshot;

#[derive(Default)]
pub(crate) struct SelectedMcpRuntimeCache {
    base_runtime: Option<Arc<McpRuntimeSnapshot>>,
    runtime: Option<CachedSelectedRuntime>,
}

struct CachedSelectedRuntime {
    bindings: Vec<(usize, ResolvedSelectedCapabilityRoot)>,
    plugins: Vec<(usize, ExecutorPluginRuntime)>,
    runtime: Arc<McpRuntimeSnapshot>,
}

impl SelectedMcpRuntimeCache {
    pub(crate) fn replace_base(&mut self, runtime: Arc<McpRuntimeSnapshot>) {
        self.base_runtime = Some(runtime);
        self.runtime = None;
    }

    pub(crate) fn base_runtime(&self) -> Arc<McpRuntimeSnapshot> {
        self.base_runtime
            .as_ref()
            .map(Arc::clone)
            .expect("base MCP runtime must be installed before capturing a step")
    }

    pub(crate) fn runtime(
        &self,
        bindings: &[(usize, ResolvedSelectedCapabilityRoot)],
    ) -> Option<Arc<McpRuntimeSnapshot>> {
        self.runtime
            .as_ref()
            .filter(|cached| same_bindings(&cached.bindings, bindings))
            .map(|cached| Arc::clone(&cached.runtime))
    }

    pub(crate) fn plugins(
        &self,
        bindings: &[(usize, ResolvedSelectedCapabilityRoot)],
    ) -> Option<Vec<(usize, ExecutorPluginRuntime)>> {
        self.runtime
            .as_ref()
            .filter(|cached| same_bindings(&cached.bindings, bindings))
            .map(|cached| cached.plugins.clone())
    }

    pub(crate) fn insert_runtime(
        &mut self,
        bindings: Vec<(usize, ResolvedSelectedCapabilityRoot)>,
        plugins: Vec<(usize, ExecutorPluginRuntime)>,
        runtime: Arc<McpRuntimeSnapshot>,
    ) {
        self.runtime = Some(CachedSelectedRuntime {
            bindings,
            plugins,
            runtime,
        });
    }
}

fn same_bindings(
    left: &[(usize, ResolvedSelectedCapabilityRoot)],
    right: &[(usize, ResolvedSelectedCapabilityRoot)],
) -> bool {
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

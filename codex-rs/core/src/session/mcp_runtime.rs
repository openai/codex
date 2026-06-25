use std::sync::Arc;

use codex_core_plugins::ExecutorPluginRuntime;
use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_mcp::McpRuntimeSnapshot;

/// One live selected-plugin MCP runtime retained between model steps.
///
/// A cached runtime is a reuse candidate only for the same ordered selected roots and the same
/// process-local environment instances. The caller additionally compares the effective MCP config
/// and runtime context before reuse. Selected environment contents are treated as stable, so
/// manifest and MCP config file changes do not invalidate this cache.
///
/// Within a live session, the selected runtime is invalidated in exactly two ways:
///
/// 1. [`Self::replace_base_and_invalidate_selected`] installs a new base MCP runtime.
/// 2. [`Self::replace_selected_runtime`] stores a newly projected runtime. This happens when the
///    bindings change, when a previously unavailable plugin appears, or when the effective config
///    or runtime context changes. An unavailable environment disappears from the binding list and
///    therefore follows this path; returning with a new environment instance rebuilds the live
///    runtime even when the stable environment ID is unchanged.
///
/// In-flight [`McpRuntimeSnapshot`] values retain their manager until their model step finishes.
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

    pub(crate) fn plugins_for_bindings(
        &self,
        bindings: &[(usize, ResolvedSelectedCapabilityRoot)],
    ) -> Option<Vec<(usize, ExecutorPluginRuntime)>> {
        self.runtime
            .as_ref()
            .filter(|cached| same_bindings(&cached.bindings, bindings))
            .map(|cached| cached.plugins.clone())
    }

    pub(crate) fn replace_selected_runtime(
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
    // Order is part of the key because later selected roots can be renamed when MCP server names
    // collide. Arc identity is part of the key because live processes and connections belong to
    // one exact environment instance, even when a replacement reuses the same stable ID.
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

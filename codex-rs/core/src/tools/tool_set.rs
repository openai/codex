use std::sync::Arc;

use codex_tools::ToolSpec;

use crate::tools::registry::CoreToolRuntime;

/// Accumulates the concrete tools available for one model request.
///
/// Runtime tools carry both their executable handler and model-visible spec.
/// Hosted model tools have no local runtime, so they are tracked beside the
/// runtimes until the final model-visible spec list is assembled.
#[derive(Default)]
pub(crate) struct ToolSetBuilder {
    runtimes: Vec<Arc<dyn CoreToolRuntime>>,
    hosted_specs: Vec<ToolSpec>,
}

impl ToolSetBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn add_runtime<T>(&mut self, runtime: T)
    where
        T: CoreToolRuntime + 'static,
    {
        self.runtimes.push(Arc::new(runtime));
    }

    pub(crate) fn add_runtime_arc(&mut self, runtime: Arc<dyn CoreToolRuntime>) {
        self.runtimes.push(runtime);
    }

    pub(crate) fn prepend_runtime_arcs<I>(&mut self, runtimes: I)
    where
        I: IntoIterator<Item = Arc<dyn CoreToolRuntime>>,
    {
        self.runtimes.splice(0..0, runtimes);
    }

    pub(crate) fn runtimes(&self) -> &[Arc<dyn CoreToolRuntime>] {
        &self.runtimes
    }

    pub(crate) fn extend_hosted_specs<I>(&mut self, specs: I)
    where
        I: IntoIterator<Item = ToolSpec>,
    {
        self.hosted_specs.extend(specs);
    }

    pub(crate) fn finish(self) -> ToolSet {
        ToolSet {
            runtimes: self.runtimes,
            hosted_specs: self.hosted_specs,
        }
    }
}

pub(crate) struct ToolSet {
    runtimes: Vec<Arc<dyn CoreToolRuntime>>,
    hosted_specs: Vec<ToolSpec>,
}

impl ToolSet {
    pub(crate) fn into_parts(self) -> (Vec<Arc<dyn CoreToolRuntime>>, Vec<ToolSpec>) {
        (self.runtimes, self.hosted_specs)
    }
}

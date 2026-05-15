use std::sync::Arc;

use crate::tools::registry::CoreToolRuntime;

pub(crate) type ToolSet = codex_tools::ToolSet<Arc<dyn CoreToolRuntime>>;
pub(crate) type ToolSetBuilder = codex_tools::ToolSetBuilder<Arc<dyn CoreToolRuntime>>;

pub(crate) trait CoreToolSetBuilderExt {
    fn add_runtime<T>(&mut self, runtime: T)
    where
        T: CoreToolRuntime + 'static;

    fn add_runtime_arc(&mut self, runtime: Arc<dyn CoreToolRuntime>);

    fn prepend_runtime_arcs<I>(&mut self, runtimes: I)
    where
        I: IntoIterator<Item = Arc<dyn CoreToolRuntime>>;
}

impl CoreToolSetBuilderExt for ToolSetBuilder {
    fn add_runtime<T>(&mut self, runtime: T)
    where
        T: CoreToolRuntime + 'static,
    {
        self.push_runtime(Arc::new(runtime));
    }

    fn add_runtime_arc(&mut self, runtime: Arc<dyn CoreToolRuntime>) {
        self.push_runtime(runtime);
    }

    fn prepend_runtime_arcs<I>(&mut self, runtimes: I)
    where
        I: IntoIterator<Item = Arc<dyn CoreToolRuntime>>,
    {
        self.prepend_runtimes(runtimes);
    }
}

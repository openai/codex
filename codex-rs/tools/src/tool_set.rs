use crate::ToolSpec;

/// Accumulates the concrete tools available for one model request.
///
/// Runtime tools carry both their executable handler and model-visible spec.
/// Hosted model tools have no local runtime, so they are tracked beside the
/// runtimes until the final model-visible spec list is assembled.
pub struct ToolSetBuilder<R> {
    runtimes: Vec<R>,
    hosted_specs: Vec<ToolSpec>,
}

impl<R> Default for ToolSetBuilder<R> {
    fn default() -> Self {
        Self {
            runtimes: Vec::new(),
            hosted_specs: Vec::new(),
        }
    }
}

impl<R> ToolSetBuilder<R> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_runtime(&mut self, runtime: R) {
        self.runtimes.push(runtime);
    }

    pub fn prepend_runtimes<I>(&mut self, runtimes: I)
    where
        I: IntoIterator<Item = R>,
    {
        self.runtimes.splice(0..0, runtimes);
    }

    pub fn runtimes(&self) -> &[R] {
        &self.runtimes
    }

    pub fn extend_hosted_specs<I>(&mut self, specs: I)
    where
        I: IntoIterator<Item = ToolSpec>,
    {
        self.hosted_specs.extend(specs);
    }

    pub fn finish(self) -> ToolSet<R> {
        ToolSet {
            runtimes: self.runtimes,
            hosted_specs: self.hosted_specs,
        }
    }
}

/// Concrete tool set produced by a `ToolSetBuilder`.
pub struct ToolSet<R> {
    runtimes: Vec<R>,
    hosted_specs: Vec<ToolSpec>,
}

impl<R> ToolSet<R> {
    pub fn into_parts(self) -> (Vec<R>, Vec<ToolSpec>) {
        (self.runtimes, self.hosted_specs)
    }
}

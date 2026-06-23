use super::*;

pub(super) struct ExecutionScope {
    pub(super) environment_id: String,
    pub(super) execution_id: String,
    state: Arc<NetworkProxyState>,
}

impl Drop for ExecutionScope {
    fn drop(&mut self) {
        self.state.unregister_execution(&self.execution_id);
    }
}

impl NetworkProxy {
    /// Returns a proxy that attributes trusted bridge connections to one execution.
    pub fn for_execution(&self, environment_id: &str, execution_id: String) -> Result<Self> {
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (environment_id, execution_id);
            return Ok(self.clone());
        }

        #[cfg(target_os = "linux")]
        {
            anyhow::ensure!(
                self.execution_scope.is_none(),
                "cannot scope an execution-scoped network proxy"
            );
            self.state.register_execution(environment_id, &execution_id);

            let mut proxy = self.clone();
            proxy.execution_scope = Some(Arc::new(ExecutionScope {
                environment_id: environment_id.to_string(),
                execution_id,
                state: Arc::clone(&self.state),
            }));
            Ok(proxy)
        }
    }
}

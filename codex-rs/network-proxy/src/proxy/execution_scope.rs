use super::*;

pub(super) struct ExecutionScope {
    pub(super) environment_id: String,
    pub(super) execution_id: String,
    pub(super) attribution_token: String,
    // Dropping the execution scope closes this channel and cancels remote reviews.
    pub(super) lifetime_tx: tokio::sync::watch::Sender<()>,
    state: Arc<NetworkProxyState>,
}

impl Drop for ExecutionScope {
    fn drop(&mut self) {
        self.state.unregister_execution(&self.attribution_token);
    }
}

impl NetworkProxy {
    /// Returns a proxy that attributes trusted bridge connections to one execution.
    pub fn for_execution(
        &self,
        environment_id: &str,
        execution_id: &str,
        attribution_token: String,
    ) -> Result<Self> {
        anyhow::ensure!(
            self.execution_scope.is_none(),
            "cannot scope an execution-scoped network proxy"
        );
        self.state
            .register_execution(&attribution_token, environment_id, execution_id);

        let (lifetime_tx, _) = tokio::sync::watch::channel(());
        let mut proxy = self.clone();
        proxy.execution_scope = Some(Arc::new(ExecutionScope {
            environment_id: environment_id.to_string(),
            execution_id: execution_id.to_string(),
            attribution_token,
            lifetime_tx,
            state: Arc::clone(&self.state),
        }));
        Ok(proxy)
    }
}

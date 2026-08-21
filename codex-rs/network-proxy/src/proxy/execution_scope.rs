use super::*;

pub(super) struct ExecutionScope {
    pub(super) environment_id: String,
    pub(super) execution_id: String,
    pub(super) attribution_token: String,
    pub(super) environment_policy: Option<crate::EnvironmentNetworkPolicy>,
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
    /// Returns a proxy with execution-specific attribution and attachment policy.
    pub fn for_execution(
        &self,
        environment_id: &str,
        execution_id: &str,
        attribution_token: String,
        environment_policy: Option<crate::EnvironmentNetworkPolicy>,
        fallback_policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
    ) -> Result<Self> {
        anyhow::ensure!(
            self.execution_scope.is_none(),
            "cannot scope an execution-scoped network proxy"
        );
        self.state
            .register_execution(&attribution_token, environment_id, execution_id);

        let (lifetime_tx, _) = tokio::sync::watch::channel(());
        let mut proxy = self.clone();
        proxy.policy_decider = proxy.policy_decider.or(fallback_policy_decider);
        // Strict attachment allowlists cannot be expanded through approval callbacks.
        if matches!(&environment_policy, Some(policy) if policy.managed_allowed_domains_only) {
            proxy.policy_decider = None;
        }
        proxy.execution_scope = Some(Arc::new(ExecutionScope {
            environment_id: environment_id.to_string(),
            execution_id: execution_id.to_string(),
            attribution_token,
            environment_policy,
            lifetime_tx,
            state: Arc::clone(&self.state),
        }));
        Ok(proxy)
    }
}

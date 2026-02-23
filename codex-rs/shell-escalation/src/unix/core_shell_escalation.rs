use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::escalate_protocol::EscalateAction;
use crate::escalation_policy::EscalationPolicy;
use crate::stopwatch::Stopwatch;
use crate::unix::escalate_server::EscalationPolicyFactory;
use codex_execpolicy::Policy;

#[async_trait]
pub trait ShellActionProvider: Send + Sync {
    async fn determine_action(
        &self,
        file: &Path,
        argv: &[String],
        workdir: &Path,
        stopwatch: &Stopwatch,
    ) -> anyhow::Result<EscalateAction>;
}

#[derive(Clone)]
pub struct ShellPolicyFactory {
    provider: Arc<dyn ShellActionProvider>,
}

impl ShellPolicyFactory {
    pub fn new<P>(provider: P) -> Self
    where
        P: ShellActionProvider + 'static,
    {
        Self {
            provider: Arc::new(provider),
        }
    }

    pub fn with_provider(provider: Arc<dyn ShellActionProvider>) -> Self {
        Self { provider }
    }
}

struct ShellEscalationPolicy {
    provider: Arc<dyn ShellActionProvider>,
    stopwatch: Stopwatch,
}

#[async_trait]
impl EscalationPolicy for ShellEscalationPolicy {
    async fn determine_action(
        &self,
        file: &Path,
        argv: &[String],
        workdir: &Path,
    ) -> anyhow::Result<EscalateAction> {
        self.provider
            .determine_action(file, argv, workdir, &self.stopwatch)
            .await
    }
}

impl EscalationPolicyFactory for ShellPolicyFactory {
    type Policy = ShellEscalationPolicy;

    fn create_policy(&self, _policy: Arc<RwLock<Policy>>, stopwatch: Stopwatch) -> Self::Policy {
        ShellEscalationPolicy {
            provider: Arc::clone(&self.provider),
            stopwatch,
        }
    }
}

use std::sync::Arc;

use crate::environment_selection::ThreadEnvironments;

#[derive(Clone)]
pub struct EnvironmentWaiter {
    environments: Arc<ThreadEnvironments>,
}

impl EnvironmentWaiter {
    pub(crate) fn new(environments: Arc<ThreadEnvironments>) -> Self {
        Self { environments }
    }

    pub async fn wait_until_ready(&self) {
        self.environments.snapshot().await;
    }
}

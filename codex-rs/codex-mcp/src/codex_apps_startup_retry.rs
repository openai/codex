use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt;
use tokio::sync::Mutex;
use tokio::time::Instant;

use crate::rmcp_client::ManagedClientFuture;
use crate::rmcp_client::StartupOutcomeError;

// Every recreated startup performs a fresh initialize + uncached tools/list and
// rewrites the tools cache on success. Each recovery episode is bounded to two
// shared attempts. A failed episode enters a cooldown before a later tool build
// can begin another episode.
pub(crate) const CODEX_APPS_STARTUP_RETRY_COOLDOWN: Duration = Duration::from_secs(30);

enum RetryPhase {
    Idle,
    InFlight {
        generation: u64,
        client: ManagedClientFuture,
    },
    Settled {
        client: ManagedClientFuture,
        retry_after: Option<Instant>,
    },
}

struct RetryState {
    next_generation: u64,
    phase: RetryPhase,
}

pub(crate) struct CodexAppsStartupRetry {
    factory: Arc<dyn Fn() -> ManagedClientFuture + Send + Sync>,
    cooldown: Duration,
    state: Mutex<RetryState>,
}

impl CodexAppsStartupRetry {
    pub(crate) fn new(factory: Arc<dyn Fn() -> ManagedClientFuture + Send + Sync>) -> Self {
        Self::new_with_cooldown(factory, CODEX_APPS_STARTUP_RETRY_COOLDOWN)
    }

    pub(crate) fn new_with_cooldown(
        factory: Arc<dyn Fn() -> ManagedClientFuture + Send + Sync>,
        cooldown: Duration,
    ) -> Self {
        Self {
            factory,
            cooldown,
            state: Mutex::new(RetryState {
                next_generation: 0,
                phase: RetryPhase::Idle,
            }),
        }
    }

    pub(crate) async fn replacement(&self) -> Option<ManagedClientFuture> {
        let state = self.state.lock().await;
        match &state.phase {
            RetryPhase::Idle => None,
            RetryPhase::InFlight { client, .. } | RetryPhase::Settled { client, .. } => {
                Some(client.clone())
            }
        }
    }

    pub(crate) async fn retry(&self) {
        let Some((generation, retry)) = self.retry_for_current_episode().await else {
            return;
        };
        let outcome = retry.clone().await;
        let retry_after = match &outcome {
            Ok(_) | Err(StartupOutcomeError::Cancelled) => None,
            Err(StartupOutcomeError::Failed { .. }) => Some(Instant::now() + self.cooldown),
        };

        let mut state = self.state.lock().await;
        if matches!(
            state.phase,
            RetryPhase::InFlight {
                generation: active_generation,
                ..
            } if active_generation == generation
        ) {
            state.phase = RetryPhase::Settled {
                client: retry,
                retry_after,
            };
        }
    }

    async fn retry_for_current_episode(&self) -> Option<(u64, ManagedClientFuture)> {
        let mut state = self.state.lock().await;
        match &state.phase {
            RetryPhase::InFlight { generation, client } => {
                return Some((*generation, client.clone()));
            }
            RetryPhase::Settled {
                retry_after: None, ..
            } => return None,
            RetryPhase::Settled {
                retry_after: Some(retry_after),
                ..
            } if Instant::now() < *retry_after => return None,
            RetryPhase::Idle | RetryPhase::Settled { .. } => {}
        }

        let generation = state.next_generation;
        state.next_generation += 1;
        let retry = {
            let factory = Arc::clone(&self.factory);
            async move {
                match (factory)().await {
                    Ok(client) => Ok(client),
                    Err(StartupOutcomeError::Failed { .. }) => (factory)().await,
                    Err(StartupOutcomeError::Cancelled) => Err(StartupOutcomeError::Cancelled),
                }
            }
            .boxed()
            .shared()
        };
        state.phase = RetryPhase::InFlight {
            generation,
            client: retry.clone(),
        };
        Some((generation, retry))
    }
}

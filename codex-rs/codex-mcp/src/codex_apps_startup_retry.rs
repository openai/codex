use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt;
use tokio::sync::Mutex;
use tokio::time::Instant;

use crate::rmcp_client::ManagedClientFuture;
use crate::rmcp_client::StartupOutcomeError;

// Every recreated startup performs a fresh initialize + uncached tools/list and
// rewrites the tools cache on success. Each recovery episode is bounded to two
// shared attempts. Failed episodes use exponential backoff with per-manager
// jitter before a later tool build can begin another episode.
const CODEX_APPS_STARTUP_RETRY_BASE_BACKOFF: Duration = Duration::from_secs(30);
const CODEX_APPS_STARTUP_RETRY_MAX_BACKOFF: Duration = Duration::from_secs(5 * 60);
const CODEX_APPS_STARTUP_RETRY_JITTER_DIVISOR: u32 = 5;

enum RetryBackoff {
    ExponentialWithJitter(RandomState),
    #[cfg(test)]
    Fixed(Duration),
}

impl RetryBackoff {
    fn exponential_with_jitter() -> Self {
        Self::ExponentialWithJitter(RandomState::new())
    }

    fn delay(&self, consecutive_failures: u32, generation: u64) -> Duration {
        match self {
            Self::ExponentialWithJitter(random_state) => jittered_exponential_backoff(
                consecutive_failures,
                random_state.hash_one((consecutive_failures, generation)),
            ),
            #[cfg(test)]
            Self::Fixed(delay) => *delay,
        }
    }
}

fn jittered_exponential_backoff(consecutive_failures: u32, jitter_sample: u64) -> Duration {
    let multiplier = 1u32
        .checked_shl(consecutive_failures.saturating_sub(1))
        .unwrap_or(u32::MAX);
    let nominal = CODEX_APPS_STARTUP_RETRY_BASE_BACKOFF
        .saturating_mul(multiplier)
        .min(CODEX_APPS_STARTUP_RETRY_MAX_BACKOFF);
    let jitter = nominal / CODEX_APPS_STARTUP_RETRY_JITTER_DIVISOR;
    let lower = nominal.saturating_sub(jitter);
    let upper = nominal
        .saturating_add(jitter)
        .min(CODEX_APPS_STARTUP_RETRY_MAX_BACKOFF);
    let lower_millis = u64::try_from(lower.as_millis()).unwrap_or(u64::MAX);
    let upper_millis = u64::try_from(upper.as_millis()).unwrap_or(u64::MAX);
    let jitter_range = upper_millis.saturating_sub(lower_millis);
    let jitter_offset = jitter_sample % jitter_range.saturating_add(1);
    Duration::from_millis(lower_millis.saturating_add(jitter_offset))
}

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
    consecutive_failures: u32,
    phase: RetryPhase,
}

pub(crate) struct CodexAppsStartupRetry {
    factory: Arc<dyn Fn() -> ManagedClientFuture + Send + Sync>,
    backoff: RetryBackoff,
    state: Mutex<RetryState>,
}

impl CodexAppsStartupRetry {
    pub(crate) fn new(factory: Arc<dyn Fn() -> ManagedClientFuture + Send + Sync>) -> Self {
        Self::new_with_backoff(factory, RetryBackoff::exponential_with_jitter())
    }

    #[cfg(test)]
    pub(crate) fn new_with_cooldown(
        factory: Arc<dyn Fn() -> ManagedClientFuture + Send + Sync>,
        cooldown: Duration,
    ) -> Self {
        Self::new_with_backoff(factory, RetryBackoff::Fixed(cooldown))
    }

    fn new_with_backoff(
        factory: Arc<dyn Fn() -> ManagedClientFuture + Send + Sync>,
        backoff: RetryBackoff,
    ) -> Self {
        Self {
            factory,
            backoff,
            state: Mutex::new(RetryState {
                next_generation: 0,
                consecutive_failures: 0,
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

        let mut state = self.state.lock().await;
        if matches!(
            state.phase,
            RetryPhase::InFlight {
                generation: active_generation,
                ..
            } if active_generation == generation
        ) {
            let retry_after = match &outcome {
                Ok(_) => {
                    state.consecutive_failures = 0;
                    None
                }
                Err(StartupOutcomeError::Cancelled) => None,
                Err(StartupOutcomeError::Failed { .. }) => {
                    state.consecutive_failures = state.consecutive_failures.saturating_add(1);
                    Some(
                        Instant::now() + self.backoff.delay(state.consecutive_failures, generation),
                    )
                }
            };
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

#[cfg(test)]
#[path = "codex_apps_startup_retry_tests.rs"]
mod tests;

//! Lazy V2 responses, cached per environment, executor session, cwd, and sandbox.
//! Invalidation replaces entries with empty forced loads; in-flight readers may finish with older results.
//! Consumers detect recovery and replacement by comparing discovery snapshots.
//! Failed loads have a per-context cooldown; lookups never sleep or schedule background retries.

use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_utils_path_uri::PathUri;
use tokio::sync::OnceCell;
use tokio::time::Instant;

use crate::DiscoverV2CapabilitiesRequest;
use crate::DiscoverV2CapabilitiesResponse;
use crate::Environment;
use crate::ExecServerError;
use crate::FileSystemSandboxContext;

const MIN_CACHED_DISCOVERY_CONTEXTS: usize = 16;
const INITIAL_DISCOVERY_RETRY_DELAY: Duration = Duration::from_secs(1);
const MAX_DISCOVERY_RETRY_DELAY: Duration = Duration::from_secs(30);

/// Shared by turn and idle consumers until Core explicitly invalidates discovery.
#[derive(Default)]
pub struct ExecutorCapabilityDiscoveryV2Cache {
    entries: Mutex<Vec<Arc<DiscoveryV2CacheEntry>>>,
    capacity: AtomicUsize,
}

// Reload controls how this entry is populated, not which inventory it identifies.
#[derive(Clone)]
struct DiscoveryCacheKey {
    environment: Weak<Environment>,
    session_id: Option<String>,
    cwd: PathUri,
    sandbox: Option<FileSystemSandboxContext>,
}

impl PartialEq for DiscoveryCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.environment.ptr_eq(&other.environment)
            && self.session_id == other.session_id
            && self.cwd == other.cwd
            && self.sandbox == other.sandbox
    }
}

struct DiscoveryV2CacheEntry {
    key: DiscoveryCacheKey,
    force_reload: bool,
    response: OnceCell<Arc<DiscoverV2CapabilitiesResponse>>,
    failure: Mutex<Option<DiscoveryV2Failure>>,
}

struct DiscoveryV2Failure {
    error: Arc<ExecServerError>,
    attempts: u32,
    next_retry_at: Instant,
}

impl DiscoveryV2CacheEntry {
    fn force_reload(&self) -> Self {
        // Discard the old snapshot and cooldown, but retain the forced reload through retries.
        Self {
            key: self.key.clone(),
            force_reload: true,
            response: OnceCell::new(),
            failure: Mutex::new(None),
        }
    }

    async fn get_or_try_init<F>(
        &self,
        discover: impl FnOnce() -> F,
    ) -> Result<Arc<DiscoverV2CapabilitiesResponse>, Arc<ExecServerError>>
    where
        F: Future<Output = Result<DiscoverV2CapabilitiesResponse, ExecServerError>>,
    {
        self.response
            .get_or_try_init(|| async {
                // Check here so waiting callers don't immediately retry a failed request.
                {
                    let failure_state = self
                        .failure
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if let Some(failure) = failure_state.as_ref()
                        && Instant::now() < failure.next_retry_at
                    {
                        return Err(Arc::clone(&failure.error));
                    }
                }
                let result = discover().await;
                let mut failure_state = self
                    .failure
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                match result {
                    Ok(response) => {
                        *failure_state = None;
                        Ok(Arc::new(response))
                    }
                    Err(error) => {
                        let attempts = failure_state
                            .as_ref()
                            .map_or(1, |failure| failure.attempts.saturating_add(1));
                        let delay = INITIAL_DISCOVERY_RETRY_DELAY
                            .saturating_mul(2_u32.saturating_pow(attempts - 1))
                            .min(MAX_DISCOVERY_RETRY_DELAY);
                        let error = Arc::new(error);
                        *failure_state = Some(DiscoveryV2Failure {
                            error: Arc::clone(&error),
                            attempts,
                            next_retry_at: Instant::now() + delay,
                        });
                        Err(error)
                    }
                }
            })
            .await
            .cloned()
    }
}

impl ExecutorCapabilityDiscoveryV2Cache {
    /// Fits a complete environment selection; never shrinks during the cache's lifetime.
    pub fn ensure_capacity(&self, minimum: usize) {
        self.capacity.fetch_max(minimum, Ordering::Relaxed);
    }

    /// Makes future lookups reload every context without interrupting existing loads.
    pub fn invalidate(&self) {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for entry in entries.iter_mut() {
            *entry = Arc::new(entry.force_reload());
        }
    }

    /// Invalidates an environment's entries across all executor sessions, cwds, and sandboxes.
    pub fn invalidate_environment(&self, environment: &Arc<Environment>) {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let environment = Arc::downgrade(environment);
        for entry in entries
            .iter_mut()
            .filter(|entry| entry.key.environment.ptr_eq(&environment))
        {
            *entry = Arc::new(entry.force_reload());
        }
    }

    /// Reuses successful inventories; failed loads retry on the first lookup after their cooldown.
    /// Cancellation adds no cooldown; explicit invalidation and new sessions start fresh loads.
    /// A connection error classification does not determine discovery's lifetime.
    /// Invalidation during a load applies to the next lookup, not its current readers.
    pub async fn discover(
        &self,
        environment_id: &str,
        environment: &Arc<Environment>,
        mut request: DiscoverV2CapabilitiesRequest,
    ) -> Result<Arc<DiscoverV2CapabilitiesResponse>, Arc<ExecServerError>> {
        // Resolve startup/replacement before lookup; a resumed session keeps its cache entries.
        let session_id = environment.executor_session_id().await.map_err(|error| {
            tracing::warn!(environment_id, "executor discovery V2 session unavailable");
            Arc::new(error)
        })?;
        let key = DiscoveryCacheKey {
            environment: Arc::downgrade(environment),
            session_id,
            cwd: request.cwd.clone(),
            sandbox: request.sandbox.clone(),
        };
        let entry = {
            let mut entries = self
                .entries
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let matching = entries.iter().position(|entry| entry.key == key);
            if let Some(entry) = matching
                .map(|index| &entries[index])
                .filter(|_| !request.force_reload)
            {
                Arc::clone(entry)
            } else {
                if let Some(index) = matching {
                    entries.remove(index);
                }
                let entry = Arc::new(DiscoveryV2CacheEntry {
                    key,
                    force_reload: request.force_reload,
                    response: OnceCell::new(),
                    failure: Mutex::new(None),
                });
                let capacity = self
                    .capacity
                    .load(Ordering::Relaxed)
                    .max(MIN_CACHED_DISCOVERY_CONTEXTS);
                // Existing readers retain their entries after replacement or eviction.
                if entries.len() >= capacity {
                    entries.remove(0);
                }
                entries.push(Arc::clone(&entry));
                entry
            }
        };
        entry
            .get_or_try_init(|| async {
                request.force_reload = entry.force_reload;
                environment
                    .discover_capabilities_v2(request)
                    .await
                    .inspect_err(|_| {
                        tracing::warn!(environment_id, "executor discovery V2 failed");
                    })
            })
            .await
    }
}

#[cfg(test)]
#[path = "capability_discovery_v2_cache_tests.rs"]
mod tests;

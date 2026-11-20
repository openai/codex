//! Lazy V2 responses, cached per environment, executor session, cwd, and sandbox.
//! Dirty entries are replaced on the next lookup; in-flight readers may finish with older results.
//! Reports recovery from retryable failures so Core can refresh dependent MCP projections.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use tokio::sync::OnceCell;

use crate::DiscoverV2CapabilitiesRequest;
use crate::DiscoverV2CapabilitiesResponse;
use crate::Environment;
use crate::ExecServerError;

const MIN_CACHED_DISCOVERY_CONTEXTS: usize = 16;

/// Shared by turn and idle consumers until Core explicitly invalidates discovery.
#[derive(Default)]
pub struct ExecutorCapabilityDiscoveryV2Cache {
    entries: Mutex<Vec<CachedEnvironmentDiscovery>>,
    capacity: AtomicUsize,
    recovered_discovery: AtomicBool,
}

struct CachedEnvironmentDiscovery {
    request: DiscoverV2CapabilitiesRequest,
    environment: Weak<Environment>,
    session_id: Option<String>,
    response: Arc<OnceCell<Result<Arc<DiscoverV2CapabilitiesResponse>, Arc<ExecServerError>>>>,
    retryable: Arc<AtomicBool>,
    dirty: bool,
}

impl ExecutorCapabilityDiscoveryV2Cache {
    /// Fits a complete environment selection; never shrinks during the cache's lifetime.
    pub fn ensure_capacity(&self, minimum: usize) {
        self.capacity.fetch_max(minimum, Ordering::Relaxed);
    }

    /// Reports whether a failed discovery recovered since the last observation.
    pub fn take_recovered_discovery(&self) -> bool {
        self.recovered_discovery.swap(false, Ordering::AcqRel)
    }

    /// Makes future lookups reload every context without interrupting existing loads.
    pub fn invalidate(&self) {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for entry in entries.iter_mut() {
            entry.dirty = true;
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
            .filter(|entry| entry.environment.ptr_eq(&environment))
        {
            entry.dirty = true;
        }
    }

    /// Reuses successes and non-retryable failures using V1's error classification.
    /// Retryable failures and cancelled loads leave the cell available for retry.
    /// Invalidation during a load applies to the next lookup, not its current readers.
    pub async fn discover(
        &self,
        environment_id: &str,
        environment: &Arc<Environment>,
        request: DiscoverV2CapabilitiesRequest,
    ) -> Result<Arc<DiscoverV2CapabilitiesResponse>, Arc<ExecServerError>> {
        // Resolve startup/replacement before lookup; a resumed session keeps its cache entries.
        let session_id = environment.executor_session_id().await.map_err(|error| {
            tracing::warn!(environment_id, "executor discovery V2 session unavailable");
            Arc::new(error)
        })?;
        let (response, retryable) = {
            let mut entries = self
                .entries
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let environment = Arc::downgrade(environment);
            if let Some(entry) = entries.iter().find(|entry| {
                !entry.dirty
                    && entry.environment.ptr_eq(&environment)
                    && entry.session_id == session_id
                    && entry.request == request
            }) {
                (Arc::clone(&entry.response), Arc::clone(&entry.retryable))
            } else {
                // Recovery belongs to the environment/request, not one executor session.
                let retryable = entries
                    .iter()
                    .find(|entry| {
                        !entry.dirty
                            && entry.environment.ptr_eq(&environment)
                            && entry.request == request
                    })
                    .map(|entry| Arc::clone(&entry.retryable))
                    .unwrap_or_default();
                let entry = CachedEnvironmentDiscovery {
                    request: request.clone(),
                    environment,
                    session_id,
                    response: Arc::new(OnceCell::new()),
                    retryable,
                    dirty: false,
                };
                let capacity = self
                    .capacity
                    .load(Ordering::Relaxed)
                    .max(MIN_CACHED_DISCOVERY_CONTEXTS);
                // Existing readers retain their response cells after replacement or eviction.
                entries.retain(|entry| !entry.dirty);
                if entries.len() >= capacity {
                    entries.remove(0);
                }
                let load = (Arc::clone(&entry.response), Arc::clone(&entry.retryable));
                entries.push(entry);
                load
            }
        };
        let response = response
            .get_or_try_init(|| async {
                match environment.discover_capabilities_v2(request).await {
                    Ok(response) => Ok(Ok(Arc::new(response))),
                    Err(error) => {
                        let can_retry = crate::client::can_retry_connection_attempt(&error);
                        tracing::warn!(environment_id, "executor discovery V2 failed");
                        let error = Arc::new(error);
                        if can_retry {
                            retryable.store(true, Ordering::Release);
                            Err(error)
                        } else {
                            Ok(Err(error))
                        }
                    }
                }
            })
            .await
            .cloned()
            .unwrap_or_else(Err);
        if response.is_ok() && retryable.swap(false, Ordering::AcqRel) {
            self.recovered_discovery.store(true, Ordering::Release);
        }
        response
    }
}

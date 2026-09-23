//! Retains watched global locations and sandbox-scoped response snapshots.
//! Replacement snapshots isolate in-flight loads; unavailable watchers force request-time reloads.

use crate::DiscoverV2CapabilitiesRequest;
use crate::DiscoverV2CapabilitiesResponse;
use crate::FileSystemSandboxContext;
use crate::LocalFileSystem;
use crate::discover_v2::capability_discoveries::load_capability_discoveries;
use crate::discover_v2::capability_locations::CapabilityLocationRequest;
use crate::discover_v2::capability_locations::CapabilityLocations;
use crate::discover_v2::capability_locations::prewarm_locations;
use codex_file_watcher::WatchRegistration;
use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;
use tokio::sync::Semaphore;

mod capability_watchers;
use capability_watchers::CapabilityWatchers;

const MAX_CACHED_SANDBOX_CONTEXTS: usize = 16;

pub(crate) struct CapabilityManager {
    file_system: LocalFileSystem,
    prewarmed_locations: OnceCell<(CapabilityLocationRequest, CapabilityLocations)>,
    current_state: Mutex<Option<Arc<LocationState>>>,
    location_refresh_lock: Semaphore,
    watchers: CapabilityWatchers,
}

struct LocationState {
    dirty: AtomicBool,
    locations: CapabilityLocations,
    sandbox_discoveries: Mutex<Vec<Arc<CachedSandboxDiscovery>>>,
    _registration: WatchRegistration,
}

struct CachedSandboxDiscovery {
    sandbox: Option<FileSystemSandboxContext>,
    response: OnceCell<Arc<DiscoverV2CapabilitiesResponse>>,
}

impl CapabilityManager {
    pub(crate) fn new(file_system: LocalFileSystem) -> Arc<Self> {
        Arc::new(Self {
            file_system,
            prewarmed_locations: OnceCell::new(),
            current_state: Mutex::default(),
            location_refresh_lock: Semaphore::new(/*permits*/ 1),
            watchers: CapabilityWatchers::new(),
        })
    }

    pub(crate) async fn prewarm_locations(
        self: &Arc<Self>,
        request: CapabilityLocationRequest,
    ) -> io::Result<&(CapabilityLocationRequest, CapabilityLocations)> {
        let cached = self
            .prewarmed_locations
            .get_or_try_init(|| async {
                let locations = prewarm_locations(&self.file_system, &request).await?;
                Ok::<_, io::Error>((request.clone(), locations))
            })
            .await?;
        // Check the winning request even when another caller initialized the cell.
        if cached.0 != request {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "capability home paths differ from the prewarmed configuration",
            ));
        }
        self.initialize_watchers(&cached.1).await;
        Ok(cached)
    }

    /// Reuses startup locations, retrying prewarming if the background scan failed.
    pub(crate) async fn get_or_refresh_discovery(
        self: &Arc<Self>,
        location_request: CapabilityLocationRequest,
        params: DiscoverV2CapabilitiesRequest,
    ) -> io::Result<Arc<DiscoverV2CapabilitiesResponse>> {
        let (location_request, _) = match self.prewarm_locations(location_request).await {
            Ok(prewarmed) => prewarmed,
            Err(error) => {
                return Ok(Arc::new(DiscoverV2CapabilitiesResponse {
                    warnings: vec![format!(
                        "could not prewarm capability discovery: {:?}",
                        error.kind()
                    )],
                    ..Default::default()
                }));
            }
        };
        let location_state = self.get_or_refresh_locations(location_request).await?;
        let discovery = {
            let mut discoveries = location_state.sandbox_discoveries.lock().await;
            if let Some(discovery) = discoveries
                .iter()
                .find(|discovery| discovery.sandbox == params.sandbox)
            {
                Arc::clone(discovery)
            } else {
                let discovery = Arc::new(CachedSandboxDiscovery {
                    sandbox: params.sandbox,
                    response: OnceCell::new(),
                });
                if discoveries.len() == MAX_CACHED_SANDBOX_CONTEXTS {
                    // Evict the oldest context (FIFO); in-flight requests retain their own Arc.
                    discoveries.remove(0);
                }
                discoveries.push(Arc::clone(&discovery));
                discovery
            }
        };
        if self.watchers.has_subscription() {
            return discovery
                .response
                .get_or_try_init(|| {
                    load_capability_discoveries(
                        &self.file_system,
                        &location_state.locations,
                        discovery.sandbox.as_ref(),
                    )
                })
                .await
                .map(Arc::clone);
        }
        load_capability_discoveries(
            &self.file_system,
            &location_state.locations,
            discovery.sandbox.as_ref(),
        )
        .await
    }

    async fn get_or_refresh_locations(
        &self,
        request: &CapabilityLocationRequest,
    ) -> io::Result<Arc<LocationState>> {
        if self.watchers.has_subscription()
            && let Some(location_state) = self.location_snapshot().await
            && !location_state.dirty.load(Ordering::Relaxed)
        {
            return Ok(location_state);
        }
        let _refresh = self
            .location_refresh_lock
            .acquire()
            .await
            .map_err(io::Error::other)?;
        if self.watchers.has_subscription()
            && let Some(location_state) = self.location_snapshot().await
            && !location_state.dirty.load(Ordering::Relaxed)
        {
            return Ok(location_state);
        }
        // Retry dirty locations; without a subscription, reload on every request.
        let locations = match prewarm_locations(&self.file_system, request).await {
            Ok(locations) => locations,
            Err(error) => {
                if let Some(location_state) = self.location_snapshot().await {
                    tracing::warn!(error_kind = ?error.kind(), "capability refresh failed; retrying on the next request");
                    return Ok(location_state);
                }
                return Err(error);
            }
        };
        let next = Arc::new(LocationState {
            dirty: AtomicBool::new(false),
            _registration: self.watchers.register_paths(&locations),
            locations,
            sandbox_discoveries: Mutex::default(),
        });
        *self.current_state.lock().await = Some(Arc::clone(&next));
        Ok(next)
    }

    async fn location_snapshot(&self) -> Option<Arc<LocationState>> {
        self.current_state.lock().await.clone()
    }
}

#[cfg(test)]
#[path = "tests/capability_manager_tests.rs"]
mod tests;

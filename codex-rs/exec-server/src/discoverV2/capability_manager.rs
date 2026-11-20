//! Retains watched global locations and sandbox-scoped response snapshots.
//! Replacement snapshots isolate in-flight loads; failed watch registration forces request-time reloads.

use crate::DiscoverV2CapabilitiesResponse;
use crate::FileSystemSandboxContext;
use crate::LocalFileSystem;
use crate::discover_v2::capability_discoveries::load_capability_discoveries;
use crate::discover_v2::capability_locations::CapabilityLocationRequest;
use crate::discover_v2::capability_locations::CapabilityLocations;
use crate::discover_v2::capability_locations::prewarm_locations;
use codex_config::loader::system_config_toml_file;
use codex_file_watcher::WatchRegistration;
use codex_utils_path_uri::PathUri;
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
    registration: WatchRegistration,
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

    /// Resolves fixed paths on first initialization and reuses the prewarmed locations.
    pub(crate) async fn prewarm_locations(
        self: &Arc<Self>,
    ) -> io::Result<&(CapabilityLocationRequest, CapabilityLocations)> {
        let cached = self
            .prewarmed_locations
            .get_or_try_init(|| async {
                let request = CapabilityLocationRequest {
                    codex_home: PathUri::from_abs_path(&codex_utils_home_dir::find_codex_home()?),
                    user_home: dirs::home_dir()
                        .and_then(|home| PathUri::from_host_native_path(home).ok()),
                    system_skills_root: system_config_toml_file()?
                        .parent()
                        .map(|root| PathUri::from_abs_path(&root.join("skills"))),
                };
                let locations = prewarm_locations(&self.file_system, &request).await?;
                Ok::<_, io::Error>((request, locations))
            })
            .await?;
        self.initialize_watchers(&cached.1).await;
        Ok(cached)
    }

    /// Reuses startup locations, retrying prewarming if the background scan failed.
    pub(crate) async fn discover(
        self: &Arc<Self>,
        sandbox: Option<FileSystemSandboxContext>,
        force_reload: bool,
    ) -> io::Result<Arc<DiscoverV2CapabilitiesResponse>> {
        let (location_request, _) = self.prewarm_locations().await?;
        let location_state = self
            .get_or_refresh_locations(location_request, force_reload)
            .await?;
        let discovery = {
            let mut discoveries = location_state.sandbox_discoveries.lock().await;
            if let Some(discovery) = discoveries
                .iter()
                .find(|discovery| discovery.sandbox == sandbox)
            {
                Arc::clone(discovery)
            } else {
                let discovery = Arc::new(CachedSandboxDiscovery {
                    sandbox,
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
        if location_state.registration.registration_succeeded() {
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
        force_reload: bool,
    ) -> io::Result<Arc<LocationState>> {
        if !force_reload
            && let Some(location_state) = self.location_snapshot().await
            && location_state.registration.registration_succeeded()
            && !location_state.dirty.load(Ordering::Relaxed)
        {
            return Ok(location_state);
        }
        let _refresh = self
            .location_refresh_lock
            .acquire()
            .await
            .map_err(io::Error::other)?;
        if !force_reload
            && let Some(location_state) = self.location_snapshot().await
            && location_state.registration.registration_succeeded()
            && !location_state.dirty.load(Ordering::Relaxed)
        {
            return Ok(location_state);
        }
        // Retry dirty locations; failed watch registration reloads on every request.
        let locations = match prewarm_locations(&self.file_system, request).await {
            Ok(locations) => locations,
            Err(error) => {
                if !force_reload && let Some(location_state) = self.location_snapshot().await {
                    tracing::warn!(error_kind = ?error.kind(), "capability refresh failed; retrying on the next request");
                    return Ok(location_state);
                }
                return Err(error);
            }
        };
        let next = Arc::new(LocationState {
            dirty: AtomicBool::new(false),
            registration: self.watchers.register_paths(&locations),
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

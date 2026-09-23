//! Refreshes global locations and invalidates sandbox responses through one watcher subscription.
//! In-flight readers retain the previous snapshot while a replacement is prepared.

use super::CapabilityManager;
use super::LocationState;
use crate::discover_v2::capability_locations::CapabilityLocations;
use crate::discover_v2::capability_locations::prewarm_locations;
use codex_file_watcher::FileWatcher;
use codex_file_watcher::FileWatcherSubscriber;
use codex_file_watcher::ThrottledWatchReceiver;
use codex_file_watcher::WatchRegistration;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_util::task::AbortOnDropHandle;

#[cfg(not(test))]
const WATCHER_THROTTLE_INTERVAL: Duration = Duration::from_secs(10);
#[cfg(test)]
const WATCHER_THROTTLE_INTERVAL: Duration = Duration::from_millis(50);

pub(super) struct CapabilityWatchers {
    file_watcher: Option<Arc<FileWatcher>>,
    subscription: OnceLock<RootSubscription>,
}

struct RootSubscription {
    subscriber: FileWatcherSubscriber,
    _listener: AbortOnDropHandle<()>,
}

impl CapabilityWatchers {
    pub(super) fn new() -> Self {
        let file_watcher = match FileWatcher::new() {
            Ok(watcher) => Some(Arc::new(watcher)),
            Err(error) => {
                tracing::warn!(%error, "capability watcher unavailable; using uncached discovery");
                None
            }
        };
        Self {
            file_watcher,
            subscription: OnceLock::new(),
        }
    }

    pub(super) fn has_subscription(&self) -> bool {
        self.subscription.get().is_some()
    }

    /// Registers fixed roots, including missing ones, without separate watches for symlink targets.
    pub(super) fn register_paths(&self, locations: &CapabilityLocations) -> WatchRegistration {
        let Some(subscription) = self.subscription.get() else {
            return WatchRegistration::default();
        };
        subscription
            .subscriber
            .register_paths(locations.watches.clone())
    }
}

impl CapabilityManager {
    pub(super) async fn initialize_watchers(self: &Arc<Self>, prewarmed: &CapabilityLocations) {
        let Some(watcher) = &self.watchers.file_watcher else {
            return;
        };
        if self.watchers.subscription.get().is_some() {
            return;
        }
        let Ok(_refresh) = self.location_refresh_lock.acquire().await else {
            return;
        };
        if self.watchers.subscription.get().is_some() {
            return;
        }
        let (subscriber, events) = watcher.add_subscriber();
        let mut events = ThrottledWatchReceiver::new(events, WATCHER_THROTTLE_INTERVAL);
        let registration = subscriber.register_paths(prewarmed.watches.clone());
        let next = Arc::new(LocationState {
            dirty: AtomicBool::new(false),
            locations: prewarmed.clone(),
            sandbox_discoveries: Mutex::default(),
            _registration: registration,
        });
        let mut current_state = self.current_state.lock().await;
        let weak = Arc::downgrade(self);
        let listener = tokio::spawn(async move {
            while events.recv().await.is_some() {
                let Some(manager) = weak.upgrade() else { break };
                manager.refresh_locations().await;
            }
        });
        // No await between retaining the subscription and publishing its snapshot.
        if self
            .watchers
            .subscription
            .set(RootSubscription {
                subscriber,
                _listener: AbortOnDropHandle::new(listener),
            })
            .is_err()
        {
            return;
        }
        *current_state = Some(next);
    }

    pub(super) async fn refresh_locations(&self) {
        let result: std::io::Result<()> = async {
            let _refresh = self
                .location_refresh_lock
                .acquire()
                .await
                .map_err(std::io::Error::other)?;
            let Some((request, _)) = self.prewarmed_locations.get() else {
                return Ok(());
            };
            // Keep failed or cancelled refreshes retryable without discarding the last snapshot.
            if let Some(location_state) = self.location_snapshot().await {
                location_state.dirty.store(true, Ordering::Relaxed);
            }
            let locations = prewarm_locations(&self.file_system, request).await?;
            let registration = self.watchers.register_paths(&locations);
            let next = Arc::new(LocationState {
                dirty: AtomicBool::new(false),
                locations,
                sandbox_discoveries: Mutex::default(),
                _registration: registration,
            });
            // Requests populate the new cache lazily; in-flight loads retain their old snapshot.
            *self.current_state.lock().await = Some(next);
            Ok(())
        }
        .await;
        if let Err(error) = result {
            tracing::warn!(error_kind = ?error.kind(), "capability refresh failed; keeping previous complete snapshot");
        }
    }
}

#[cfg(test)]
#[path = "../tests/capability_watchers_tests.rs"]
pub(super) mod tests;

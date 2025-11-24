use crate::signal::os::{OsExtraData, OsStorage};
use crate::sync::watch;

use std::ops;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

pub(crate) type EventId = usize;

/// State for a specific event, whether a notification is pending delivery,
/// and what listeners are registered.
#[derive(Debug)]
pub(crate) struct EventInfo {
    pending: AtomicBool,
    tx: watch::Sender<()>,
}

impl Default for EventInfo {
    fn default() -> Self {
        let (tx, _rx) = watch::channel(());

        Self {
            pending: AtomicBool::new(false),
            tx,
        }
    }
}

/// An interface for retrieving the `EventInfo` for a particular `eventId`.
pub(crate) trait Storage {
    /// Gets the `EventInfo` for `id` if it exists.
    fn event_info(&self, id: EventId) -> Option<&EventInfo>;

    /// Invokes `f` once for each defined `EventInfo` in this storage.
    fn for_each<'a, F>(&'a self, f: F)
    where
        F: FnMut(&'a EventInfo);
}

impl Storage for Vec<EventInfo> {
    fn event_info(&self, id: EventId) -> Option<&EventInfo> {
        self.get(id)
    }

    fn for_each<'a, F>(&'a self, f: F)
    where
        F: FnMut(&'a EventInfo),
    {
        self.iter().for_each(f);
    }
}

/// An interface for initializing a type. Useful for situations where we cannot
/// inject a configured instance in the constructor of another type.
pub(crate) trait Init {
    fn init() -> Self;
}

/// Manages and distributes event notifications to any registered listeners.
///
/// Generic over the underlying storage to allow for domain specific
/// optimizations (e.g. `eventIds` may or may not be contiguous).
#[derive(Debug)]
pub(crate) struct Registry<S> {
    storage: S,
}

impl<S> Registry<S> {
    fn new(storage: S) -> Self {
        Self { storage }
    }
}

impl<S: Storage> Registry<S> {
    /// Registers a new listener for `event_id`.
    fn register_listener(&self, event_id: EventId) -> watch::Receiver<()> {
        self.storage
            .event_info(event_id)
            .unwrap_or_else(|| panic!("invalid event_id: {event_id}"))
            .tx
            .subscribe()
    }

    /// Marks `event_id` as having been delivered, without broadcasting it to
    /// any listeners.
    fn record_event(&self, event_id: EventId) {
        if let Some(event_info) = self.storage.event_info(event_id) {
            event_info.pending.store(true, Ordering::SeqCst);
        }
    }

    /// Broadcasts all previously recorded events to their respective listeners.
    ///
    /// Returns `true` if an event was delivered to at least one listener.
    fn broadcast(&self) -> bool {
        let mut did_notify = false;
        self.storage.for_each(|event_info| {
            // Any signal of this kind arrived since we checked last?
            if !event_info.pending.swap(false, Ordering::SeqCst) {
                return;
            }

            // Ignore errors if there are no listeners
            if event_info.tx.send(()).is_ok() {
                did_notify = true;
            }
        });

        did_notify
    }
}

pub(crate) struct Globals {
    extra: OsExtraData,
    registry: Registry<OsStorage>,
}

impl ops::Deref for Globals {
    type Target = OsExtraData;

    fn deref(&self) -> &Self::Target {
        &self.extra
    }
}

impl Globals {
    /// Registers a new listener for `event_id`.
    pub(crate) fn register_listener(&self, event_id: EventId) -> watch::Receiver<()> {
        self.registry.register_listener(event_id)
    }

    /// Marks `event_id` as having been delivered, without broadcasting it to
    /// any listeners.
    pub(crate) fn record_event(&self, event_id: EventId) {
        self.registry.record_event(event_id);
    }

    /// Broadcasts all previously recorded events to their respective listeners.
    ///
    /// Returns `true` if an event was delivered to at least one listener.
    pub(crate) fn broadcast(&self) -> bool {
        self.registry.broadcast()
    }

    #[cfg(unix)]
    pub(crate) fn storage(&self) -> &OsStorage {
        &self.registry.storage
    }
}

fn globals_init() -> Globals
where
    OsExtraData: 'static + Send + Sync + Init,
    OsStorage: 'static + Send + Sync + Init,
{
    Globals {
        extra: OsExtraData::init(),
        registry: Registry::new(OsStorage::init()),
    }
}

pub(crate) fn globals() -> &'static Globals
where
    OsExtraData: 'static + Send + Sync + Init,
    OsStorage: 'static + Send + Sync + Init,
{
    static GLOBALS: OnceLock<Globals> = OnceLock::new();

    GLOBALS.get_or_init(globals_init)
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::*;
    use crate::runtime::{self, Runtime};
    use crate::sync::{oneshot, watch};

    use futures::future;

    #[test]
    fn smoke() {
        let rt = rt();
        rt.block_on(async move {
            let registry = Registry::new(vec![
                EventInfo::default(),
                EventInfo::default(),
                EventInfo::default(),
            ]);

            let first = registry.register_listener(0);
            let second = registry.register_listener(1);
            let third = registry.register_listener(2);

            let (fire, wait) = oneshot::channel();

            crate::spawn(async {
                wait.await.expect("wait failed");

                // Record some events which should get coalesced
                registry.record_event(0);
                registry.record_event(0);
                registry.record_event(1);
                registry.record_event(1);
                registry.broadcast();

                // Yield so the previous broadcast can get received
                //
                // This yields many times since the block_on task is only polled every 61
                // ticks.
                for _ in 0..100 {
                    crate::task::yield_now().await;
                }

                // Send subsequent signal
                registry.record_event(0);
                registry.broadcast();

                drop(registry);
            });

            let _ = fire.send(());
            let all = future::join3(collect(first), collect(second), collect(third));

            let (first_results, second_results, third_results) = all.await;
            assert_eq!(2, first_results.len());
            assert_eq!(1, second_results.len());
            assert_eq!(0, third_results.len());
        });
    }

    #[test]
    #[should_panic = "invalid event_id: 1"]
    fn register_panics_on_invalid_input() {
        let registry = Registry::new(vec![EventInfo::default()]);

        registry.register_listener(1);
    }

    #[test]
    fn record_invalid_event_does_nothing() {
        let registry = Registry::new(vec![EventInfo::default()]);
        registry.record_event(1302);
    }

    #[test]
    fn broadcast_returns_if_at_least_one_event_fired() {
        let registry = Registry::new(vec![EventInfo::default(), EventInfo::default()]);

        registry.record_event(0);
        assert!(!registry.broadcast());

        let first = registry.register_listener(0);
        let second = registry.register_listener(1);

        registry.record_event(0);
        assert!(registry.broadcast());

        drop(first);
        registry.record_event(0);
        assert!(!registry.broadcast());

        drop(second);
    }

    fn rt() -> Runtime {
        runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
    }

    async fn collect(mut rx: watch::Receiver<()>) -> Vec<()> {
        let mut ret = vec![];

        while let Ok(v) = rx.changed().await {
            ret.push(v);
        }

        ret
    }
}

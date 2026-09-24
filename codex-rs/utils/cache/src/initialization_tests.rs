//! Checks that initialization releases the global lock and survives LRU eviction.

use super::BlockingLruCache;
use std::num::NonZeroUsize;
use std::sync::mpsc;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn initialization_keeps_other_keys_available_through_eviction() {
    let cache = BlockingLruCache::new(NonZeroUsize::new(/*n*/ 2).expect("capacity"));
    assert_eq!(cache.get_or_init("hot", || 3), 3);
    let handle = tokio::runtime::Handle::current();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();

    std::thread::scope(|scope| {
        let cache = &cache;
        let initializing = scope.spawn(move || {
            let _entered = handle.enter();
            cache.get_or_init("busy", || {
                started_tx.send(()).expect("signal initialization");
                release_rx
                    .recv_timeout(Duration::from_secs(/*secs*/ 5))
                    .expect("other keys should finish before initialization is released");
                7
            })
        });
        started_rx
            .recv_timeout(Duration::from_secs(/*secs*/ 5))
            .expect("initialization should start");

        assert_eq!(cache.get_or_init("hot", || panic!("cached value")), 3);
        assert_eq!(cache.get_or_init("other", || 5), 5);
        assert!(cache.get(&"busy").is_none());

        // The evicted initializer still owns its cell. A new resident cell can
        // independently initialize the same deterministic value for this key.
        assert_eq!(cache.get_or_init("busy", || 7), 7);
        release_tx.send(()).expect("release initialization");
        assert_eq!(initializing.join().expect("initialization thread"), 7);
        assert_eq!(
            cache.with_mut(|entries| {
                entries
                    .iter()
                    .map(|(key, value)| (*key, *value.get().expect("initialized value")))
                    .collect::<Vec<_>>()
            }),
            vec![("busy", 7), ("other", 5)]
        );
    });
}

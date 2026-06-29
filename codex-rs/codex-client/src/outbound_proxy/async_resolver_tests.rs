use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use pretty_assertions::assert_eq;
use tokio::sync::Notify;
use tokio::time::timeout;

use super::*;

const SLOW_URL: &str = "https://example.test/slow";
const FAST_URL: &str = "https://example.test/fast";
const TEST_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 1);

#[tokio::test]
async fn slow_resolution_keeps_runtime_responsive_and_coalesces_duplicates() {
    let slow_calls = Arc::new(AtomicUsize::new(0));
    let slow_started = Arc::new(Notify::new());
    let release_slow = Arc::new((StdMutex::new(false), Condvar::new()));
    let resolver = Arc::new(AsyncSystemProxyResolver::new(
        /*max_concurrent_resolutions*/ 2,
        {
            let slow_calls = Arc::clone(&slow_calls);
            let slow_started = Arc::clone(&slow_started);
            let release_slow = Arc::clone(&release_slow);
            move |request_url| {
                if request_url == SLOW_URL {
                    slow_calls.fetch_add(/*val*/ 1, Ordering::SeqCst);
                    slow_started.notify_one();
                    let (released, condition) = &*release_slow;
                    let mut released = released.lock().unwrap();
                    while !*released {
                        released = condition.wait(released).unwrap();
                    }
                }
                SystemProxyRouteDecision::Direct
            }
        },
    ));

    let first = tokio::spawn({
        let resolver = Arc::clone(&resolver);
        async move { resolver.resolve(SLOW_URL).await }
    });
    timeout(TEST_TIMEOUT, slow_started.notified())
        .await
        .unwrap();

    let duplicate = tokio::spawn({
        let resolver = Arc::clone(&resolver);
        async move { resolver.resolve(SLOW_URL).await }
    });
    let cache_key = system_proxy_cache_key(SLOW_URL);
    let duplicate_joined = timeout(TEST_TIMEOUT, async {
        loop {
            let receiver_count = resolver
                .in_flight
                .lock()
                .await
                .get(&cache_key)
                .map(ResolutionSender::receiver_count)
                .unwrap_or_default();
            if receiver_count >= 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    let fast = timeout(TEST_TIMEOUT, resolver.resolve(FAST_URL)).await;

    let (released, condition) = &*release_slow;
    *released.lock().unwrap() = true;
    condition.notify_all();
    let first = first.await.unwrap();
    let duplicate = duplicate.await.unwrap();

    assert!(duplicate_joined.is_ok());
    assert_eq!(fast.unwrap(), SystemProxyRouteDecision::Direct);
    assert_eq!(first, SystemProxyRouteDecision::Direct);
    assert_eq!(duplicate, SystemProxyRouteDecision::Direct);
    assert_eq!(slow_calls.load(Ordering::SeqCst), 1);
}

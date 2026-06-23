use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use pretty_assertions::assert_eq;
use tokio::sync::Notify;

use super::*;

#[tokio::test]
async fn refreshes_immediately_periodically_and_stops_when_dropped() {
    let refresh_count = Arc::new(AtomicUsize::new(0));
    let refreshed = Arc::new(Notify::new());
    let hold_second_refresh = Arc::new(Notify::new());
    let worker = spawn(Duration::from_millis(10), {
        let refresh_count = Arc::clone(&refresh_count);
        let refreshed = Arc::clone(&refreshed);
        let hold_second_refresh = Arc::clone(&hold_second_refresh);
        move || {
            let refresh_count = Arc::clone(&refresh_count);
            let refreshed = Arc::clone(&refreshed);
            let hold_second_refresh = Arc::clone(&hold_second_refresh);
            async move {
                let refresh_index = refresh_count.fetch_add(1, Ordering::SeqCst);
                refreshed.notify_one();
                if refresh_index == 1 {
                    hold_second_refresh.notified().await;
                }
                RefreshControl::Continue
            }
        }
    });

    tokio::time::timeout(Duration::from_secs(1), async {
        while refresh_count.load(Ordering::SeqCst) < 2 {
            refreshed.notified().await;
        }
    })
    .await
    .expect("expected two refreshes");
    drop(worker);
    hold_second_refresh.notify_one();
    tokio::time::sleep(Duration::from_millis(30)).await;

    assert_eq!(refresh_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn stops_when_refresh_requests_it() {
    let refresh_count = Arc::new(AtomicUsize::new(0));
    let refreshed = Arc::new(Notify::new());
    let worker = spawn(Duration::from_millis(10), {
        let refresh_count = Arc::clone(&refresh_count);
        let refreshed = Arc::clone(&refreshed);
        move || {
            let refresh_count = Arc::clone(&refresh_count);
            let refreshed = Arc::clone(&refreshed);
            async move {
                refresh_count.fetch_add(1, Ordering::SeqCst);
                refreshed.notify_one();
                RefreshControl::Stop
            }
        }
    });

    tokio::time::timeout(Duration::from_secs(1), refreshed.notified())
        .await
        .expect("expected refresh");
    drop(worker);

    assert_eq!(refresh_count.load(Ordering::SeqCst), 1);
}

#![cfg(test)]

use super::*;
use std::time::Duration;

#[tokio::test]
async fn upgrades_follow_probe_order_even_when_the_second_probe_finishes_first() {
    let state = ThreadListState::default();
    let first = state
        .acquire_resume(/*shared*/ true)
        .await
        .expect("first probe");
    let second = state
        .acquire_resume(/*shared*/ true)
        .await
        .expect("second probe");
    let writer = state.acquire_exclusive();
    tokio::pin!(writer);
    assert!(futures::poll!(&mut writer).is_pending());
    let second_upgrade = second.upgrade();
    tokio::pin!(second_upgrade);
    assert!(futures::poll!(&mut second_upgrade).is_pending());
    tokio::time::timeout(Duration::from_secs(1), async {
        let first = first.upgrade().await.expect("first upgrade");
        assert!(futures::poll!(&mut second_upgrade).is_pending());
        drop(first);
        drop(second_upgrade.await.expect("second upgrade"));
        let _writer = writer.await.expect("writer proceeds");
    })
    .await
    .expect("upgrades and writer must finish");
}

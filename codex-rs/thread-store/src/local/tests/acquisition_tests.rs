//! Persistence acquisition stays owned when its caller is cancelled after writer installation.

use super::*;
use crate::LiveThreadInitGuard;
use futures::poll;
use tokio::sync::oneshot;

#[tokio::test]
async fn cancelled_acquisition_waits_for_writer_handoff_before_discarding() {
    let home = TempDir::new().expect("temp dir");
    let store = Arc::new(LocalThreadStore::new(
        test_config(home.path()),
        /*state_db*/ None,
    ));
    let thread_id = ThreadId::default();
    let params = create_thread_params(thread_id);
    let (installed, installation) = oneshot::channel();
    let (release, released) = oneshot::channel();
    let creating_store = Arc::clone(&store);
    // Model the cloud wrapper: creation runs independently, installs its writer, then awaits
    // initial metadata before returning. Dropping the JoinHandle would detach that work.
    let creating = tokio::spawn(async move {
        let live_thread = LiveThread::create(creating_store, params).await?;
        installed.send(()).expect("installation observed");
        released.await.expect("release acquisition");
        Ok(live_thread)
    });
    let mut guard = LiveThreadInitGuard::default();
    let mut start = Box::pin(guard.acquire(async move { creating.await.expect("creation task") }));
    assert!(poll!(&mut start).is_pending());
    installation
        .await
        .expect("writer installed before cancellation");
    drop(start);
    store
        .flush_thread(thread_id)
        .await
        .expect("writer installed");

    let mut cleanup = Box::pin(guard.discard());
    assert!(
        poll!(&mut cleanup).is_pending(),
        "cleanup must retain the unfinished acquisition"
    );
    release.send(()).expect("creation still owned");
    cleanup.await;
    assert!(matches!(
        store.flush_thread(thread_id).await,
        Err(ThreadStoreError::ThreadNotFound { .. })
    ));
}

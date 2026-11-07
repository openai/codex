//! Checks the persistence boundary when an operation loses its caller.

use super::*;
use codex_core::test_support::auth_manager_from_auth;
use codex_login::CodexAuth;
use futures::poll;
use pretty_assertions::assert_eq;
use tokio::sync::oneshot;

#[tokio::test]
async fn cancelled_commit_keeps_its_permit_and_is_drained() -> io::Result<()> {
    let auth = RemoteControlAuth::capture(
        auth_manager_from_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
        /*runtime_auth*/ None,
    )
    .0;
    let persistence = RemoteControlPersistence::default();
    let writes = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let old_writes = writes.clone();
    let mut old = Box::pin(commit(&auth, &persistence, async move {
        started_tx.send(()).expect("test receiver is open");
        release_rx.await.map_err(io::Error::other)?;
        old_writes.lock().await.push("old");
        Ok(())
    }));
    assert!(poll!(&mut old).is_pending());
    started_rx.await.map_err(io::Error::other)?;
    drop(old);

    let new_writes = writes.clone();
    let mut new = Box::pin(commit(&auth, &persistence, async move {
        new_writes.lock().await.push("new");
        Ok(())
    }));
    assert!(poll!(&mut new).is_pending());
    persistence.tasks.close();
    let mut drained = Box::pin(persistence.tasks.wait());
    assert!(poll!(&mut drained).is_pending());
    release_tx.send(()).expect("commit still owns its receiver");
    tokio::time::timeout(std::time::Duration::from_secs(5), drained).await?;
    new.await?;
    assert_eq!(*writes.lock().await, vec!["old", "new"]);
    let error = commit::<()>(&auth, &persistence, async {
        panic!("closed storage must reject new work")
    })
    .await
    .expect_err("shutdown closes admission");
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    Ok(())
}

use super::RefreshCredentialLock;
use anyhow::Result;
use std::time::Duration;
use tempfile::tempdir;

#[tokio::test]
async fn acquisition_times_out_without_stealing() -> Result<()> {
    let codex_home = tempdir()?;
    let store_key = "test-store-key";
    let held_lock = RefreshCredentialLock::acquire_in(
        codex_home.path(),
        store_key,
        Duration::from_millis(/*millis*/ 100),
    )
    .await?;

    let error = RefreshCredentialLock::acquire_in(
        codex_home.path(),
        store_key,
        Duration::from_millis(/*millis*/ 50),
    )
    .await
    .err()
    .expect("contending lock acquisition should time out");
    assert!(
        error
            .to_string()
            .contains("timed out after 50ms waiting for OAuth refresh lock"),
        "unexpected error: {error:#}"
    );

    drop(held_lock);
    let _reacquired = RefreshCredentialLock::acquire_in(
        codex_home.path(),
        store_key,
        Duration::from_millis(/*millis*/ 100),
    )
    .await?;
    Ok(())
}

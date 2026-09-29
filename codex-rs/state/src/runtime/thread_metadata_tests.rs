//! Regression coverage for batched metadata reads and independent row decoding failures.

use std::collections::HashMap;

use codex_protocol::ThreadId;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;

use super::StateRuntime;
use crate::SqliteConfig;
use crate::runtime::test_support::test_thread_metadata;
use crate::runtime::test_support::unique_temp_dir;

#[tokio::test]
async fn get_threads_preserves_valid_metadata_across_batches() -> anyhow::Result<()> {
    let home = unique_temp_dir();
    let runtime = StateRuntime::init(
        SqliteConfig::new_for_testing(home.as_path().abs()),
        "test-provider".to_string(),
    )
    .await?;
    let mut ids = (0..1_001).map(|_| ThreadId::new()).collect::<Vec<_>>();
    let mut expected = HashMap::new();
    for id in [ids[0], ids[1_000]] {
        let mut metadata = test_thread_metadata(&home, id, home.clone());
        metadata.title = format!("Title for {id}");
        metadata.name = Some(format!("Name for {id}"));
        runtime.upsert_thread(&metadata).await?;
        expected.insert(id, runtime.get_thread(id).await?.expect("persisted thread"));
    }

    // An unreadable record must not discard the other names in the same page.
    let invalid = test_thread_metadata(&home, ids[1], home.clone());
    runtime.upsert_thread(&invalid).await?;
    sqlx::query("UPDATE threads SET created_at_ms = ? WHERE id = ?")
        .bind(i64::MAX)
        .bind(invalid.id.to_string())
        .execute(runtime.pool.as_ref())
        .await?;
    assert!(runtime.get_thread(invalid.id).await.is_err());
    ids.push(ids[0]);

    assert_eq!(runtime.get_threads(&ids).await?, expected);
    assert_eq!(runtime.get_threads(&[]).await?, HashMap::new());
    runtime.close().await;
    std::fs::remove_dir_all(home)?;
    Ok(())
}

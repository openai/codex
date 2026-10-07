//! Storage-level coverage for bounded review retention and actor deletion.

use super::*;
use crate::SqliteConfig;
use crate::runtime::test_support::test_thread_metadata;
use crate::runtime::test_support::unique_temp_dir;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn retention_survives_reopen_and_bounds_count_and_bytes() -> anyhow::Result<()> {
    let home = unique_temp_dir();
    let sqlite = SqliteConfig::new_for_testing(home.clone().try_into()?);
    let state = StateRuntime::init(sqlite.clone(), "test".to_string()).await?;
    let mut ids = Vec::new();
    for _ in 0..=MAX_GUARDIAN_REVIEW_RECORDS {
        let id = ThreadId::new();
        state
            .upsert_thread(&test_thread_metadata(&home, id, home.clone()))
            .await?;
        let record = GuardianReviewRecord::new(id, b"{}".to_vec());
        state.record_guardian_review_failure(&record).await?;
        ids.push(id);
    }
    state.close().await;
    let state = StateRuntime::init(sqlite, "test".to_string()).await?;
    assert_eq!(
        state
            .list_guardian_review_records()
            .await?
            .iter()
            .map(|record| record.thread_id)
            .collect::<Vec<_>>(),
        ids[1..],
    );
    let id = ids[0];
    for index in 0..12 {
        state
            .record_guardian_review_failure(&GuardianReviewRecord::new(
                id,
                index.to_string().into_bytes(),
            ))
            .await?;
    }
    assert_eq!(
        state
            .list_guardian_review_records()
            .await?
            .into_iter()
            .filter(|record| record.thread_id == id)
            .map(|record| record.record)
            .collect::<Vec<_>>(),
        (4..12)
            .map(|index| index.to_string().into_bytes())
            .collect::<Vec<_>>(),
    );
    let payload = vec![b' '; MAX_GUARDIAN_REVIEW_BYTES / 3];
    for _ in 0..4 {
        state
            .record_guardian_review_failure(&GuardianReviewRecord::new(id, payload.clone()))
            .await?;
    }
    assert_eq!(
        state
            .list_guardian_review_records()
            .await?
            .into_iter()
            .map(|record| (record.thread_id, record.record))
            .collect::<Vec<_>>(),
        vec![(id, payload.clone()), (id, payload)],
    );
    state.close().await;
    std::fs::remove_dir_all(home)?;
    Ok(())
}

#[tokio::test]
async fn deleting_an_actor_removes_evidence_and_rejects_late_captures() -> anyhow::Result<()> {
    let home = unique_temp_dir();
    let sqlite = SqliteConfig::new_for_testing(home.clone().try_into()?);
    let state = StateRuntime::init(sqlite.clone(), "test".to_string()).await?;
    let id = ThreadId::new();
    state
        .upsert_thread(&test_thread_metadata(&home, id, home.clone()))
        .await?;
    let record = GuardianReviewRecord::new(id, b"private review context".to_vec());
    state.record_guardian_review_failure(&record).await?;
    // Older binaries delete the actor without knowing about the review table.
    sqlx::query("DELETE FROM threads WHERE id = ?")
        .bind(id.to_string())
        .execute(state.pool.as_ref())
        .await?;
    state.close().await;
    let state = StateRuntime::init(sqlite, "test".to_string()).await?;
    assert!(state.list_guardian_review_records().await?.is_empty());
    assert!(state.record_guardian_review_failure(&record).await.is_err());
    state.close().await;
    std::fs::remove_dir_all(home)?;
    Ok(())
}

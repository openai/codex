//! Live completion, cross-window edits and rebuildable-metadata isolation.
use codex_protocol::ThreadId;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;

use super::*;
use crate::SqliteConfig;
use crate::runtime::test_support::test_thread_metadata;
use crate::runtime::test_support::unique_temp_dir;

#[tokio::test]
async fn a_stale_read_cannot_erase_a_later_completion_or_another_mark() -> anyhow::Result<()> {
    let home = unique_temp_dir();
    let runtime = StateRuntime::init(
        SqliteConfig::new_for_testing(home.as_path().abs()),
        "test-provider".to_string(),
    )
    .await?;
    let id = ThreadId::new();
    let other = ThreadId::new();
    runtime
        .upsert_thread(&test_thread_metadata(&home, id, home.clone()))
        .await?;
    runtime
        .upsert_thread(&test_thread_metadata(&home, other, home.clone()))
        .await?;
    let baseline = runtime.thread_read_states(&[id]).await?[&id].clone();
    assert_eq!(baseline.first_unread_turn, None);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM thread_read_receipts")
            .fetch_one(runtime.pool.as_ref())
            .await?,
        0
    );
    assert!(matches!(
        runtime
            .update_thread_read_state(other, &baseline.revision, ReadStateOperation::Unread)
            .await?,
        ReadStateUpdate::Conflict(_)
    ));
    // A mark is meaningful even without any completed turn.
    let ReadStateUpdate::Applied(marked) = runtime
        .update_thread_read_state(id, &baseline.revision, ReadStateOperation::Unread)
        .await?
    else {
        panic!("could not mark an empty thread unread")
    };
    assert_eq!(marked.first_unread_turn, Some(String::new()));
    assert_eq!(
        runtime
            .update_thread_read_state(id, &baseline.revision, ReadStateOperation::Read)
            .await?,
        ReadStateUpdate::Conflict(marked.clone())
    );
    // Even another mark of an already-unread thread guards against an older explicit read.
    let ReadStateUpdate::Applied(marked_again) = runtime
        .update_thread_read_state(id, &marked.revision, ReadStateOperation::Unread)
        .await?
    else {
        panic!("could not mark again")
    };
    assert_eq!(
        runtime
            .update_thread_read_state(id, &marked.revision, ReadStateOperation::Read)
            .await?,
        ReadStateUpdate::Conflict(marked_again.clone())
    );
    let ReadStateUpdate::Applied(read) = runtime
        .update_thread_read_state(id, &marked_again.revision, ReadStateOperation::Read)
        .await?
    else {
        panic!("could not read at current revision")
    };
    assert_eq!(read.first_unread_turn, None);
    assert!(runtime.publish_thread_attention(id, "turn-1").await?);
    let first = runtime.thread_read_states(&[id]).await?[&id].clone();
    assert_eq!(first.first_unread_turn.as_deref(), Some("turn-1"));
    assert!(!runtime.publish_thread_attention(id, "turn-1").await?);
    assert_eq!(runtime.thread_read_states(&[id]).await?[&id], first);
    assert!(runtime.publish_thread_attention(id, "turn-2").await?);
    let second = runtime.thread_read_states(&[id]).await?[&id].clone();
    assert_eq!(second.first_unread_turn.as_deref(), Some("turn-1"));
    assert_ne!(second.revision, first.revision);
    assert_eq!(
        runtime
            .update_thread_read_state(id, &first.revision, ReadStateOperation::Read)
            .await?,
        ReadStateUpdate::Conflict(second.clone())
    );
    assert_eq!(
        runtime
            .update_thread_read_state(id, &first.revision, ReadStateOperation::Unread)
            .await?,
        ReadStateUpdate::Conflict(second.clone())
    );
    let ReadStateUpdate::Applied(read) = runtime
        .update_thread_read_state(id, &second.revision, ReadStateOperation::Read)
        .await?
    else {
        panic!("could not acknowledge both visible results")
    };
    assert_eq!(read.first_unread_turn, None);
    assert!(!runtime.publish_thread_attention(id, "turn-2").await?);
    assert_eq!(runtime.thread_read_states(&[id]).await?[&id], read);
    runtime.close().await;
    std::fs::remove_dir_all(home)?;
    Ok(())
}

#[tokio::test]
async fn position_and_dedupe_survive_metadata_rebuild_restart_and_revert() -> anyhow::Result<()> {
    let home = unique_temp_dir();
    let sqlite = SqliteConfig::new_for_testing(home.as_path().abs());
    let runtime = StateRuntime::init(sqlite.clone(), "test-provider".to_string()).await?;
    let id = ThreadId::new();
    let metadata = test_thread_metadata(&home, id, home.clone());
    runtime.upsert_thread(&metadata).await?;
    let original_metadata = runtime.get_thread(id).await?;
    assert!(runtime.publish_thread_attention(id, "removed-turn").await?);
    let before = runtime.thread_read_states(&[id]).await?[&id].clone();
    // After a revert the caller checks whether the position still exists, then clears by revision.
    let ReadStateUpdate::Applied(removed) = runtime
        .update_thread_read_state(id, &before.revision, ReadStateOperation::Read)
        .await?
    else {
        panic!("could not retire the removed position")
    };
    assert_eq!(removed.first_unread_turn, None);
    assert_eq!(runtime.get_thread(id).await?, original_metadata);
    sqlx::query("DELETE FROM threads WHERE id=?")
        .bind(id.to_string())
        .execute(runtime.pool.as_ref())
        .await?;
    assert!(runtime.thread_read_states(&[id]).await?.is_empty());
    runtime.upsert_thread(&metadata).await?;
    runtime.close().await;
    let reopened = StateRuntime::init(sqlite, "test-provider".to_string()).await?;
    assert_eq!(reopened.thread_read_states(&[id]).await?[&id], removed);
    assert!(
        !reopened
            .publish_thread_attention(id, "removed-turn")
            .await?
    );
    assert_eq!(reopened.thread_read_states(&[id]).await?[&id], removed);
    assert!(
        reopened
            .publish_thread_attention(id, "new-live-turn")
            .await?
    );
    assert_eq!(
        reopened.thread_read_states(&[id]).await?[&id]
            .first_unread_turn
            .as_deref(),
        Some("new-live-turn")
    );
    reopened.delete_thread(id).await?;
    assert!(!reopened.publish_thread_attention(id, "late").await?);
    assert_eq!(
        reopened
            .update_thread_read_state(id, &removed.revision, ReadStateOperation::Unread)
            .await?,
        ReadStateUpdate::Unavailable
    );
    reopened.close().await;
    std::fs::remove_dir_all(home)?;
    Ok(())
}

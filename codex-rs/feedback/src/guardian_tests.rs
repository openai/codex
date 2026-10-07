use super::*;
use pretty_assertions::assert_eq;

#[test]
fn records_keep_recent_failures_and_select_the_requested_tree() {
    let mut records = ReviewRecords::default();
    let root = ThreadId::new();
    let child = ThreadId::new();
    let unrelated = ThreadId::new();
    records.push(GuardianReviewRecord::new(
        child,
        br#"{"child":"denied"}"#.to_vec(),
    ));
    records.push(GuardianReviewRecord::new(
        unrelated,
        br#"{"unrelated":"denied"}"#.to_vec(),
    ));
    for index in 0..12 {
        records.push(GuardianReviewRecord::new(
            root,
            format!(r#"{{"root":{index}}}"#).into_bytes(),
        ));
    }
    let snapshot = records.snapshot(&[root, child]);
    let attachment = snapshot.attachment.expect("task-tree records");
    let expected = std::iter::once("{\"child\":\"denied\"}\n".to_string())
        .chain((4..12).map(|index| format!("{{\"root\":{index}}}\n")))
        .collect::<String>();
    assert_eq!(attachment.buffer, expected.into_bytes());
    assert_eq!(snapshot.thread_ids, vec![root, child]);
    assert_eq!(snapshot.process_discarded_records, 4);
    let unrelated_snapshot = records.snapshot(&[unrelated]);
    assert_eq!(unrelated_snapshot.thread_ids, vec![unrelated]);
    // The diagnostic counter must not be mistaken for tree-specific omissions.
    assert_eq!(unrelated_snapshot.process_discarded_records, 4);
}

#[test]
fn record_count_and_bytes_are_bounded_across_threads() {
    let mut records = ReviewRecords::default();
    for _ in 0..MAX_RECORDS + 1 {
        records.push(GuardianReviewRecord::new(ThreadId::new(), b"{}".to_vec()));
    }
    assert_eq!(records.records.len(), MAX_RECORDS);
    let payload = vec![b' '; MAX_BYTES / 3];
    let thread_id = ThreadId::new();
    for _ in 0..4 {
        records.push(GuardianReviewRecord::new(thread_id, payload.clone()));
    }
    assert!(records.bytes <= MAX_BYTES);
    assert_eq!(records.records.len(), 2);
    assert_eq!(
        records
            .snapshot(&[thread_id])
            .attachment
            .expect("bounded records")
            .buffer
            .len(),
        2 * (payload.len() + 1)
    );
    assert_eq!(records.discarded_records, MAX_RECORDS + 3);
}

#[tokio::test]
async fn busy_storage_retains_feedback_without_delaying_review_completion() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let sqlite = codex_state::SqliteConfig::new_for_testing(home.path().try_into()?);
    let state = StateRuntime::init(sqlite.clone(), "test".to_string()).await?;
    let thread_id = ThreadId::new();
    let pool = sqlite.open_read_write_pool(&sqlite.state_db_path()).await?;
    let transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    tokio::time::timeout(
        Duration::from_secs(/*secs*/ 1),
        record_guardian_review_failure(
            Some(&state),
            GuardianReviewRecord::new(thread_id, br#"{"fallback":true}"#.to_vec()),
        ),
    )
    .await?;
    transaction.rollback().await?;
    let snapshot = guardian_review_failures(Some(&state), &[thread_id]).await;
    assert_eq!(snapshot.thread_ids, vec![thread_id]);
    assert_eq!(
        snapshot.attachment.unwrap().buffer,
        b"{\"fallback\":true}\n"
    );
    pool.close().await;
    state.close().await;
    Ok(())
}

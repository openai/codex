#![allow(warnings, clippy::all)]

use super::*;
use crate::list::parse_cursor;
use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Timelike;
use chrono::Utc;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[test]
fn cursor_to_anchor_normalizes_timestamp_format() {
    let ts_str = "2026-01-27T12-34-56";
    let cursor = parse_cursor(ts_str).expect("cursor should parse");
    let anchor = cursor_to_anchor(Some(&cursor)).expect("anchor should parse");

    let naive =
        NaiveDateTime::parse_from_str(ts_str, "%Y-%m-%dT%H-%M-%S").expect("ts should parse");
    let expected_ts = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc)
        .with_nanosecond(0)
        .expect("nanosecond");

    assert_eq!(anchor.ts, expected_ts);
}

#[tokio::test]
async fn try_init_waits_for_concurrent_startup_backfill() -> anyhow::Result<()> {
    let home = TempDir::new().expect("temp dir");
    let runtime =
        codex_state::StateRuntime::init(home.path().to_path_buf(), "test-provider".to_string())
            .await?;
    let claimed = runtime.try_claim_backfill(/*lease_seconds*/ 60).await?;
    assert!(claimed);
    let runtime_for_completion = runtime.clone();
    let complete_backfill = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        runtime_for_completion
            .mark_backfill_complete(/*last_watermark*/ None)
            .await
    });

    let initialized = try_init_with_roots_and_backfill_lease(
        home.path().to_path_buf(),
        home.path().to_path_buf(),
        "test-provider".to_string(),
        /*backfill_lease_seconds*/ 60,
    )
    .await?;
    complete_backfill.await??;
    assert_eq!(
        initialized.get_backfill_state().await?.status,
        codex_state::BackfillStatus::Complete
    );

    Ok(())
}

#[tokio::test]
async fn try_init_times_out_waiting_for_stuck_startup_backfill() -> anyhow::Result<()> {
    let home = TempDir::new().expect("temp dir");
    let runtime =
        codex_state::StateRuntime::init(home.path().to_path_buf(), "test-provider".to_string())
            .await?;
    let claimed = runtime.try_claim_backfill(/*lease_seconds*/ 60).await?;
    assert!(claimed);

    let result = try_init_with_roots_and_backfill_lease(
        home.path().to_path_buf(),
        home.path().to_path_buf(),
        "test-provider".to_string(),
        /*backfill_lease_seconds*/ 60,
    )
    .await;
    let err = match result {
        Ok(_) => panic!("state db init should not wait forever for incomplete backfill"),
        Err(err) => err,
    };
    assert!(
        err.to_string()
            .contains("timed out waiting for state db backfill"),
        "unexpected error: {err}"
    );

    Ok(())
}

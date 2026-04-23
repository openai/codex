#![allow(warnings, clippy::all)]

use super::*;
use crate::config::RolloutConfig;
use crate::list::parse_cursor;
use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Timelike;
use chrono::Utc;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::sync::Arc;
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

fn test_config(home: &Path) -> RolloutConfig {
    RolloutConfig {
        codex_home: home.to_path_buf(),
        sqlite_home: home.to_path_buf(),
        cwd: home.to_path_buf(),
        model_provider_id: "test-provider".to_string(),
        generate_memories: false,
    }
}

#[tokio::test]
async fn init_reuses_cached_runtime_for_same_home() {
    let temp = TempDir::new().expect("temp dir");
    let config = test_config(temp.path());

    let first = init(&config).await.expect("state db init should succeed");
    first
        .mark_backfill_complete(/*last_watermark*/ None)
        .await
        .expect("backfill should be marked complete");
    let second = init(&config)
        .await
        .expect("cached state db init should succeed");

    assert!(Arc::ptr_eq(&first, &second));
}

#[tokio::test]
async fn get_state_db_reuses_cached_runtime() {
    let temp = TempDir::new().expect("temp dir");
    let config = test_config(temp.path());

    let first = init(&config).await.expect("state db init should succeed");
    first
        .mark_backfill_complete(/*last_watermark*/ None)
        .await
        .expect("backfill should be marked complete");

    let reopened = get_state_db(&config)
        .await
        .expect("cached state db should be returned");

    assert!(Arc::ptr_eq(&first, &reopened));
    assert_eq!(reopened.codex_home(), config.sqlite_home.as_path());
}

#[tokio::test]
async fn concurrent_init_reuses_single_cached_runtime() {
    let temp = TempDir::new().expect("temp dir");
    let config = Arc::new(test_config(temp.path()));

    let mut handles = Vec::new();
    for _ in 0..8 {
        let config = Arc::clone(&config);
        handles.push(tokio::spawn(async move {
            init(config.as_ref())
                .await
                .expect("state db init should succeed")
        }));
    }

    let mut runtimes = Vec::new();
    for handle in handles {
        runtimes.push(handle.await.expect("task should join"));
    }
    let first = runtimes
        .first()
        .cloned()
        .expect("at least one runtime should exist");
    first
        .mark_backfill_complete(/*last_watermark*/ None)
        .await
        .expect("backfill should be marked complete");

    for runtime in runtimes.iter().skip(1) {
        assert!(Arc::ptr_eq(&first, runtime));
    }
}

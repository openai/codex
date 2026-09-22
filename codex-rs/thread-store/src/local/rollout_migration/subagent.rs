//! Selects a smaller resume context for legacy subagent rollouts.
//!
//! Legacy subagents copied the parent's full rollout into every child rollout. Migrating those
//! files verbatim can preserve gigabytes of duplicated history even though a resumed subagent only
//! needs its latest bounded model context.
//!
//! This module reverse-scans the legacy rollout for the newest compaction with replacement history
//! and a window number. Malformed records are skipped. If no such compaction exists, the caller
//! falls back to full tolerant replay.

use std::fs::File;
use std::path::PathBuf;

use codex_rollout::ModelContextScan;
use codex_rollout::ModelContextScanProgress;
use codex_rollout::ReverseJsonlScanner;
use codex_rollout::RolloutItem;
use codex_rollout::ScanOutcome;
use serde_json::Value;

use super::line_parser;
use super::migration_error;
use crate::ThreadStoreResult;

/// Returns the suffix starting at the newest usable compaction, if one exists.
pub(super) async fn select_bounded_context(
    rollout_path: PathBuf,
) -> ThreadStoreResult<Option<Vec<RolloutItem>>> {
    tokio::task::spawn_blocking(move || {
        let file = File::open(rollout_path).map_err(migration_error)?;
        let mut scanner = ReverseJsonlScanner::new(file)
            .map_err(migration_error)?
            .with_max_record_bytes(super::MAX_ROLLOUT_LINE_BYTES);
        let mut scan = ModelContextScan::default();

        while let Some(outcome) = scanner.scan_next::<Value>().map_err(migration_error)? {
            let value = match outcome {
                ScanOutcome::Parsed(value) => value,
                ScanOutcome::Rejected(_) => continue,
            };
            let Ok(Some(line)) = line_parser::parse_legacy_rollout_value(value) else {
                continue;
            };
            if scan.push(line.item) == ModelContextScanProgress::Complete {
                let mut items = scan.finish();
                items.retain(|item| !matches!(item, RolloutItem::SessionMeta(_)));
                return Ok(Some(items));
            }
        }

        Ok(None)
    })
    .await
    .map_err(migration_error)?
}

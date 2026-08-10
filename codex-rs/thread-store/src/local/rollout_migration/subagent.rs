//! Selects a smaller resume context for legacy subagent rollouts.
//!
//! Legacy subagents copied the parent's full rollout into every child rollout. Migrating those
//! files verbatim can preserve gigabytes of duplicated history even though a resumed subagent only
//! needs its latest bounded model context.
//!
//! This module reverse-scans the legacy rollout for a safe model-context checkpoint and returns the
//! smallest replay needed to resume from there. Malformed reverse-scanned records are skipped. If
//! we cannot prove that a bounded replay is safe, we return `None` and let the caller fall back to
//! full tolerant replay.

use std::fs::File;
use std::path::PathBuf;

use codex_protocol::protocol::SessionMetaLine;
use codex_rollout::ModelContextScan;
use codex_rollout::ModelContextScanProgress;
use codex_rollout::ReverseJsonlScanner;
use codex_rollout::RolloutItem;
use codex_rollout::ScanOutcome;
use serde_json::Value;

use super::line_parser;
use super::migration_error;
use crate::ThreadStoreResult;

/// Returns a bounded replay when the legacy child has enough durable context to reconstruct from
/// a suffix. Without that proof, the caller streams the complete legacy replay instead.
pub(super) async fn select_bounded_context(
    rollout_path: PathBuf,
    session_meta: SessionMetaLine,
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
                let mut items = scan.finish(session_meta);
                items.retain(|item| !matches!(item, RolloutItem::SessionMeta(_)));
                return Ok(Some(items));
            }
        }

        Ok(None)
    })
    .await
    .map_err(migration_error)?
}

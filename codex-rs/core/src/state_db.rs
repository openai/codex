use crate::config::Config;
use crate::features::Feature;
use crate::rollout::list::Cursor;
use crate::rollout::list::ThreadSortKey;
use crate::rollout::metadata;
use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Timelike;
use chrono::Utc;
use codex_otel::OtelManager;
use codex_protocol::ThreadId;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use codex_state::DB_METRIC_BACKFILL;
use codex_state::STATE_DB_FILENAME;
use codex_state::ThreadMetadataBuilder;
use serde_json::Value;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;
use tracing::warn;
use uuid::Uuid;

/// Core-facing handle to the optional SQLite-backed state runtime.
pub type StateDbHandle = Arc<codex_state::StateRuntime>;

/// Initialize the state runtime when the `sqlite` feature flag is enabled.
pub async fn init_if_enabled(config: &Config, otel: Option<&OtelManager>) -> Option<StateDbHandle> {
    if !config.features.enabled(Feature::Sqlite) {
        // We delete the file on best effort basis to maintain retro-compatibility in the future.
        tokio::fs::remove_file(config.codex_home.join(STATE_DB_FILENAME))
            .await
            .ok();
        return None;
    }

    let state_path = config.codex_home.join(STATE_DB_FILENAME);
    let existed = tokio::fs::try_exists(&state_path).await.unwrap_or(false);
    let runtime = match codex_state::StateRuntime::init(
        config.codex_home.clone(),
        config.model_provider_id.clone(),
        otel.cloned(),
    )
    .await
    {
        Ok(runtime) => runtime,
        Err(err) => {
            warn!(
                "failed to initialize state runtime at {}: {err}",
                config.codex_home.display()
            );
            if let Some(otel) = otel {
                otel.counter("codex.db.init", 1, &[("status", "init_error")]);
            }
            return None;
        }
    };
    if !existed {
        let stats = metadata::backfill_sessions(runtime.as_ref(), config, otel).await;
        info!(
            "state db backfill scanned={}, upserted={}, failed={}",
            stats.scanned, stats.upserted, stats.failed
        );
        if let Some(otel) = otel {
            otel.counter(
                DB_METRIC_BACKFILL,
                stats.upserted as i64,
                &[("status", "upserted")],
            );
            otel.counter(
                DB_METRIC_BACKFILL,
                stats.failed as i64,
                &[("status", "failed")],
            );
        }
    }
    Some(runtime)
}

/// Open the state runtime when the SQLite file exists, without feature gating.
pub async fn open_if_present(codex_home: &Path, default_provider: &str) -> Option<StateDbHandle> {
    let db_path = codex_home.join(STATE_DB_FILENAME);
    if !tokio::fs::try_exists(&db_path).await.unwrap_or(false) {
        return None;
    }
    let runtime = codex_state::StateRuntime::init(
        codex_home.to_path_buf(),
        default_provider.to_string(),
        None,
    )
    .await
    .ok()?;
    Some(runtime)
}

/// Extract rollout metadata by scanning the rollout file in core.
pub async fn extract_metadata_from_rollout(
    rollout_path: &Path,
    default_provider: &str,
    otel: Option<&OtelManager>,
) -> anyhow::Result<codex_state::ExtractionOutcome> {
    metadata::extract_metadata_from_rollout(rollout_path, default_provider, otel).await
}

fn cursor_to_anchor(cursor: Option<&Cursor>) -> Option<codex_state::Anchor> {
    let cursor = cursor?;
    let value = serde_json::to_value(cursor).ok()?;
    let cursor_str = value.as_str()?;
    let (ts_str, id_str) = cursor_str.split_once('|')?;
    if id_str.contains('|') {
        return None;
    }
    let id = Uuid::parse_str(id_str).ok()?;
    let ts = if let Ok(naive) = NaiveDateTime::parse_from_str(ts_str, "%Y-%m-%dT%H-%M-%S") {
        DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc)
    } else if let Ok(dt) = DateTime::parse_from_rfc3339(ts_str) {
        dt.with_timezone(&Utc)
    } else {
        return None;
    }
    .with_nanosecond(0)?;
    Some(codex_state::Anchor { ts, id })
}

fn thread_sort_key(sort_key: ThreadSortKey) -> codex_state::SortKey {
    match sort_key {
        ThreadSortKey::CreatedAt => codex_state::SortKey::CreatedAt,
        ThreadSortKey::UpdatedAt => codex_state::SortKey::UpdatedAt,
    }
}

fn sources_to_strings(sources: &[SessionSource]) -> Vec<String> {
    sources
        .iter()
        .map(|value| match serde_json::to_value(value) {
            Ok(Value::String(s)) => s,
            Ok(other) => other.to_string(),
            Err(_) => String::new(),
        })
        .collect()
}

fn model_providers_to_vec(model_providers: Option<&[String]>) -> Option<Vec<String>> {
    model_providers.map(<[String]>::to_vec)
}

/// Query parameters for listing threads from SQLite.
pub struct ListThreadsDbQuery<'a> {
    pub page_size: usize,
    pub cursor: Option<&'a Cursor>,
    pub sort_key: ThreadSortKey,
    pub allowed_sources: &'a [SessionSource],
    pub model_providers: Option<&'a [String]>,
    pub archived_only: bool,
    pub stage: &'a str,
}

/// List thread ids from SQLite for parity checks without rollout scanning.
pub async fn list_thread_ids_db(
    context: Option<&codex_state::StateRuntime>,
    codex_home: &Path,
    query: ListThreadsDbQuery<'_>,
) -> Option<Vec<ThreadId>> {
    let ctx = context?;
    let ListThreadsDbQuery {
        page_size,
        cursor,
        sort_key,
        allowed_sources,
        model_providers,
        archived_only,
        stage,
    } = query;
    if ctx.codex_home() != codex_home {
        warn!(
            "state db codex_home mismatch: expected {}, got {}",
            ctx.codex_home().display(),
            codex_home.display()
        );
    }

    let anchor = cursor_to_anchor(cursor);
    let allowed_sources = sources_to_strings(allowed_sources);
    let model_providers = model_providers_to_vec(model_providers);
    match ctx
        .list_thread_ids(
            page_size,
            anchor.as_ref(),
            thread_sort_key(sort_key),
            allowed_sources.as_slice(),
            model_providers.as_deref(),
            archived_only,
        )
        .await
    {
        Ok(ids) => Some(ids),
        Err(err) => {
            warn!("state db list_thread_ids failed during {stage}: {err}");
            None
        }
    }
}

/// Look up the rollout path for a thread id using SQLite.
pub async fn find_rollout_path_by_id(
    context: Option<&codex_state::StateRuntime>,
    thread_id: ThreadId,
    archived_only: Option<bool>,
    stage: &str,
) -> Option<PathBuf> {
    let ctx = context?;
    ctx.find_rollout_path_by_id(thread_id, archived_only)
        .await
        .unwrap_or_else(|err| {
            warn!("state db find_rollout_path_by_id failed during {stage}: {err}");
            None
        })
}

/// Reconcile rollout items into SQLite, falling back to scanning the rollout file.
pub async fn reconcile_rollout(
    context: Option<&codex_state::StateRuntime>,
    rollout_path: &Path,
    default_provider: &str,
    builder: Option<&ThreadMetadataBuilder>,
    items: &[RolloutItem],
) {
    let Some(ctx) = context else {
        return;
    };
    if builder.is_some() || !items.is_empty() {
        apply_rollout_items(
            Some(ctx),
            rollout_path,
            default_provider,
            builder,
            items,
            "reconcile_rollout",
        )
        .await;
        return;
    }
    let outcome =
        match metadata::extract_metadata_from_rollout(rollout_path, default_provider, None).await {
            Ok(outcome) => outcome,
            Err(err) => {
                warn!(
                    "state db reconcile_rollout extraction failed {}: {err}",
                    rollout_path.display()
                );
                return;
            }
        };
    if let Err(err) = ctx.upsert_thread(&outcome.metadata).await {
        warn!(
            "state db reconcile_rollout upsert failed {}: {err}",
            rollout_path.display()
        );
    }
}

/// Apply rollout items incrementally to SQLite.
pub async fn apply_rollout_items(
    context: Option<&codex_state::StateRuntime>,
    rollout_path: &Path,
    _default_provider: &str,
    builder: Option<&ThreadMetadataBuilder>,
    items: &[RolloutItem],
    stage: &str,
) {
    let Some(ctx) = context else {
        return;
    };
    let mut builder = match builder {
        Some(builder) => builder.clone(),
        None => match metadata::builder_from_items(items, rollout_path) {
            Some(builder) => builder,
            None => {
                warn!(
                    "state db apply_rollout_items missing builder during {stage}: {}",
                    rollout_path.display()
                );
                record_discrepancy(stage, "missing_builder");
                return;
            }
        },
    };
    builder.rollout_path = rollout_path.to_path_buf();
    if let Err(err) = ctx.apply_rollout_items(&builder, items, None).await {
        warn!(
            "state db apply_rollout_items failed during {stage} for {}: {err}",
            rollout_path.display()
        );
    }
}

/// Mark a thread as archived in SQLite using the canonical archived rollout path.
pub async fn mark_archived(
    context: Option<&codex_state::StateRuntime>,
    thread_id: ThreadId,
    rollout_path: &Path,
) {
    let Some(ctx) = context else {
        return;
    };
    if let Err(err) = ctx
        .mark_archived(thread_id, rollout_path, chrono::Utc::now())
        .await
    {
        warn!(
            "failed to mark archived in state db {}: {err}",
            rollout_path.display()
        );
    }
}

/// Mark a thread as unarchived in SQLite using the canonical restored rollout path.
pub async fn mark_unarchived(
    context: Option<&codex_state::StateRuntime>,
    thread_id: ThreadId,
    rollout_path: &Path,
) {
    let Some(ctx) = context else {
        return;
    };
    if let Err(err) = ctx.mark_unarchived(thread_id, rollout_path).await {
        warn!(
            "failed to mark unarchived in state db {}: {err}",
            rollout_path.display()
        );
    }
}

/// Extract the thread id UUID string from a rollout path, if it matches the standard naming scheme.
pub fn rollout_id_from_path(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_string_lossy();
    let core = file_name.strip_prefix("rollout-")?.strip_suffix(".jsonl")?;
    let (_sep_idx, uuid) = core.match_indices('-').rev().find_map(|(idx, _)| {
        Uuid::parse_str(&core[idx + 1..])
            .ok()
            .map(|uuid| (idx, uuid))
    })?;
    Some(uuid.to_string())
}

/// Record a state discrepancy metric with a stage and reason tag.
pub fn record_discrepancy(stage: &str, reason: &str) {
    // We access the global metric because the call sites might not have access to the broader
    // OtelManager.
    if let Some(metric) = codex_otel::metrics::global() {
        let _ = metric.counter(
            "codex.db.discrepancy",
            1,
            &[("stage", stage), ("reason", reason)],
        );
    }
}

/// Emit a startup summary when SQLite state is enabled and initialized.
pub async fn startup_summary(context: Option<&codex_state::StateRuntime>, config: &Config) {
    if config.features.enabled(Feature::Sqlite)
        && let Some(ctx) = context
    {
        ctx.startup_summary().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rollout::list::parse_cursor;
    use pretty_assertions::assert_eq;
    use std::path::Path;

    #[test]
    fn rollout_id_from_path_parses_full_uuid() {
        let uuid = Uuid::new_v4();
        let path_str = format!("rollout-2026-01-27T12-34-56-{uuid}.jsonl");
        let path = Path::new(path_str.as_str());
        assert_eq!(rollout_id_from_path(path), Some(uuid.to_string()));
    }

    #[test]
    fn cursor_to_anchor_normalizes_timestamp_format() {
        let uuid = Uuid::new_v4();
        let ts_str = "2026-01-27T12-34-56";
        let token = format!("{ts_str}|{uuid}");
        let cursor = parse_cursor(token.as_str()).expect("cursor should parse");
        let anchor = cursor_to_anchor(Some(&cursor)).expect("anchor should parse");

        let naive =
            NaiveDateTime::parse_from_str(ts_str, "%Y-%m-%dT%H-%M-%S").expect("ts should parse");
        let expected_ts = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc)
            .with_nanosecond(0)
            .expect("nanosecond");

        assert_eq!(anchor.id, uuid);
        assert_eq!(anchor.ts, expected_ts);
    }
}

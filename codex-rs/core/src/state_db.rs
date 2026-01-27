use crate::config::Config;
use crate::features::Feature;
use crate::rollout::list::Cursor;
use crate::rollout::list::ThreadSortKey;
use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::SecondsFormat;
use chrono::Utc;
use codex_otel::OtelManager;
use codex_protocol::ThreadId;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use codex_state::STATE_DB_FILENAME;
use serde_json::Value;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::warn;
use uuid::Uuid;

type StateRuntimeHandle = Arc<codex_state::StateRuntime>;

/// Core-facing handle to the optional SQLite-backed state runtime.
#[derive(Clone)]
pub struct StateDbContext {
    runtime: StateRuntimeHandle,
}

impl StateDbContext {
    pub fn new(runtime: StateRuntimeHandle) -> Self {
        Self { runtime }
    }

    pub fn codex_home(&self) -> &Path {
        self.runtime.codex_home()
    }
}

/// Initialize the state runtime when the `sqlite` feature flag is enabled.
pub async fn init_if_enabled(
    config: &Config,
    otel: Option<&OtelManager>,
) -> Option<Arc<StateDbContext>> {
    if !config.features.enabled(Feature::Sqlite) {
        // We delete the file on best effort basis to maintain retro-compatibility in the future.
        tokio::fs::remove_file(config.codex_home.join(STATE_DB_FILENAME))
            .await
            .ok();
        return None;
    }

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
    Some(Arc::new(StateDbContext::new(runtime)))
}

/// Open the state runtime when the SQLite file exists, without feature gating.
pub async fn open_if_present(
    codex_home: &Path,
    default_provider: &str,
) -> Option<Arc<StateDbContext>> {
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
    Some(Arc::new(StateDbContext::new(runtime)))
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
            .to_rfc3339_opts(SecondsFormat::Secs, true)
    } else if let Ok(dt) = DateTime::parse_from_rfc3339(ts_str) {
        dt.with_timezone(&Utc)
            .to_rfc3339_opts(SecondsFormat::Secs, true)
    } else {
        return None;
    };
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
    context: Option<&StateDbContext>,
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
        .runtime
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
    context: Option<&StateDbContext>,
    thread_id: ThreadId,
    archived_only: Option<bool>,
    stage: &str,
) -> Option<PathBuf> {
    let ctx = context?;
    ctx.runtime
        .find_rollout_path_by_id(thread_id, archived_only)
        .await
        .unwrap_or_else(|err| {
            warn!("state db find_rollout_path_by_id failed during {stage}: {err}");
            None
        })
}

/// Reconcile a rollout file into SQLite by extracting and upserting metadata.
pub async fn reconcile_rollout(context: Option<&StateDbContext>, path: &Path) {
    let Some(ctx) = context else {
        return;
    };
    ctx.runtime.ingest_rollout_file(path, None).await;
}

/// Apply rollout items incrementally to SQLite.
pub async fn apply_rollout_items(
    context: Option<&StateDbContext>,
    path: &Path,
    items: &[RolloutItem],
) {
    let Some(ctx) = context else {
        return;
    };
    ctx.runtime.apply_rollout_items(path, items, None).await;
}

/// Mark a thread as archived in SQLite using the canonical archived rollout path.
pub async fn mark_archived(
    context: Option<&StateDbContext>,
    thread_id: ThreadId,
    rollout_path: &Path,
) {
    let Some(ctx) = context else {
        return;
    };
    let archived_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    if let Err(err) = ctx
        .runtime
        .mark_archived(thread_id, rollout_path, archived_at.as_str())
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
    context: Option<&StateDbContext>,
    thread_id: ThreadId,
    rollout_path: &Path,
) {
    let Some(ctx) = context else {
        return;
    };
    if let Err(err) = ctx.runtime.mark_unarchived(thread_id, rollout_path).await {
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
pub async fn startup_summary(context: Option<&StateDbContext>, config: &Config) {
    if config.features.enabled(Feature::Sqlite)
        && let Some(ctx) = context
    {
        ctx.runtime.startup_summary().await;
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
            .to_rfc3339_opts(SecondsFormat::Secs, true);

        assert_eq!(anchor.id, uuid);
        assert_eq!(anchor.ts, expected_ts);
    }
}

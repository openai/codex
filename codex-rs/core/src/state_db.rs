use crate::config::Config;
use crate::features::Feature;
use crate::rollout::list::Cursor;
use crate::rollout::list::ThreadSortKey;
use codex_otel::OtelManager;
use codex_protocol::ThreadId;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use codex_state::STATE_DB_FILENAME;
use serde_json::Value;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::warn;
use uuid::Uuid;

type StateRuntimeHandle = Arc<codex_state::StateRuntime>;

/// Core-facing handle to the optional SQLite-backed state runtime.
#[derive(Clone)]
pub struct StateDbContext {
    runtime: StateRuntimeHandle,
}

static STATE_DB: OnceCell<Option<Arc<StateDbContext>>> = OnceCell::const_new();

/// Return the initialized state runtime context, if available.
pub async fn context() -> Option<Arc<StateDbContext>> {
    STATE_DB.get().cloned().flatten()
}

/// Initialize the state runtime when the `sqlite` feature flag is enabled.
pub async fn init_if_enabled(config: &Config, otel: &OtelManager) -> Option<Arc<StateDbContext>> {
    STATE_DB
        .get_or_init(|| async move {
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
                otel.clone(),
            )
            .await
            {
                Ok(runtime) => runtime,
                Err(err) => {
                    warn!(
                        "failed to initialize state runtime at {}: {err}",
                        config.codex_home.display()
                    );
                    otel.counter("codex.db.init", 1, &[("status", "init_error")]);
                    return None;
                }
            };
            Some(Arc::new(StateDbContext { runtime }))
        })
        .await
        .clone()
}

/// Extract rollout metadata using the state runtime.
pub async fn extract_rollout_metadata(path: &Path) -> Option<codex_state::ThreadMetadata> {
    let ctx = context().await?;
    ctx.runtime.extract(path).await
}

fn cursor_to_anchor(cursor: Option<&Cursor>) -> Option<codex_state::Anchor> {
    let cursor = cursor?;
    let value = serde_json::to_value(cursor).ok()?;
    let cursor_str = value.as_str()?;
    let mut parts = cursor_str.split('|');
    let ts = parts.next()?.to_string();
    let id_str = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let id = Uuid::parse_str(id_str).ok()?;
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

/// List threads from SQLite for comparison against canonical rollout listings.
pub async fn list_threads_db(
    codex_home: &Path,
    query: ListThreadsDbQuery<'_>,
) -> Option<codex_state::ThreadsPage> {
    let ctx = context().await?;
    let ListThreadsDbQuery {
        page_size,
        cursor,
        sort_key,
        allowed_sources,
        model_providers,
        archived_only,
        stage,
    } = query;
    if ctx.runtime.codex_home() != codex_home {
        warn!(
            "state db codex_home mismatch: expected {}, got {}",
            ctx.runtime.codex_home().display(),
            codex_home.display()
        );
    }

    let anchor = cursor_to_anchor(cursor);
    let allowed_sources = sources_to_strings(allowed_sources);
    let model_providers = model_providers_to_vec(model_providers);
    match ctx
        .runtime
        .list_threads(
            page_size,
            anchor.as_ref(),
            thread_sort_key(sort_key),
            allowed_sources.as_slice(),
            model_providers.as_deref(),
            archived_only,
        )
        .await
    {
        Ok(page) => Some(page),
        Err(err) => {
            warn!("state db list_threads failed during {stage}: {err}");
            None
        }
    }
}

/// Load a thread's metadata from SQLite by id.
pub async fn get_thread(thread_id: ThreadId, stage: &str) -> Option<codex_state::ThreadMetadata> {
    let ctx = context().await?;
    ctx.runtime
        .get_thread(thread_id)
        .await
        .unwrap_or_else(|err| {
            warn!("failed to load thread {thread_id} from state db during {stage}: {err}");
            None
        })
}

/// Compare a rollout file against the database and record discrepancies.
pub async fn compare_rollout(path: &Path, stage: &str) {
    let Some(rollout_metadata) = extract_rollout_metadata(path).await else {
        return;
    };
    let get_stage = format!("{stage}.get_thread");
    match get_thread(rollout_metadata.id, get_stage.as_str()).await {
        Some(db_metadata) => {
            let diffs = rollout_metadata.diff_fields(&db_metadata);
            if diffs.is_empty() {
                return;
            }
            let diffs_display = diffs.join(", ");
            warn!(
                "state db discrepancy for thread {} during {stage}: {diffs_display}",
                rollout_metadata.id
            );
            record_discrepancy(stage, "field_mismatch");
        }
        None => {
            warn!(
                "state db missing thread {} during {stage} compare",
                rollout_metadata.id
            );
            record_discrepancy(stage, "missing_db_row");
        }
    }
}

/// Look up the rollout path for a thread id using SQLite.
pub async fn find_rollout_path_by_id(
    thread_id: ThreadId,
    archived_only: Option<bool>,
    stage: &str,
) -> Option<PathBuf> {
    let ctx = context().await?;
    ctx.runtime
        .find_rollout_path_by_id(thread_id, archived_only)
        .await
        .unwrap_or_else(|err| {
            warn!("state db find_rollout_path_by_id failed during {stage}: {err}");
            None
        })
}

/// Reconcile a rollout file into SQLite by extracting and upserting metadata.
pub async fn reconcile_rollout(path: &Path) {
    let Some(ctx) = context().await else {
        return;
    };
    ctx.runtime.ingest_rollout_file(path).await;
}

/// Apply rollout items incrementally to SQLite.
pub async fn apply_rollout_items(path: &Path, items: &[RolloutItem]) {
    let Some(ctx) = context().await else {
        return;
    };
    ctx.runtime.apply_rollout_items(path, items).await;
}

/// Mark a thread as archived in SQLite using the canonical archived rollout path.
pub async fn mark_archived(thread_id: ThreadId, rollout_path: &Path) {
    let Some(ctx) = context().await else {
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
pub async fn mark_unarchived(thread_id: ThreadId, rollout_path: &Path) {
    let Some(ctx) = context().await else {
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
    let id_str = file_name.strip_prefix("rollout-")?;
    let id_str = id_str.strip_suffix(".jsonl")?;
    let uuid_part = id_str.rsplit_once('-')?.1;
    Some(uuid_part.to_string())
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
pub async fn startup_summary(config: &Config) {
    if !config.features.enabled(Feature::Sqlite) {
        return;
    }
    let Some(ctx) = context().await else {
        return;
    };
    ctx.runtime.startup_summary().await;
}

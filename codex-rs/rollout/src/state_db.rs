use crate::config::RolloutConfig;
use crate::config::RolloutConfigView;
use crate::list::Cursor;
use crate::list::SortDirection;
use crate::list::ThreadSortKey;
use crate::metadata;
use crate::sqlite_metrics;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
pub use codex_state::LogEntry;
use codex_state::ThreadMetadataBuilder;
use codex_utils_path::normalize_for_path_comparison;
use serde_json::Value;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tracing::info;
use tracing::warn;

/// Core-facing handle to the SQLite-backed state runtime.
pub type StateDbHandle = Arc<codex_state::StateRuntime>;

#[cfg(not(test))]
const STARTUP_BACKFILL_POLL_INTERVAL: Duration = Duration::from_secs(1);
#[cfg(test)]
const STARTUP_BACKFILL_POLL_INTERVAL: Duration = Duration::from_millis(10);
#[cfg(not(test))]
const STARTUP_BACKFILL_WAIT_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(test)]
const STARTUP_BACKFILL_WAIT_TIMEOUT: Duration = Duration::from_secs(2);

/// Initialize the state runtime for thread state persistence.
///
/// This is the process entry point for local state: it opens the SQLite-backed
/// runtime, applies rollout metadata backfills as needed, and returns the
/// initialized handle.
pub async fn init(config: &impl RolloutConfigView) -> Option<StateDbHandle> {
    let config = RolloutConfig::from_view(config);
    match try_init_with_roots(
        config.codex_home,
        config.sqlite_home,
        config.model_provider_id,
    )
    .await
    {
        Ok(runtime) => Some(runtime),
        Err(err) => {
            emit_startup_warning(&format!("failed to initialize state runtime: {err}"));
            None
        }
    }
}

/// Initialize the state runtime and return any initialization error to the caller.
///
/// Prefer [`init`] unless the caller needs to surface the exact failure after
/// tracing or UI setup has completed.
pub async fn try_init(config: &impl RolloutConfigView) -> anyhow::Result<StateDbHandle> {
    let config = RolloutConfig::from_view(config);
    try_init_with_roots(
        config.codex_home,
        config.sqlite_home,
        config.model_provider_id,
    )
    .await
}

async fn try_init_with_roots(
    codex_home: PathBuf,
    sqlite_home: PathBuf,
    default_model_provider_id: String,
) -> anyhow::Result<StateDbHandle> {
    try_init_with_roots_inner(
        codex_home,
        sqlite_home,
        default_model_provider_id,
        /*backfill_lease_seconds*/ None,
    )
    .await
}

#[cfg(test)]
async fn try_init_with_roots_and_backfill_lease(
    codex_home: PathBuf,
    sqlite_home: PathBuf,
    default_model_provider_id: String,
    backfill_lease_seconds: i64,
) -> anyhow::Result<StateDbHandle> {
    try_init_with_roots_inner(
        codex_home,
        sqlite_home,
        default_model_provider_id,
        Some(backfill_lease_seconds),
    )
    .await
}

async fn try_init_with_roots_inner(
    codex_home: PathBuf,
    sqlite_home: PathBuf,
    default_model_provider_id: String,
    backfill_lease_seconds: Option<i64>,
) -> anyhow::Result<StateDbHandle> {
    let runtime =
        codex_state::StateRuntime::init(sqlite_home.clone(), default_model_provider_id.clone())
            .await
            .map_err(|err| {
                anyhow::anyhow!(
                    "failed to initialize state runtime at {}: {err}",
                    sqlite_home.display()
                )
            })?;
    let backfill_gate_started = Instant::now();
    let backfill_gate_result = wait_for_backfill_gate(
        runtime.as_ref(),
        codex_home.as_path(),
        default_model_provider_id.as_str(),
        backfill_lease_seconds,
    )
    .await;
    codex_state::record_backfill_gate(
        /*telemetry*/ None,
        backfill_gate_started.elapsed(),
        &backfill_gate_result,
    );
    backfill_gate_result?;
    Ok(runtime)
}

async fn wait_for_backfill_gate(
    runtime: &codex_state::StateRuntime,
    codex_home: &Path,
    default_model_provider_id: &str,
    backfill_lease_seconds: Option<i64>,
) -> anyhow::Result<()> {
    let wait_started = Instant::now();
    let mut reported_wait = false;
    loop {
        let backfill_state = runtime.get_backfill_state().await.map_err(|err| {
            anyhow::anyhow!(
                "failed to read backfill state at {}: {err}",
                codex_home.display()
            )
        })?;
        if backfill_state.status == codex_state::BackfillStatus::Complete {
            return Ok(());
        }

        if let Some(backfill_lease_seconds) = backfill_lease_seconds {
            metadata::backfill_sessions_with_lease(
                runtime,
                codex_home,
                default_model_provider_id,
                backfill_lease_seconds,
            )
            .await;
        } else {
            metadata::backfill_sessions(runtime, codex_home, default_model_provider_id).await;
        }
        let backfill_state = runtime.get_backfill_state().await.map_err(|err| {
            anyhow::anyhow!(
                "failed to read backfill state at {} after startup backfill: {err}",
                codex_home.display()
            )
        })?;
        if backfill_state.status == codex_state::BackfillStatus::Complete {
            return Ok(());
        }
        if wait_started.elapsed() >= STARTUP_BACKFILL_WAIT_TIMEOUT {
            return Err(anyhow::anyhow!(
                "timed out waiting for state db backfill at {} after {:?} (status: {})",
                codex_home.display(),
                STARTUP_BACKFILL_WAIT_TIMEOUT,
                backfill_state.status.as_str()
            ));
        }

        let message = format!(
            "state db backfill is {} at {}; waiting up to {:?} before retrying startup initialization",
            backfill_state.status.as_str(),
            codex_home.display(),
            STARTUP_BACKFILL_WAIT_TIMEOUT,
        );
        if reported_wait {
            info!("{message}");
        } else {
            emit_startup_warning(&message);
            reported_wait = true;
        }
        tokio::time::sleep(STARTUP_BACKFILL_POLL_INTERVAL).await;
    }
}

fn emit_startup_warning(message: &str) {
    warn!("{message}");
    if !tracing::dispatcher::has_been_set() {
        #[allow(clippy::print_stderr)]
        {
            eprintln!("{message}");
        }
    }
}

/// Open the DB if it exists and its startup backfill has already completed.
///
/// Unlike [`init`], this helper does not run rollout backfill. It is for
/// optional local reads from non-owning contexts such as remote app-server mode.
pub async fn get_state_db(config: &impl RolloutConfigView) -> Option<StateDbHandle> {
    let state_path = codex_state::state_db_path(config.sqlite_home());
    if !tokio::fs::try_exists(&state_path).await.unwrap_or(false) {
        codex_state::record_fallback(
            "get_state_db",
            "db_unavailable",
            /*telemetry_override*/ None,
        );
        return None;
    }
    let runtime = match codex_state::StateRuntime::init(
        config.sqlite_home().to_path_buf(),
        config.model_provider_id().to_string(),
    )
    .await
    {
        Ok(runtime) => runtime,
        Err(_) => {
            codex_state::record_fallback(
                "get_state_db",
                "db_error",
                /*telemetry_override*/ None,
            );
            return None;
        }
    };
    require_backfill_complete(runtime, config.sqlite_home()).await
}

/// Build a SQLite telemetry recorder backed by an OTEL metrics client.
pub fn sqlite_telemetry_recorder(
    metrics: codex_otel::MetricsClient,
    originator: &str,
) -> codex_state::DbTelemetryHandle {
    sqlite_metrics::recorder(metrics, originator)
}

async fn require_backfill_complete(
    runtime: StateDbHandle,
    codex_home: &Path,
) -> Option<StateDbHandle> {
    match runtime.get_backfill_state().await {
        Ok(state) if state.status == codex_state::BackfillStatus::Complete => Some(runtime),
        Ok(state) => {
            warn!(
                "state db backfill not complete at {} (status: {})",
                codex_home.display(),
                state.status.as_str()
            );
            codex_state::record_fallback(
                "get_state_db",
                "backfill_incomplete",
                /*telemetry_override*/ None,
            );
            None
        }
        Err(err) => {
            warn!(
                "failed to read backfill state at {}: {err}",
                codex_home.display()
            );
            codex_state::record_fallback(
                "get_state_db",
                "db_error",
                /*telemetry_override*/ None,
            );
            None
        }
    }
}

fn cursor_to_anchor(cursor: Option<&Cursor>) -> Option<codex_state::Anchor> {
    let cursor = cursor?;
    let millis = cursor.timestamp().unix_timestamp_nanos() / 1_000_000;
    let millis = i64::try_from(millis).ok()?;
    let ts = chrono::DateTime::<Utc>::from_timestamp_millis(millis)?;
    Some(codex_state::Anchor { ts })
}

pub fn normalize_cwd_for_state_db(cwd: &Path) -> PathBuf {
    normalize_for_path_comparison(cwd).unwrap_or_else(|_| cwd.to_path_buf())
}

/// List thread ids from SQLite for parity checks without rollout scanning.
#[allow(clippy::too_many_arguments)]
pub async fn list_thread_ids_db(
    context: Option<&codex_state::StateRuntime>,
    codex_home: &Path,
    page_size: usize,
    cursor: Option<&Cursor>,
    sort_key: ThreadSortKey,
    allowed_sources: &[SessionSource],
    model_providers: Option<&[String]>,
    archived_only: bool,
    stage: &str,
) -> Option<Vec<ThreadId>> {
    let ctx = context?;
    if ctx.codex_home() != codex_home {
        warn!(
            "state db codex_home mismatch: expected {}, got {}",
            ctx.codex_home().display(),
            codex_home.display()
        );
    }

    let anchor = cursor_to_anchor(cursor);
    let allowed_sources: Vec<String> = allowed_sources
        .iter()
        .map(|value| match serde_json::to_value(value) {
            Ok(Value::String(s)) => s,
            Ok(other) => other.to_string(),
            Err(_) => String::new(),
        })
        .collect();
    let model_providers = model_providers.map(<[String]>::to_vec);
    match ctx
        .list_thread_ids(
            page_size,
            anchor.as_ref(),
            match sort_key {
                ThreadSortKey::CreatedAt => codex_state::SortKey::CreatedAt,
                ThreadSortKey::UpdatedAt => codex_state::SortKey::UpdatedAt,
            },
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

/// List thread metadata from SQLite without rollout directory traversal.
#[allow(clippy::too_many_arguments)]
pub async fn list_threads_db(
    context: Option<&codex_state::StateRuntime>,
    codex_home: &Path,
    page_size: usize,
    cursor: Option<&Cursor>,
    sort_key: ThreadSortKey,
    sort_direction: SortDirection,
    allowed_sources: &[SessionSource],
    model_providers: Option<&[String]>,
    cwd_filters: Option<&[PathBuf]>,
    archived: bool,
    search_term: Option<&str>,
) -> Option<codex_state::ThreadsPage> {
    let ctx = context?;
    if ctx.codex_home() != codex_home {
        warn!(
            "state db codex_home mismatch: expected {}, got {}",
            ctx.codex_home().display(),
            codex_home.display()
        );
    }

    let anchor = cursor_to_anchor(cursor);
    let allowed_sources: Vec<String> = allowed_sources
        .iter()
        .map(|value| match serde_json::to_value(value) {
            Ok(Value::String(s)) => s,
            Ok(other) => other.to_string(),
            Err(_) => String::new(),
        })
        .collect();
    let model_providers = model_providers.map(<[String]>::to_vec);
    let normalized_cwd_filters = cwd_filters.map(|filters| {
        filters
            .iter()
            .map(|cwd| normalize_cwd_for_state_db(cwd))
            .collect::<Vec<_>>()
    });
    match ctx
        .list_threads(
            page_size,
            codex_state::ThreadFilterOptions {
                archived_only: archived,
                allowed_sources: allowed_sources.as_slice(),
                model_providers: model_providers.as_deref(),
                cwd_filters: normalized_cwd_filters.as_deref(),
                anchor: anchor.as_ref(),
                sort_key: match sort_key {
                    ThreadSortKey::CreatedAt => codex_state::SortKey::CreatedAt,
                    ThreadSortKey::UpdatedAt => codex_state::SortKey::UpdatedAt,
                },
                sort_direction: match sort_direction {
                    SortDirection::Asc => codex_state::SortDirection::Asc,
                    SortDirection::Desc => codex_state::SortDirection::Desc,
                },
                search_term,
            },
        )
        .await
    {
        Ok(mut page) => {
            let mut valid_items = Vec::with_capacity(page.items.len());
            for item in page.items {
                if tokio::fs::try_exists(&item.rollout_path)
                    .await
                    .unwrap_or(false)
                {
                    valid_items.push(item);
                } else {
                    warn!(
                        "state db list_threads returned stale rollout path for thread {}: {}",
                        item.id,
                        item.rollout_path.display()
                    );
                    warn!("state db discrepancy during list_threads_db: stale_db_path_dropped");
                    let _ = ctx.delete_thread(item.id).await;
                }
            }
            page.items = valid_items;
            Some(page)
        }
        Err(err) => {
            warn!("state db list_threads failed: {err}");
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

/// Get dynamic tools for a thread id using SQLite.
pub async fn get_dynamic_tools(
    context: Option<&codex_state::StateRuntime>,
    thread_id: ThreadId,
    stage: &str,
) -> Option<Vec<DynamicToolSpec>> {
    let ctx = context?;
    match ctx.get_dynamic_tools(thread_id).await {
        Ok(tools) => tools,
        Err(err) => {
            warn!("state db get_dynamic_tools failed during {stage}: {err}");
            None
        }
    }
}

/// Persist dynamic tools for a thread id using SQLite, if none exist yet.
pub async fn persist_dynamic_tools(
    context: Option<&codex_state::StateRuntime>,
    thread_id: ThreadId,
    tools: Option<&[DynamicToolSpec]>,
    stage: &str,
) {
    let Some(ctx) = context else {
        return;
    };
    if let Err(err) = ctx.persist_dynamic_tools(thread_id, tools).await {
        warn!("state db persist_dynamic_tools failed during {stage}: {err}");
    }
}

pub async fn mark_thread_memory_mode_polluted(
    context: Option<&codex_state::StateRuntime>,
    thread_id: ThreadId,
    stage: &str,
) {
    let Some(ctx) = context else {
        return;
    };
    if let Err(err) = ctx.mark_thread_memory_mode_polluted(thread_id).await {
        warn!("state db mark_thread_memory_mode_polluted failed during {stage}: {err}");
    }
}

/// Reconcile rollout items into SQLite, falling back to scanning the rollout file.
pub async fn reconcile_rollout(
    context: Option<&codex_state::StateRuntime>,
    rollout_path: &Path,
    default_provider: &str,
    builder: Option<&ThreadMetadataBuilder>,
    items: &[RolloutItem],
    archived_only: Option<bool>,
    new_thread_memory_mode: Option<&str>,
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
            new_thread_memory_mode,
            /*updated_at_override*/ None,
        )
        .await;
        return;
    }
    let outcome =
        match metadata::extract_metadata_from_rollout(rollout_path, default_provider).await {
            Ok(outcome) => outcome,
            Err(err) => {
                warn!(
                    "state db reconcile_rollout extraction failed {}: {err}",
                    rollout_path.display()
                );
                return;
            }
        };
    let mut metadata = outcome.metadata;
    let memory_mode = outcome.memory_mode.unwrap_or_else(|| "enabled".to_string());
    metadata.cwd = normalize_cwd_for_state_db(&metadata.cwd);
    if let Ok(Some(existing_metadata)) = ctx.get_thread(metadata.id).await {
        metadata.prefer_existing_git_info(&existing_metadata);
    }
    match archived_only {
        Some(true) if metadata.archived_at.is_none() => {
            metadata.archived_at = Some(metadata.updated_at);
        }
        Some(false) => {
            metadata.archived_at = None;
        }
        Some(true) | None => {}
    }
    if let Err(err) = ctx.upsert_thread(&metadata).await {
        warn!(
            "state db reconcile_rollout upsert failed {}: {err}",
            rollout_path.display()
        );
        return;
    }
    if let Err(err) = ctx
        .set_thread_memory_mode(metadata.id, memory_mode.as_str())
        .await
    {
        warn!(
            "state db reconcile_rollout memory_mode update failed {}: {err}",
            rollout_path.display()
        );
        return;
    }
    if let Ok(meta_line) = crate::list::read_session_meta_line(rollout_path).await {
        persist_dynamic_tools(
            Some(ctx),
            meta_line.meta.id,
            meta_line.meta.dynamic_tools.as_deref(),
            "reconcile_rollout",
        )
        .await;
    } else {
        warn!(
            "state db reconcile_rollout missing session meta {}",
            rollout_path.display()
        );
    }
}

/// Repair a thread's rollout path after filesystem fallback succeeds.
pub async fn read_repair_rollout_path(
    context: Option<&codex_state::StateRuntime>,
    thread_id: Option<ThreadId>,
    archived_only: Option<bool>,
    rollout_path: &Path,
) {
    let Some(ctx) = context else {
        return;
    };

    // Fast path: update an existing metadata row in place, but avoid writes when
    // read-repair computes no effective change.
    let mut saw_existing_metadata = false;
    if let Some(thread_id) = thread_id
        && let Ok(Some(metadata)) = ctx.get_thread(thread_id).await
    {
        saw_existing_metadata = true;
        let mut repaired = metadata.clone();
        repaired.rollout_path = rollout_path.to_path_buf();
        repaired.cwd = normalize_cwd_for_state_db(&repaired.cwd);
        match archived_only {
            Some(true) if repaired.archived_at.is_none() => {
                repaired.archived_at = Some(repaired.updated_at);
            }
            Some(false) => {
                repaired.archived_at = None;
            }
            Some(true) | None => {}
        }
        if repaired == metadata {
            return;
        }
        warn!("state db discrepancy during read_repair_rollout_path: upsert_needed (fast path)");
        if let Err(err) = ctx.upsert_thread(&repaired).await {
            warn!(
                "state db read-repair upsert failed for {}: {err}",
                rollout_path.display()
            );
        } else {
            return;
        }
    }

    // Slow path: when the row is missing/unreadable (or direct upsert failed),
    // rebuild metadata from rollout contents and reconcile it into SQLite.
    if !saw_existing_metadata {
        warn!("state db discrepancy during read_repair_rollout_path: upsert_needed (slow path)");
    }
    let default_provider = crate::list::read_session_meta_line(rollout_path)
        .await
        .ok()
        .and_then(|meta| meta.meta.model_provider)
        .unwrap_or_default();
    reconcile_rollout(
        Some(ctx),
        rollout_path,
        default_provider.as_str(),
        /*builder*/ None,
        &[],
        archived_only,
        /*new_thread_memory_mode*/ None,
    )
    .await;
}

/// Apply rollout items incrementally to SQLite.
#[allow(clippy::too_many_arguments)]
pub async fn apply_rollout_items(
    context: Option<&codex_state::StateRuntime>,
    rollout_path: &Path,
    default_provider: &str,
    builder: Option<&ThreadMetadataBuilder>,
    items: &[RolloutItem],
    stage: &str,
    new_thread_memory_mode: Option<&str>,
    updated_at_override: Option<DateTime<Utc>>,
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
                warn!("state db discrepancy during apply_rollout_items: {stage}, missing_builder");
                return;
            }
        },
    };
    if builder.model_provider.is_none() {
        builder.model_provider = Some(default_provider.to_string());
    }
    builder.rollout_path = rollout_path.to_path_buf();
    builder.cwd = normalize_cwd_for_state_db(&builder.cwd);
    if let Err(err) = ctx
        .apply_rollout_items(&builder, items, new_thread_memory_mode, updated_at_override)
        .await
    {
        warn!(
            "state db apply_rollout_items failed during {stage} for {}: {err}",
            rollout_path.display()
        );
    }
}

pub async fn touch_thread_updated_at(
    context: Option<&codex_state::StateRuntime>,
    thread_id: Option<ThreadId>,
    updated_at: DateTime<Utc>,
    stage: &str,
) -> bool {
    let Some(ctx) = context else {
        return false;
    };
    let Some(thread_id) = thread_id else {
        return false;
    };
    ctx.touch_thread_updated_at(thread_id, updated_at)
        .await
        .unwrap_or_else(|err| {
            warn!("state db touch_thread_updated_at failed during {stage} for {thread_id}: {err}");
            false
        })
}

#[cfg(test)]
#[path = "state_db_tests.rs"]
mod tests;

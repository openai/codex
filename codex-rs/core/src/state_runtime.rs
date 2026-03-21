use crate::config::Config;
use codex_protocol::ThreadId;
use codex_protocol::dynamic_tools::DynamicToolSpec;
pub use codex_state::LogEntry;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::warn;

/// Core-facing handle to the SQLite-backed state runtime.
pub type StateDbHandle = Arc<codex_state::StateRuntime>;

/// Initialize the state runtime for thread state persistence. To only be used
/// inside `core`. The initialization should not be done anywhere else.
pub(crate) async fn init(config: &Config) -> Option<StateDbHandle> {
    match codex_state::StateRuntime::init(
        config.sqlite_home.clone(),
        config.model_provider_id.clone(),
    )
    .await
    {
        Ok(runtime) => Some(runtime),
        Err(err) => {
            warn!(
                "failed to initialize state runtime at {}: {err}",
                config.sqlite_home.display()
            );
            None
        }
    }
}

/// Get the DB if the feature is enabled and the DB exists.
pub async fn get_state_db(config: &Config) -> Option<StateDbHandle> {
    let state_path = codex_state::state_db_path(config.sqlite_home.as_path());
    if !tokio::fs::try_exists(&state_path).await.unwrap_or(false) {
        return None;
    }
    let runtime = codex_state::StateRuntime::init(
        config.sqlite_home.clone(),
        config.model_provider_id.clone(),
    )
    .await
    .ok()?;
    require_backfill_complete(runtime, config.sqlite_home.as_path()).await
}

/// Open the state runtime when the SQLite file exists, without feature gating.
///
/// This is used for parity checks during the SQLite migration phase.
pub async fn open_if_present(codex_home: &Path, default_provider: &str) -> Option<StateDbHandle> {
    let db_path = codex_state::state_db_path(codex_home);
    if !tokio::fs::try_exists(&db_path).await.unwrap_or(false) {
        return None;
    }
    let runtime =
        codex_state::StateRuntime::init(codex_home.to_path_buf(), default_provider.to_string())
            .await
            .ok()?;
    require_backfill_complete(runtime, codex_home).await
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
            None
        }
        Err(err) => {
            warn!(
                "failed to read backfill state at {}: {err}",
                codex_home.display()
            );
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

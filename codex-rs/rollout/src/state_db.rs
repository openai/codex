use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Timelike;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use codex_state::ThreadMetadataBuilder;
use serde_json::Value;
use tracing::warn;
use uuid::Uuid;

use crate::list::Cursor;
use crate::list::ThreadSortKey;
use crate::list::read_session_meta_line;
use crate::metadata;
use crate::path_utils::normalize_for_path_comparison;

pub type StateDbHandle = Arc<codex_state::StateRuntime>;

pub(crate) async fn open_if_present(
    codex_home: &Path,
    default_provider: &str,
) -> Option<StateDbHandle> {
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

pub(crate) fn normalize_cwd_for_state_db(cwd: &Path) -> PathBuf {
    normalize_for_path_comparison(cwd).unwrap_or_else(|_| cwd.to_path_buf())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn list_thread_ids_db(
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
            Ok(Value::String(value)) => value,
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

#[allow(clippy::too_many_arguments)]
pub(crate) async fn list_threads_db(
    context: Option<&codex_state::StateRuntime>,
    codex_home: &Path,
    page_size: usize,
    cursor: Option<&Cursor>,
    sort_key: ThreadSortKey,
    allowed_sources: &[SessionSource],
    model_providers: Option<&[String]>,
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
            Ok(Value::String(value)) => value,
            Ok(other) => other.to_string(),
            Err(_) => String::new(),
        })
        .collect();
    let model_providers = model_providers.map(<[String]>::to_vec);
    match ctx
        .list_threads(
            page_size,
            anchor.as_ref(),
            match sort_key {
                ThreadSortKey::CreatedAt => codex_state::SortKey::CreatedAt,
                ThreadSortKey::UpdatedAt => codex_state::SortKey::UpdatedAt,
            },
            allowed_sources.as_slice(),
            model_providers.as_deref(),
            archived,
            search_term,
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

pub(crate) async fn find_rollout_path_by_id(
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

pub(crate) async fn persist_dynamic_tools(
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

#[allow(clippy::too_many_arguments)]
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
    if let Ok(meta_line) = read_session_meta_line(rollout_path).await {
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

pub async fn read_repair_rollout_path(
    context: Option<&codex_state::StateRuntime>,
    thread_id: Option<ThreadId>,
    archived_only: Option<bool>,
    rollout_path: &Path,
) {
    let Some(ctx) = context else {
        return;
    };
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
    if !saw_existing_metadata {
        warn!("state db discrepancy during read_repair_rollout_path: upsert_needed (slow path)");
    }
    let default_provider = read_session_meta_line(rollout_path)
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

#[allow(clippy::too_many_arguments)]
pub(crate) async fn apply_rollout_items(
    context: Option<&codex_state::StateRuntime>,
    rollout_path: &Path,
    _default_provider: &str,
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

pub(crate) async fn touch_thread_updated_at(
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

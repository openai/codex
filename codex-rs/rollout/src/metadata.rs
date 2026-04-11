use crate::ARCHIVED_SESSIONS_SUBDIR;
use crate::SESSIONS_SUBDIR;
use crate::config::RolloutConfigView;
use crate::list;
use crate::list::parse_timestamp_uuid_from_filename;
use crate::recorder::RolloutRecorder;
use crate::state_db::normalize_cwd_for_state_db;
use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Timelike;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_state::BackfillState;
use codex_state::BackfillStats;
use codex_state::BackfillStatus;
use codex_state::DB_ERROR_METRIC;
use codex_state::DB_METRIC_BACKFILL;
use codex_state::DB_METRIC_BACKFILL_DURATION_MS;
use codex_state::ExtractionOutcome;
use codex_state::ThreadMetadataBuilder;
use codex_state::apply_rollout_item;
use std::path::Path;
use std::path::PathBuf;
use tracing::info;
use tracing::warn;

const ROLLOUT_PREFIX: &str = "rollout-";
const ROLLOUT_SUFFIX: &str = ".jsonl";
const BACKFILL_BATCH_SIZE: usize = 200;
#[cfg(not(test))]
const BACKFILL_LEASE_SECONDS: i64 = 900;
#[cfg(test)]
const BACKFILL_LEASE_SECONDS: i64 = 1;

pub(crate) fn builder_from_session_meta(
    session_meta: &SessionMetaLine,
    rollout_path: &Path,
) -> Option<ThreadMetadataBuilder> {
    let created_at = parse_timestamp_to_utc(session_meta.meta.timestamp.as_str())?;
    let mut builder = ThreadMetadataBuilder::new(
        session_meta.meta.id,
        rollout_path.to_path_buf(),
        created_at,
        session_meta.meta.source.clone(),
    );
    builder.model_provider = session_meta.meta.model_provider.clone();
    builder.agent_nickname = session_meta.meta.agent_nickname.clone();
    builder.agent_role = session_meta.meta.agent_role.clone();
    builder.agent_path = session_meta.meta.agent_path.clone();
    builder.cwd = session_meta.meta.cwd.clone();
    builder.cli_version = Some(session_meta.meta.cli_version.clone());
    builder.sandbox_policy = SandboxPolicy::new_read_only_policy();
    builder.approval_mode = AskForApproval::OnRequest;
    if let Some(git) = session_meta.git.as_ref() {
        builder.git_sha = git.commit_hash.as_ref().map(|sha| sha.0.clone());
        builder.git_branch = git.branch.clone();
        builder.git_origin_url = git.repository_url.clone();
    }
    Some(builder)
}

pub fn builder_from_items(
    items: &[RolloutItem],
    rollout_path: &Path,
) -> Option<ThreadMetadataBuilder> {
    if let Some(session_meta) = items.iter().find_map(|item| match item {
        RolloutItem::SessionMeta(meta_line) => Some(meta_line),
        RolloutItem::ResponseItem(_)
        | RolloutItem::Compacted(_)
        | RolloutItem::TurnContext(_)
        | RolloutItem::EventMsg(_) => None,
    }) && let Some(builder) = builder_from_session_meta(session_meta, rollout_path)
    {
        return Some(builder);
    }

    let file_name = rollout_path.file_name()?.to_str()?;
    if !file_name.starts_with(ROLLOUT_PREFIX) || !file_name.ends_with(ROLLOUT_SUFFIX) {
        return None;
    }
    let (created_ts, uuid) = parse_timestamp_uuid_from_filename(file_name)?;
    let created_at =
        DateTime::<Utc>::from_timestamp(created_ts.unix_timestamp(), 0)?.with_nanosecond(0)?;
    let id = ThreadId::from_string(&uuid.to_string()).ok()?;
    Some(ThreadMetadataBuilder::new(
        id,
        rollout_path.to_path_buf(),
        created_at,
        SessionSource::default(),
    ))
}

pub async fn extract_metadata_from_rollout(
    rollout_path: &Path,
    default_provider: &str,
) -> anyhow::Result<ExtractionOutcome> {
    let (items, _thread_id, parse_errors) =
        RolloutRecorder::load_rollout_items(rollout_path).await?;
    if items.is_empty() {
        return Err(anyhow::anyhow!(
            "empty session file: {}",
            rollout_path.display()
        ));
    }
    let builder = builder_from_items(items.as_slice(), rollout_path).ok_or_else(|| {
        anyhow::anyhow!(
            "rollout missing metadata builder: {}",
            rollout_path.display()
        )
    })?;
    let mut metadata = builder.build(default_provider);
    for item in &items {
        apply_rollout_item(&mut metadata, item, default_provider);
    }
    if let Some(updated_at) = file_modified_time_utc(rollout_path).await {
        metadata.updated_at = updated_at;
    }
    Ok(ExtractionOutcome {
        metadata,
        memory_mode: items.iter().rev().find_map(|item| match item {
            RolloutItem::SessionMeta(meta_line) => meta_line.meta.memory_mode.clone(),
            RolloutItem::ResponseItem(_)
            | RolloutItem::Compacted(_)
            | RolloutItem::TurnContext(_)
            | RolloutItem::EventMsg(_) => None,
        }),
        parse_errors,
    })
}

pub(crate) async fn backfill_sessions(
    runtime: &codex_state::StateRuntime,
    config: &impl RolloutConfigView,
) {
    let metric_client = codex_otel::global();
    let timer = metric_client
        .as_ref()
        .and_then(|otel| otel.start_timer(DB_METRIC_BACKFILL_DURATION_MS, &[]).ok());
    let backfill_state = match runtime.get_backfill_state().await {
        Ok(state) => state,
        Err(err) => {
            warn!(
                "failed to read backfill state at {}: {err}",
                config.codex_home().display()
            );
            BackfillState::default()
        }
    };
    if backfill_state.status == BackfillStatus::Complete {
        return;
    }
    let claimed = match runtime.try_claim_backfill(BACKFILL_LEASE_SECONDS).await {
        Ok(claimed) => claimed,
        Err(err) => {
            warn!(
                "failed to claim backfill worker at {}: {err}",
                config.codex_home().display()
            );
            return;
        }
    };
    if !claimed {
        info!(
            "state db backfill already running at {}; skipping duplicate worker",
            config.codex_home().display()
        );
        return;
    }
    let mut backfill_state = match runtime.get_backfill_state().await {
        Ok(state) => state,
        Err(err) => {
            warn!(
                "failed to read claimed backfill state at {}: {err}",
                config.codex_home().display()
            );
            BackfillState {
                status: BackfillStatus::Running,
                ..Default::default()
            }
        }
    };
    if backfill_state.status != BackfillStatus::Running {
        if let Err(err) = runtime.mark_backfill_running().await {
            warn!(
                "failed to mark backfill running at {}: {err}",
                config.codex_home().display()
            );
        } else {
            backfill_state.status = BackfillStatus::Running;
        }
    }

    let sessions_root = config.codex_home().join(SESSIONS_SUBDIR);
    let archived_root = config.codex_home().join(ARCHIVED_SESSIONS_SUBDIR);
    let mut rollout_paths: Vec<BackfillRolloutPath> = Vec::new();
    for (root, archived) in [(sessions_root, false), (archived_root, true)] {
        if !tokio::fs::try_exists(&root).await.unwrap_or(false) {
            continue;
        }
        match collect_rollout_paths(&root).await {
            Ok(paths) => {
                rollout_paths.extend(paths.into_iter().map(|path| BackfillRolloutPath {
                    watermark: backfill_watermark_for_path(config.codex_home(), &path),
                    path,
                    archived,
                }));
            }
            Err(err) => {
                warn!(
                    "failed to collect rollout paths under {}: {err}",
                    root.display()
                );
            }
        }
    }
    rollout_paths.sort_by(|a, b| a.watermark.cmp(&b.watermark));
    if let Some(last_watermark) = backfill_state.last_watermark.as_deref() {
        rollout_paths.retain(|entry| entry.watermark.as_str() > last_watermark);
    }

    let mut stats = BackfillStats {
        scanned: 0,
        upserted: 0,
        failed: 0,
    };
    let mut last_watermark = backfill_state.last_watermark.clone();
    for batch in rollout_paths.chunks(BACKFILL_BATCH_SIZE) {
        for rollout in batch {
            stats.scanned = stats.scanned.saturating_add(1);
            match extract_metadata_from_rollout(&rollout.path, config.model_provider_id()).await {
                Ok(outcome) => {
                    if outcome.parse_errors > 0
                        && let Some(ref metric_client) = metric_client
                    {
                        let _ = metric_client.counter(
                            DB_ERROR_METRIC,
                            outcome.parse_errors as i64,
                            &[("stage", "backfill_sessions")],
                        );
                    }
                    let mut metadata = outcome.metadata;
                    metadata.cwd = normalize_cwd_for_state_db(&metadata.cwd);
                    let memory_mode = outcome.memory_mode.unwrap_or_else(|| "enabled".to_string());
                    if let Ok(Some(existing_metadata)) = runtime.get_thread(metadata.id).await {
                        metadata.prefer_existing_git_info(&existing_metadata);
                    }
                    if rollout.archived && metadata.archived_at.is_none() {
                        let fallback_archived_at = metadata.updated_at;
                        metadata.archived_at = file_modified_time_utc(&rollout.path)
                            .await
                            .or(Some(fallback_archived_at));
                    }
                    if let Err(err) = runtime.upsert_thread(&metadata).await {
                        stats.failed = stats.failed.saturating_add(1);
                        warn!("failed to upsert rollout {}: {err}", rollout.path.display());
                    } else {
                        if let Err(err) = runtime
                            .set_thread_memory_mode(metadata.id, memory_mode.as_str())
                            .await
                        {
                            stats.failed = stats.failed.saturating_add(1);
                            warn!(
                                "failed to restore memory mode for {}: {err}",
                                rollout.path.display()
                            );
                            continue;
                        }
                        stats.upserted = stats.upserted.saturating_add(1);
                        if let Ok(meta_line) = list::read_session_meta_line(&rollout.path).await {
                            if let Err(err) = runtime
                                .persist_dynamic_tools(
                                    meta_line.meta.id,
                                    meta_line.meta.dynamic_tools.as_deref(),
                                )
                                .await
                            {
                                warn!(
                                    "failed to backfill dynamic tools {}: {err}",
                                    rollout.path.display()
                                );
                            }
                        } else {
                            warn!(
                                "failed to read session meta for dynamic tools {}",
                                rollout.path.display()
                            );
                        }
                    }
                }
                Err(err) => {
                    stats.failed = stats.failed.saturating_add(1);
                    warn!(
                        "failed to extract rollout {}: {err}",
                        rollout.path.display()
                    );
                }
            }
        }

        if let Some(last_entry) = batch.last() {
            if let Err(err) = runtime
                .checkpoint_backfill(last_entry.watermark.as_str())
                .await
            {
                warn!(
                    "failed to checkpoint backfill at {}: {err}",
                    config.codex_home().display()
                );
            } else {
                last_watermark = Some(last_entry.watermark.clone());
            }
        }
    }
    if let Err(err) = runtime
        .mark_backfill_complete(last_watermark.as_deref())
        .await
    {
        warn!(
            "failed to mark backfill complete at {}: {err}",
            config.codex_home().display()
        );
    }

    info!(
        "state db backfill scanned={}, upserted={}, failed={}",
        stats.scanned, stats.upserted, stats.failed
    );
    if let Some(metric_client) = metric_client {
        let _ = metric_client.counter(
            DB_METRIC_BACKFILL,
            stats.upserted as i64,
            &[("status", "upserted")],
        );
        let _ = metric_client.counter(
            DB_METRIC_BACKFILL,
            stats.failed as i64,
            &[("status", "failed")],
        );
    }
    if let Some(timer) = timer.as_ref() {
        let status = if stats.failed == 0 {
            "success"
        } else if stats.upserted == 0 {
            "failed"
        } else {
            "partial_failure"
        };
        let _ = timer.record(&[("status", status)]);
    }
}

#[derive(Debug, Clone)]
struct BackfillRolloutPath {
    watermark: String,
    path: PathBuf,
    archived: bool,
}

fn backfill_watermark_for_path(codex_home: &Path, path: &Path) -> String {
    path.strip_prefix(codex_home)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

async fn file_modified_time_utc(path: &Path) -> Option<DateTime<Utc>> {
    let modified = tokio::fs::metadata(path).await.ok()?.modified().ok()?;
    let updated_at: DateTime<Utc> = modified.into();
    updated_at.with_nanosecond(0)
}

fn parse_timestamp_to_utc(ts: &str) -> Option<DateTime<Utc>> {
    const FILENAME_TS_FORMAT: &str = "%Y-%m-%dT%H-%M-%S";
    if let Ok(naive) = NaiveDateTime::parse_from_str(ts, FILENAME_TS_FORMAT) {
        let dt = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);
        return dt.with_nanosecond(0);
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(ts) {
        return dt.with_timezone(&Utc).with_nanosecond(0);
    }
    None
}

async fn collect_rollout_paths(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut stack = vec![root.to_path_buf()];
    let mut paths = Vec::new();
    while let Some(dir) = stack.pop() {
        let mut read_dir = match tokio::fs::read_dir(&dir).await {
            Ok(read_dir) => read_dir,
            Err(err) => {
                warn!("failed to read directory {}: {err}", dir.display());
                continue;
            }
        };
        loop {
            let next_entry = match read_dir.next_entry().await {
                Ok(next_entry) => next_entry,
                Err(err) => {
                    warn!(
                        "failed to read directory entry under {}: {err}",
                        dir.display()
                    );
                    continue;
                }
            };
            let Some(entry) = next_entry else {
                break;
            };
            let path = entry.path();
            let file_type = match entry.file_type().await {
                Ok(file_type) => file_type,
                Err(err) => {
                    warn!("failed to read file type for {}: {err}", path.display());
                    continue;
                }
            };
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let file_name = entry.file_name();
            let Some(name) = file_name.to_str() else {
                continue;
            };
            if name.starts_with(ROLLOUT_PREFIX) && name.ends_with(ROLLOUT_SUFFIX) {
                paths.push(path);
            }
        }
    }
    Ok(paths)
}

#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;

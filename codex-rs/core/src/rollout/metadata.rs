use crate::config::Config;
use crate::rollout;
use crate::rollout::list::parse_timestamp_uuid_from_filename;
use crate::rollout::recorder::RolloutRecorder;
use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Timelike;
use chrono::Utc;
use codex_otel::OtelManager;
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
    builder.cwd = session_meta.meta.cwd.clone();
    builder.cli_version = Some(session_meta.meta.cli_version.clone());
    builder.sandbox_policy = SandboxPolicy::new_read_only_policy();
    builder.approval_mode = AskForApproval::OnRequest;
    if let Some(git) = session_meta.git.as_ref() {
        builder.git_sha = git.commit_hash.clone();
        builder.git_branch = git.branch.clone();
        builder.git_origin_url = git.repository_url.clone();
    }
    Some(builder)
}

pub(crate) fn builder_from_items(
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

pub(crate) async fn extract_metadata_from_rollout(
    rollout_path: &Path,
    default_provider: &str,
    otel: Option<&OtelManager>,
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
    if parse_errors > 0
        && let Some(otel) = otel
    {
        otel.counter(
            DB_ERROR_METRIC,
            parse_errors as i64,
            &[("stage", "extract_metadata_from_rollout")],
        );
    }
    Ok(ExtractionOutcome {
        metadata,
        parse_errors,
    })
}

pub(crate) async fn backfill_sessions(
    runtime: &codex_state::StateRuntime,
    config: &Config,
    otel: Option<&OtelManager>,
) {
    let timer = otel.and_then(|otel| otel.start_timer(DB_METRIC_BACKFILL_DURATION_MS, &[]).ok());
    let backfill_state = match runtime.get_backfill_state().await {
        Ok(state) => state,
        Err(err) => {
            warn!(
                "failed to read backfill state at {}: {err}",
                config.codex_home.display()
            );
            if let Some(otel) = otel {
                otel.counter(DB_ERROR_METRIC, 1, &[("stage", "backfill_state_read")]);
            }
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
                config.codex_home.display()
            );
            if let Some(otel) = otel {
                otel.counter(
                    DB_ERROR_METRIC,
                    1,
                    &[("stage", "backfill_state_claim_running")],
                );
            }
            return;
        }
    };
    if !claimed {
        info!(
            "state db backfill already running at {}; skipping duplicate worker",
            config.codex_home.display()
        );
        return;
    }
    let mut backfill_state = match runtime.get_backfill_state().await {
        Ok(state) => state,
        Err(err) => {
            warn!(
                "failed to read claimed backfill state at {}: {err}",
                config.codex_home.display()
            );
            if let Some(otel) = otel {
                otel.counter(
                    DB_ERROR_METRIC,
                    1,
                    &[("stage", "backfill_state_read_claimed")],
                );
            }
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
                config.codex_home.display()
            );
            if let Some(otel) = otel {
                otel.counter(
                    DB_ERROR_METRIC,
                    1,
                    &[("stage", "backfill_state_mark_running")],
                );
            }
        } else {
            backfill_state.status = BackfillStatus::Running;
        }
    }

    let sessions_root = config.codex_home.join(rollout::SESSIONS_SUBDIR);
    let archived_root = config.codex_home.join(rollout::ARCHIVED_SESSIONS_SUBDIR);
    let mut rollout_paths: Vec<BackfillRolloutPath> = Vec::new();
    for (root, archived) in [(sessions_root, false), (archived_root, true)] {
        if !tokio::fs::try_exists(&root).await.unwrap_or(false) {
            continue;
        }
        match collect_rollout_paths(&root).await {
            Ok(paths) => {
                rollout_paths.extend(paths.into_iter().map(|path| BackfillRolloutPath {
                    watermark: backfill_watermark_for_path(config.codex_home.as_path(), &path),
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
            match extract_metadata_from_rollout(
                &rollout.path,
                config.model_provider_id.as_str(),
                otel,
            )
            .await
            {
                Ok(outcome) => {
                    if outcome.parse_errors > 0
                        && let Some(otel) = otel
                    {
                        otel.counter(
                            DB_ERROR_METRIC,
                            outcome.parse_errors as i64,
                            &[("stage", "backfill_sessions")],
                        );
                    }
                    let mut metadata = outcome.metadata;
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
                        stats.upserted = stats.upserted.saturating_add(1);
                        if let Ok(meta_line) =
                            rollout::list::read_session_meta_line(&rollout.path).await
                        {
                            if let Err(err) = runtime
                                .persist_dynamic_tools(
                                    meta_line.meta.id,
                                    meta_line.meta.dynamic_tools.as_deref(),
                                )
                                .await
                            {
                                if let Some(otel) = otel {
                                    otel.counter(
                                        DB_ERROR_METRIC,
                                        1,
                                        &[("stage", "backfill_dynamic_tools")],
                                    );
                                }
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
                    config.codex_home.display()
                );
                if let Some(otel) = otel {
                    otel.counter(
                        DB_ERROR_METRIC,
                        1,
                        &[("stage", "backfill_state_checkpoint")],
                    );
                }
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
            config.codex_home.display()
        );
        if let Some(otel) = otel {
            otel.counter(
                DB_ERROR_METRIC,
                1,
                &[("stage", "backfill_state_mark_complete")],
            );
        }
    }

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
mod tests {
    use super::*;
    use chrono::DateTime;
    use chrono::NaiveDateTime;
    use chrono::Timelike;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::CompactedItem;
    use codex_protocol::protocol::RolloutItem;
    use codex_protocol::protocol::RolloutLine;
    use codex_protocol::protocol::SessionMeta;
    use codex_protocol::protocol::SessionMetaLine;
    use codex_protocol::protocol::SessionSource;
    use codex_state::BackfillStatus;
    use codex_state::ThreadMetadataBuilder;
    use pretty_assertions::assert_eq;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use uuid::Uuid;

    #[tokio::test]
    async fn extract_metadata_from_rollout_uses_session_meta() {
        let dir = tempdir().expect("tempdir");
        let uuid = Uuid::new_v4();
        let id = ThreadId::from_string(&uuid.to_string()).expect("thread id");
        let path = dir
            .path()
            .join(format!("rollout-2026-01-27T12-34-56-{uuid}.jsonl"));

        let session_meta = SessionMeta {
            id,
            forked_from_id: None,
            timestamp: "2026-01-27T12:34:56Z".to_string(),
            cwd: dir.path().to_path_buf(),
            originator: "cli".to_string(),
            cli_version: "0.0.0".to_string(),
            source: SessionSource::default(),
            agent_nickname: None,
            agent_role: None,
            model_provider: Some("openai".to_string()),
            base_instructions: None,
            dynamic_tools: None,
        };
        let session_meta_line = SessionMetaLine {
            meta: session_meta,
            git: None,
        };
        let rollout_line = RolloutLine {
            timestamp: "2026-01-27T12:34:56Z".to_string(),
            item: RolloutItem::SessionMeta(session_meta_line.clone()),
        };
        let json = serde_json::to_string(&rollout_line).expect("rollout json");
        let mut file = File::create(&path).expect("create rollout");
        writeln!(file, "{json}").expect("write rollout");

        let outcome = extract_metadata_from_rollout(&path, "openai", None)
            .await
            .expect("extract");

        let builder =
            builder_from_session_meta(&session_meta_line, path.as_path()).expect("builder");
        let mut expected = builder.build("openai");
        apply_rollout_item(&mut expected, &rollout_line.item, "openai");
        expected.updated_at = file_modified_time_utc(&path).await.expect("mtime");

        assert_eq!(outcome.metadata, expected);
        assert_eq!(outcome.parse_errors, 0);
    }

    #[test]
    fn builder_from_items_falls_back_to_filename() {
        let dir = tempdir().expect("tempdir");
        let uuid = Uuid::new_v4();
        let path = dir
            .path()
            .join(format!("rollout-2026-01-27T12-34-56-{uuid}.jsonl"));
        let items = vec![RolloutItem::Compacted(CompactedItem {
            message: "noop".to_string(),
            replacement_history: None,
        })];

        let builder = builder_from_items(items.as_slice(), path.as_path()).expect("builder");
        let naive = NaiveDateTime::parse_from_str("2026-01-27T12-34-56", "%Y-%m-%dT%H-%M-%S")
            .expect("timestamp");
        let created_at = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc)
            .with_nanosecond(0)
            .expect("nanosecond");
        let expected = ThreadMetadataBuilder::new(
            ThreadId::from_string(&uuid.to_string()).expect("thread id"),
            path,
            created_at,
            SessionSource::default(),
        );

        assert_eq!(builder, expected);
    }

    #[tokio::test]
    async fn backfill_sessions_resumes_from_watermark_and_marks_complete() {
        let dir = tempdir().expect("tempdir");
        let codex_home = dir.path().to_path_buf();
        let first_uuid = Uuid::new_v4();
        let second_uuid = Uuid::new_v4();
        let first_path = write_rollout_in_sessions(
            codex_home.as_path(),
            "2026-01-27T12-34-56",
            "2026-01-27T12:34:56Z",
            first_uuid,
        );
        let second_path = write_rollout_in_sessions(
            codex_home.as_path(),
            "2026-01-27T12-35-56",
            "2026-01-27T12:35:56Z",
            second_uuid,
        );

        let runtime =
            codex_state::StateRuntime::init(codex_home.clone(), "test-provider".to_string(), None)
                .await
                .expect("initialize runtime");
        let first_watermark =
            backfill_watermark_for_path(codex_home.as_path(), first_path.as_path());
        runtime.mark_backfill_running().await.expect("mark running");
        runtime
            .checkpoint_backfill(first_watermark.as_str())
            .await
            .expect("checkpoint first watermark");
        tokio::time::sleep(std::time::Duration::from_secs(
            (BACKFILL_LEASE_SECONDS + 1) as u64,
        ))
        .await;

        let mut config = crate::config::test_config();
        config.codex_home = codex_home.clone();
        config.model_provider_id = "test-provider".to_string();
        backfill_sessions(runtime.as_ref(), &config, None).await;

        let first_id = ThreadId::from_string(&first_uuid.to_string()).expect("first thread id");
        let second_id = ThreadId::from_string(&second_uuid.to_string()).expect("second thread id");
        assert_eq!(
            runtime
                .get_thread(first_id)
                .await
                .expect("get first thread"),
            None
        );
        assert!(
            runtime
                .get_thread(second_id)
                .await
                .expect("get second thread")
                .is_some()
        );

        let state = runtime
            .get_backfill_state()
            .await
            .expect("get backfill state");
        assert_eq!(state.status, BackfillStatus::Complete);
        assert_eq!(
            state.last_watermark,
            Some(backfill_watermark_for_path(
                codex_home.as_path(),
                second_path.as_path()
            ))
        );
        assert!(state.last_success_at.is_some());
    }

    fn write_rollout_in_sessions(
        codex_home: &Path,
        filename_ts: &str,
        event_ts: &str,
        thread_uuid: Uuid,
    ) -> PathBuf {
        let id = ThreadId::from_string(&thread_uuid.to_string()).expect("thread id");
        let sessions_dir = codex_home.join("sessions");
        std::fs::create_dir_all(sessions_dir.as_path()).expect("create sessions dir");
        let path = sessions_dir.join(format!("rollout-{filename_ts}-{thread_uuid}.jsonl"));
        let session_meta = SessionMeta {
            id,
            forked_from_id: None,
            timestamp: event_ts.to_string(),
            cwd: codex_home.to_path_buf(),
            originator: "cli".to_string(),
            cli_version: "0.0.0".to_string(),
            source: SessionSource::default(),
            agent_nickname: None,
            agent_role: None,
            model_provider: Some("test-provider".to_string()),
            base_instructions: None,
            dynamic_tools: None,
        };
        let session_meta_line = SessionMetaLine {
            meta: session_meta,
            git: None,
        };
        let rollout_line = RolloutLine {
            timestamp: event_ts.to_string(),
            item: RolloutItem::SessionMeta(session_meta_line),
        };
        let json = serde_json::to_string(&rollout_line).expect("serialize rollout");
        let mut file = File::create(&path).expect("create rollout");
        writeln!(file, "{json}").expect("write rollout");
        path
    }
}

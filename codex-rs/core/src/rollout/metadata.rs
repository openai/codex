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
use codex_state::BackfillStats;
use codex_state::DB_ERROR_METRIC;
use codex_state::DB_METRIC_BACKFILL;
use codex_state::DB_METRIC_BACKFILL_DURATION_MS;
use codex_state::ExtractionOutcome;
use codex_state::ThreadMetadataBuilder;
use codex_state::apply_rollout_item;
use std::cmp::Reverse;
use std::path::Path;
use std::path::PathBuf;
use tracing::info;
use tracing::warn;

const ROLLOUT_PREFIX: &str = "rollout-";
const ROLLOUT_SUFFIX: &str = ".jsonl";

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
    builder.cwd = session_meta.meta.cwd.clone();
    builder.sandbox_policy = SandboxPolicy::ReadOnly;
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
    let sessions_root = config.codex_home.join(rollout::SESSIONS_SUBDIR);
    let archived_root = config.codex_home.join(rollout::ARCHIVED_SESSIONS_SUBDIR);
    let mut rollout_paths: Vec<(PathBuf, bool)> = Vec::new();
    for (root, archived) in [(sessions_root, false), (archived_root, true)] {
        if !tokio::fs::try_exists(&root).await.unwrap_or(false) {
            continue;
        }
        match collect_rollout_paths(&root).await {
            Ok(paths) => {
                rollout_paths.extend(paths.into_iter().map(|path| (path, archived)));
            }
            Err(err) => {
                warn!(
                    "failed to collect rollout paths under {}: {err}",
                    root.display()
                );
            }
        }
    }
    rollout_paths.sort_by_key(|(path, _archived)| {
        let parsed = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(parse_timestamp_uuid_from_filename)
            .unwrap_or((time::OffsetDateTime::UNIX_EPOCH, uuid::Uuid::nil()));
        (Reverse(parsed.0), Reverse(parsed.1))
    });
    let mut stats = BackfillStats {
        scanned: 0,
        upserted: 0,
        failed: 0,
    };
    for (path, archived) in rollout_paths {
        stats.scanned = stats.scanned.saturating_add(1);
        match extract_metadata_from_rollout(&path, config.model_provider_id.as_str(), otel).await {
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
                if archived && metadata.archived_at.is_none() {
                    let fallback_archived_at = metadata.updated_at;
                    metadata.archived_at = file_modified_time_utc(&path)
                        .await
                        .or(Some(fallback_archived_at));
                }
                if let Err(err) = runtime.upsert_thread(&metadata).await {
                    stats.failed = stats.failed.saturating_add(1);
                    warn!("failed to upsert rollout {}: {err}", path.display());
                } else {
                    stats.upserted = stats.upserted.saturating_add(1);
                    if let Ok(meta_line) = rollout::list::read_session_meta_line(&path).await {
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
                            warn!("failed to backfill dynamic tools {}: {err}", path.display());
                        }
                    } else {
                        warn!(
                            "failed to read session meta for dynamic tools {}",
                            path.display()
                        );
                    }
                }
            }
            Err(err) => {
                stats.failed = stats.failed.saturating_add(1);
                warn!("failed to extract rollout {}: {err}", path.display());
            }
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
        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            let file_type = entry.file_type().await?;
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
    use codex_state::ThreadMetadataBuilder;
    use pretty_assertions::assert_eq;
    use std::fs::File;
    use std::io::Write;
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
}

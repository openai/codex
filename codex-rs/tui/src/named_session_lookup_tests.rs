use std::path::PathBuf;
use std::sync::Arc;

use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_state::ThreadMetadataBuilder;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::lookup;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::ThreadParamsMode;
use crate::legacy_core::config::Config;
use crate::legacy_core::config::ConfigBuilder;
use crate::tests::start_test_embedded_app_server;

async fn build_config(temp_dir: &TempDir) -> std::io::Result<Config> {
    ConfigBuilder::default()
        .codex_home(temp_dir.path().to_path_buf())
        .build()
        .await
}

async fn state_runtime(config: &Config) -> std::io::Result<Arc<codex_state::StateRuntime>> {
    let runtime = codex_state::StateRuntime::init(
        codex_state::SqliteConfig::new_for_testing(config.codex_home.as_path().abs()),
        config.model_provider_id.clone(),
    )
    .await
    .map_err(std::io::Error::other)?;
    runtime
        .mark_backfill_complete(/*last_watermark*/ None)
        .await
        .map_err(std::io::Error::other)?;
    Ok(runtime)
}

async fn lookup_name(
    config: &Config,
    name: &str,
) -> color_eyre::Result<Option<crate::resume_picker::SessionTarget>> {
    let mut app_server = AppServerSession::new(
        codex_app_server_client::AppServerClient::InProcess(
            start_test_embedded_app_server(config.clone()).await?,
        ),
        ThreadParamsMode::Embedded,
    );
    let target = lookup(&mut app_server, config, name).await?;
    app_server.shutdown().await?;
    Ok(target)
}

async fn upsert_thread(
    runtime: &codex_state::StateRuntime,
    metadata: codex_state::ThreadMetadata,
) -> std::io::Result<()> {
    runtime
        .upsert_thread(&metadata)
        .await
        .map_err(std::io::Error::other)
}

fn thread_metadata(
    config: &Config,
    thread_id: ThreadId,
    rollout_path: PathBuf,
    title: &str,
) -> codex_state::ThreadMetadata {
    let created_at = chrono::DateTime::parse_from_rfc3339("2025-02-01T10:00:00Z")
        .expect("timestamp should parse")
        .with_timezone(&chrono::Utc);
    let mut builder = ThreadMetadataBuilder::new(
        thread_id,
        rollout_path,
        created_at,
        serde_json::from_value(serde_json::json!("cli"))
            .expect("cli session source should deserialize"),
    );
    builder.cwd = config.codex_home.join("project").to_path_buf();
    let mut metadata = builder.build(config.model_provider_id.as_str());
    metadata.title = title.to_string();
    metadata.first_user_message = Some("preview text".to_string());
    metadata.preview = metadata.first_user_message.clone();
    metadata
}

fn write_rollout(
    config: &Config,
    thread_id: ThreadId,
    timestamp: &str,
    preview: &str,
    source: SessionSource,
    history_mode: ThreadHistoryMode,
) -> color_eyre::Result<PathBuf> {
    let rollout_path = config
        .codex_home
        .join("sessions/2025/02/01")
        .join(format!("rollout-2025-02-01T10-00-00-{thread_id}.jsonl"));
    std::fs::create_dir_all(rollout_path.parent().expect("rollout parent"))?;
    let session_meta = SessionMetaLine {
        meta: SessionMeta {
            session_id: thread_id.into(),
            id: thread_id,
            timestamp: timestamp.to_string(),
            cwd: config.codex_home.join("project").to_path_buf(),
            originator: "codex".to_string(),
            cli_version: "0.0.0".to_string(),
            source,
            model_provider: Some(config.model_provider_id.clone()),
            history_mode,
            ..Default::default()
        },
        git: None,
    };
    let lines = [
        serde_json::json!({
            "timestamp": timestamp,
            "type": "session_meta",
            "payload": session_meta,
        }),
        serde_json::json!({
            "timestamp": timestamp,
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": preview}],
            },
        }),
    ];
    std::fs::write(
        &rollout_path,
        lines
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )?;
    Ok(rollout_path.to_path_buf())
}

async fn add_scan_sentinel(config: &Config, search_term: &str) -> color_eyre::Result<ThreadId> {
    let thread_id = ThreadId::new();
    write_rollout(
        config,
        thread_id,
        "2025-02-01T11:00:00Z",
        "scan sentinel",
        SessionSource::Cli,
        ThreadHistoryMode::Legacy,
    )?;
    codex_rollout::append_thread_name(
        config.codex_home.as_path(),
        thread_id,
        &format!("{search_term}-scan-sentinel"),
    )
    .await?;
    Ok(thread_id)
}

#[tokio::test]
async fn uses_sqlite_title_without_scanning_rollouts() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let config = build_config(&temp_dir).await?;
    let runtime = state_runtime(&config).await?;
    let thread_id = ThreadId::new();
    let rollout_path = write_rollout(
        &config,
        thread_id,
        "2025-02-01T10:00:00Z",
        "preview",
        SessionSource::Cli,
        ThreadHistoryMode::Legacy,
    )?;
    upsert_thread(
        &runtime,
        thread_metadata(&config, thread_id, rollout_path.clone(), "saved-session"),
    )
    .await?;
    // Scan-and-repair would discover this matching rollout and insert it into SQLite.
    let scan_sentinel_id = add_scan_sentinel(&config, "saved-session").await?;
    let duplicate_id = ThreadId::new();
    write_rollout(
        &config,
        duplicate_id,
        "2025-02-01T11:00:00Z",
        "newer duplicate",
        SessionSource::Cli,
        ThreadHistoryMode::Legacy,
    )?;
    codex_rollout::append_thread_name(config.codex_home.as_path(), duplicate_id, "saved-session")
        .await?;

    let target = lookup_name(&config, "saved-session").await?;

    assert_eq!(
        (
            target.map(|target| (target.path, target.thread_id)),
            runtime
                .get_thread(scan_sentinel_id)
                .await
                .map_err(std::io::Error::other)?,
            runtime
                .get_thread(duplicate_id)
                .await
                .map_err(std::io::Error::other)?,
        ),
        (Some((Some(rollout_path), thread_id)), None, None),
    );
    Ok(())
}

#[tokio::test]
async fn skips_stale_sqlite_candidate_for_valid_duplicate() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let config = build_config(&temp_dir).await?;
    let runtime = state_runtime(&config).await?;
    let valid_id = ThreadId::new();
    let valid_path = write_rollout(
        &config,
        valid_id,
        "2025-02-01T10:00:00Z",
        "valid preview",
        SessionSource::Cli,
        ThreadHistoryMode::Legacy,
    )?;
    upsert_thread(
        &runtime,
        thread_metadata(&config, valid_id, valid_path.clone(), "saved-session"),
    )
    .await?;

    let stale_id = ThreadId::new();
    let stale_path = write_rollout(
        &config,
        ThreadId::new(),
        "2025-02-01T11:00:00Z",
        "stale preview",
        SessionSource::Cli,
        ThreadHistoryMode::Legacy,
    )?;
    upsert_thread(
        &runtime,
        thread_metadata(&config, stale_id, stale_path, "saved-session"),
    )
    .await?;

    let target = lookup_name(&config, "saved-session").await?;

    assert_eq!(
        target.map(|target| (target.path, target.thread_id)),
        Some((Some(valid_path), valid_id)),
    );
    Ok(())
}

#[tokio::test]
async fn validates_sqlite_candidate_filters_against_rollout() -> color_eyre::Result<()> {
    let mut targets = Vec::new();
    for (source, model_provider) in [
        (SessionSource::Exec, None),
        (SessionSource::Cli, Some("other-provider")),
    ] {
        let temp_dir = TempDir::new()?;
        let config = build_config(&temp_dir).await?;
        let runtime = state_runtime(&config).await?;
        let thread_id = ThreadId::new();
        let mut rollout_config = config.clone();
        if let Some(model_provider) = model_provider {
            rollout_config.model_provider_id = model_provider.to_string();
        }
        let rollout_path = write_rollout(
            &rollout_config,
            thread_id,
            "2025-02-01T10:00:00Z",
            "preview",
            source,
            ThreadHistoryMode::Legacy,
        )?;
        upsert_thread(
            &runtime,
            thread_metadata(&config, thread_id, rollout_path, "saved-session"),
        )
        .await?;

        targets.push(
            lookup_name(&config, "saved-session")
                .await?
                .map(|target| target.thread_id),
        );
    }

    assert_eq!(targets, vec![None, None]);
    Ok(())
}

#[tokio::test]
async fn recovers_legacy_index_name_without_requiring_sidecar_rewrite() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let config = build_config(&temp_dir).await?;
    let runtime = state_runtime(&config).await?;
    let stale_id = ThreadId::new();
    upsert_thread(
        &runtime,
        thread_metadata(
            &config,
            stale_id,
            config
                .codex_home
                .join("missing-rollout.jsonl")
                .to_path_buf(),
            "saved-session",
        ),
    )
    .await?;

    let thread_id = ThreadId::new();
    let rollout_path = write_rollout(
        &config,
        thread_id,
        "2025-02-01T11:00:00Z",
        "preview",
        SessionSource::Cli,
        ThreadHistoryMode::Legacy,
    )?;
    codex_rollout::append_thread_name(config.codex_home.as_path(), thread_id, "saved-session")
        .await?;
    // The direct index lookup should not scan and repair other matching rollouts.
    let scan_sentinel_id = add_scan_sentinel(&config, "saved-session").await?;

    let index_path = config.codex_home.join("session_index.jsonl");
    let original_permissions = std::fs::metadata(&index_path)?.permissions();
    let mut read_only_permissions = original_permissions.clone();
    read_only_permissions.set_readonly(true);
    std::fs::set_permissions(&index_path, read_only_permissions)?;
    let target = lookup_name(&config, "saved-session").await;
    std::fs::set_permissions(&index_path, original_permissions)?;
    let target = target?;

    assert_eq!(
        (
            target.map(|target| (target.path, target.thread_id)),
            runtime
                .get_thread(thread_id)
                .await
                .map_err(std::io::Error::other)?
                .map(|metadata| metadata.title),
            runtime
                .get_thread(scan_sentinel_id)
                .await
                .map_err(std::io::Error::other)?,
        ),
        (
            Some((Some(rollout_path), thread_id)),
            Some("saved-session".to_string()),
            None,
        ),
    );
    Ok(())
}

#[tokio::test]
async fn skips_stale_legacy_index_candidate() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let config = build_config(&temp_dir).await?;
    let runtime = state_runtime(&config).await?;
    let stale_id = ThreadId::new();
    let stale_path = write_rollout(
        &config,
        stale_id,
        "2025-02-01T10:00:00Z",
        "preview",
        SessionSource::Cli,
        ThreadHistoryMode::Legacy,
    )?;
    upsert_thread(
        &runtime,
        thread_metadata(&config, stale_id, stale_path.clone(), "current-session"),
    )
    .await?;
    codex_rollout::append_thread_name(config.codex_home.as_path(), stale_id, "saved-session")
        .await?;

    let fallback_id = ThreadId::new();
    let fallback_path = write_rollout(
        &config,
        fallback_id,
        "2025-02-01T11:00:00Z",
        "fallback",
        SessionSource::Cli,
        ThreadHistoryMode::Legacy,
    )?;
    codex_rollout::append_thread_name(config.codex_home.as_path(), fallback_id, "saved-session")
        .await?;
    for (path, timestamp) in [
        (&stale_path, "2025-02-04T10:00:00Z"),
        (&fallback_path, "2025-02-03T10:00:00Z"),
    ] {
        let modified = chrono::DateTime::parse_from_rfc3339(timestamp)?.into();
        std::fs::OpenOptions::new()
            .append(true)
            .open(path)?
            .set_times(std::fs::FileTimes::new().set_modified(modified))?;
    }

    let target = lookup_name(&config, "saved-session").await?;

    assert_eq!(
        (
            target.map(|target| (target.path, target.thread_id)),
            runtime
                .get_thread(stale_id)
                .await
                .map_err(std::io::Error::other)?
                .map(|metadata| metadata.title),
        ),
        (
            Some((Some(fallback_path), fallback_id)),
            Some("current-session".to_string()),
        ),
    );
    Ok(())
}

#[tokio::test]
async fn keeps_newer_paginated_sqlite_name_when_index_is_stale() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let config = build_config(&temp_dir).await?;
    let runtime = state_runtime(&config).await?;
    let thread_id = ThreadId::new();
    let rollout_path = write_rollout(
        &config,
        thread_id,
        "2025-02-01T10:00:00Z",
        "preview",
        SessionSource::Cli,
        ThreadHistoryMode::Paginated,
    )?;
    let mut metadata = thread_metadata(&config, thread_id, rollout_path, "preview text");
    metadata.history_mode = ThreadHistoryMode::Paginated;
    upsert_thread(&runtime, metadata).await?;
    runtime
        .update_thread_name(thread_id, Some("current-session"))
        .await
        .map_err(std::io::Error::other)?;
    codex_rollout::append_thread_name(config.codex_home.as_path(), thread_id, "old-session")
        .await?;

    let target = lookup_name(&config, "old-session").await?;

    assert_eq!(
        (
            target.map(|target| target.thread_id),
            runtime
                .get_thread(thread_id)
                .await
                .map_err(std::io::Error::other)?
                .and_then(|metadata| metadata.name),
        ),
        (None, Some("current-session".to_string())),
    );
    Ok(())
}

#[tokio::test]
async fn prefers_most_recent_duplicate_legacy_name() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let config = build_config(&temp_dir).await?;
    let _runtime = state_runtime(&config).await?;
    let recent_id = ThreadId::new();
    let recent_path = write_rollout(
        &config,
        recent_id,
        "2025-02-01T10:00:00Z",
        "recent",
        SessionSource::Cli,
        ThreadHistoryMode::Legacy,
    )?;
    let older_id = ThreadId::new();
    let older_path = write_rollout(
        &config,
        older_id,
        "2025-02-02T10:00:00Z",
        "older",
        SessionSource::Cli,
        ThreadHistoryMode::Legacy,
    )?;
    codex_rollout::append_thread_name(config.codex_home.as_path(), recent_id, "same-name").await?;
    codex_rollout::append_thread_name(config.codex_home.as_path(), older_id, "same-name").await?;
    for (path, timestamp) in [
        (&recent_path, "2025-02-04T10:00:00Z"),
        (&older_path, "2025-02-03T10:00:00Z"),
    ] {
        let modified = chrono::DateTime::parse_from_rfc3339(timestamp)?.into();
        std::fs::OpenOptions::new()
            .append(true)
            .open(path)?
            .set_times(std::fs::FileTimes::new().set_modified(modified))?;
    }

    let target = lookup_name(&config, "same-name").await?;

    assert_eq!(
        target.map(|target| (target.path, target.thread_id)),
        Some((Some(recent_path), recent_id)),
    );
    Ok(())
}

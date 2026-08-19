use std::path::PathBuf;
use std::sync::Arc;

use chrono::DateTime;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_state::ThreadMetadataBuilder;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::DeleteConfirmation;
use super::SessionArchiveAction;
use super::run_session_archive_action_with_app_server;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::ThreadParamsMode;
use crate::legacy_core::config::Config;
use crate::legacy_core::config::ConfigBuilder;
use crate::session_queue_commands::run_session_queue_action_with_app_server;
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

async fn start_app_server(config: Config) -> color_eyre::Result<AppServerSession> {
    Ok(AppServerSession::new(
        codex_app_server_client::AppServerClient::InProcess(
            start_test_embedded_app_server(config).await?,
        ),
        ThreadParamsMode::Embedded,
    ))
}

fn write_rollout(
    config: &Config,
    thread_id: ThreadId,
    archived: bool,
    timestamp: &str,
    preview: &str,
    source: SessionSource,
) -> color_eyre::Result<PathBuf> {
    let subdir = if archived {
        "archived_sessions"
    } else {
        "sessions/2025/02/01"
    };
    let rollout_path = config
        .codex_home
        .join(subdir)
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
        serde_json::json!({
            "timestamp": timestamp,
            "type": "event_msg",
            "payload": {
                "type": "user_message",
                "message": preview,
                "kind": "plain",
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

fn thread_metadata(
    config: &Config,
    thread_id: ThreadId,
    rollout_path: PathBuf,
    name: &str,
    archived: bool,
) -> codex_state::ThreadMetadata {
    let created_at = DateTime::parse_from_rfc3339("2025-02-01T10:00:00Z")
        .expect("timestamp should parse")
        .with_timezone(&Utc);
    let mut builder = ThreadMetadataBuilder::new(
        thread_id,
        rollout_path,
        created_at,
        serde_json::from_value(serde_json::json!("cli"))
            .expect("cli session source should deserialize"),
    );
    builder.cwd = config.codex_home.join("project").to_path_buf();
    let mut metadata = builder.build(config.model_provider_id.as_str());
    metadata.title = name.to_string();
    metadata.first_user_message = Some("preview text".to_string());
    metadata.preview = metadata.first_user_message.clone();
    metadata.archived_at = archived.then_some(created_at);
    metadata
}

#[tokio::test]
async fn archives_by_sqlite_name() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let config = build_config(&temp_dir).await?;
    let runtime = state_runtime(&config).await?;
    let thread_id = ThreadId::new();
    let rollout_path = write_rollout(
        &config,
        thread_id,
        /*archived*/ false,
        "2025-02-01T10:00:00Z",
        "preview",
        SessionSource::Cli,
    )?;
    runtime
        .upsert_thread(&thread_metadata(
            &config,
            thread_id,
            rollout_path,
            "saved-session",
            /*archived*/ false,
        ))
        .await
        .map_err(std::io::Error::other)?;

    let mut app_server = start_app_server(config.clone()).await?;
    let message = run_session_archive_action_with_app_server(
        &mut app_server,
        config.codex_home.as_path(),
        SessionArchiveAction::Archive,
        "saved-session",
    )
    .await?;
    app_server.shutdown().await?;

    assert_eq!(
        (
            message,
            runtime
                .get_thread(thread_id)
                .await
                .map_err(std::io::Error::other)?
                .is_some_and(|metadata| metadata.archived_at.is_some()),
        ),
        (
            format!("Archived session saved-session ({thread_id})."),
            true,
        ),
    );
    Ok(())
}

#[tokio::test]
async fn unarchives_by_sqlite_name() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let config = build_config(&temp_dir).await?;
    let runtime = state_runtime(&config).await?;
    let thread_id = ThreadId::new();
    let rollout_path = write_rollout(
        &config,
        thread_id,
        /*archived*/ true,
        "2025-02-01T10:00:00Z",
        "preview",
        SessionSource::Cli,
    )?;
    runtime
        .upsert_thread(&thread_metadata(
            &config,
            thread_id,
            rollout_path,
            "saved-session",
            /*archived*/ true,
        ))
        .await
        .map_err(std::io::Error::other)?;

    let mut app_server = start_app_server(config.clone()).await?;
    let message = run_session_archive_action_with_app_server(
        &mut app_server,
        config.codex_home.as_path(),
        SessionArchiveAction::Unarchive,
        "saved-session",
    )
    .await?;
    app_server.shutdown().await?;

    assert_eq!(
        (
            message,
            runtime
                .get_thread(thread_id)
                .await
                .map_err(std::io::Error::other)?
                .is_some_and(|metadata| metadata.archived_at.is_some()),
        ),
        (
            format!("Unarchived session saved-session ({thread_id})."),
            false,
        ),
    );
    Ok(())
}

#[tokio::test]
async fn deletes_valid_duplicate_after_stale_sqlite_hit() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let config = build_config(&temp_dir).await?;
    let runtime = state_runtime(&config).await?;
    let stale_id = ThreadId::new();
    let stale_rollout_path = write_rollout(
        &config,
        stale_id,
        /*archived*/ true,
        "2025-02-01T10:00:00Z",
        "stale preview",
        SessionSource::Cli,
    )?;
    runtime
        .upsert_thread(&thread_metadata(
            &config,
            stale_id,
            stale_rollout_path.clone(),
            "saved-session",
            /*archived*/ false,
        ))
        .await
        .map_err(std::io::Error::other)?;

    let thread_id = ThreadId::new();
    let rollout_path = write_rollout(
        &config,
        thread_id,
        /*archived*/ false,
        "2025-02-01T10:00:00Z",
        "preview",
        SessionSource::Cli,
    )?;
    codex_rollout::append_thread_name(config.codex_home.as_path(), thread_id, "saved-session")
        .await?;

    let mut app_server = start_app_server(config.clone()).await?;
    let message = run_session_archive_action_with_app_server(
        &mut app_server,
        config.codex_home.as_path(),
        SessionArchiveAction::Delete(DeleteConfirmation::Skip),
        "saved-session",
    )
    .await?;
    app_server.shutdown().await?;

    assert_eq!(
        (
            message,
            rollout_path.exists(),
            stale_rollout_path.exists(),
            runtime
                .get_thread(thread_id)
                .await
                .map_err(std::io::Error::other)?,
            runtime
                .get_thread(stale_id)
                .await
                .map_err(std::io::Error::other)?
                .is_some(),
        ),
        (
            format!("Deleted session saved-session ({thread_id})."),
            false,
            true,
            None,
            true,
        ),
    );
    Ok(())
}

#[tokio::test]
async fn trusts_sqlite_name_over_legacy_index_for_delete() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let config = build_config(&temp_dir).await?;
    let runtime = state_runtime(&config).await?;
    let thread_id = ThreadId::new();
    let rollout_path = write_rollout(
        &config,
        thread_id,
        /*archived*/ false,
        "2025-02-01T10:00:00Z",
        "preview",
        SessionSource::Cli,
    )?;
    runtime
        .upsert_thread(&thread_metadata(
            &config,
            thread_id,
            rollout_path.clone(),
            "new-session",
            /*archived*/ false,
        ))
        .await
        .map_err(std::io::Error::other)?;
    codex_rollout::append_thread_name(config.codex_home.as_path(), thread_id, "old-session")
        .await?;

    let mut app_server = start_app_server(config.clone()).await?;
    let error = run_session_queue_action_with_app_server(
        &mut app_server,
        config.codex_home.as_path(),
        "old-session",
        "do the thing",
        "stable-client-message-id",
    )
    .await
    .expect_err("stale legacy names must not select renamed threads");
    assert_eq!(
        error.to_string(),
        "No active session found matching 'old-session'."
    );
    let message = run_session_archive_action_with_app_server(
        &mut app_server,
        config.codex_home.as_path(),
        SessionArchiveAction::Delete(DeleteConfirmation::Skip),
        "new-session",
    )
    .await?;
    app_server.shutdown().await?;

    assert_eq!(
        (
            message,
            rollout_path.exists(),
            runtime
                .get_thread(thread_id)
                .await
                .map_err(std::io::Error::other)?,
        ),
        (
            format!("Deleted session new-session ({thread_id})."),
            false,
            None,
        ),
    );
    Ok(())
}

#[tokio::test]
async fn queues_non_interactive_and_custom_sessions_without_scanning_rollouts()
-> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let config = build_config(&temp_dir).await?;
    let runtime = state_runtime(&config).await?;
    let thread_id = ThreadId::new();
    let rollout_path = write_rollout(
        &config,
        thread_id,
        /*archived*/ false,
        "2025-02-01T10:00:00Z",
        "preview",
        SessionSource::Exec,
    )?;
    let mut metadata = thread_metadata(
        &config,
        thread_id,
        rollout_path,
        "saved-session",
        /*archived*/ false,
    );
    metadata.source = "exec".to_string();
    runtime
        .upsert_thread(&metadata)
        .await
        .map_err(std::io::Error::other)?;

    let mut app_server = start_app_server(config.clone()).await?;
    let (resolved_thread_id, response) = run_session_queue_action_with_app_server(
        &mut app_server,
        config.codex_home.as_path(),
        "saved-session",
        "do the thing",
        "stable-client-message-id",
    )
    .await?;
    let queued = runtime
        .thread_queue()
        .list_page(thread_id, /*offset*/ 0, /*limit*/ 10)
        .await
        .map_err(|error| color_eyre::eyre::eyre!("{error}"))?;
    assert_eq!(resolved_thread_id, thread_id);
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].id, response.queued_submission.id);
    assert_eq!(
        response.queued_submission.client_user_message_id,
        "stable-client-message-id"
    );

    let custom_thread_id = ThreadId::new();
    let custom_source = SessionSource::Custom("atlas".to_string());
    let custom_rollout_path = write_rollout(
        &config,
        custom_thread_id,
        /*archived*/ false,
        "2025-02-01T10:00:00Z",
        "preview",
        custom_source.clone(),
    )?;
    let mut custom_metadata = thread_metadata(
        &config,
        custom_thread_id,
        custom_rollout_path,
        "atlas-session",
        /*archived*/ false,
    );
    custom_metadata.source = serde_json::to_string(&custom_source)?;
    custom_metadata.recency_at += chrono::Duration::hours(/*hours*/ 1);
    custom_metadata.updated_at += chrono::Duration::hours(/*hours*/ 1);
    runtime
        .upsert_thread(&custom_metadata)
        .await
        .map_err(std::io::Error::other)?;
    let legacy_thread_id = ThreadId::new();
    write_rollout(
        &config,
        legacy_thread_id,
        /*archived*/ false,
        "2025-02-01T10:00:00Z",
        "preview",
        SessionSource::Cli,
    )?;
    codex_rollout::append_thread_name(
        config.codex_home.as_path(),
        legacy_thread_id,
        "saved-session",
    )
    .await?;
    let (resolved_custom_thread_id, _) = run_session_queue_action_with_app_server(
        &mut app_server,
        config.codex_home.as_path(),
        "atlas-session",
        "do the thing",
        "custom-client-message-id",
    )
    .await?;
    assert_eq!(
        (
            resolved_custom_thread_id,
            runtime
                .get_thread(legacy_thread_id)
                .await
                .map_err(std::io::Error::other)?,
        ),
        (custom_thread_id, None),
    );
    runtime
        .update_thread_title(custom_thread_id, "saved-session")
        .await
        .map_err(std::io::Error::other)?;

    let (resolved_duplicate_id, _) = run_session_queue_action_with_app_server(
        &mut app_server,
        config.codex_home.as_path(),
        "saved-session",
        "do the thing",
        "duplicate-client-message-id",
    )
    .await?;
    app_server.shutdown().await?;

    assert_eq!(
        (
            resolved_duplicate_id,
            runtime
                .get_thread(legacy_thread_id)
                .await
                .map_err(std::io::Error::other)?,
        ),
        (custom_thread_id, None),
    );
    Ok(())
}

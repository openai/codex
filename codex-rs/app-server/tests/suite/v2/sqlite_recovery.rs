//! Verifies database failures preserve saved threads and report actionable diagnostics.

use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::ThreadStartParams;
use codex_state::SqliteConfig;
use codex_state::StateRuntime;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

#[tokio::test]
async fn startup_reports_shared_and_fallback_recovery() -> Result<()> {
    for log_dir_is_configured in [false, true] {
        let home = TempDir::new()?;
        if log_dir_is_configured {
            std::fs::write(
                home.path().join("config.toml"),
                format!("log_dir = {:?}\n", home.path().join("log")),
            )?;
        }
        let sqlite = core_test_support::load_default_config_for_test(&home)
            .await
            .sqlite;
        let runtime = StateRuntime::init(sqlite.clone(), "openai".to_string()).await?;
        runtime.close().await;
        let logs_path = sqlite.logs_db_path();
        let pool = sqlite.open_read_write_pool(&logs_path).await?;
        // The schema and migrations remain readable; opening the database alone succeeds.
        sqlx::raw_sql(
            "CREATE TABLE sample(value INTEGER);
             INSERT INTO sample VALUES (NULL);
             PRAGMA writable_schema=ON;
             UPDATE sqlite_schema SET sql='CREATE TABLE sample(value INTEGER NOT NULL)' WHERE name='sample';
             PRAGMA writable_schema=OFF;",
        )
        .execute(&pool)
        .await?;
        pool.close().await;

        // Logs recover inside the pool opener; the unreadable goals header forces
        // the outer startup fallback and a retry after that first recovery.
        tokio::fs::write(sqlite.goals_db_path(), b"damaged goals database").await?;

        let read_timeout = Duration::from_secs(/*secs*/ 30);
        let mut server = TestAppServer::builder()
            .with_codex_home(sqlite.home())
            .build_initialized_with_timeout(read_timeout)
            .await?;
        let warning: ConfigWarningNotification =
            timeout(read_timeout, server.read_notification("configWarning")).await??;
        assert_eq!(warning.summary, "Codex rebuilt its local database");
        let details = warning.details.expect("recovery details");
        assert!(details.contains("Some database-only metadata may be unavailable"));
        let mut backups = std::fs::read_dir(sqlite.home().join("db-backups"))?
            .collect::<std::io::Result<Vec<_>>>()?;
        let logs_home = logs_path.parent().expect("logs directory");
        if logs_home != sqlite.home() {
            backups.extend(
                std::fs::read_dir(logs_home.join("db-backups"))?
                    .collect::<std::io::Result<Vec<_>>>()?,
            );
        }
        assert_eq!(backups.len(), 2);
        for backup in &backups {
            assert!(details.contains(&backup.path().display().to_string()));
        }
        assert!(details.contains(&sqlite.logs_db_path().display().to_string()));
        assert!(details.contains(&sqlite.goals_db_path().display().to_string()));
        let backup_dir = backups
            .iter()
            .map(std::fs::DirEntry::path)
            .find(|path| {
                path.join(logs_path.file_name().expect("log filename"))
                    .exists()
            })
            .expect("preserved logs database");
        let backup_pool = sqlite
            .open_read_only_pool(
                &backup_dir.join(logs_path.file_name().expect("log filename")),
                /*busy_timeout*/ None,
            )
            .await?;
        assert_eq!(
            sqlx::query_scalar::<_, Option<i64>>("SELECT value FROM sample")
                .fetch_all(&backup_pool)
                .await?,
            vec![None]
        );
        backup_pool.close().await;
        server.start_thread(ThreadStartParams::default()).await?;
        timeout(read_timeout, server.shutdown_gracefully()).await??;

        // An unreadable logs header must also reach the outer recovery path.
        tokio::fs::write(&logs_path, b"damaged logs database").await?;
        let mut server = TestAppServer::builder()
            .with_codex_home(sqlite.home())
            .build_initialized_with_timeout(read_timeout)
            .await?;
        let warning: ConfigWarningNotification =
            timeout(read_timeout, server.read_notification("configWarning")).await??;
        assert_eq!(warning.summary, "Codex rebuilt its local database");
        assert!(
            warning
                .details
                .expect("recovery details")
                .contains(&logs_path.display().to_string())
        );
        server.start_thread(ThreadStartParams::default()).await?;
        timeout(read_timeout, server.shutdown_gracefully()).await??;
    }
    Ok(())
}

#[tokio::test]
async fn state_recovery_restores_saved_threads() -> Result<()> {
    use codex_app_server_protocol::ThreadListParams;
    use codex_app_server_protocol::ThreadListResponse;
    use codex_app_server_protocol::ThreadReadParams;
    use codex_app_server_protocol::ThreadReadResponse;
    use codex_app_server_protocol::TurnStartParams;
    use codex_app_server_protocol::TurnStatus;
    use codex_app_server_protocol::UserInput;

    let home = TempDir::new()?;
    let mock =
        app_test_support::create_mock_responses_server_repeating_assistant("Saved reply").await;
    app_test_support::MockResponsesConfig::new(&mock.uri()).write(home.path())?;
    let sqlite = SqliteConfig::new_for_testing(home.path().canonicalize()?.as_path().abs());
    let read_timeout = Duration::from_secs(/*secs*/ 30);
    let mut server = TestAppServer::builder()
        .with_codex_home(sqlite.home())
        .build_initialized_with_timeout(read_timeout)
        .await?;
    let thread_id = server
        .start_thread(ThreadStartParams::default())
        .await?
        .thread
        .id;
    let completed = server
        .start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![UserInput::Text {
                text: "Keep this conversation".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    let request = server
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread_id.clone(),
            include_turns: true,
        })
        .await?;
    let before: ThreadReadResponse = timeout(read_timeout, server.read_response(request)).await??;
    timeout(read_timeout, server.shutdown_gracefully()).await??;

    // A logs outage must be visible after startup installs tracing, and must
    // reject deletion before touching the saved conversation.
    let config_path = home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    let blocked_logs = home.path().join("blocked-logs");
    std::fs::write(&blocked_logs, b"not a directory")?;
    std::fs::write(
        &config_path,
        format!("log_dir = {blocked_logs:?}\n{config}"),
    )?;
    let mut server = TestAppServer::builder()
        .with_codex_home(sqlite.home())
        .with_json_logging("warn")
        .build_initialized_with_timeout(read_timeout)
        .await?;
    let warning = server
        .wait_for_json_log_event("diagnostic_database_unavailable", read_timeout)
        .await?;
    let cause = warning["fields"]["error"]
        .as_str()
        .expect("original startup error");
    assert!(cause.contains(&blocked_logs.display().to_string()));
    assert!(
        cause.contains(
            &std::fs::create_dir_all(&blocked_logs)
                .unwrap_err()
                .to_string()
        )
    );
    let rollout_path = before.thread.path.as_ref().expect("saved rollout");
    let rollout = std::fs::read(rollout_path)?;
    let request = server
        .send_thread_delete_request(codex_app_server_protocol::ThreadDeleteParams {
            thread_id: thread_id.clone(),
        })
        .await?;
    let error = timeout(
        read_timeout,
        server.read_stream_until_error_message(codex_app_server_protocol::RequestId::Integer(
            request,
        )),
    )
    .await??;
    assert_eq!(error.error.code, -32603);
    assert!(error.error.message.contains(cause));
    assert_eq!(std::fs::read(rollout_path)?, rollout);
    let request = server
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread_id.clone(),
            include_turns: true,
        })
        .await?;
    let retained: ThreadReadResponse =
        timeout(read_timeout, server.read_response(request)).await??;
    assert_eq!(retained.thread.turns, before.thread.turns);
    timeout(read_timeout, server.shutdown_gracefully()).await??;
    std::fs::write(config_path, config)?;

    // Introduce corruption only after the owning process has closed its pools.
    let pool = sqlite.open_read_write_pool(&sqlite.state_db_path()).await?;
    sqlx::raw_sql(
        "CREATE TABLE sample(value INTEGER);
         INSERT INTO sample VALUES (NULL);
         PRAGMA writable_schema=ON;
         UPDATE sqlite_schema SET sql='CREATE TABLE sample(value INTEGER NOT NULL)' WHERE name='sample';
         PRAGMA writable_schema=OFF;",
    ).execute(&pool).await?;
    pool.close().await;

    let mut server = TestAppServer::builder()
        .with_codex_home(sqlite.home())
        .build_initialized_with_timeout(read_timeout)
        .await?;
    let request = server
        .send_thread_list_request(ThreadListParams {
            current_account_only: false,
            excluded_thread_ids: None,
            attached_to_thread_id: None,
            originators: None,
            cursor: None,
            limit: None,
            sort_key: None,
            sort_direction: None,
            model_providers: None,
            source_kinds: None,
            archived: None,
            section_id: None,
            project_id: None,
            cwd: None,
            use_state_db_only: true,
            search_term: None,
            parent_thread_id: None,
            ancestor_thread_id: None,
        })
        .await?;
    let listed: ThreadListResponse = timeout(read_timeout, server.read_response(request)).await??;
    assert_eq!(
        listed
            .data
            .iter()
            .map(|thread| thread.id.as_str())
            .collect::<Vec<_>>(),
        vec![thread_id.as_str()]
    );
    let request = server
        .send_thread_read_request(ThreadReadParams {
            thread_id,
            include_turns: true,
        })
        .await?;
    let after: ThreadReadResponse = timeout(read_timeout, server.read_response(request)).await??;
    assert_eq!(after.thread.turns, before.thread.turns);
    let warning: ConfigWarningNotification =
        timeout(read_timeout, server.read_notification("configWarning")).await??;
    assert_eq!(warning.summary, "Codex rebuilt its local database");
    let details = warning.details.expect("recovery details");
    assert!(details.contains("Some database-only metadata may be unavailable"));
    let backups = std::fs::read_dir(sqlite.home().join("db-backups"))?
        .collect::<std::io::Result<Vec<_>>>()?;
    assert_eq!(backups.len(), 1);
    assert!(details.contains(&backups[0].path().display().to_string()));
    assert!(
        backups[0]
            .path()
            .join(sqlite.state_db_path().file_name().expect("state filename"))
            .is_file()
    );
    timeout(read_timeout, server.shutdown_gracefully()).await??;
    Ok(())
}

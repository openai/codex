//! Exercises SQLite log collection, legacy storage, and feedback snapshot ordering.
use super::*;
use codex_core::config::ConfigBuilder;
use codex_state::SqliteConfig;
use codex_state::StateRuntime;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use tracing_subscriber::prelude::*;

#[derive(PartialEq)]
enum Scenario {
    Healthy,
    WriteFailure,
    Corruption,
}

#[tokio::test]
async fn feedback_includes_flush_and_query_failures_in_the_same_submission() -> anyhow::Result<()> {
    for scenario in [
        Scenario::Healthy,
        Scenario::WriteFailure,
        Scenario::Corruption,
    ] {
        let home = tempfile::tempdir()?;
        let config = core_test_support::load_default_config_for_test(&home).await;
        let sqlite = &config.sqlite;
        let runtime = StateRuntime::init(sqlite.clone(), "test-provider".to_string()).await?;
        let pool = sqlite.open_read_write_pool(&sqlite.logs_db_path()).await?;
        let feedback = CodexFeedback::new();
        let writer =
            codex_state::log_db::start(runtime.clone(), std::sync::Arc::new(feedback.clone()));
        let _guard = tracing_subscriber::registry()
            .with(feedback.logger_layer())
            .with(writer.clone())
            .set_default();
        let thread_id = ThreadId::new();
        tracing::info_span!("thread", thread_id = %thread_id)
            .in_scope(|| tracing::info!("persisted log"));
        writer.flush().await;
        match scenario {
            Scenario::WriteFailure => {
                sqlx::query("CREATE TRIGGER fail_log_insert BEFORE INSERT ON logs BEGIN SELECT RAISE(ABORT, 'synthetic-secret'); END").execute(&pool).await?;
            }
            Scenario::Corruption => {
                sqlx::raw_sql("PRAGMA writable_schema = ON; UPDATE sqlite_schema SET rootpage = 2147483647 WHERE name = 'logs'; PRAGMA schema_version = 1000000;").execute(&pool).await?;
            }
            Scenario::Healthy => {}
        }
        let snapshot = feedback.snapshot(Some(thread_id));
        tracing::info_span!("thread", thread_id = %thread_id)
            .in_scope(|| tracing::info!("latest log"));
        let (snapshot, sqlite_logs, legacy_logs) = collect_feedback_logs(
            &feedback,
            Some(&writer),
            Some(&runtime),
            &config,
            snapshot,
            &[thread_id],
        )
        .await;
        assert_eq!(legacy_logs, None);
        assert_eq!(sqlite_logs.is_some(), scenario == Scenario::Healthy);
        let logs = String::from_utf8(snapshot.log_attachment(sqlite_logs).buffer)?;
        assert!(logs.contains("persisted log"));
        assert!(logs.contains("latest log"));
        let expected_error = match scenario {
            Scenario::Healthy => None,
            Scenario::WriteFailure => Some("constraint"),
            Scenario::Corruption => Some("corrupt"),
        };
        assert_eq!(
            logs.matches("failed to flush logs to SQLite").count(),
            usize::from(expected_error.is_some())
        );
        if let Some(error) = expected_error {
            assert!(logs.contains(&format!("failed to flush logs to SQLite error={error:?}")));
        }
        assert_eq!(
            logs.contains("failed to query feedback logs from sqlite"),
            scenario == Scenario::Corruption
        );
        pool.close().await;
        runtime.close().await;
    }
    Ok(())
}

#[tokio::test]
async fn feedback_preserves_legacy_logs_when_the_directory_is_unset_or_current_logs_are_empty()
-> anyhow::Result<()> {
    for configured_log_dir in [false, true] {
        let home = tempfile::tempdir()?;
        let sqlite_home = home.path().join("sqlite");
        let mut overrides = vec![(
            "sqlite_home".to_string(),
            toml::Value::String(sqlite_home.to_string_lossy().into_owned()),
        )];
        if configured_log_dir {
            overrides.push((
                "log_dir".to_string(),
                toml::Value::String(home.path().join("log").to_string_lossy().into_owned()),
            ));
        }
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .loader_overrides(codex_config::LoaderOverrides::without_managed_config_for_tests())
            .cli_overrides(overrides)
            .build()
            .await?;
        let legacy_sqlite = SqliteConfig::new_for_testing(sqlite_home.abs());
        let legacy = StateRuntime::init(legacy_sqlite.clone(), "test-provider".to_string()).await?;
        let pool = legacy_sqlite
            .open_read_write_pool(&legacy_sqlite.logs_db_path())
            .await?;
        let thread_id = ThreadId::new();
        let now = chrono::Utc::now();
        let timestamp = now.format("%Y-%m-%dT%H:%M:%S.000000Z");
        for (id, body) in [
            (thread_id, "legacy history"),
            (ThreadId::new(), "unrelated history"),
        ] {
            sqlx::query(
                "INSERT INTO logs (ts, ts_nanos, level, target, feedback_log_body, thread_id)
                 VALUES (?, 0, 'INFO', 'test', ?, ?)",
            )
            .bind(now.timestamp())
            .bind(body)
            .bind(id.to_string())
            .execute(&pool)
            .await?;
        }
        pool.close().await;
        legacy.close().await;
        let legacy_before = std::fs::read(legacy_sqlite.logs_db_path())?;

        let runtime =
            StateRuntime::init(config.sqlite.clone(), "test-provider".to_string()).await?;
        let pool = config
            .sqlite
            .open_read_write_pool(&config.sqlite.logs_db_path())
            .await?;
        let feedback = CodexFeedback::new();
        for current_has_logs in [false, true] {
            if current_has_logs {
                sqlx::query(
                    "INSERT INTO logs (ts, ts_nanos, level, target, feedback_log_body, thread_id)
                     VALUES (?, 0, 'INFO', 'test', 'current history', ?)",
                )
                .bind(now.timestamp())
                .bind(thread_id.to_string())
                .execute(&pool)
                .await?;
            }
            let (_, current_logs, legacy_logs) = collect_feedback_logs(
                &feedback,
                /*log_db*/ None,
                Some(&runtime),
                &config,
                feedback.snapshot(Some(thread_id)),
                &[thread_id],
            )
            .await;
            let mut expected_current = String::new();
            if !configured_log_dir {
                expected_current.push_str(&format!("{timestamp}  INFO legacy history\n"));
            }
            if current_has_logs {
                expected_current.push_str(&format!("{timestamp}  INFO current history\n"));
            }
            assert_eq!(
                current_logs,
                (!expected_current.is_empty()).then(|| expected_current.into_bytes())
            );
            assert_eq!(
                legacy_logs,
                (configured_log_dir && !current_has_logs)
                    .then(|| format!("{timestamp}  INFO legacy history\n").into_bytes())
            );
        }
        if configured_log_dir {
            assert_eq!(std::fs::read(legacy_sqlite.logs_db_path())?, legacy_before);
        }
        pool.close().await;
        runtime.close().await;
    }
    Ok(())
}

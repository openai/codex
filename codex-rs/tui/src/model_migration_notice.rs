use codex_core::config::Config;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

/// Pending "show on next run" model migration notice.
///
/// This is persisted outside `config.toml` to avoid growing the config file with
/// ephemeral UI state.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PendingModelMigrationNotice {
    pub from_model: String,
    pub to_model: String,
}

const MODEL_MIGRATION_NOTICE_FILENAME: &str = "model_migration_notice.json";

fn pending_model_migration_notice_filepath(config: &Config) -> PathBuf {
    config.codex_home.join(MODEL_MIGRATION_NOTICE_FILENAME)
}

/// Read a pending notice if present.
///
/// Returns `Ok(None)` if no pending notice is scheduled.
pub async fn read_pending_model_migration_notice(
    config: &Config,
) -> anyhow::Result<Option<PendingModelMigrationNotice>> {
    let path = pending_model_migration_notice_filepath(config);
    let contents = match tokio::fs::read_to_string(&path).await {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    Ok(Some(serde_json::from_str(contents.trim())?))
}

/// Persist a pending notice to be displayed on the next run.
pub async fn write_pending_model_migration_notice(
    config: &Config,
    from_model: &str,
    to_model: &str,
) -> anyhow::Result<()> {
    let path = pending_model_migration_notice_filepath(config);
    let notice = PendingModelMigrationNotice {
        from_model: from_model.to_string(),
        to_model: to_model.to_string(),
    };
    let json_line = format!("{}\n", serde_json::to_string(&notice)?);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(path, json_line).await?;
    Ok(())
}

/// Clear any pending notice.
pub async fn clear_pending_model_migration_notice(config: &Config) -> anyhow::Result<()> {
    let path = pending_model_migration_notice_filepath(config);
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::config::ConfigBuilder;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn write_read_clear_round_trips() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let config = ConfigBuilder::default()
            .codex_home(tmp.path().to_path_buf())
            .build()
            .await
            .expect("config");

        assert_eq!(
            read_pending_model_migration_notice(&config).await.unwrap(),
            None
        );

        write_pending_model_migration_notice(&config, "gpt-5", "gpt-5.1")
            .await
            .expect("write");

        assert_eq!(
            read_pending_model_migration_notice(&config)
                .await
                .expect("read"),
            Some(PendingModelMigrationNotice {
                from_model: "gpt-5".to_string(),
                to_model: "gpt-5.1".to_string(),
            })
        );

        clear_pending_model_migration_notice(&config)
            .await
            .expect("clear");

        assert_eq!(
            read_pending_model_migration_notice(&config).await.unwrap(),
            None
        );
    }
}

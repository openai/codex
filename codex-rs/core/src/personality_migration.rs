use crate::config::edit::ConfigEditsBuilder;
use codex_config::config_toml::ConfigToml;
use codex_protocol::config_types::Personality;
use codex_thread_store::ListThreadsParams;
use codex_thread_store::LocalThreadStore;
use codex_thread_store::ThreadSortKey;
use codex_thread_store::ThreadStore;
use std::io;
use std::path::Path;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

pub const PERSONALITY_MIGRATION_FILENAME: &str = ".personality_migration";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonalityMigrationStatus {
    SkippedMarker,
    SkippedExplicitPersonality,
    SkippedNoSessions,
    Applied,
}

pub async fn maybe_migrate_personality(
    codex_home: &Path,
    config_toml: &ConfigToml,
) -> io::Result<PersonalityMigrationStatus> {
    let marker_path = codex_home.join(PERSONALITY_MIGRATION_FILENAME);
    if tokio::fs::try_exists(&marker_path).await? {
        return Ok(PersonalityMigrationStatus::SkippedMarker);
    }

    let config_profile = config_toml
        .get_config_profile(/*override_profile*/ None)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    if config_toml.personality.is_some() || config_profile.personality.is_some() {
        create_marker(&marker_path).await?;
        return Ok(PersonalityMigrationStatus::SkippedExplicitPersonality);
    }

    let model_provider_id = config_profile
        .model_provider
        .or_else(|| config_toml.model_provider.clone())
        .unwrap_or_else(|| "openai".to_string());

    if !has_recorded_sessions(codex_home, model_provider_id.as_str()).await? {
        create_marker(&marker_path).await?;
        return Ok(PersonalityMigrationStatus::SkippedNoSessions);
    }

    ConfigEditsBuilder::new(codex_home)
        .set_personality(Some(Personality::Pragmatic))
        .apply()
        .await
        .map_err(|err| {
            io::Error::other(format!("failed to persist personality migration: {err}"))
        })?;

    create_marker(&marker_path).await?;
    Ok(PersonalityMigrationStatus::Applied)
}

async fn has_recorded_sessions(codex_home: &Path, default_provider: &str) -> io::Result<bool> {
    let store = LocalThreadStore::new(codex_rollout::RolloutConfig {
        codex_home: codex_home.to_path_buf(),
        sqlite_home: codex_home.to_path_buf(),
        cwd: codex_home.to_path_buf(),
        model_provider_id: default_provider.to_string(),
        generate_memories: false,
    });
    if has_threads(&store, /*archived*/ false).await? {
        return Ok(true);
    }
    has_threads(&store, /*archived*/ true).await
}

async fn has_threads(store: &LocalThreadStore, archived: bool) -> io::Result<bool> {
    store
        .list_threads(ListThreadsParams {
            page_size: 1,
            cursor: None,
            sort_key: ThreadSortKey::CreatedAt,
            sort_direction: codex_thread_store::SortDirection::Desc,
            allowed_sources: Vec::new(),
            model_providers: None,
            cwd_filters: None,
            archived,
            search_term: None,
            use_state_db_only: false,
        })
        .await
        .map(|page| !page.items.is_empty())
        .map_err(io::Error::other)
}

async fn create_marker(marker_path: &Path) -> io::Result<()> {
    match OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(marker_path)
        .await
    {
        Ok(mut file) => file.write_all(b"v1\n").await,
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
#[path = "personality_migration_tests.rs"]
mod tests;

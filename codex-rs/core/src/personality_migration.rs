use crate::config::ConfigToml;
use crate::config::edit::ConfigEditsBuilder;
use crate::rollout::ARCHIVED_SESSIONS_SUBDIR;
use crate::rollout::SESSIONS_SUBDIR;
use crate::rollout::list::ThreadListConfig;
use crate::rollout::list::ThreadListLayout;
use crate::rollout::list::ThreadSortKey;
use crate::rollout::list::get_threads_in_root;
use crate::state_db;
use codex_protocol::config_types::Personality;
use codex_protocol::protocol::SessionSource;
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
        .get_config_profile(None)
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
    let allowed_sources: &[SessionSource] = &[];

    if let Some(state_db_ctx) = state_db::open_if_present(codex_home, default_provider).await
        && let Some(ids) = state_db::list_thread_ids_db(
            Some(state_db_ctx.as_ref()),
            codex_home,
            1,
            None,
            ThreadSortKey::CreatedAt,
            allowed_sources,
            None,
            false,
            "personality_migration",
        )
        .await
        && !ids.is_empty()
    {
        return Ok(true);
    }

    let sessions = get_threads_in_root(
        codex_home.join(SESSIONS_SUBDIR),
        1,
        None,
        ThreadSortKey::CreatedAt,
        ThreadListConfig {
            allowed_sources,
            model_providers: None,
            default_provider,
            layout: ThreadListLayout::NestedByDate,
        },
    )
    .await?;
    if !sessions.items.is_empty() {
        return Ok(true);
    }

    let archived_sessions = get_threads_in_root(
        codex_home.join(ARCHIVED_SESSIONS_SUBDIR),
        1,
        None,
        ThreadSortKey::CreatedAt,
        ThreadListConfig {
            allowed_sources,
            model_providers: None,
            default_provider,
            layout: ThreadListLayout::Flat,
        },
    )
    .await?;
    Ok(!archived_sessions.items.is_empty())
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

use codex_core::ARCHIVED_SESSIONS_SUBDIR;
use codex_core::SESSIONS_SUBDIR;
use codex_core::config::ConfigToml;
use codex_core::personality_migration::PERSONALITY_MIGRATION_FILENAME;
use codex_core::personality_migration::PersonalityMigrationStatus;
use codex_core::personality_migration::maybe_migrate_personality;
use codex_protocol::ThreadId;
use codex_protocol::config_types::Personality;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::UserMessageEvent;
use pretty_assertions::assert_eq;
use std::io;
use std::path::Path;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;

const TEST_TIMESTAMP: &str = "2025-01-01T00-00-00";

async fn read_config_toml(codex_home: &Path) -> io::Result<ConfigToml> {
    let contents = tokio::fs::read_to_string(codex_home.join("config.toml")).await?;
    toml::from_str(&contents).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

async fn write_session_with_user_event(codex_home: &Path) -> io::Result<()> {
    let thread_id = ThreadId::new();
    let dir = codex_home
        .join(SESSIONS_SUBDIR)
        .join("2025")
        .join("01")
        .join("01");
    write_rollout_with_user_event(&dir, thread_id).await
}

async fn write_archived_session_with_user_event(codex_home: &Path) -> io::Result<()> {
    let thread_id = ThreadId::new();
    let dir = codex_home.join(ARCHIVED_SESSIONS_SUBDIR);
    write_rollout_with_user_event(&dir, thread_id).await
}

async fn write_session_with_meta_only(codex_home: &Path) -> io::Result<()> {
    let thread_id = ThreadId::new();
    let dir = codex_home
        .join(SESSIONS_SUBDIR)
        .join("2025")
        .join("01")
        .join("01");
    write_rollout_with_meta_only(&dir, thread_id).await
}

async fn write_rollout_with_user_event(dir: &Path, thread_id: ThreadId) -> io::Result<()> {
    tokio::fs::create_dir_all(&dir).await?;
    let file_path = dir.join(format!("rollout-{TEST_TIMESTAMP}-{thread_id}.jsonl"));
    let mut file = tokio::fs::File::create(&file_path).await?;

    let session_meta = SessionMetaLine {
        meta: SessionMeta {
            id: thread_id,
            forked_from_id: None,
            timestamp: TEST_TIMESTAMP.to_string(),
            cwd: std::path::PathBuf::from("."),
            originator: "test_originator".to_string(),
            cli_version: "test_version".to_string(),
            source: SessionSource::Cli,
            model_provider: None,
            base_instructions: None,
            dynamic_tools: None,
        },
        git: None,
    };
    let meta_line = RolloutLine {
        timestamp: TEST_TIMESTAMP.to_string(),
        item: RolloutItem::SessionMeta(session_meta),
    };
    let user_event = RolloutLine {
        timestamp: TEST_TIMESTAMP.to_string(),
        item: RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            message: "hello".to_string(),
            images: None,
            local_images: Vec::new(),
            text_elements: Vec::new(),
        })),
    };

    let meta_json = serde_json::to_string(&meta_line)?;
    file.write_all(format!("{meta_json}\n").as_bytes()).await?;
    let user_json = serde_json::to_string(&user_event)?;
    file.write_all(format!("{user_json}\n").as_bytes()).await?;
    Ok(())
}

async fn write_rollout_with_meta_only(dir: &Path, thread_id: ThreadId) -> io::Result<()> {
    tokio::fs::create_dir_all(&dir).await?;
    let file_path = dir.join(format!("rollout-{TEST_TIMESTAMP}-{thread_id}.jsonl"));
    let mut file = tokio::fs::File::create(&file_path).await?;

    let session_meta = SessionMetaLine {
        meta: SessionMeta {
            id: thread_id,
            forked_from_id: None,
            timestamp: TEST_TIMESTAMP.to_string(),
            cwd: std::path::PathBuf::from("."),
            originator: "test_originator".to_string(),
            cli_version: "test_version".to_string(),
            source: SessionSource::Cli,
            model_provider: None,
            base_instructions: None,
            dynamic_tools: None,
        },
        git: None,
    };
    let meta_line = RolloutLine {
        timestamp: TEST_TIMESTAMP.to_string(),
        item: RolloutItem::SessionMeta(session_meta),
    };

    let meta_json = serde_json::to_string(&meta_line)?;
    file.write_all(format!("{meta_json}\n").as_bytes()).await?;
    Ok(())
}

fn parse_config_toml(contents: &str) -> io::Result<ConfigToml> {
    toml::from_str(contents).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

#[tokio::test]
async fn migration_marker_exists_no_sessions_no_change() -> io::Result<()> {
    let temp = TempDir::new()?;
    let marker_path = temp.path().join(PERSONALITY_MIGRATION_FILENAME);
    tokio::fs::write(&marker_path, "v1\n").await?;

    let status = maybe_migrate_personality(temp.path(), &ConfigToml::default()).await?;

    assert_eq!(status, PersonalityMigrationStatus::SkippedMarker);
    assert_eq!(
        tokio::fs::try_exists(temp.path().join("config.toml")).await?,
        false
    );
    Ok(())
}

#[tokio::test]
async fn no_marker_no_sessions_no_change() -> io::Result<()> {
    let temp = TempDir::new()?;

    let status = maybe_migrate_personality(temp.path(), &ConfigToml::default()).await?;

    assert_eq!(status, PersonalityMigrationStatus::SkippedNoSessions);
    assert_eq!(
        tokio::fs::try_exists(temp.path().join(PERSONALITY_MIGRATION_FILENAME)).await?,
        true
    );
    assert_eq!(
        tokio::fs::try_exists(temp.path().join("config.toml")).await?,
        false
    );
    Ok(())
}

#[tokio::test]
async fn no_marker_sessions_sets_personality() -> io::Result<()> {
    let temp = TempDir::new()?;
    write_session_with_user_event(temp.path()).await?;

    let status = maybe_migrate_personality(temp.path(), &ConfigToml::default()).await?;

    assert_eq!(status, PersonalityMigrationStatus::Applied);
    assert_eq!(
        tokio::fs::try_exists(temp.path().join(PERSONALITY_MIGRATION_FILENAME)).await?,
        true
    );

    let persisted = read_config_toml(temp.path()).await?;
    assert_eq!(persisted.personality, Some(Personality::Pragmatic));
    Ok(())
}

#[tokio::test]
async fn no_marker_sessions_preserves_existing_config_fields() -> io::Result<()> {
    let temp = TempDir::new()?;
    write_session_with_user_event(temp.path()).await?;
    tokio::fs::write(temp.path().join("config.toml"), "model = \"gpt-5-codex\"\n").await?;
    let config_toml = read_config_toml(temp.path()).await?;

    let status = maybe_migrate_personality(temp.path(), &config_toml).await?;

    assert_eq!(status, PersonalityMigrationStatus::Applied);
    let persisted = read_config_toml(temp.path()).await?;
    assert_eq!(persisted.model, Some("gpt-5-codex".to_string()));
    assert_eq!(persisted.personality, Some(Personality::Pragmatic));
    Ok(())
}

#[tokio::test]
async fn no_marker_meta_only_rollout_is_treated_as_no_sessions() -> io::Result<()> {
    let temp = TempDir::new()?;
    write_session_with_meta_only(temp.path()).await?;

    let status = maybe_migrate_personality(temp.path(), &ConfigToml::default()).await?;

    assert_eq!(status, PersonalityMigrationStatus::SkippedNoSessions);
    assert_eq!(
        tokio::fs::try_exists(temp.path().join(PERSONALITY_MIGRATION_FILENAME)).await?,
        true
    );
    assert_eq!(
        tokio::fs::try_exists(temp.path().join("config.toml")).await?,
        false
    );
    Ok(())
}

#[tokio::test]
async fn no_marker_explicit_global_personality_skips_migration() -> io::Result<()> {
    let temp = TempDir::new()?;
    write_session_with_user_event(temp.path()).await?;
    let config_toml = parse_config_toml("personality = \"friendly\"\n")?;

    let status = maybe_migrate_personality(temp.path(), &config_toml).await?;

    assert_eq!(
        status,
        PersonalityMigrationStatus::SkippedExplicitPersonality
    );
    assert_eq!(
        tokio::fs::try_exists(temp.path().join(PERSONALITY_MIGRATION_FILENAME)).await?,
        true
    );
    assert_eq!(
        tokio::fs::try_exists(temp.path().join("config.toml")).await?,
        false
    );
    Ok(())
}

#[tokio::test]
async fn no_marker_profile_personality_skips_migration() -> io::Result<()> {
    let temp = TempDir::new()?;
    write_session_with_user_event(temp.path()).await?;
    let config_toml = parse_config_toml(
        r#"
profile = "work"

[profiles.work]
personality = "friendly"
"#,
    )?;

    let status = maybe_migrate_personality(temp.path(), &config_toml).await?;

    assert_eq!(
        status,
        PersonalityMigrationStatus::SkippedExplicitPersonality
    );
    assert_eq!(
        tokio::fs::try_exists(temp.path().join(PERSONALITY_MIGRATION_FILENAME)).await?,
        true
    );
    assert_eq!(
        tokio::fs::try_exists(temp.path().join("config.toml")).await?,
        false
    );
    Ok(())
}

#[tokio::test]
async fn marker_short_circuits_invalid_profile_resolution() -> io::Result<()> {
    let temp = TempDir::new()?;
    tokio::fs::write(temp.path().join(PERSONALITY_MIGRATION_FILENAME), "v1\n").await?;
    let config_toml = parse_config_toml("profile = \"missing\"\n")?;

    let status = maybe_migrate_personality(temp.path(), &config_toml).await?;

    assert_eq!(status, PersonalityMigrationStatus::SkippedMarker);
    Ok(())
}

#[tokio::test]
async fn invalid_selected_profile_returns_error_and_does_not_write_marker() -> io::Result<()> {
    let temp = TempDir::new()?;
    let config_toml = parse_config_toml("profile = \"missing\"\n")?;

    let err = maybe_migrate_personality(temp.path(), &config_toml)
        .await
        .expect_err("missing profile should fail");

    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    assert_eq!(
        tokio::fs::try_exists(temp.path().join(PERSONALITY_MIGRATION_FILENAME)).await?,
        false
    );
    Ok(())
}

#[tokio::test]
async fn applied_migration_is_idempotent_on_second_run() -> io::Result<()> {
    let temp = TempDir::new()?;
    write_session_with_user_event(temp.path()).await?;

    let first_status = maybe_migrate_personality(temp.path(), &ConfigToml::default()).await?;
    let second_status = maybe_migrate_personality(temp.path(), &ConfigToml::default()).await?;

    assert_eq!(first_status, PersonalityMigrationStatus::Applied);
    assert_eq!(second_status, PersonalityMigrationStatus::SkippedMarker);
    let persisted = read_config_toml(temp.path()).await?;
    assert_eq!(persisted.personality, Some(Personality::Pragmatic));
    Ok(())
}

#[tokio::test]
async fn no_marker_archived_sessions_sets_personality() -> io::Result<()> {
    let temp = TempDir::new()?;
    write_archived_session_with_user_event(temp.path()).await?;

    let status = maybe_migrate_personality(temp.path(), &ConfigToml::default()).await?;

    assert_eq!(status, PersonalityMigrationStatus::Applied);
    assert_eq!(
        tokio::fs::try_exists(temp.path().join(PERSONALITY_MIGRATION_FILENAME)).await?,
        true
    );

    let persisted = read_config_toml(temp.path()).await?;
    assert_eq!(persisted.personality, Some(Personality::Pragmatic));
    Ok(())
}

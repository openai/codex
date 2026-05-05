use super::*;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::UserMessageEvent;
use codex_rollout::ARCHIVED_SESSIONS_SUBDIR;
use codex_rollout::RolloutConfig;
use codex_rollout::SESSIONS_SUBDIR;
use codex_rollout::state_db::StateDbHandle;
use codex_state::state_db_path;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;

const TEST_TIMESTAMP: &str = "2025-01-01T00-00-00";

async fn read_config_toml(codex_home: &Path) -> io::Result<ConfigToml> {
    let contents = tokio::fs::read_to_string(codex_home.join("config.toml")).await?;
    toml::from_str(&contents).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

async fn state_db_for_test(codex_home: &Path) -> io::Result<StateDbHandle> {
    state_db_for_test_with_sqlite_home(codex_home, codex_home).await
}

async fn state_db_for_test_with_sqlite_home(
    codex_home: &Path,
    sqlite_home: &Path,
) -> io::Result<StateDbHandle> {
    let config = RolloutConfig {
        codex_home: codex_home.to_path_buf(),
        sqlite_home: sqlite_home.to_path_buf(),
        cwd: codex_home.to_path_buf(),
        model_provider_id: "openai".to_string(),
        generate_memories: false,
    };
    codex_rollout::state_db::try_init(&config)
        .await
        .map_err(io::Error::other)
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
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
            model_provider: None,
            base_instructions: None,
            dynamic_tools: None,
            memory_mode: None,
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

    file.write_all(format!("{}\n", serde_json::to_string(&meta_line)?).as_bytes())
        .await?;
    file.write_all(format!("{}\n", serde_json::to_string(&user_event)?).as_bytes())
        .await?;
    Ok(())
}

#[tokio::test]
async fn applies_when_sessions_exist_and_no_personality() -> io::Result<()> {
    let temp = TempDir::new()?;
    write_session_with_user_event(temp.path()).await?;

    let config_toml = ConfigToml::default();
    let state_db = state_db_for_test(temp.path()).await?;
    let status = maybe_migrate_personality(temp.path(), &config_toml, state_db).await?;

    assert_eq!(status, PersonalityMigrationStatus::Applied);
    assert!(temp.path().join(PERSONALITY_MIGRATION_FILENAME).exists());

    let persisted = read_config_toml(temp.path()).await?;
    assert_eq!(persisted.personality, Some(Personality::Pragmatic));
    Ok(())
}

#[tokio::test]
async fn applies_when_only_archived_sessions_exist_and_no_personality() -> io::Result<()> {
    let temp = TempDir::new()?;
    write_archived_session_with_user_event(temp.path()).await?;

    let config_toml = ConfigToml::default();
    let state_db = state_db_for_test(temp.path()).await?;
    let status = maybe_migrate_personality(temp.path(), &config_toml, state_db).await?;

    assert_eq!(status, PersonalityMigrationStatus::Applied);
    assert!(temp.path().join(PERSONALITY_MIGRATION_FILENAME).exists());

    let persisted = read_config_toml(temp.path()).await?;
    assert_eq!(persisted.personality, Some(Personality::Pragmatic));
    Ok(())
}

#[tokio::test]
async fn skips_when_marker_exists() -> io::Result<()> {
    let temp = TempDir::new()?;
    create_marker(&temp.path().join(PERSONALITY_MIGRATION_FILENAME)).await?;

    let config_toml = ConfigToml::default();
    let state_db = state_db_for_test(temp.path()).await?;
    let status = maybe_migrate_personality(temp.path(), &config_toml, state_db).await?;

    assert_eq!(status, PersonalityMigrationStatus::SkippedMarker);
    assert!(!temp.path().join("config.toml").exists());
    Ok(())
}

#[tokio::test]
async fn skips_when_personality_explicit() -> io::Result<()> {
    let temp = TempDir::new()?;
    ConfigEditsBuilder::new(temp.path())
        .set_personality(Some(Personality::Friendly))
        .apply()
        .await
        .map_err(|err| io::Error::other(format!("failed to write config: {err}")))?;

    let config_toml = read_config_toml(temp.path()).await?;
    let state_db = state_db_for_test(temp.path()).await?;
    let status = maybe_migrate_personality(temp.path(), &config_toml, state_db).await?;

    assert_eq!(
        status,
        PersonalityMigrationStatus::SkippedExplicitPersonality
    );
    assert!(temp.path().join(PERSONALITY_MIGRATION_FILENAME).exists());

    let persisted = read_config_toml(temp.path()).await?;
    assert_eq!(persisted.personality, Some(Personality::Friendly));
    Ok(())
}

#[tokio::test]
async fn skips_when_no_sessions() -> io::Result<()> {
    let temp = TempDir::new()?;
    let config_toml = ConfigToml::default();
    let state_db = state_db_for_test(temp.path()).await?;
    let status = maybe_migrate_personality(temp.path(), &config_toml, state_db).await?;

    assert_eq!(status, PersonalityMigrationStatus::SkippedNoSessions);
    assert!(temp.path().join(PERSONALITY_MIGRATION_FILENAME).exists());
    assert!(!temp.path().join("config.toml").exists());
    Ok(())
}

#[tokio::test]
async fn uses_configured_sqlite_home_when_checking_for_sessions() -> io::Result<()> {
    let codex_home = TempDir::new()?;
    let sqlite_home = TempDir::new()?;
    write_session_with_user_event(codex_home.path()).await?;

    let config_toml = ConfigToml::default();
    let state_db =
        state_db_for_test_with_sqlite_home(codex_home.path(), sqlite_home.path()).await?;
    let status = maybe_migrate_personality(codex_home.path(), &config_toml, state_db).await?;

    assert_eq!(status, PersonalityMigrationStatus::Applied);
    assert!(
        codex_home
            .path()
            .join(PERSONALITY_MIGRATION_FILENAME)
            .exists()
    );

    let persisted = read_config_toml(codex_home.path()).await?;
    assert_eq!(persisted.personality, Some(Personality::Pragmatic));
    assert!(!state_db_path(codex_home.path()).exists());
    assert!(state_db_path(sqlite_home.path()).exists());
    Ok(())
}

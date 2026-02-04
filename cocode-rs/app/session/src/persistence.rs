//! Session persistence for saving and loading sessions to/from files.
//!
//! Sessions are stored as JSON files in the `~/.cocode/sessions/` directory.

use std::path::Path;

use cocode_message::MessageHistory;
use serde::Deserialize;
use serde::Serialize;
use tokio::fs;
use tracing::debug;
use tracing::info;

use crate::session::Session;

/// Persisted session data.
#[derive(Debug, Serialize, Deserialize)]
pub struct PersistedSession {
    /// Session metadata.
    pub session: Session,

    /// Message history.
    pub history: MessageHistory,

    /// File format version for future compatibility.
    #[serde(default = "default_version")]
    pub version: i32,
}

fn default_version() -> i32 {
    1
}

impl PersistedSession {
    /// Create a new persisted session.
    pub fn new(session: Session, history: MessageHistory) -> Self {
        Self {
            session,
            history,
            version: 1,
        }
    }
}

/// Save a session and its history to a JSON file.
///
/// # Arguments
///
/// * `session` - The session metadata to save
/// * `history` - The message history to save
/// * `path` - The file path to save to
///
/// # Example
///
/// ```ignore
/// use cocode_session::{Session, save_session_to_file};
/// use cocode_message::MessageHistory;
/// use std::path::Path;
///
/// let session = Session::new(...);
/// let history = MessageHistory::new();
/// save_session_to_file(&session, &history, Path::new("session.json")).await?;
/// ```
pub async fn save_session_to_file(
    session: &Session,
    history: &MessageHistory,
    path: &Path,
) -> anyhow::Result<()> {
    info!(
        session_id = %session.id,
        path = %path.display(),
        "Saving session"
    );

    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let persisted = PersistedSession::new(session.clone(), history.clone());
    let json = serde_json::to_string_pretty(&persisted)?;

    fs::write(path, json).await?;

    debug!(
        session_id = %session.id,
        bytes = persisted.session.id.len(),
        "Session saved"
    );

    Ok(())
}

/// Load a session and its history from a JSON file.
///
/// # Arguments
///
/// * `path` - The file path to load from
///
/// # Returns
///
/// A tuple of (Session, MessageHistory)
///
/// # Example
///
/// ```ignore
/// use cocode_session::load_session_from_file;
/// use std::path::Path;
///
/// let (session, history) = load_session_from_file(Path::new("session.json")).await?;
/// println!("Loaded session: {}", session.id);
/// ```
pub async fn load_session_from_file(path: &Path) -> anyhow::Result<(Session, MessageHistory)> {
    info!(path = %path.display(), "Loading session");

    let content = fs::read_to_string(path).await?;
    let persisted: PersistedSession = serde_json::from_str(&content)?;

    debug!(
        session_id = %persisted.session.id,
        version = persisted.version,
        "Session loaded"
    );

    Ok((persisted.session, persisted.history))
}

/// Check if a session file exists.
pub async fn session_exists(path: &Path) -> bool {
    fs::metadata(path).await.is_ok()
}

/// Delete a session file.
pub async fn delete_session_file(path: &Path) -> anyhow::Result<()> {
    info!(path = %path.display(), "Deleting session file");
    fs::remove_file(path).await?;
    Ok(())
}

/// Get the default sessions directory.
pub fn default_sessions_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".cocode")
        .join("sessions")
}

/// Get the path for a session file by ID.
pub fn session_file_path(session_id: &str) -> std::path::PathBuf {
    default_sessions_dir().join(format!("{session_id}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cocode_protocol::ModelSpec;
    use cocode_protocol::RoleSelection;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_save_and_load_session() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test_session.json");

        let selection = RoleSelection::new(ModelSpec::new("openai", "gpt-5"));
        let session = Session::new(PathBuf::from("/test"), selection);
        let history = MessageHistory::new();

        // Save
        save_session_to_file(&session, &history, &path)
            .await
            .unwrap();

        assert!(session_exists(&path).await);

        // Load
        let (loaded_session, _loaded_history) = load_session_from_file(&path).await.unwrap();

        assert_eq!(loaded_session.id, session.id);
        assert_eq!(loaded_session.model(), session.model());
        assert_eq!(loaded_session.provider(), session.provider());
    }

    #[tokio::test]
    async fn test_delete_session_file() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("to_delete.json");

        let selection = RoleSelection::new(ModelSpec::new("openai", "gpt-5"));
        let session = Session::new(PathBuf::from("/test"), selection);
        let history = MessageHistory::new();

        save_session_to_file(&session, &history, &path)
            .await
            .unwrap();
        assert!(session_exists(&path).await);

        delete_session_file(&path).await.unwrap();
        assert!(!session_exists(&path).await);
    }

    #[test]
    fn test_session_file_path() {
        let path = session_file_path("test-id-123");
        assert!(path.to_string_lossy().contains("sessions"));
        assert!(path.to_string_lossy().ends_with("test-id-123.json"));
    }

    #[test]
    fn test_persisted_session_version() {
        let selection = RoleSelection::new(ModelSpec::new("openai", "gpt-5"));
        let session = Session::new(PathBuf::from("/test"), selection);
        let history = MessageHistory::new();
        let persisted = PersistedSession::new(session, history);

        assert_eq!(persisted.version, 1);
    }
}

use crate::error::Result;
use crate::find_conversation_path_by_id_str;
use crate::rollout::list::read_head_for_summary;
use codex_protocol::ConversationId;
use codex_protocol::protocol::SessionMetaLine;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::Error as IoError;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SavedSessionEntry {
    pub name: String,
    pub conversation_id: ConversationId,
    pub rollout_path: PathBuf,
    pub cwd: PathBuf,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,
    pub saved_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedSessionsFile {
    #[serde(default)]
    entries: BTreeMap<String, SavedSessionEntry>,
}

fn saved_sessions_path(codex_home: &Path) -> PathBuf {
    codex_home.join("saved_sessions.json")
}

async fn load_saved_sessions_file(path: &Path) -> Result<SavedSessionsFile> {
    match tokio::fs::read_to_string(path).await {
        Ok(text) => {
            let parsed = serde_json::from_str(&text)
                .map_err(|e| IoError::other(format!("failed to parse saved sessions: {e}")))?;
            Ok(parsed)
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(SavedSessionsFile::default()),
        Err(err) => Err(err.into()),
    }
}

async fn write_saved_sessions_file(path: &Path, file: &SavedSessionsFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let json = serde_json::to_string_pretty(file)
        .map_err(|e| IoError::other(format!("failed to serialize saved sessions: {e}")))?;
    let tmp_path = path.with_extension("json.tmp");
    tokio::fs::write(&tmp_path, json).await?;
    tokio::fs::rename(tmp_path, path).await?;
    Ok(())
}

/// Create a new entry from the rollout's SessionMeta line.
pub async fn build_saved_session_entry(
    name: String,
    rollout_path: PathBuf,
    model: String,
) -> Result<SavedSessionEntry> {
    let head = read_head_for_summary(&rollout_path).await?;
    let first = head.first().ok_or_else(|| {
        IoError::other(format!(
            "rollout at {} has no SessionMeta",
            rollout_path.display()
        ))
    })?;
    let SessionMetaLine { mut meta, .. } = serde_json::from_value::<SessionMetaLine>(first.clone())
        .map_err(|e| IoError::other(format!("failed to parse SessionMeta: {e}")))?;
    meta.name = Some(name.clone());
    let saved_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|e| IoError::other(format!("failed to format timestamp: {e}")))?;
    let created_at = if meta.timestamp.is_empty() {
        None
    } else {
        Some(meta.timestamp.clone())
    };
    Ok(SavedSessionEntry {
        name,
        conversation_id: meta.id,
        rollout_path,
        cwd: meta.cwd,
        model,
        model_provider: meta.model_provider,
        saved_at,
        created_at,
    })
}

/// Insert or replace a saved session entry in `saved_sessions.json`.
pub async fn upsert_saved_session(codex_home: &Path, entry: SavedSessionEntry) -> Result<()> {
    let path = saved_sessions_path(codex_home);
    let mut file = load_saved_sessions_file(&path).await?;
    file.entries.insert(entry.name.clone(), entry);
    write_saved_sessions_file(&path, &file).await
}

/// Lookup a saved session by name, if present.
pub async fn resolve_saved_session(
    codex_home: &Path,
    name: &str,
) -> Result<Option<SavedSessionEntry>> {
    let path = saved_sessions_path(codex_home);
    let file = load_saved_sessions_file(&path).await?;
    Ok(file.entries.get(name).cloned())
}

/// Return all saved sessions ordered by newest `saved_at` first.
pub async fn list_saved_sessions(codex_home: &Path) -> Result<Vec<SavedSessionEntry>> {
    let path = saved_sessions_path(codex_home);
    let file = load_saved_sessions_file(&path).await?;
    let mut entries: Vec<SavedSessionEntry> = file.entries.values().cloned().collect();
    entries.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
    Ok(entries)
}

/// Resolve a rollout path from either a saved-session name or rollout id string.
/// Returns `Ok(None)` when nothing matches.
pub async fn resolve_rollout_path(codex_home: &Path, identifier: &str) -> Result<Option<PathBuf>> {
    if let Some(entry) = resolve_saved_session(codex_home, identifier).await? {
        if entry.rollout_path.exists() {
            return Ok(Some(entry.rollout_path));
        }
        warn!(
            "saved session '{}' points to missing rollout at {}",
            identifier,
            entry.rollout_path.display()
        );
    }
    find_conversation_path_by_id_str(codex_home, identifier)
        .await
        .map_err(Into::into) // todo jif
}

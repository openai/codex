//! Resolve saved-session state needed before resuming or forking a thread.
//!
//! The app-server API owns normal thread lifecycle data. This module coordinates
//! the TUI-specific cwd prompt and falls back to local rollout metadata only
//! before the app server has resumed the selected thread.

use std::io;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;
use std::path::PathBuf;

use crate::cwd_prompt;
use crate::cwd_prompt::CwdPromptAction;
use crate::cwd_prompt::CwdPromptOutcome;
use crate::cwd_prompt::CwdSelection;
use crate::resume_picker::SessionTarget;
use crate::tui::Tui;
use codex_protocol::ThreadId;
use codex_rollout::open_rollout_line_reader;
use codex_state::StateRuntime;
use codex_utils_path as path_utils;
use serde::Deserialize;
use serde_json::Value;

const ROLLOUT_RESUME_TAIL_READ_CHUNK_SIZE: usize = 64 * 1024;
const TURN_CONTEXT_NEEDLE: &[u8] = b"turn_context";

pub(crate) async fn verified_rollout_path_for_thread(
    path: &Path,
    thread_id: ThreadId,
) -> Option<PathBuf> {
    let existing_path = codex_rollout::existing_rollout_path(path).await?;
    if rollout_file_name_matches_thread_id(existing_path.as_path(), thread_id)
        || codex_rollout::read_session_meta_line(existing_path.as_path())
            .await
            .is_ok_and(|session_meta| session_meta.meta.id == thread_id)
    {
        return Some(codex_rollout::plain_rollout_path(existing_path.as_path()));
    }
    None
}

fn rollout_file_name_matches_thread_id(path: &Path, thread_id: ThreadId) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let plain_suffix = format!("{thread_id}.jsonl");
    let compressed_suffix = format!("{plain_suffix}.zst");
    file_name.starts_with("rollout-")
        && (file_name.ends_with(plain_suffix.as_str())
            || file_name.ends_with(compressed_suffix.as_str()))
}

#[derive(Default)]
struct RolloutResumeState {
    thread_id: Option<ThreadId>,
    cwd: Option<PathBuf>,
    model: Option<String>,
}

#[derive(Deserialize)]
struct SessionMetadata {
    id: ThreadId,
    cwd: PathBuf,
}

#[derive(Deserialize)]
struct TurnContextResumeState {
    cwd: PathBuf,
    model: String,
}

#[derive(Deserialize)]
struct RawRecord {
    #[serde(rename = "type")]
    item_type: String,
    payload: Option<Value>,
}

pub(crate) enum ResolveCwdOutcome {
    Continue(Option<PathBuf>),
    Exit,
}

pub(crate) async fn resolve_session_thread_id(
    path: &Path,
    id_str_if_uuid: Option<&str>,
) -> Option<ThreadId> {
    match id_str_if_uuid {
        Some(id_str) => ThreadId::from_string(id_str).ok(),
        None => read_rollout_resume_state(path)
            .await
            .ok()
            .and_then(|state| state.thread_id),
    }
}

pub(crate) async fn read_session_model(
    state_db_ctx: Option<&StateRuntime>,
    thread_id: ThreadId,
    path: Option<&Path>,
) -> Option<String> {
    if let Some(state_db_ctx) = state_db_ctx
        && let Ok(Some(metadata)) = state_db_ctx.get_thread(thread_id).await
        && let Some(model) = metadata.model
    {
        return Some(model);
    }

    let path = path?;
    read_rollout_resume_state(path)
        .await
        .ok()
        .and_then(|state| state.model)
}

pub(crate) async fn resolve_cwd_for_resume_or_fork(
    tui: &mut Tui,
    state_db_ctx: Option<&StateRuntime>,
    current_cwd: &Path,
    target_session: &SessionTarget,
    action: CwdPromptAction,
    allow_prompt: bool,
) -> color_eyre::Result<ResolveCwdOutcome> {
    let history_cwd = if target_session.path.is_some() {
        read_session_cwd(
            state_db_ctx,
            target_session.thread_id,
            target_session.path.as_deref(),
        )
        .await
        .or_else(|| target_session.cwd.clone())
    } else if let Some(cwd) = target_session.cwd.as_ref() {
        Some(cwd.clone())
    } else {
        read_session_cwd(state_db_ctx, target_session.thread_id, /*path*/ None).await
    };
    let Some(history_cwd) = history_cwd else {
        return Ok(ResolveCwdOutcome::Continue(None));
    };
    if allow_prompt && cwds_differ(current_cwd, &history_cwd) {
        let selection_outcome =
            cwd_prompt::run_cwd_selection_prompt(tui, action, current_cwd, &history_cwd).await?;
        return Ok(match selection_outcome {
            CwdPromptOutcome::Selection(CwdSelection::Current) => {
                ResolveCwdOutcome::Continue(Some(current_cwd.to_path_buf()))
            }
            CwdPromptOutcome::Selection(CwdSelection::Session) => {
                ResolveCwdOutcome::Continue(Some(history_cwd))
            }
            CwdPromptOutcome::Exit => ResolveCwdOutcome::Exit,
        });
    }
    Ok(ResolveCwdOutcome::Continue(Some(history_cwd)))
}

async fn read_session_cwd(
    state_db_ctx: Option<&StateRuntime>,
    thread_id: ThreadId,
    path: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(path) = path {
        match read_rollout_resume_state(path).await {
            Ok(state) => {
                if state.thread_id == Some(thread_id) && state.cwd.is_some() {
                    return state.cwd;
                }
            }
            Err(err) => {
                let rollout_path = path.display().to_string();
                tracing::warn!(
                    %rollout_path,
                    %err,
                    "Failed to read session metadata from rollout"
                );
            }
        }
    }

    if let Some(state_db_ctx) = state_db_ctx
        && let Ok(Some(metadata)) = state_db_ctx.get_thread(thread_id).await
    {
        return Some(metadata.cwd);
    }
    None
}

pub(crate) fn cwds_differ(current_cwd: &Path, session_cwd: &Path) -> bool {
    !path_utils::paths_match_after_normalization(current_cwd, session_cwd)
}

async fn read_rollout_resume_state(path: &Path) -> io::Result<RolloutResumeState> {
    if let Some(state) = try_read_rollout_resume_state_fast(path).await? {
        return Ok(state);
    }
    read_rollout_resume_state_full(path).await
}

async fn try_read_rollout_resume_state_fast(path: &Path) -> io::Result<Option<RolloutResumeState>> {
    let Some(existing_path) = codex_rollout::existing_rollout_path(path).await else {
        return Ok(None);
    };
    if is_compressed_rollout_path(existing_path.as_path()) {
        return Ok(None);
    }

    let Some(session_meta) = read_initial_session_metadata(existing_path.as_path()).await? else {
        return Ok(None);
    };
    let latest_turn_context =
        match latest_turn_context_from_plain_rollout(existing_path.as_path()).await {
            Ok(turn_context) => turn_context,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };

    let mut state = RolloutResumeState {
        thread_id: Some(session_meta.id),
        cwd: Some(session_meta.cwd),
        ..Default::default()
    };
    if let Some(turn_context) = latest_turn_context {
        state.cwd = Some(turn_context.cwd);
        state.model = Some(turn_context.model);
    }
    Ok(Some(state))
}

async fn read_initial_session_metadata(path: &Path) -> io::Result<Option<SessionMetadata>> {
    let mut reader = open_rollout_line_reader(path).await?;
    while let Some(line) = reader.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<RawRecord>(trimmed) else {
            continue;
        };
        let Some(payload) = record.payload else {
            return Ok(None);
        };
        if record.item_type != "session_meta" {
            return Ok(None);
        }
        return Ok(serde_json::from_value::<SessionMetadata>(payload).ok());
    }
    Ok(None)
}

async fn latest_turn_context_from_plain_rollout(
    path: &Path,
) -> io::Result<Option<TurnContextResumeState>> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        latest_turn_context_from_plain_rollout_blocking(path.as_path())
    })
    .await
    .map_err(io::Error::other)?
}

fn latest_turn_context_from_plain_rollout_blocking(
    path: &Path,
) -> io::Result<Option<TurnContextResumeState>> {
    let mut file = std::fs::File::open(path)?;
    let mut remaining = file.metadata()?.len();
    let mut line_rev = Vec::new();
    let mut buf = vec![0u8; ROLLOUT_RESUME_TAIL_READ_CHUNK_SIZE];

    while remaining > 0 {
        let read_size = usize::try_from(remaining.min(ROLLOUT_RESUME_TAIL_READ_CHUNK_SIZE as u64))
            .map_err(io::Error::other)?;
        remaining -= read_size as u64;
        file.seek(SeekFrom::Start(remaining))?;
        file.read_exact(&mut buf[..read_size])?;

        for &byte in buf[..read_size].iter().rev() {
            if byte == b'\n' {
                if let Some(turn_context) = parse_turn_context_from_rev_line(&mut line_rev)? {
                    return Ok(Some(turn_context));
                }
            } else {
                line_rev.push(byte);
            }
        }
    }

    parse_turn_context_from_rev_line(&mut line_rev)
}

fn parse_turn_context_from_rev_line(
    line_rev: &mut Vec<u8>,
) -> io::Result<Option<TurnContextResumeState>> {
    if line_rev.is_empty() {
        return Ok(None);
    }
    line_rev.reverse();
    let line = std::mem::take(line_rev);
    let trimmed = trim_ascii_whitespace(line.as_slice());
    if trimmed.is_empty() || !contains_bytes(trimmed, TURN_CONTEXT_NEEDLE) {
        return Ok(None);
    }
    let Ok(record) = serde_json::from_slice::<RawRecord>(trimmed) else {
        return Ok(None);
    };
    if record.item_type != "turn_context" {
        return Ok(None);
    }
    let Some(payload) = record.payload else {
        return Ok(None);
    };
    Ok(serde_json::from_value::<TurnContextResumeState>(payload).ok())
}

fn is_compressed_rollout_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".jsonl.zst"))
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn trim_ascii_whitespace(bytes: &[u8]) -> &[u8] {
    let mut start = 0usize;
    let mut end = bytes.len();
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &bytes[start..end]
}

async fn read_rollout_resume_state_full(path: &Path) -> io::Result<RolloutResumeState> {
    let mut reader = open_rollout_line_reader(path).await?;
    let mut state = RolloutResumeState::default();
    let mut saw_record = false;

    while let Some(line) = reader.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<RawRecord>(trimmed) else {
            continue;
        };
        saw_record = true;
        let Some(payload) = record.payload else {
            continue;
        };

        match record.item_type.as_str() {
            "session_meta" if state.thread_id.is_none() => {
                if let Ok(metadata) = serde_json::from_value::<SessionMetadata>(payload) {
                    state.thread_id = Some(metadata.id);
                    state.cwd.get_or_insert(metadata.cwd);
                }
            }
            "turn_context" => {
                if let Ok(turn_context) = serde_json::from_value::<TurnContextResumeState>(payload)
                {
                    state.cwd = Some(turn_context.cwd);
                    state.model = Some(turn_context.model);
                }
            }
            _ => {}
        }
    }

    if saw_record {
        Ok(state)
    } else {
        Err(io::Error::other(format!(
            "rollout at {} is empty",
            path.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn rollout_line(
        timestamp: &str,
        item_type: &str,
        payload: serde_json::Value,
    ) -> serde_json::Value {
        serde_json::json!({
            "timestamp": timestamp,
            "type": item_type,
            "payload": payload,
        })
    }

    fn write_rollout_lines(path: &Path, lines: &[serde_json::Value]) -> std::io::Result<()> {
        let mut text = String::new();
        for line in lines {
            text.push_str(&serde_json::to_string(line).expect("serialize rollout"));
            text.push('\n');
        }
        std::fs::write(path, text)
    }

    #[tokio::test]
    async fn rollout_resume_state_prefers_latest_turn_context() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let thread_id = ThreadId::new();
        let original = temp_dir.path().join("original");
        let latest = temp_dir.path().join("latest");
        let rollout_path = temp_dir.path().join("rollout.jsonl");
        write_rollout_lines(
            &rollout_path,
            &[
                rollout_line(
                    "t0",
                    "session_meta",
                    serde_json::json!({
                        "id": thread_id,
                        "cwd": original,
                        "originator": "test",
                        "cli_version": "test",
                    }),
                ),
                rollout_line(
                    "t1",
                    "turn_context",
                    serde_json::json!({ "cwd": temp_dir.path().join("middle"), "model": "middle" }),
                ),
                rollout_line(
                    "t2",
                    "turn_context",
                    serde_json::json!({ "cwd": latest.clone(), "model": "latest" }),
                ),
            ],
        )?;

        let state = read_rollout_resume_state(&rollout_path).await?;

        assert_eq!(state.thread_id, Some(thread_id));
        assert_eq!(state.cwd, Some(latest));
        assert_eq!(state.model, Some("latest".to_string()));
        Ok(())
    }

    #[tokio::test]
    async fn rollout_resume_state_falls_back_to_session_meta() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let thread_id = ThreadId::new();
        let cwd = temp_dir.path().join("session");
        let rollout_path = temp_dir.path().join("rollout.jsonl");
        write_rollout_lines(
            &rollout_path,
            &[rollout_line(
                "t0",
                "session_meta",
                serde_json::json!({
                    "id": thread_id,
                    "cwd": cwd.clone(),
                    "originator": "test",
                    "cli_version": "test",
                }),
            )],
        )?;

        let state = read_rollout_resume_state(&rollout_path).await?;

        assert_eq!(state.thread_id, Some(thread_id));
        assert_eq!(state.cwd, Some(cwd));
        assert_eq!(state.model, None);
        Ok(())
    }

    #[tokio::test]
    async fn rollout_resume_state_skips_malformed_lines() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let thread_id = ThreadId::new();
        let cwd = temp_dir.path().join("session");
        let rollout_path = temp_dir.path().join("rollout.jsonl");
        let valid_line = serde_json::to_string(&rollout_line(
            "t0",
            "session_meta",
            serde_json::json!({
                "id": thread_id,
                "cwd": cwd.clone(),
                "originator": "test",
                "cli_version": "test",
            }),
        ))
        .expect("serialize rollout line");
        std::fs::write(&rollout_path, format!("{valid_line}\n{{"))?;

        let state = read_rollout_resume_state(&rollout_path).await?;

        assert_eq!(state.thread_id, Some(thread_id));
        assert_eq!(state.cwd, Some(cwd));
        Ok(())
    }
}

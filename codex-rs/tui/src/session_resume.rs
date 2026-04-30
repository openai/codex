//! Resolve saved-session state needed before resuming or forking a thread.
//!
//! The app-server API owns normal thread lifecycle data. This module coordinates
//! the TUI-specific cwd prompt and falls back to local rollout metadata only
//! before the app server has resumed the selected thread.

use std::io;
use std::path::Path;
use std::path::PathBuf;

use crate::cwd_prompt;
use crate::cwd_prompt::CwdPromptAction;
use crate::cwd_prompt::CwdPromptOutcome;
use crate::cwd_prompt::CwdSelection;
use crate::legacy_core::config::Config;
use crate::tui::Tui;
use codex_protocol::ThreadId;
use codex_rollout::state_db::get_state_db;
use codex_utils_path as path_utils;
use serde::Deserialize;
use serde_json::Value;
use tokio::io::AsyncBufReadExt;

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
    config: &Config,
    thread_id: ThreadId,
    path: Option<&Path>,
) -> Option<String> {
    if let Some(state_db_ctx) = get_state_db(config).await
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
    config: &Config,
    current_cwd: &Path,
    thread_id: ThreadId,
    path: Option<&Path>,
    action: CwdPromptAction,
    allow_prompt: bool,
) -> color_eyre::Result<ResolveCwdOutcome> {
    let Some(history_cwd) = read_session_cwd(config, thread_id, path).await else {
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
    config: &Config,
    thread_id: ThreadId,
    path: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(state_db_ctx) = get_state_db(config).await
        && let Ok(Some(metadata)) = state_db_ctx.get_thread(thread_id).await
    {
        return Some(metadata.cwd);
    }

    let path = path?;
    match read_rollout_resume_state(path).await {
        Ok(state) => state.cwd,
        Err(err) => {
            let rollout_path = path.display().to_string();
            tracing::warn!(
                %rollout_path,
                %err,
                "Failed to read session metadata from rollout"
            );
            None
        }
    }
}

pub(crate) fn cwds_differ(current_cwd: &Path, session_cwd: &Path) -> bool {
    !path_utils::paths_match_after_normalization(current_cwd, session_cwd)
}

async fn read_rollout_resume_state(path: &Path) -> io::Result<RolloutResumeState> {
    let file = tokio::fs::File::open(path).await?;
    let reader = tokio::io::BufReader::new(file);
    let mut lines = reader.lines();
    let mut state = RolloutResumeState::default();
    let mut saw_record = false;

    while let Some(line) = lines.next_line().await? {
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

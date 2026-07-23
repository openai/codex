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
use crate::legacy_core::config::Config;
use crate::resume_picker::SessionTarget;
use crate::tui::Tui;
use codex_config::types::ResumeCwdMode;
use codex_protocol::ThreadId;
use codex_rollout::open_rollout_line_reader;
use codex_state::StateRuntime;
use codex_utils_path as path_utils;
use serde::Deserialize;
use serde_json::Value;

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

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ResolveCwdOutcome {
    Continue(Option<PathBuf>),
    Exit,
}

pub(crate) struct ResumeCwdContext<'path> {
    pub(crate) current_cwd: &'path Path,
    pub(crate) remembered_current_cwd: &'path Path,
    pub(crate) allow_remember_current: bool,
    pub(crate) mode: Option<ResumeCwdMode>,
}

pub(crate) fn effective_resume_cwd_mode(
    configured_mode: Option<ResumeCwdMode>,
    cwd_override: Option<&Path>,
) -> Option<ResumeCwdMode> {
    if cwd_override.is_some() {
        Some(ResumeCwdMode::Current)
    } else {
        configured_mode
    }
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
    config: &Config,
    state_db_ctx: Option<&StateRuntime>,
    target_session: &SessionTarget,
    action: CwdPromptAction,
    cwd_context: ResumeCwdContext<'_>,
) -> color_eyre::Result<ResolveCwdOutcome> {
    if matches!(cwd_context.mode, Some(ResumeCwdMode::Current)) {
        return Ok(ResolveCwdOutcome::Continue(Some(
            cwd_context.remembered_current_cwd.to_path_buf(),
        )));
    }
    let Some(history_cwd) = read_session_cwd(
        state_db_ctx,
        target_session.thread_id,
        target_session.path.as_deref(),
    )
    .await
    else {
        if matches!(cwd_context.mode, Some(ResumeCwdMode::Session)) {
            color_eyre::eyre::bail!(
                "failed to determine the working directory recorded for the selected session"
            );
        }
        return Ok(ResolveCwdOutcome::Continue(None));
    };
    match cwd_context.mode {
        Some(ResumeCwdMode::Session) => {
            return Ok(ResolveCwdOutcome::Continue(Some(history_cwd)));
        }
        Some(ResumeCwdMode::Current) | None => {}
    }
    if cwds_differ(cwd_context.current_cwd, &history_cwd) {
        let selection_outcome = cwd_prompt::run_cwd_selection_prompt(
            tui,
            config,
            action,
            cwd_context.current_cwd,
            &history_cwd,
            cwd_context.remembered_current_cwd,
            cwd_context.allow_remember_current,
        )
        .await?;
        return Ok(match selection_outcome {
            CwdPromptOutcome::Selection(selection) => ResolveCwdOutcome::Continue(Some(
                selection
                    .selected_cwd(
                        cwd_context.current_cwd,
                        &history_cwd,
                        cwd_context.remembered_current_cwd,
                    )
                    .to_path_buf(),
            )),
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
    if let Some(state_db_ctx) = state_db_ctx
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
    use codex_utils_absolute_path::test_support::PathExt;
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

    #[tokio::test]
    async fn rollout_resume_state_preserves_legacy_fork_child_context() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let thread_id = ThreadId::from_string("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa")
            .expect("legacy thread id");
        let parent_thread_id = ThreadId::from_string("bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb")
            .expect("legacy parent id");
        let child_cwd = temp_dir.path().join("child");
        let rollout_path = temp_dir.path().join("rollout.jsonl");
        write_rollout_lines(
            &rollout_path,
            &[
                rollout_line(
                    "t0",
                    "session_meta",
                    serde_json::json!({
                        "id": thread_id,
                        "forked_from_id": parent_thread_id,
                        "cwd": temp_dir.path().join("initial"),
                        "originator": "test",
                        "cli_version": "test",
                    }),
                ),
                rollout_line(
                    "t1",
                    "event_msg",
                    serde_json::json!({ "type": "task_started", "turn_id": "legacy-child-turn" }),
                ),
                rollout_line(
                    "t2",
                    "turn_context",
                    serde_json::json!({ "cwd": child_cwd.clone(), "model": "child-model" }),
                ),
            ],
        )?;

        let state = read_rollout_resume_state(&rollout_path).await?;

        assert_eq!(state.thread_id, Some(thread_id));
        assert_eq!(state.cwd, Some(child_cwd));
        assert_eq!(state.model.as_deref(), Some("child-model"));
        Ok(())
    }

    #[tokio::test]
    async fn session_cwd_prefers_state_metadata_over_rollout_context() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let thread_id = ThreadId::new();
        let session_cwd = temp_dir.path().join("child");
        let rollout_path = temp_dir.path().join("rollout.jsonl");
        write_rollout_lines(
            &rollout_path,
            &[
                rollout_line(
                    "t0",
                    "session_meta",
                    serde_json::json!({
                        "id": thread_id,
                        "cwd": session_cwd,
                        "originator": "test",
                        "cli_version": "test",
                    }),
                ),
                rollout_line(
                    "t1",
                    "turn_context",
                    serde_json::json!({
                        "cwd": temp_dir.path().join("copied-parent"),
                        "model": "parent-model",
                    }),
                ),
            ],
        )?;
        let state_runtime = StateRuntime::init(
            codex_state::SqliteConfig::new_for_testing(temp_dir.path().abs()),
            "test-provider".to_string(),
        )
        .await
        .map_err(std::io::Error::other)?;
        let created_at = chrono::DateTime::parse_from_rfc3339("2025-01-05T12:00:00Z")
            .expect("timestamp should parse")
            .with_timezone(&chrono::Utc);
        let mut builder = codex_state::ThreadMetadataBuilder::new(
            thread_id,
            rollout_path.clone(),
            created_at,
            serde_json::from_value(serde_json::json!("cli"))
                .expect("cli session source should deserialize"),
        );
        builder.cwd = session_cwd.clone();
        state_runtime
            .upsert_thread(&builder.build("test-provider"))
            .await
            .map_err(std::io::Error::other)?;

        let cwd =
            read_session_cwd(Some(state_runtime.as_ref()), thread_id, Some(&rollout_path)).await;

        assert_eq!(cwd, Some(session_cwd));
        Ok(())
    }

    #[tokio::test]
    async fn configured_resume_cwd_skips_prompt() -> color_eyre::Result<()> {
        let temp_dir = TempDir::new()?;
        let thread_id = ThreadId::new();
        let session_cwd = temp_dir.path().join("session");
        let rollout_path = temp_dir.path().join("rollout.jsonl");
        let config = crate::legacy_core::config::ConfigBuilder::default()
            .codex_home(temp_dir.path().to_path_buf())
            .build()
            .await?;
        let current_cwd = config.cwd.to_path_buf();
        write_rollout_lines(
            &rollout_path,
            &[rollout_line(
                "t0",
                "session_meta",
                serde_json::json!({
                    "id": thread_id,
                    "cwd": session_cwd.clone(),
                    "originator": "test",
                    "cli_version": "test",
                }),
            )],
        )?;
        let mut tui = crate::tui::test_support::make_test_tui()?;

        for (cwd_mode, expected_cwd) in [
            (ResumeCwdMode::Current, current_cwd.clone()),
            (ResumeCwdMode::Session, session_cwd),
        ] {
            let outcome = resolve_cwd_for_resume_or_fork(
                &mut tui,
                &config,
                /*state_db_ctx*/ None,
                &SessionTarget {
                    path: Some(rollout_path.clone()),
                    thread_id,
                },
                CwdPromptAction::Fork,
                ResumeCwdContext {
                    current_cwd: &current_cwd,
                    remembered_current_cwd: &current_cwd,
                    allow_remember_current: true,
                    mode: Some(cwd_mode),
                },
            )
            .await?;

            assert_eq!(outcome, ResolveCwdOutcome::Continue(Some(expected_cwd)));
        }
        Ok(())
    }

    #[tokio::test]
    async fn configured_session_cwd_rejects_missing_metadata() -> color_eyre::Result<()> {
        let temp_dir = TempDir::new()?;
        let config = crate::legacy_core::config::ConfigBuilder::default()
            .codex_home(temp_dir.path().to_path_buf())
            .build()
            .await?;
        let current_cwd = config.cwd.to_path_buf();
        let mut tui = crate::tui::test_support::make_test_tui()?;

        let error = resolve_cwd_for_resume_or_fork(
            &mut tui,
            &config,
            /*state_db_ctx*/ None,
            &SessionTarget {
                path: None,
                thread_id: ThreadId::new(),
            },
            CwdPromptAction::Resume,
            ResumeCwdContext {
                current_cwd: &current_cwd,
                remembered_current_cwd: &current_cwd,
                allow_remember_current: true,
                mode: Some(ResumeCwdMode::Session),
            },
        )
        .await
        .expect_err("session mode should reject unavailable metadata");

        assert_eq!(
            error.to_string(),
            "failed to determine the working directory recorded for the selected session"
        );
        Ok(())
    }
}

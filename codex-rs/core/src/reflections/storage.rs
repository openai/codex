use std::path::Path;
use std::path::PathBuf;

use chrono::SecondsFormat;
use chrono::Utc;
use codex_analytics::CompactionTrigger;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use serde::Deserialize;
use serde::Serialize;
use tokio::io::AsyncWriteExt;

const STATE_SCHEMA: &str = "reflections.state.v1";
const TRANSCRIPT_FILE: &str = "transcript.md";
const WINDOW_DIR_WIDTH: usize = 5;
const MAX_SHARED_NOTES_PARENT_DEPTH: usize = 64;
pub(crate) const WINDOW_DIR_PATTERN: &str = "cwNNNNN";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WrittenWindow {
    pub(crate) window: String,
    pub(crate) logs_path: PathBuf,
    pub(crate) transcript_path: PathBuf,
    pub(crate) notes_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ReflectionsState {
    schema: String,
    next_window_index: u64,
    latest_window: Option<String>,
    rollout_path: PathBuf,
    pub(super) windows: Vec<WindowState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct WindowState {
    pub(super) window: String,
    pub(super) trigger: String,
    pub(super) created_at: String,
    transcript_path: PathBuf,
    pub(super) context_window_size: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) rollout_start_line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) rollout_end_line: Option<usize>,
}

pub(crate) fn sidecar_path_for_rollout(rollout_path: &Path) -> PathBuf {
    rollout_path.with_extension("reflections")
}

pub(crate) async fn ensure_sidecar_dirs(sidecar_path: &Path) -> std::io::Result<()> {
    tokio::fs::create_dir_all(sidecar_path).await?;
    tokio::fs::create_dir_all(sidecar_path.join("notes")).await?;
    tokio::fs::create_dir_all(sidecar_path.join("logs")).await
}

pub(crate) async fn resolve_reflections_shared_notes_path(
    codex_home: &Path,
    current_rollout_path: &Path,
    current_thread_id: ThreadId,
    session_source: &SessionSource,
) -> Option<PathBuf> {
    let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
        parent_thread_id, ..
    }) = session_source
    else {
        return Some(sidecar_path_for_rollout(current_rollout_path).join("shared_notes"));
    };

    let mut next_parent_thread_id = *parent_thread_id;
    let mut seen_thread_ids = std::collections::HashSet::new();
    seen_thread_ids.insert(current_thread_id.to_string());

    for _ in 0..MAX_SHARED_NOTES_PARENT_DEPTH {
        let parent_thread_id = next_parent_thread_id.to_string();
        if !seen_thread_ids.insert(parent_thread_id.clone()) {
            return None;
        }
        let parent_rollout_path =
            crate::rollout::find_thread_path_by_id_str(codex_home, &parent_thread_id)
                .await
                .ok()??;
        let parent_session_meta =
            crate::rollout::read_session_meta_line(parent_rollout_path.as_path())
                .await
                .ok()?;
        match parent_session_meta.meta.source {
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id, ..
            }) => {
                next_parent_thread_id = parent_thread_id;
            }
            SessionSource::Cli
            | SessionSource::VSCode
            | SessionSource::Exec
            | SessionSource::Mcp
            | SessionSource::Custom(_)
            | SessionSource::SubAgent(_)
            | SessionSource::Unknown => {
                return Some(sidecar_path_for_rollout(&parent_rollout_path).join("shared_notes"));
            }
        }
    }

    None
}

pub(crate) async fn write_window(
    sidecar_path: &Path,
    rollout_path: &Path,
    trigger: CompactionTrigger,
    context_window_size: Option<i64>,
    rollout_start_line: usize,
    rollout_end_line: usize,
    transcript: String,
) -> std::io::Result<WrittenWindow> {
    ensure_sidecar_dirs(sidecar_path).await?;
    let notes_path = sidecar_path.join("notes");
    let logs_path = sidecar_path.join("logs");

    let mut state = read_state(sidecar_path, rollout_path).await?;
    let window_index = allocate_window_index(&state, &logs_path).await;
    let window = window_dir_name(window_index);
    let window_path = logs_path.join(&window);
    tokio::fs::create_dir_all(&window_path).await?;
    let transcript_path = window_path.join(TRANSCRIPT_FILE);
    write_atomic(&transcript_path, transcript.as_bytes()).await?;

    let created_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    state.next_window_index = window_index.saturating_add(1);
    state.latest_window = Some(window.clone());
    state.windows.push(WindowState {
        window: window.clone(),
        trigger: trigger_label(trigger).to_string(),
        created_at,
        transcript_path: PathBuf::from("logs").join(&window).join(TRANSCRIPT_FILE),
        context_window_size,
        rollout_start_line: Some(rollout_start_line),
        rollout_end_line: Some(rollout_end_line),
    });
    write_state(sidecar_path, &state).await?;

    Ok(WrittenWindow {
        window,
        logs_path,
        transcript_path,
        notes_path,
    })
}

pub(super) async fn read_state(
    sidecar_path: &Path,
    rollout_path: &Path,
) -> std::io::Result<ReflectionsState> {
    let state_path = sidecar_path.join("state.json");
    match tokio::fs::read_to_string(&state_path).await {
        Ok(contents) => {
            let mut state: ReflectionsState = serde_json::from_str(&contents)
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
            if state.schema != STATE_SCHEMA {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unsupported Reflections state schema `{}`", state.schema),
                ));
            }
            state.rollout_path = rollout_path.to_path_buf();
            Ok(state)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(ReflectionsState {
            schema: STATE_SCHEMA.to_string(),
            next_window_index: 0,
            latest_window: None,
            rollout_path: rollout_path.to_path_buf(),
            windows: Vec::new(),
        }),
        Err(err) => Err(err),
    }
}

async fn allocate_window_index(state: &ReflectionsState, logs_path: &Path) -> u64 {
    let mut candidate = state.next_window_index;
    while tokio::fs::try_exists(logs_path.join(window_dir_name(candidate)))
        .await
        .unwrap_or(false)
    {
        candidate = candidate.saturating_add(1);
    }
    candidate
}

fn window_dir_name(index: u64) -> String {
    format!("cw{index:0WINDOW_DIR_WIDTH$}")
}

async fn write_state(sidecar_path: &Path, state: &ReflectionsState) -> std::io::Result<()> {
    let state_json = serde_json::to_vec_pretty(state)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    write_atomic(&sidecar_path.join("state.json"), &state_json).await
}

pub(super) async fn write_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let tmp_path = path.with_extension("tmp");
    let mut file = tokio::fs::File::create(&tmp_path).await?;
    file.write_all(contents).await?;
    file.flush().await?;
    drop(file);
    tokio::fs::rename(&tmp_path, path).await
}

fn trigger_label(trigger: CompactionTrigger) -> &'static str {
    match trigger {
        CompactionTrigger::Manual => "manual_compact",
        CompactionTrigger::Auto => "auto_compact",
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_reflections_shared_notes_path;
    use super::sidecar_path_for_rollout;
    use super::write_window;
    use codex_analytics::CompactionTrigger;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::RolloutItem;
    use codex_protocol::protocol::RolloutLine;
    use codex_protocol::protocol::SessionMeta;
    use codex_protocol::protocol::SessionMetaLine;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::SubAgentSource;
    use pretty_assertions::assert_eq;
    use std::path::Path;
    use std::path::PathBuf;

    #[tokio::test]
    async fn write_window_allocates_transcript_and_state() -> std::io::Result<()> {
        let temp = tempfile::tempdir()?;
        let rollout = temp.path().join("rollout-2026-04-14T00-00-00-thread.jsonl");
        let sidecar = sidecar_path_for_rollout(&rollout);

        let first = write_window(
            &sidecar,
            &rollout,
            CompactionTrigger::Manual,
            Some(98304),
            1,
            1,
            "first".to_string(),
        )
        .await?;
        let second = write_window(
            &sidecar,
            &rollout,
            CompactionTrigger::Auto,
            Some(98304),
            2,
            2,
            "second".to_string(),
        )
        .await?;

        assert_eq!(first.window, "cw00000");
        assert_eq!(second.window, "cw00001");
        assert_eq!(first.logs_path, sidecar.join("logs"));
        assert_eq!(
            tokio::fs::read_to_string(first.transcript_path).await?,
            "first"
        );
        assert!(sidecar.join("notes").is_dir());
        let state = tokio::fs::read_to_string(sidecar.join("state.json")).await?;
        assert!(state.contains("\"latest_window\": \"cw00001\""));
        assert!(state.contains("\"rollout_start_line\": 1"));
        assert!(state.contains("\"rollout_end_line\": 2"));
        Ok(())
    }

    #[tokio::test]
    async fn shared_notes_resolve_to_current_sidecar_for_root_thread() -> std::io::Result<()> {
        let temp = tempfile::tempdir()?;
        let thread_id = ThreadId::new();
        let rollout = temp.path().join(format!(
            "sessions/2026/04/16/rollout-2026-04-16T00-00-00-{thread_id}.jsonl"
        ));

        let shared_notes = resolve_reflections_shared_notes_path(
            temp.path(),
            &rollout,
            thread_id,
            &SessionSource::Cli,
        )
        .await
        .expect("root shared notes should resolve");

        assert_eq!(
            shared_notes,
            sidecar_path_for_rollout(&rollout).join("shared_notes")
        );
        Ok(())
    }

    #[tokio::test]
    async fn shared_notes_resolve_to_root_sidecar_for_descendants() -> std::io::Result<()> {
        let temp = tempfile::tempdir()?;
        let root_id = ThreadId::new();
        let child_id = ThreadId::new();
        let grandchild_id = ThreadId::new();
        let root_rollout = write_minimal_rollout(temp.path(), root_id, SessionSource::Cli)?;
        let child_rollout =
            write_minimal_rollout(temp.path(), child_id, thread_spawn_source(root_id, 1))?;
        let grandchild_rollout = temp.path().join(format!(
            "sessions/2026/04/16/rollout-2026-04-16T00-02-00-{grandchild_id}.jsonl"
        ));

        let child_shared_notes = resolve_reflections_shared_notes_path(
            temp.path(),
            &child_rollout,
            child_id,
            &thread_spawn_source(root_id, 1),
        )
        .await
        .expect("child shared notes should resolve");
        let grandchild_shared_notes = resolve_reflections_shared_notes_path(
            temp.path(),
            &grandchild_rollout,
            grandchild_id,
            &thread_spawn_source(child_id, 2),
        )
        .await
        .expect("grandchild shared notes should resolve");

        let expected = sidecar_path_for_rollout(&root_rollout).join("shared_notes");
        assert_eq!(child_shared_notes, expected);
        assert_eq!(grandchild_shared_notes, expected);
        Ok(())
    }

    #[tokio::test]
    async fn shared_notes_resolution_failure_returns_none() -> std::io::Result<()> {
        let temp = tempfile::tempdir()?;
        let current_id = ThreadId::new();
        let missing_parent_id = ThreadId::new();
        let rollout = temp.path().join("rollout.jsonl");

        let shared_notes = resolve_reflections_shared_notes_path(
            temp.path(),
            &rollout,
            current_id,
            &thread_spawn_source(missing_parent_id, 1),
        )
        .await;

        assert_eq!(shared_notes, None);
        Ok(())
    }

    #[tokio::test]
    async fn shared_notes_resolution_cycle_returns_none() -> std::io::Result<()> {
        let temp = tempfile::tempdir()?;
        let current_id = ThreadId::new();
        let parent_id = ThreadId::new();
        write_minimal_rollout(temp.path(), parent_id, thread_spawn_source(current_id, 1))?;
        let rollout = temp.path().join("rollout.jsonl");

        let shared_notes = resolve_reflections_shared_notes_path(
            temp.path(),
            &rollout,
            current_id,
            &thread_spawn_source(parent_id, 1),
        )
        .await;

        assert_eq!(shared_notes, None);
        Ok(())
    }

    fn thread_spawn_source(parent_thread_id: ThreadId, depth: i32) -> SessionSource {
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            depth,
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
        })
    }

    fn write_minimal_rollout(
        codex_home: &Path,
        thread_id: ThreadId,
        source: SessionSource,
    ) -> std::io::Result<PathBuf> {
        let sessions_dir = codex_home.join("sessions/2026/04/16");
        std::fs::create_dir_all(&sessions_dir)?;
        let rollout = sessions_dir.join(format!("rollout-2026-04-16T00-00-00-{thread_id}.jsonl"));
        let line = RolloutLine {
            timestamp: "2026-04-16T00:00:00.000Z".to_string(),
            item: RolloutItem::SessionMeta(SessionMetaLine {
                meta: SessionMeta {
                    id: thread_id,
                    forked_from_id: None,
                    timestamp: "2026-04-16T00:00:00Z".to_string(),
                    cwd: codex_home.to_path_buf(),
                    originator: "test".to_string(),
                    cli_version: "test".to_string(),
                    source,
                    agent_nickname: None,
                    agent_role: None,
                    agent_path: None,
                    model_provider: None,
                    base_instructions: None,
                    dynamic_tools: None,
                    memory_mode: None,
                },
                git: None,
            }),
        };
        std::fs::write(&rollout, format!("{}\n", serde_json::to_string(&line)?))?;
        Ok(rollout)
    }
}

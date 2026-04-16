use std::path::Path;
use std::path::PathBuf;

use chrono::SecondsFormat;
use chrono::Utc;
use codex_analytics::CompactionTrigger;
use serde::Deserialize;
use serde::Serialize;
use tokio::io::AsyncWriteExt;

const STATE_SCHEMA: &str = "reflections.state.v1";
const TRANSCRIPT_FILE: &str = "transcript.md";
const WINDOW_DIR_WIDTH: usize = 5;
pub(crate) const WINDOW_DIR_PATTERN: &str = "cwNNNNN";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WrittenWindow {
    pub(crate) window: String,
    pub(crate) logs_path: PathBuf,
    pub(crate) transcript_path: PathBuf,
    pub(crate) notes_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReflectionsState {
    schema: String,
    next_window_index: u64,
    latest_window: Option<String>,
    rollout_path: PathBuf,
    windows: Vec<WindowState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WindowState {
    window: String,
    trigger: String,
    created_at: String,
    transcript_path: PathBuf,
    context_window_size: Option<i64>,
}

pub(crate) fn sidecar_path_for_rollout(rollout_path: &Path) -> PathBuf {
    rollout_path.with_extension("reflections")
}

pub(crate) async fn ensure_sidecar_dirs(sidecar_path: &Path) -> std::io::Result<()> {
    tokio::fs::create_dir_all(sidecar_path).await?;
    tokio::fs::create_dir_all(sidecar_path.join("notes")).await?;
    tokio::fs::create_dir_all(sidecar_path.join("logs")).await
}

pub(crate) async fn write_window(
    sidecar_path: &Path,
    rollout_path: &Path,
    trigger: CompactionTrigger,
    context_window_size: Option<i64>,
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
    });
    write_state(sidecar_path, &state).await?;

    Ok(WrittenWindow {
        window,
        logs_path,
        transcript_path,
        notes_path,
    })
}

async fn read_state(sidecar_path: &Path, rollout_path: &Path) -> std::io::Result<ReflectionsState> {
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

async fn write_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
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
    use super::sidecar_path_for_rollout;
    use super::write_window;
    use codex_analytics::CompactionTrigger;

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
            "first".to_string(),
        )
        .await?;
        let second = write_window(
            &sidecar,
            &rollout,
            CompactionTrigger::Auto,
            Some(98304),
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
        Ok(())
    }
}

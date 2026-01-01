use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct EvalCaptureState {
    #[serde(default)]
    intro_dismissed: bool,
}

const STATE_FILENAME: &str = "eval_capture.json";

fn state_path(codex_home: &Path) -> PathBuf {
    codex_home.join(STATE_FILENAME)
}

pub(crate) fn should_show_eval_capture_intro(codex_home: &Path) -> bool {
    let path = state_path(codex_home);
    let Ok(contents) = std::fs::read_to_string(path) else {
        return true;
    };
    serde_json::from_str::<EvalCaptureState>(&contents)
        .ok()
        .map(|s| !s.intro_dismissed)
        .unwrap_or(true)
}

pub(crate) fn persist_eval_capture_intro_dismissed(codex_home: &Path) -> anyhow::Result<()> {
    let path = state_path(codex_home);
    let state = EvalCaptureState {
        intro_dismissed: true,
    };
    let json_line = format!("{}\n", serde_json::to_string(&state)?);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, json_line)?;
    Ok(())
}

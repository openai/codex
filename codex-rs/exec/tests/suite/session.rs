#![allow(clippy::expect_used)]

use anyhow::Context;
use assert_cmd::prelude::*;
use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use serde_json::Value;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn json_mode_emits_session_configured_event() -> anyhow::Result<()> {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        eprintln!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return Ok(());
    }

    let home = TempDir::new().context("create temp CODEX_HOME")?;
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../core/tests/cli_responses_fixture.sse");
    assert!(fixture.exists(), "fixture missing at {}", fixture.display());

    let output = Command::cargo_bin("codex-exec")
        .context("should find binary for codex-exec")?
        .current_dir(home.path())
        .env("CODEX_HOME", home.path())
        .env("OPENAI_API_KEY", "dummy")
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--json")
        .arg("--skip-git-repo-check")
        .arg("hello from json test")
        .output()
        .context("failed running codex-exec")?;

    assert!(
        output.status.success(),
        "codex-exec exited with {:?} (stderr: {})",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).context("stdout not valid UTF-8")?;
    let session_line = stdout
        .lines()
        .find(|line| line.contains("\"type\":\"session_configured\""))
        .context("session_configured line missing")?;
    let value: Value = serde_json::from_str(session_line).context("parse session line JSON")?;
    let session_id = value["msg"]["session_id"]
        .as_str()
        .context("session_id missing in event")?;
    assert!(!session_id.is_empty(), "session_id should be non-empty");
    let rollout_path = value["msg"]["rollout_path"]
        .as_str()
        .context("rollout_path missing in event")?;
    let rollout_path = Path::new(rollout_path);
    assert!(
        rollout_path.exists(),
        "rollout_path {} does not exist",
        rollout_path.display()
    );

    Ok(())
}

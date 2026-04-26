use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::thread::sleep;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use tempfile::tempdir;

#[test]
#[ignore = "requires tmux and a locally built codex binary; run with --ignored for manual resize smoke"]
fn tmux_split_preserves_fresh_session_composer_row_after_resize_reflow() -> Result<()> {
    if cfg!(windows) {
        return Ok(());
    }
    if Command::new("tmux").arg("-V").output().is_err() {
        eprintln!("skipping resize smoke because tmux is unavailable");
        return Ok(());
    }

    let repo_root = codex_utils_cargo_bin::repo_root()?;
    let codex = codex_binary(&repo_root)?;
    let codex_home = tempdir()?;
    let fixture_dir = tempdir()?;
    let fixture = fixture_dir.path().join("resize-reflow.sse");
    write_fixture(&fixture)?;
    write_config(
        codex_home.path(),
        &repo_root,
        /*terminal_resize_reflow_enabled*/ true,
    )?;
    write_auth(codex_home.path())?;

    let session_name = format!("codex-resize-reflow-smoke-{}", std::process::id());
    let _session = TmuxSession {
        name: session_name.clone(),
    };

    let prompt = "Say hi.";
    let start_output = checked_output(
        Command::new("tmux")
            .arg("new-session")
            .arg("-d")
            .arg("-P")
            .arg("-F")
            .arg("#{pane_id}")
            .arg("-x")
            .arg("120")
            .arg("-y")
            .arg("40")
            .arg("-s")
            .arg(&session_name)
            .arg("--")
            .arg("env")
            .arg(format!("CODEX_HOME={}", codex_home.path().display()))
            .arg("OPENAI_API_KEY=dummy")
            .arg(format!("CODEX_RS_SSE_FIXTURE={}", fixture.display()))
            .arg(codex)
            .arg("-c")
            .arg("analytics.enabled=false")
            .arg("--no-alt-screen")
            .arg("-C")
            .arg(&repo_root)
            .arg(prompt),
    )?;
    let codex_pane = stdout_text(&start_output).trim().to_string();
    anyhow::ensure!(!codex_pane.is_empty(), "tmux did not report a pane id");

    wait_for_capture_contains(
        &codex_pane,
        "resize reflow sentinel",
        Duration::from_secs(/*secs*/ 15),
    )?;
    wait_for_capture_contains(
        &codex_pane,
        "gpt-5.4 default",
        Duration::from_secs(/*secs*/ 15),
    )?;
    let draft = "Notice where we are here in terms of y location.";
    check(
        Command::new("tmux")
            .arg("send-keys")
            .arg("-t")
            .arg(&codex_pane)
            .arg("-l")
            .arg(draft),
    )?;
    let baseline_capture =
        wait_for_capture_contains(&codex_pane, draft, Duration::from_secs(/*secs*/ 15))?;
    let baseline_row = last_composer_row(&baseline_capture).context("composer row before split")?;
    let baseline_history_row = first_row_containing(&baseline_capture, "resize reflow sentinel")
        .context("history row before split")?;

    let split_output = checked_output(
        Command::new("tmux")
            .arg("split-window")
            .arg("-d")
            .arg("-P")
            .arg("-F")
            .arg("#{pane_id}")
            .arg("-v")
            .arg("-l")
            .arg("12")
            .arg("-t")
            .arg(&codex_pane)
            .arg("sleep")
            .arg("30"),
    )?;
    let split_pane = stdout_text(&split_output).trim().to_string();

    sleep(Duration::from_millis(/*millis*/ 250));
    let first_capture = capture_pane(&codex_pane)?;
    let first_row = last_composer_row(&first_capture).context("composer row after split")?;

    sleep(Duration::from_millis(/*millis*/ 1_000));
    let second_capture = capture_pane(&codex_pane)?;
    let second_row =
        last_composer_row(&second_capture).context("composer row after reflow wait")?;

    anyhow::ensure!(
        first_row == second_row,
        "composer row drifted after split: before={first_row}, after={second_row}\n\
         before:\n{first_capture}\n\
         after:\n{second_capture}"
    );
    anyhow::ensure!(
        second_row <= baseline_row + 1,
        "composer row snapped downward after split: baseline={baseline_row}, after={second_row}\n\
         baseline:\n{baseline_capture}\n\
         after:\n{second_capture}"
    );

    check(
        Command::new("tmux")
            .arg("kill-pane")
            .arg("-t")
            .arg(&split_pane),
    )?;

    sleep(Duration::from_millis(/*millis*/ 500));
    let final_capture = capture_pane(&codex_pane)?;
    let final_row =
        last_composer_row(&final_capture).context("composer row after closing split")?;
    anyhow::ensure!(
        final_row == baseline_row,
        "composer row drifted after closing split: baseline={baseline_row}, after={final_row}\n\
         capture:\n{final_capture}"
    );
    let final_history_row = first_row_containing(&final_capture, "resize reflow sentinel")
        .context("history row after closing split")?;
    anyhow::ensure!(
        final_history_row == baseline_history_row,
        "history row drifted after closing split: baseline={baseline_history_row}, \
         after={final_history_row}\n\
         baseline:\n{baseline_capture}\n\
         after:\n{final_capture}"
    );

    Ok(())
}

#[test]
#[ignore = "requires tmux and a locally built codex binary; run with --ignored for manual resize smoke"]
fn tmux_repeated_resizes_do_not_push_composer_down() -> Result<()> {
    if cfg!(windows) {
        return Ok(());
    }
    if Command::new("tmux").arg("-V").output().is_err() {
        eprintln!("skipping resize smoke because tmux is unavailable");
        return Ok(());
    }

    run_repeated_resize_smoke(/*terminal_resize_reflow_enabled*/ false)?;
    run_repeated_resize_smoke(/*terminal_resize_reflow_enabled*/ true)?;

    Ok(())
}

#[test]
#[ignore = "requires tmux and a locally built codex binary; run with --ignored for manual resize smoke"]
fn tmux_width_resize_restore_keeps_visible_content_anchored() -> Result<()> {
    if cfg!(windows) {
        return Ok(());
    }
    if Command::new("tmux").arg("-V").output().is_err() {
        eprintln!("skipping resize smoke because tmux is unavailable");
        return Ok(());
    }

    let repo_root = codex_utils_cargo_bin::repo_root()?;
    let codex = codex_binary(&repo_root)?;
    let codex_home = tempdir()?;
    let fixture_dir = tempdir()?;
    let fixture = fixture_dir.path().join("resize-reflow.sse");
    write_fixture(&fixture)?;
    write_config(
        codex_home.path(),
        &repo_root,
        /*terminal_resize_reflow_enabled*/ true,
    )?;
    write_auth(codex_home.path())?;

    let session_name = format!("codex-resize-width-{}", std::process::id());
    let _session = TmuxSession {
        name: session_name.clone(),
    };

    let prompt = "Send me a large paragraph of text for testing.";
    let start_output = checked_output(
        Command::new("tmux")
            .arg("new-session")
            .arg("-d")
            .arg("-P")
            .arg("-F")
            .arg("#{pane_id}")
            .arg("-x")
            .arg("120")
            .arg("-y")
            .arg("40")
            .arg("-s")
            .arg(&session_name)
            .arg("--")
            .arg("env")
            .arg(format!("CODEX_HOME={}", codex_home.path().display()))
            .arg("OPENAI_API_KEY=dummy")
            .arg(format!("CODEX_RS_SSE_FIXTURE={}", fixture.display()))
            .arg(codex)
            .arg("-c")
            .arg("analytics.enabled=false")
            .arg("--no-alt-screen")
            .arg("-C")
            .arg(&repo_root)
            .arg(prompt),
    )?;
    let codex_pane = stdout_text(&start_output).trim().to_string();
    anyhow::ensure!(!codex_pane.is_empty(), "tmux did not report a pane id");

    wait_for_capture_contains(
        &codex_pane,
        "resize reflow sentinel",
        Duration::from_secs(/*secs*/ 15),
    )?;
    wait_for_capture_contains(
        &codex_pane,
        "gpt-5.4 default",
        Duration::from_secs(/*secs*/ 15),
    )?;
    let draft = "Notice where we are here in terms of y location.";
    check(
        Command::new("tmux")
            .arg("send-keys")
            .arg("-t")
            .arg(&codex_pane)
            .arg("-l")
            .arg(draft),
    )?;
    let baseline_capture =
        wait_for_capture_contains(&codex_pane, draft, Duration::from_secs(/*secs*/ 15))?;
    let baseline_row = last_composer_row(&baseline_capture).context("composer row before split")?;
    let baseline_history_row = first_row_containing(&baseline_capture, "resize reflow sentinel")
        .context("history row before split")?;

    let split_output = checked_output(
        Command::new("tmux")
            .arg("split-window")
            .arg("-d")
            .arg("-P")
            .arg("-F")
            .arg("#{pane_id}")
            .arg("-h")
            .arg("-l")
            .arg("40")
            .arg("-t")
            .arg(&codex_pane)
            .arg("sleep")
            .arg("30"),
    )?;
    let split_pane = stdout_text(&split_output).trim().to_string();

    sleep(Duration::from_millis(/*millis*/ 750));
    check(
        Command::new("tmux")
            .arg("kill-pane")
            .arg("-t")
            .arg(&split_pane),
    )?;

    sleep(Duration::from_millis(/*millis*/ 1_000));
    let restored_capture = capture_pane(&codex_pane)?;
    let restored_row =
        last_composer_row(&restored_capture).context("composer row after width restore")?;
    let restored_history_row = first_row_containing(&restored_capture, "resize reflow sentinel")
        .context("history row after width restore")?;
    anyhow::ensure!(
        restored_row == baseline_row,
        "composer row drifted after width restore: baseline={baseline_row}, \
         restored={restored_row}\n\
         baseline:\n{baseline_capture}\n\
         restored:\n{restored_capture}"
    );
    anyhow::ensure!(
        restored_history_row == baseline_history_row,
        "history row drifted after width restore: baseline={baseline_history_row}, \
         restored={restored_history_row}\n\
         baseline:\n{baseline_capture}\n\
         restored:\n{restored_capture}"
    );

    Ok(())
}

fn run_repeated_resize_smoke(terminal_resize_reflow_enabled: bool) -> Result<()> {
    let repo_root = codex_utils_cargo_bin::repo_root()?;
    let codex = codex_binary(&repo_root)?;
    let codex_home = tempdir()?;
    let fixture_dir = tempdir()?;
    let fixture = fixture_dir.path().join("resize-reflow.sse");
    write_fixture(&fixture)?;
    write_config(
        codex_home.path(),
        &repo_root,
        terminal_resize_reflow_enabled,
    )?;
    write_auth(codex_home.path())?;

    let suffix = if terminal_resize_reflow_enabled {
        "enabled"
    } else {
        "disabled"
    };
    let session_name = format!("codex-resize-repeat-{suffix}-{}", std::process::id());
    let _session = TmuxSession {
        name: session_name.clone(),
    };

    let prompt = "Send me a large paragraph of text for testing.";
    let start_output = checked_output(
        Command::new("tmux")
            .arg("new-session")
            .arg("-d")
            .arg("-P")
            .arg("-F")
            .arg("#{pane_id}")
            .arg("-x")
            .arg("120")
            .arg("-y")
            .arg("40")
            .arg("-s")
            .arg(&session_name)
            .arg("--")
            .arg("env")
            .arg(format!("CODEX_HOME={}", codex_home.path().display()))
            .arg("OPENAI_API_KEY=dummy")
            .arg(format!("CODEX_RS_SSE_FIXTURE={}", fixture.display()))
            .arg(codex)
            .arg("-c")
            .arg("analytics.enabled=false")
            .arg("--no-alt-screen")
            .arg("-C")
            .arg(&repo_root)
            .arg(prompt),
    )?;
    let codex_pane = stdout_text(&start_output).trim().to_string();
    anyhow::ensure!(!codex_pane.is_empty(), "tmux did not report a pane id");

    wait_for_capture_contains(
        &codex_pane,
        "resize reflow sentinel",
        Duration::from_secs(/*secs*/ 15),
    )?;
    wait_for_capture_contains(
        &codex_pane,
        "gpt-5.4 default",
        Duration::from_secs(/*secs*/ 15),
    )?;
    let draft = "Notice where we are here in terms of y location.";
    check(
        Command::new("tmux")
            .arg("send-keys")
            .arg("-t")
            .arg(&codex_pane)
            .arg("-l")
            .arg(draft),
    )?;
    let baseline_capture =
        wait_for_capture_contains(&codex_pane, draft, Duration::from_secs(/*secs*/ 15))?;
    let baseline_row = last_composer_row(&baseline_capture).context("composer row before split")?;
    let baseline_history_row = first_row_containing(&baseline_capture, "resize reflow sentinel")
        .context("history row before split")?;

    for cycle in 1..=3 {
        let split_output = checked_output(
            Command::new("tmux")
                .arg("split-window")
                .arg("-d")
                .arg("-P")
                .arg("-F")
                .arg("#{pane_id}")
                .arg("-v")
                .arg("-l")
                .arg("12")
                .arg("-t")
                .arg(&codex_pane)
                .arg("sleep")
                .arg("30"),
        )?;
        let split_pane = stdout_text(&split_output).trim().to_string();

        sleep(Duration::from_millis(/*millis*/ 250));
        check(
            Command::new("tmux")
                .arg("kill-pane")
                .arg("-t")
                .arg(&split_pane),
        )?;

        sleep(Duration::from_millis(/*millis*/ 500));
        let restored_capture = capture_pane(&codex_pane)?;
        let restored_row = last_composer_row(&restored_capture)
            .with_context(|| format!("composer row after resize cycle {cycle}"))?;
        let restored_history_row =
            first_row_containing(&restored_capture, "resize reflow sentinel")
                .with_context(|| format!("history row after resize cycle {cycle}"))?;
        if terminal_resize_reflow_enabled {
            anyhow::ensure!(
                restored_row == baseline_row,
                "composer row drifted after resize cycle {cycle} with terminal_resize_reflow={terminal_resize_reflow_enabled}: \
                 baseline={baseline_row}, restored={restored_row}\n\
                 baseline:\n{baseline_capture}\n\
                 restored:\n{restored_capture}"
            );
            anyhow::ensure!(
                restored_history_row == baseline_history_row,
                "history row drifted after resize cycle {cycle} with terminal_resize_reflow={terminal_resize_reflow_enabled}: \
                 baseline={baseline_history_row}, restored={restored_history_row}\n\
                 baseline:\n{baseline_capture}\n\
                 restored:\n{restored_capture}"
            );
        } else {
            anyhow::ensure!(
                restored_row <= baseline_row + 1,
                "composer row snapped downward after resize cycle {cycle} with terminal_resize_reflow={terminal_resize_reflow_enabled}: \
                 baseline={baseline_row}, restored={restored_row}\n\
                 baseline:\n{baseline_capture}\n\
                 restored:\n{restored_capture}"
            );
        }
    }

    Ok(())
}

struct TmuxSession {
    name: String,
}

impl Drop for TmuxSession {
    fn drop(&mut self) {
        let _ = Command::new("tmux")
            .arg("kill-session")
            .arg("-t")
            .arg(&self.name)
            .output();
    }
}

fn codex_binary(repo_root: &Path) -> Result<PathBuf> {
    if let Ok(path) = codex_utils_cargo_bin::cargo_bin("codex") {
        return Ok(path);
    }

    let fallback = repo_root.join("codex-rs/target/debug/codex");
    anyhow::ensure!(
        fallback.is_file(),
        "codex binary is unavailable; run `cargo build -p codex-cli` first"
    );
    Ok(fallback)
}

fn write_config(
    codex_home: &Path,
    repo_root: &Path,
    terminal_resize_reflow_enabled: bool,
) -> Result<()> {
    let repo_root_display = repo_root.display();
    let config = format!(
        r#"model = "gpt-5.4"
model_provider = "openai"
suppress_unstable_features_warning = true

[features]
terminal_resize_reflow = {terminal_resize_reflow_enabled}

[projects."{repo_root_display}"]
trust_level = "trusted"
"#
    );
    std::fs::write(codex_home.join("config.toml"), config)?;
    Ok(())
}

fn write_auth(codex_home: &Path) -> Result<()> {
    std::fs::write(
        codex_home.join("auth.json"),
        r#"{"OPENAI_API_KEY":"dummy","tokens":null,"last_refresh":null}"#,
    )?;
    Ok(())
}

fn write_fixture(path: &Path) -> Result<()> {
    let text = "resize reflow sentinel says hi. This paragraph is intentionally long enough to exercise terminal wrapping, scrollback redraw, and pane resize behavior without requiring a live model response. It includes enough ordinary prose to wrap across several rows in a narrow tmux pane, then keep going so repeated split and restore cycles have visible history above the composer. If a resize path accidentally inserts blank rows or anchors the viewport lower on each pass, the composer row will drift after the pane returns to its original height.";
    let created = serde_json::json!({
        "type": "response.created",
        "response": { "id": "resp-resize-smoke" },
    });
    let done = serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "type": "message",
            "role": "assistant",
            "content": [
                { "type": "output_text", "text": text }
            ],
        },
    });
    let completed = serde_json::json!({
        "type": "response.completed",
        "response": { "id": "resp-resize-smoke", "output": [] },
    });
    let fixture = format!(
        "event: response.created\ndata: {created}\n\n\
         event: response.output_item.done\ndata: {done}\n\n\
         event: response.completed\ndata: {completed}\n\n"
    );
    std::fs::write(path, fixture)?;
    Ok(())
}

fn wait_for_capture_contains(pane: &str, needle: &str, timeout: Duration) -> Result<String> {
    let deadline = Instant::now() + timeout;
    let mut last_capture = String::new();
    while Instant::now() < deadline {
        last_capture = capture_pane(pane)?;
        if last_capture.contains(needle) {
            return Ok(last_capture);
        }
        sleep(Duration::from_millis(/*millis*/ 100));
    }

    anyhow::bail!("timed out waiting for {needle:?}; last capture:\n{last_capture}");
}

fn capture_pane(pane: &str) -> Result<String> {
    let output = output(
        Command::new("tmux")
            .arg("capture-pane")
            .arg("-p")
            .arg("-t")
            .arg(pane),
    )?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn last_composer_row(capture: &str) -> Option<usize> {
    capture
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            if line.trim_start().starts_with('\u{203a}') {
                Some(index)
            } else {
                None
            }
        })
        .last()
}

fn first_row_containing(capture: &str, needle: &str) -> Option<usize> {
    capture
        .lines()
        .enumerate()
        .find_map(|(index, line)| line.contains(needle).then_some(index))
}

fn check(command: &mut Command) -> Result<()> {
    checked_output(command)?;
    Ok(())
}

fn checked_output(command: &mut Command) -> Result<Output> {
    let output = output(command)?;
    anyhow::ensure!(
        output.status.success(),
        "command failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output)
}

fn output(command: &mut Command) -> Result<Output> {
    command
        .output()
        .with_context(|| format!("failed to run {command:?}"))
}

fn stdout_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

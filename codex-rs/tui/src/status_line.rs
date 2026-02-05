//! Status line runner for the TUI.
//!
//! This module executes an external command (configured via `tui_status_line`) to generate
//! a customizable status line. The command receives a JSON payload via stdin containing
//! session state (model, conversation stats, rate limits, etc.) and should output a single
//! line to stdout, optionally with ANSI escape codes for styling.
//!
//! Key behaviors:
//! - Request coalescing: only one command runs at a time; if updates arrive during execution,
//!   the command reruns once with the latest payload after completing.
//! - Timeout handling: commands that exceed the fixed timeout are killed and
//!   a warning is emitted (once per session).
//! - Error resilience: failures are logged but don't crash the TUI; the last successful
//!   output continues to display.

use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use codex_ansi_escape::ansi_escape_line;
use codex_core::config::Config;
use ratatui::text::Line;
use ratatui::text::Span;
use tokio::io::AsyncWriteExt;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

#[derive(Debug, Clone)]
pub(crate) struct StatusLineRunner {
    config: Config,
    state: Arc<Mutex<StatusLine>>,
    app_tx: AppEventSender,
}

impl StatusLineRunner {
    pub(crate) fn new(config: Config, app_tx: AppEventSender) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(StatusLine::default())),
            app_tx,
        }
    }

    pub(crate) fn update_payload(&self, payload: String) -> anyhow::Result<()> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.latest_payload = Some(payload);
        Ok(())
    }

    pub(crate) fn request_update(&self) -> anyhow::Result<()> {
        let Some(command) = self
            .config
            .tui_status_line
            .as_ref()
            .filter(|cmd| !cmd.is_empty())
            .cloned()
        else {
            return Ok(());
        };
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.latest_payload.is_none() {
                return Ok(());
            }
            if state.in_flight {
                state.pending = true;
                return Ok(());
            }
            state.in_flight = true;
        }

        let timeout = DEFAULT_STATUS_LINE_TIMEOUT;
        let config = self.config.clone();
        let state = self.state.clone();
        let app_tx = self.app_tx.clone();
        let run = async move {
            loop {
                let wait_for = {
                    let state = state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if state.latest_payload.is_none() {
                        break;
                    }
                    state.last_started_at.and_then(|last| {
                        let elapsed = last.elapsed();
                        if elapsed >= MIN_STATUS_LINE_INTERVAL {
                            None
                        } else {
                            Some(MIN_STATUS_LINE_INTERVAL - elapsed)
                        }
                    })
                };
                if let Some(wait_for) = wait_for {
                    tokio::time::sleep(wait_for).await;
                }
                let payload = {
                    let state = state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    match state.latest_payload.clone() {
                        Some(payload) => payload,
                        None => break,
                    }
                };
                {
                    let mut state = state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    state.last_started_at = Some(Instant::now());
                }
                let request = StatusLineRequest {
                    command: command.clone(),
                    payload,
                    cwd: config.cwd.clone(),
                    timeout,
                };
                let result = run_request(&request).await;
                let mut update = None;
                let mut rerun = false;
                let mut emit_timeout_warning = None;
                let mut emit_error_warning = None;

                {
                    let mut state = state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    match result {
                        Ok(line) => {
                            let parsed = ansi_escape_line(&line);
                            let status_line_value = StatusLineValue {
                                text: line,
                                spans: Some(parsed.spans),
                            };
                            state.latest = Some(status_line_value.clone());
                            state.last_error = None;
                            state.last_updated_at = Some(Instant::now());
                            update = Some(status_line_value);
                        }
                        Err(err) => {
                            tracing::warn!("status line execution failed: {}", err);
                            if err == TIMEOUT_ERR && !state.warned_timeout {
                                state.warned_timeout = true;
                                tracing::warn!(
                                    "status line command timed out. Consider optimizing the command."
                                );
                                emit_timeout_warning = Some(AppEvent::StatusLineTimeoutWarning {
                                    timeout_ms: request.timeout.as_millis() as u64,
                                });
                            } else if err != TIMEOUT_ERR && !state.warned_error {
                                state.warned_error = true;
                                emit_error_warning = Some(AppEvent::StatusLineErrorWarning {
                                    message: err.clone(),
                                });
                            }
                            state.last_error = Some(err);
                        }
                    }

                    if state.pending {
                        state.pending = false;
                        rerun = true;
                    } else {
                        state.in_flight = false;
                    }
                }

                if let Some(event) = emit_timeout_warning {
                    app_tx.send(event);
                }

                if let Some(event) = emit_error_warning {
                    app_tx.send(event);
                }

                if let Some(status_line_value) = update {
                    app_tx.send(AppEvent::StatusLineUpdated(status_line_value));
                }

                if rerun {
                    continue;
                }
                break;
            }
        };

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(run);
        } else {
            std::thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                let Ok(runtime) = runtime else {
                    tracing::error!("status line runtime failed to build");
                    return;
                };
                runtime.block_on(run);
            });
        }
        Ok(())
    }
}

const TIMEOUT_ERR: &str = "status line command timed out";
const DEFAULT_STATUS_LINE_TIMEOUT: Duration = Duration::from_millis(500);
const MIN_STATUS_LINE_INTERVAL: Duration = Duration::from_millis(300);

#[derive(Debug, Clone)]
pub(crate) struct StatusLineRequest {
    pub command: Vec<String>,
    pub payload: String,
    pub cwd: std::path::PathBuf,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Default)]
struct StatusLine {
    latest: Option<StatusLineValue>,
    last_updated_at: Option<Instant>,
    last_error: Option<String>,
    latest_payload: Option<String>,
    /// guards against concurrent runs
    in_flight: bool,
    /// refresh requested while in flight
    pending: bool,
    /// whether a timeout warning has been emitted
    warned_timeout: bool,
    /// whether an error warning has been emitted
    warned_error: bool,
    /// last time a status line command was started
    last_started_at: Option<Instant>,
}

#[derive(Debug, Clone)]
pub(crate) struct StatusLineValue {
    pub text: String,
    pub spans: Option<Vec<Span<'static>>>,
}

impl StatusLineValue {
    pub(crate) fn as_line(&self) -> Line<'static> {
        if let Some(spans) = &self.spans {
            Line::from(spans.clone())
        } else {
            Line::from(self.text.clone())
        }
    }
}

async fn run_request(request: &StatusLineRequest) -> Result<String, String> {
    let Some(program) = request.command.first() else {
        return Err("status line command is empty".to_string());
    };

    let mut cmd = tokio::process::Command::new(program);
    if request.command.len() > 1 {
        cmd.args(&request.command[1..]);
    }
    cmd.current_dir(&request.cwd);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);

    let mut child = cmd
        .spawn()
        .map_err(|err| format!("failed to spawn status line command: {err}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(request.payload.as_bytes())
            .await
            .map_err(|err| format!("failed to write status line payload: {err}"))?;
        stdin
            .shutdown()
            .await
            .map_err(|err| format!("failed to shutdown status line stdin: {err}"))?;
    }

    let output = tokio::time::timeout(request.timeout, child.wait_with_output())
        .await
        .map_err(|_| TIMEOUT_ERR.to_string())?
        .map_err(|err| format!("failed to read status line output: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("status line command failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next().unwrap_or("").trim_end().to_string();
    Ok(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use codex_core::config::ConfigBuilder;
    #[cfg(unix)]
    use pretty_assertions::assert_eq;
    #[cfg(unix)]
    use tempfile::tempdir;
    #[cfg(unix)]
    use tokio::sync::mpsc::unbounded_channel;
    #[cfg(unix)]
    use tokio::time::timeout;

    #[cfg(unix)]
    fn request(command: &[&str], payload: &str) -> StatusLineRequest {
        StatusLineRequest {
            command: command.iter().map(|v| (*v).to_string()).collect(),
            payload: payload.to_string(),
            cwd: std::env::current_dir().expect("cwd"),
            timeout: Duration::from_millis(200),
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_request_returns_first_line_from_stdout() {
        let req = request(&["/bin/sh", "-c", "cat"], "first\nsecond\n");
        let got = run_request(&req).await.expect("run_request");
        assert_eq!(got, "first");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_request_errors_on_non_zero_exit() {
        let req = request(&["/bin/sh", "-c", "echo fail 1>&2; exit 2"], "");
        let err = run_request(&req).await.expect_err("expected error");
        assert!(err.contains("status line command failed"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn request_update_emits_error_warning_on_failure() {
        let temp_home = tempdir().expect("temp home");
        let mut config = ConfigBuilder::default()
            .codex_home(temp_home.path().to_path_buf())
            .build()
            .await
            .expect("config");

        config.tui_status_line = Some(vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "cat >/dev/null; echo fail 1>&2; exit 2".to_string(),
        ]);

        let (tx, mut rx) = unbounded_channel();
        let runner = StatusLineRunner::new(config, AppEventSender::new(tx));

        runner
            .update_payload("{\"ok\":true}".to_string())
            .expect("payload");
        runner.request_update().expect("request update");

        let event = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout waiting for status line event")
            .expect("missing event");

        match event {
            AppEvent::StatusLineErrorWarning { message } => {
                assert!(message.contains("status line command failed"));
            }
            other => panic!("expected StatusLineErrorWarning, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn request_update_throttles_status_line_runs() {
        let temp_home = tempdir().expect("temp home");
        let mut config = ConfigBuilder::default()
            .codex_home(temp_home.path().to_path_buf())
            .build()
            .await
            .expect("config");

        config.tui_status_line = Some(vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "cat".to_string(),
        ]);

        let (tx, mut rx) = unbounded_channel();
        let runner = StatusLineRunner::new(config, AppEventSender::new(tx));

        let start = Instant::now();
        runner.update_payload("first".to_string()).expect("payload");
        runner.request_update().expect("request update");
        runner
            .update_payload("second".to_string())
            .expect("payload");
        runner.request_update().expect("request update");

        let first = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout waiting for first status line update")
            .expect("missing event");
        match first {
            AppEvent::StatusLineUpdated(_value) => {}
            other => panic!("expected StatusLineUpdated, got {other:?}"),
        }

        let second = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout waiting for second status line update")
            .expect("missing event");
        match second {
            AppEvent::StatusLineUpdated(_value) => {}
            other => panic!("expected StatusLineUpdated, got {other:?}"),
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed >= MIN_STATUS_LINE_INTERVAL,
            "status line throttle elapsed too short: {elapsed:?}"
        );
    }
}

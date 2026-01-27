use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use codex_core::config::Config;
use ratatui::text::Span;
use tokio::io::AsyncWriteExt;

use crate::app_event_sender::AppEventSender;
use crate::chatwidget::ChatWidget;

#[derive(Debug, Clone)]
pub(crate) struct StatusLineRunner {
    config: Config,
    state: Arc<Mutex<StatusLine>>,
    app_rx: AppEventSender,
}

impl StatusLineRunner {
    pub(crate) fn new(config: Config, app_rx: AppEventSender) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(StatusLine::default())),
            app_rx,
        }
    }

    pub(crate) fn update_payload(&self, payload: String) -> anyhow::Result<()> {
        let mut state = self.state.lock().expect("status line lock poisoned");
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
            let mut state = self.state.lock().expect("status line lock poisoned");
            if state.latest_payload.is_none() {
                return Ok(());
            }
            if state.in_flight {
                state.pending = true;
                return Ok(());
            }
            state.in_flight = true;
        }

        let state = self.state.clone();
        let app_rx = self.app_rx.clone();
        let run = async move {
            loop {
                let payload = {
                    let state = state.lock().expect("status line lock poisoned");
                    match state.latest_payload.clone() {
                        Some(payload) => payload,
                        None => break,
                    }
                };
                let request = StatusLineRequest {
                    command: command.clone(),
                    payload,
                    timeout: DEFAULT_STATUS_LINE_TIMEOUT,
                };
                let result = run_request(&request).await;
                let mut state = state.lock().expect("status line lock poisoned");
                match result {
                    Ok(line) => {
                        state.latest = Some(StatusLineValue {
                            text: line,
                            spans: None,
                        });
                        state.last_error = None;
                        state.last_updated_at = Some(Instant::now());
                    }
                    Err(err) => {
                        state.last_error = Some(err);
                    }
                }

                // TODO: emit a redraw/status-line-updated event when the UI is wired up.
                let _ = &app_rx;

                if state.pending {
                    state.pending = false;
                    continue;
                }
                state.in_flight = false;
                break;
            }
        };

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(run);
        } else {
            std::thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("status line runtime");
                runtime.block_on(run);
            });
        }
        Ok(())
    }
}

const DEFAULT_STATUS_LINE_TIMEOUT: Duration = Duration::from_millis(300);

#[derive(Debug, Clone)]
pub(crate) struct StatusLineRequest {
    pub command: Vec<String>,
    pub payload: String,
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
    // nice to haves
    //  - last_error: Option<String>
    //      - Script failure/timeout; used for fallback or logs.
    //  - last_exit_status: Option<i32>
    //      - Helpful for diagnostics.
    //  - last_run_duration: Option<Duration>
    //      - For metrics + timeout tuning.
    //  - timeout_count: u64 / error_count: u64
    //      - Backoff decisions.
    //  - last_payload_hash: Option<u64>
    //      - Skip runs when payload hasn’t changed.
    //  - last_trigger: Option<StatusLineTrigger>
    //      - Which event caused the refresh (token update, cwd change, etc.)
}

#[derive(Debug, Clone)]
pub(crate) struct StatusLineValue {
    pub text: String,
    pub spans: Option<Vec<Span<'static>>>,
}

async fn run_request(request: &StatusLineRequest) -> Result<String, String> {
    let Some(program) = request.command.first() else {
        return Err("status line command is empty".to_string());
    };

    let mut cmd = tokio::process::Command::new(program);
    if request.command.len() > 1 {
        cmd.args(&request.command[1..]);
    }
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|err| format!("failed to spawn status line command: {err}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(request.payload.as_bytes())
            .await
            .map_err(|err| format!("failed to write status line payload: {err}"))?;
    }

    let output = tokio::time::timeout(request.timeout, child.wait_with_output())
        .await
        .map_err(|_| "status line command timed out".to_string())?
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
    use pretty_assertions::assert_eq;

    fn request(command: &[&str], payload: &str) -> StatusLineRequest {
        StatusLineRequest {
            command: command.iter().map(|v| (*v).to_string()).collect(),
            payload: payload.to_string(),
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
}

use std::io;
use std::io::IsTerminal;
use std::io::Write;
use std::sync::Mutex;
use std::time::Duration;

use super::CheckStatus;

/// Receives check lifecycle events while doctor builds the final report.
///
/// Progress implementations must not write to stdout. The final report owns
/// stdout so JSON and redirected human reports stay clean.
pub(super) trait DoctorProgress: Send + Sync {
    fn begin(&self, label: &'static str);
    fn heartbeat(&self, label: &'static str, elapsed: Duration);
    fn finish(&self, label: &'static str, status: CheckStatus);
    fn settle(&self);
}

/// Selects the progress implementation for the current output mode.
///
/// JSON output is always quiet so stdout remains valid JSON. Human output uses a
/// transient stderr line only for interactive terminals, then clears it before
/// the final report is printed.
pub(super) fn doctor_progress(json: bool) -> std::sync::Arc<dyn DoctorProgress> {
    if should_show_progress(
        json,
        std::env::var("TERM").ok().as_deref(),
        io::stderr().is_terminal(),
    ) {
        std::sync::Arc::new(StderrProgress::default())
    } else {
        std::sync::Arc::new(QuietProgress)
    }
}

fn should_show_progress(json: bool, term: Option<&str>, stderr_is_tty: bool) -> bool {
    !json && stderr_is_tty && term != Some("dumb")
}

struct QuietProgress;

impl DoctorProgress for QuietProgress {
    fn begin(&self, _label: &'static str) {}

    fn heartbeat(&self, _label: &'static str, _elapsed: Duration) {}

    fn finish(&self, _label: &'static str, _status: CheckStatus) {}

    fn settle(&self) {}
}

#[derive(Default)]
struct StderrProgress {
    state: Mutex<StderrProgressState>,
}

#[derive(Default)]
struct StderrProgressState {
    wrote_line: bool,
}

impl StderrProgress {
    fn render(&self, message: String) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let mut stderr = io::stderr().lock();
        let _ = write!(stderr, "\r\x1b[2K{message}");
        let _ = stderr.flush();
        state.wrote_line = true;
    }
}

impl DoctorProgress for StderrProgress {
    fn begin(&self, label: &'static str) {
        self.render(format!("Checking {label}..."));
    }

    fn heartbeat(&self, label: &'static str, elapsed: Duration) {
        self.render(format!("Still checking {label}... {}s", elapsed.as_secs()));
    }

    fn finish(&self, _label: &'static str, _status: CheckStatus) {}

    fn settle(&self) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if !state.wrote_line {
            return;
        }
        let mut stderr = io::stderr().lock();
        let _ = write!(stderr, "\r\x1b[2K");
        let _ = stderr.flush();
        state.wrote_line = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_is_quiet_for_json() {
        assert!(!should_show_progress(
            /*json*/ true,
            Some("xterm-256color"),
            /*stderr_is_tty*/ true,
        ));
    }

    #[test]
    fn progress_is_quiet_for_non_tty() {
        assert!(!should_show_progress(
            /*json*/ false,
            Some("xterm-256color"),
            /*stderr_is_tty*/ false,
        ));
    }

    #[test]
    fn progress_is_quiet_for_dumb_terminal() {
        assert!(!should_show_progress(
            /*json*/ false,
            Some("dumb"),
            /*stderr_is_tty*/ true,
        ));
    }

    #[test]
    fn progress_is_shown_for_human_tty_output() {
        assert!(should_show_progress(
            /*json*/ false,
            Some("xterm-256color"),
            /*stderr_is_tty*/ true,
        ));
    }
}

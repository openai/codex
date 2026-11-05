//! Probe the containing tmux session for input settings and pane-local graphics support.
//!
//! Mouse capture is suppressed only when the containing session explicitly reports mouse off.
//! Missing tmux or an inconclusive probe preserves the normal input policy.

use std::io::Read;
use std::io::Seek;
use std::process::Command;
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;
use std::time::Instant;

use itertools::Itertools;

const PROBE_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 1);
const PROBE_POLL_INTERVAL: Duration = Duration::from_millis(/*millis*/ 10);
const MAX_PROBE_OUTPUT_BYTES: u64 = 4096;
static PROBE_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// Capture policy for the current terminal setup, shared by fullscreen and overlays.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum MouseCapture {
    #[default]
    Enabled,
    DisabledByTmux,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Options {
    pub(super) extended_keys_format: Option<String>,
    pub(super) mouse_capture: MouseCapture,
    pub(super) kitty_image_base: Option<u32>,
}

pub(super) fn options() -> Options {
    let pane = std::env::var("TMUX_PANE").ok();
    if std::env::var_os("TMUX").is_none() && pane.is_none() {
        return Options::default();
    }
    let Some(executable) = codex_utils_path::system_executable("tmux") else {
        return Options::default();
    };
    let Ok(path) = codex_utils_path::system_path() else {
        return Options::default();
    };
    if PROBE_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        return Options::default();
    }
    let deadline = Instant::now() + PROBE_TIMEOUT;
    let (sender, receiver) = mpsc::sync_channel(1);
    if std::thread::Builder::new()
        .name("tmux-options".into())
        .spawn(move || {
            let options = read_options(pane.as_deref(), |args| {
                let mut command = Command::new(&executable);
                command
                    .env("PATH", &path)
                    .args(args)
                    .stdin(Stdio::null())
                    .stderr(Stdio::null());
                command_stdout_before_deadline(&mut command, deadline)
            });
            PROBE_IN_FLIGHT.store(false, Ordering::Release);
            let _ = sender.send(options);
        })
        .is_err()
    {
        PROBE_IN_FLIGHT.store(false, Ordering::Release);
        return Options::default();
    }

    receiver
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .unwrap_or_default()
}

fn command_stdout_before_deadline(command: &mut Command, deadline: Instant) -> Option<Vec<u8>> {
    if Instant::now() >= deadline {
        return None;
    }
    let mut stdout = tempfile::tempfile().ok()?;
    command.stdout(stdout.try_clone().ok()?);
    let mut child = command.spawn().ok()?;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                stdout.rewind().ok()?;
                let mut output = Vec::new();
                stdout
                    .take(MAX_PROBE_OUTPUT_BYTES)
                    .read_to_end(&mut output)
                    .ok()?;
                return Some(output);
            }
            Ok(None) if !remaining.is_zero() => {
                std::thread::sleep(remaining.min(PROBE_POLL_INTERVAL));
            }
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

const OPTIONS_FORMAT: &str = concat!(
    "#{extended-keys-format}\t#{mouse}\t#{allow-passthrough}\t",
    "#{client_termname}\t#{client_termtype}\t#{client_termfeatures}",
);

fn read_options(pane: Option<&str>, mut run: impl FnMut(&[&str]) -> Option<Vec<u8>>) -> Options {
    let mut args = vec!["display-message", "-p"];
    if let Some(pane) = pane {
        args.extend(["-t", pane]);
    }
    args.push(OPTIONS_FORMAT);
    let mut options = Options::default();
    if let Some(output) = run(&args).and_then(|output| String::from_utf8(output).ok())
        && let Some((format, rest)) = output.trim_end_matches(['\r', '\n']).split_once('\t')
    {
        let mut fields = rest.split('\t');
        let mouse = fields.next().unwrap_or_default();
        options.extended_keys_format =
            (!format.trim().is_empty()).then(|| format.trim().to_owned());
        if matches!(mouse.trim(), "0" | "off") {
            options.mouse_capture = MouseCapture::DisabledByTmux;
        }
        // tmux drops uploads from hidden panes unless passthrough is set to all.
        if let Some(("all", term, kind, features)) = fields.next_tuple()
            && (matches!(term, "xterm-ghostty" | "xterm-kitty")
                || kind.starts_with("ghostty ")
                || kind.starts_with("kitty("))
            && features.split(',').any(|feature| feature == "RGB")
        {
            // Each pane owns 256 IDs in the upper half of the 24-bit placeholder color space.
            options.kitty_image_base = pane
                .and_then(|p| p.strip_prefix('%'))
                .and_then(|p| p.parse::<u32>().ok())
                .filter(|id| *id < 0x8000)
                .map(|id| 0x80_0000 + (id << 8));
        }
    }
    // Preserve keyboard compatibility with tmux versions that cannot expand the format.
    if options.extended_keys_format.is_none() {
        options.extended_keys_format = run(&["show-options", "-gqv", "extended-keys-format"])
            .and_then(|output| String::from_utf8(output).ok())
            .map(|output| output.trim().to_owned())
            .filter(|output| !output.is_empty());
    }
    options
}

#[cfg(test)]
#[path = "tmux_tests.rs"]
mod tests;

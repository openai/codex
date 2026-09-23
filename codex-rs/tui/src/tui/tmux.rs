//! Read tmux's current input settings with one probe shared by keyboard and mouse setup.
//!
//! Mouse capture is suppressed only when the containing session explicitly reports mouse off.
//! Missing tmux or an inconclusive probe preserves the normal input policy.

use std::process::Command;
use std::process::Stdio;

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
    read_options(pane.as_deref(), |args| {
        let output = Command::new(&executable)
            .env("PATH", &path)
            .args(args)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        output.status.success().then_some(output.stdout)
    })
}

fn read_options(pane: Option<&str>, mut run: impl FnMut(&[&str]) -> Option<Vec<u8>>) -> Options {
    let mut args = vec!["display-message", "-p"];
    if let Some(pane) = pane {
        args.extend(["-t", pane]);
    }
    args.push("#{extended-keys-format}\t#{mouse}");
    let mut options = Options::default();
    if let Some(output) = run(&args).and_then(|output| String::from_utf8(output).ok())
        && let Some((format, mouse)) = output.trim_end_matches(['\r', '\n']).split_once('\t')
    {
        options.extended_keys_format =
            (!format.trim().is_empty()).then(|| format.trim().to_owned());
        if matches!(mouse.trim(), "0" | "off") {
            options.mouse_capture = MouseCapture::DisabledByTmux;
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

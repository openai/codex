//! OSC 21337 tab-status output helpers for the TUI.
//!
//! Callers decide when the tab status changes; this module formats activity
//! detail and owns the low-level terminal write path. OSC 21337 values escape
//! `;` and `\`, and every field is emitted on every transition so iTerm clears
//! stale values.

use std::fmt;
use std::io;
use std::io::IsTerminal;
use std::io::stdout;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_protocol::parse_command::ParsedCommand;
use crossterm::Command;
use ratatui::crossterm::execute;

static EMITTED: AtomicBool = AtomicBool::new(/*v*/ false);
const MAX_TAB_STATUS_DETAIL_CHARS: usize = 200;
const MAX_UNKNOWN_COMMAND_CHARS: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TabStatus {
    Working,
    Waiting,
    Idle,
}

impl TabStatus {
    fn label(self) -> &'static str {
        match self {
            TabStatus::Working => "Working",
            TabStatus::Waiting => "Waiting",
            TabStatus::Idle => "Idle",
        }
    }

    fn indicator(self) -> &'static str {
        match self {
            TabStatus::Working => "#ff9500",
            TabStatus::Waiting => "#5f87ff",
            TabStatus::Idle => "#00d75f",
        }
    }

    fn text_color(self) -> &'static str {
        match self {
            TabStatus::Working => "#ff9500",
            TabStatus::Waiting => "#5f87ff",
            TabStatus::Idle => "#888888",
        }
    }
}

pub(crate) fn format_command_for_tab_status(argv: &[String]) -> String {
    crate::exec_command::strip_bash_lc_and_escape(argv)
}

pub(crate) fn format_parsed_command_for_tab_status(parsed: &[ParsedCommand]) -> Option<String> {
    if parsed.is_empty() {
        return None;
    }

    if parsed
        .iter()
        .all(|command| matches!(command, ParsedCommand::Read { .. }))
    {
        let names = parsed
            .iter()
            .filter_map(|command| match command {
                ParsedCommand::Read { name, .. } => Some(name.as_str()),
                ParsedCommand::ListFiles { .. }
                | ParsedCommand::Search { .. }
                | ParsedCommand::Unknown { .. } => None,
            })
            .collect::<Vec<_>>();
        let (head, rest) = names.split_at(names.len().min(/*other*/ 3));
        let mut summary = format!("Read {}", head.join(", "));
        if !rest.is_empty() {
            summary.push_str(", …");
        }
        return Some(summary);
    }

    match &parsed[0] {
        ParsedCommand::Read { name, .. } => Some(format!("Read {name}")),
        ParsedCommand::ListFiles { path, .. } => {
            Some(format!("List {}", path.as_deref().unwrap_or(".")))
        }
        ParsedCommand::Search { query, path, cmd } => Some(match (query, path) {
            (Some(query), Some(path)) => format!("Search \"{query}\" in {path}"),
            (Some(query), None) => format!("Search \"{query}\""),
            (None, Some(path)) => format!("Search in {path}"),
            (None, None) => format!("Run {}", oneline_truncated(cmd)),
        }),
        ParsedCommand::Unknown { cmd } => Some(format!("Run {}", oneline_truncated(cmd))),
    }
}

fn oneline_truncated(value: &str) -> String {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= MAX_UNKNOWN_COMMAND_CHARS {
        return collapsed;
    }
    format!(
        "{}…",
        collapsed
            .chars()
            .take(MAX_UNKNOWN_COMMAND_CHARS)
            .collect::<String>()
    )
}

pub(crate) fn set_tab_status(status: TabStatus, detail: Option<&str>) -> io::Result<()> {
    if !stdout().is_terminal() {
        return Ok(());
    }
    execute!(stdout(), SetTabStatus(status, detail.map(sanitize_detail)))?;
    EMITTED.store(/*val*/ true, Ordering::Relaxed);
    Ok(())
}

pub(crate) fn clear_tab_status() -> io::Result<()> {
    if !stdout().is_terminal() || !EMITTED.load(Ordering::Relaxed) {
        return Ok(());
    }
    execute!(stdout(), ClearTabStatus)?;
    EMITTED.store(/*val*/ false, Ordering::Relaxed);
    Ok(())
}

fn sanitize_detail(detail: &str) -> String {
    let mut out = String::with_capacity(detail.len());
    let mut chars_written = 0;
    let mut pending_space = false;
    let mut truncated = false;

    for ch in detail.chars() {
        if ch.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if crate::osc_text::is_disallowed_osc_text_char(ch) {
            continue;
        }
        if pending_space {
            if chars_written >= MAX_TAB_STATUS_DETAIL_CHARS {
                truncated = true;
                break;
            }
            out.push(' ');
            chars_written += 1;
            pending_space = false;
        }
        if chars_written >= MAX_TAB_STATUS_DETAIL_CHARS {
            truncated = true;
            break;
        }
        if matches!(ch, ';' | '\\') {
            out.push('\\');
        }
        out.push(ch);
        chars_written += 1;
    }
    if truncated {
        out.push('…');
    }
    out
}

#[derive(Debug, Clone)]
struct SetTabStatus(TabStatus, Option<String>);

impl Command for SetTabStatus {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        let detail = self.1.as_deref().unwrap_or("");
        write!(
            f,
            "\x1b]21337;status={};indicator={};status-color={};detail={}\x07",
            self.0.label(),
            self.0.indicator(),
            self.0.text_color(),
            detail
        )
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(std::io::Error::other(
            "tried to execute SetTabStatus using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy)]
struct ClearTabStatus;

impl Command for ClearTabStatus {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b]21337;status=;indicator=;status-color=;detail=\x07")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(std::io::Error::other(
            "tried to execute ClearTabStatus using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

#[cfg(test)]
#[path = "tab_status_tests.rs"]
mod tests;

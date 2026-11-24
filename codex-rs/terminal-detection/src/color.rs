//! Resolve stdout color depth and terminal-specific fallbacks without probing the terminal.

use super::Environment;
use super::GHOSTTY_TERM;
use super::ProcessEnvironment;
use super::TerminalName;
use super::terminal_info;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StdoutColorLevel {
    TrueColor,
    Ansi256,
    Ansi16,
    Unknown,
}

/// Color depth for stdout, including terminal types missed by supports-color.
pub fn stdout_color_level() -> StdoutColorLevel {
    match supports_color::on_cached(supports_color::Stream::Stdout) {
        Some(level) if level.has_16m => StdoutColorLevel::TrueColor,
        Some(level) if level.has_256 => StdoutColorLevel::Ansi256,
        Some(_) => ansi_color_level(&ProcessEnvironment),
        None => StdoutColorLevel::Unknown,
    }
}

pub(super) fn ansi_color_level(env: &dyn Environment) -> StdoutColorLevel {
    // SSH may forward TERM without COLORTERM. Check TERM itself, since a
    // multiplexer can inherit Ghostty's identity but advertise a different palette.
    if env.var("TERM").as_deref() == Some(GHOSTTY_TERM)
        && !env.has("FORCE_COLOR")
        && env.var("CLICOLOR_FORCE").is_none_or(|value| value == "0")
    {
        StdoutColorLevel::TrueColor
    } else {
        StdoutColorLevel::Ansi16
    }
}

/// Color depth for interactive palettes, including Windows Terminal sessions
/// where stdout detection can be inconclusive.
pub fn terminal_color_level() -> StdoutColorLevel {
    color_level_for_terminal(
        stdout_color_level(),
        terminal_info().name,
        &ProcessEnvironment,
    )
}

pub(super) fn color_level_for_terminal(
    level: StdoutColorLevel,
    name: TerminalName,
    env: &dyn Environment,
) -> StdoutColorLevel {
    // TERM_PROGRAM can mask WT_SESSION during terminal identification.
    if (env.has("WT_SESSION")
        || (level == StdoutColorLevel::Ansi16 && name == TerminalName::WindowsTerminal))
        && !env.has("FORCE_COLOR")
    {
        StdoutColorLevel::TrueColor
    } else {
        level
    }
}

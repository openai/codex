//! Derives TUI capabilities from terminal identity and session environment.
//! Keep identity heuristics and probe interpretation private. Callers request capabilities
//! independently so rendering never starts an input probe or freezes live session settings.

pub(crate) mod images;
pub(crate) mod keyboard_modes;
mod resize_reflow_cap;
#[cfg(unix)]
mod terminal_identity;
pub(crate) mod terminal_probe;
mod tmux;
pub(crate) mod tmux_clipboard;

use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use codex_terminal_detection::terminal_info;
use std::io::IsTerminal;

pub use codex_terminal_detection::StdoutColorLevel;
pub use codex_terminal_detection::stdout_color_level;
pub(crate) use codex_terminal_detection::terminal_color_level;
pub(crate) use resize_reflow_cap::resize_reflow_max_rows;
#[cfg(unix)]
pub(crate) use terminal_identity::requires_inline_screen;
pub(crate) use tmux::MouseCapture;
pub(crate) use tmux::sample_mouse;

pub(crate) fn is_ssh_session() -> bool {
    std::env::var_os("SSH_TTY").is_some() || std::env::var_os("SSH_CONNECTION").is_some()
}

pub(crate) fn is_tmux_session() -> bool {
    tmux_transport() || std::env::var_os("TMUX_PANE").is_some()
}

fn tmux_transport() -> bool {
    std::env::var_os("TMUX").is_some()
}

fn screen_active() -> bool {
    std::env::var_os("STY").is_some()
}

pub(crate) struct SessionCapabilities {
    pub(crate) mouse_capture: MouseCapture,
    virtual_image_base: Option<u32>,
}

impl SessionCapabilities {
    pub(crate) fn virtual_image_base(&self) -> Option<u32> {
        if tmux_transport()
            && !zellij_session()
            && !crossterm::style::Colored::ansi_color_disabled_memoized()
        {
            self.virtual_image_base
        } else {
            None
        }
    }
}

fn zellij_session() -> bool {
    ["ZELLIJ", "ZELLIJ_SESSION_NAME", "ZELLIJ_VERSION"]
        .iter()
        .any(|key| std::env::var_os(key).is_some())
}

#[cfg(unix)]
pub(crate) fn should_probe_inline_screen() -> bool {
    terminal_info().multiplexer.is_none() && is_ssh_session()
}

#[cfg(unix)]
pub(crate) fn should_probe_kitty_graphics() -> bool {
    let info = terminal_info();
    info.multiplexer.is_none() && info.term_program.as_deref() == Some("herdr")
}

pub(crate) fn needs_session_polling(info: &TerminalInfo) -> bool {
    cfg!(unix) && matches!(info.multiplexer, Some(Multiplexer::Tmux { .. }))
}

pub(crate) fn screen_capabilities() -> (ScrollbackStrategy, bool) {
    let info = terminal_info();
    (scrollback_strategy(&info), needs_session_polling(&info))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScrollbackStrategy {
    Standard,
    ScrollRegionUp,
    FullScreen,
}

pub(crate) fn scrollback_strategy(terminal: &TerminalInfo) -> ScrollbackStrategy {
    if terminal.is_zellij() {
        ScrollbackStrategy::ScrollRegionUp
    } else if terminal.name == TerminalName::WindowsTerminal
        || std::env::var_os("WT_SESSION").is_some()
    {
        ScrollbackStrategy::FullScreen
    } else {
        ScrollbackStrategy::Standard
    }
}

/// How clipboard writes should reach the attached terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClipboardCopyMode {
    FallbackOsc52,
    Osc52,
    Tmux,
    /// The native clipboard may belong to a private desktop.
    PreferTerminal {
        native_for_large_copies: bool,
        tmux: bool,
    },
}

impl ClipboardCopyMode {
    pub(crate) fn for_terminal(terminal: TerminalName) -> Self {
        if terminal == TerminalName::Rex {
            Self::Osc52
        } else {
            Self::FallbackOsc52
        }
    }
}

pub(crate) struct ClipboardCapabilities {
    pub(crate) copy_mode: ClipboardCopyMode,
    pub(crate) forwarded_x11: bool,
}

impl ClipboardCapabilities {
    pub(crate) fn detect() -> Self {
        let terminal = codex_terminal_detection::terminal_info();
        let ssh = is_ssh_session();
        let tmux = is_tmux_session();
        Self {
            copy_mode: if terminal.is_vscode_dev_container() {
                ClipboardCopyMode::PreferTerminal {
                    native_for_large_copies: cfg!(target_os = "linux")
                        && std::env::var_os("WAYLAND_DISPLAY")
                            .is_some_and(|display| !display.is_empty()),
                    tmux,
                }
            } else if tmux {
                ClipboardCopyMode::Tmux
            } else if ssh || std::env::var("HERDR_ENV").is_ok_and(|value| value == "1") {
                ClipboardCopyMode::Osc52
            } else {
                ClipboardCopyMode::for_terminal(terminal.name)
            },
            forwarded_x11: ssh && std::env::var("DISPLAY").is_ok_and(|display| !display.is_empty()),
        }
    }
}

pub(crate) fn title_encoding() -> crate::terminal_title::TitleEncoding {
    use crate::terminal_title::TitleEncoding;
    // Screen remains an output hop when another multiplexer runs inside it.
    // TERM=screen alone is not sufficient: tmux also commonly uses that TERM.
    if screen_active() {
        TitleEncoding::Ascii
    } else {
        TitleEncoding::Unicode
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum WebLinkDisplay {
    LabelOnly,
    WithDestination,
}

pub(crate) fn web_link_display(terminal: &TerminalInfo, term: Option<&str>) -> WebLinkDisplay {
    if terminal.multiplexer.is_some()
        || term.is_some_and(|term| {
            term == "dumb" || term.starts_with("screen") || term.starts_with("tmux")
        })
    {
        return WebLinkDisplay::WithDestination;
    }
    match terminal.name {
        TerminalName::Ghostty
        | TerminalName::Iterm2
        | TerminalName::WezTerm
        | TerminalName::Rex
        | TerminalName::Orca
        | TerminalName::Kitty
        | TerminalName::VsCode
        | TerminalName::Alacritty
        | TerminalName::WindowsTerminal
        | TerminalName::Konsole
        | TerminalName::GnomeTerminal
        | TerminalName::Vte => WebLinkDisplay::LabelOnly,
        TerminalName::AppleTerminal
        | TerminalName::WarpTerminal
        | TerminalName::Dumb
        | TerminalName::Unknown => WebLinkDisplay::WithDestination,
    }
}

pub(crate) fn detect_web_link_display() -> WebLinkDisplay {
    if !std::io::stdout().is_terminal() || screen_active() {
        return WebLinkDisplay::WithDestination;
    }
    web_link_display(&terminal_info(), std::env::var("TERM").ok().as_deref())
}

/// Why interactive screen control may be unavailable at launch.
pub fn interactive_terminal_warning() -> Option<&'static str> {
    (terminal_info().name == TerminalName::Dumb).then_some("TERM is set to \"dumb\"")
}

/// Whether stdout supports colored exit messages.
pub fn stdout_supports_color() -> bool {
    stdout_color_level() != StdoutColorLevel::Unknown
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum PointerReset {
    Text,
    Release,
}

impl PointerReset {
    /// Only request CSS pointer shapes when their OSC 22 reset is known.
    pub(crate) fn detect() -> Option<Self> {
        let info = terminal_info();
        if info.multiplexer.is_some() {
            return None;
        }
        match info.name {
            TerminalName::Kitty => Some(Self::Release),
            TerminalName::Ghostty => Some(Self::Text),
            _ => None,
        }
    }
}

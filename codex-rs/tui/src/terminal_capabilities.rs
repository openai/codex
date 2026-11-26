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
mod tmux_clipboard;

use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalBackend;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use codex_terminal_detection::terminal_info;
use std::io::IsTerminal;

pub use codex_terminal_detection::StdoutColorLevel;
pub use codex_terminal_detection::stdout_color_level;
pub(crate) use codex_terminal_detection::terminal_color_level;
pub(crate) use codex_terminal_detection::user_agent;
pub(crate) use resize_reflow_cap::resize_reflow_max_rows;
#[cfg(unix)]
pub(crate) use terminal_identity::requires_inline_screen;
pub(crate) use tmux::MouseCapture;
pub(crate) use tmux::sample_mouse;
pub(crate) use tmux_clipboard::copy as copy_to_client_buffer;

#[cfg(target_os = "linux")]
pub(crate) fn is_wsl_session() -> bool {
    crate::clipboard_paste::is_probably_wsl()
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn is_wsl_session() -> bool {
    false
}

pub(crate) fn is_ssh_session() -> bool {
    std::env::var_os("SSH_TTY").is_some() || std::env::var_os("SSH_CONNECTION").is_some()
}

fn is_tmux_session() -> bool {
    tmux_transport() || std::env::var_os("TMUX_PANE").is_some()
}

fn tmux_transport() -> bool {
    std::env::var_os("TMUX").is_some()
}

fn screen_active() -> bool {
    std::env::var_os("STY").is_some()
}

/// Escape embedded commands for a DCS passthrough transport.
pub(crate) fn escaped_dcs(command: &str) -> String {
    let escaped = command.replace('\x1b', "\x1b\x1b");
    format!("\x1bPtmux;{escaped}\x1b\\")
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

fn direct_stdout() -> bool {
    std::io::stdout().is_terminal() && !screen_active()
}

fn is_direct_terminal(terminal: &TerminalInfo, term: Option<&str>) -> bool {
    terminal.multiplexer.is_none()
        && !term.is_some_and(|term| {
            term == "dumb" || term.starts_with("screen") || term.starts_with("tmux")
        })
}

fn direct_terminal_name() -> Option<TerminalName> {
    let terminal = terminal_info();
    (direct_stdout() && is_direct_terminal(&terminal, std::env::var("TERM").ok().as_deref()))
        .then_some(terminal.name)
}

/// A terminal's explicit bidi mode overrides assumptions about its renderer.
/// Silence or an unrecognized mode alone cannot rule out native or partial RTL.
pub(crate) fn needs_bidi_layout(implicit_bidi: Option<bool>) -> bool {
    let Some(name) = direct_terminal_name() else {
        return false;
    };
    if let Some(implicit) = implicit_bidi {
        return !implicit;
    }
    matches!(
        name,
        TerminalName::WindowsTerminal | TerminalName::Ghostty | TerminalName::Alacritty
    )
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
    ClientBuffer,
    /// The native clipboard may belong to a private desktop.
    PreferTerminal {
        native_for_large_copies: bool,
        client_buffer: bool,
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
        let terminal = terminal_info();
        let ssh = is_ssh_session();
        let tmux = is_tmux_session();
        Self {
            copy_mode: if terminal.is_vscode_dev_container() {
                ClipboardCopyMode::PreferTerminal {
                    native_for_large_copies: cfg!(target_os = "linux")
                        && std::env::var_os("WAYLAND_DISPLAY")
                            .is_some_and(|display| !display.is_empty()),
                    client_buffer: tmux,
                }
            } else if tmux {
                ClipboardCopyMode::ClientBuffer
            } else if ssh || std::env::var("HERDR_ENV").is_ok_and(|value| value == "1") {
                ClipboardCopyMode::Osc52
            } else {
                ClipboardCopyMode::for_terminal(terminal.name)
            },
            forwarded_x11: ssh && std::env::var("DISPLAY").is_ok_and(|display| !display.is_empty()),
        }
    }
}

pub(crate) fn forwards_copy_shortcut() -> bool {
    forwards_copy_shortcut_for(&terminal_info())
}

pub(crate) fn forwards_copy_shortcut_for(terminal: &TerminalInfo) -> bool {
    terminal.multiplexer.is_none()
        && match terminal.name {
            // Since 1.2, both Ghostty's Cmd-C and Ctrl-Shift-C bindings are
            // "performable": they forward the key if Ghostty has no selection.
            TerminalName::Ghostty => terminal
                .version
                .as_deref()
                .and_then(|version| semver::Version::parse(version).ok())
                .is_some_and(|version| {
                    version
                        >= semver::Version::new(
                            /*major*/ 1, /*minor*/ 2, /*patch*/ 0,
                        )
                }),
            // Kitty's Cmd-C uses copy_or_noop. Its Ctrl-Shift-C consumes the key.
            TerminalName::Kitty => cfg!(target_os = "macos"),
            // Windows Terminal's Copy action forwards its key when unselected,
            // including when the CLI runs in WSL.
            TerminalName::WindowsTerminal => true,
            // VS Code gates Copy on native selection. Only Windows' default
            // (plain Ctrl-C) is also forwarded by the legacy xterm.js encoder.
            TerminalName::VsCode => cfg!(target_os = "windows"),
            TerminalName::AppleTerminal
            | TerminalName::Iterm2
            | TerminalName::WarpTerminal
            | TerminalName::WezTerm
            | TerminalName::Rex
            | TerminalName::Orca
            | TerminalName::Alacritty
            | TerminalName::Konsole
            | TerminalName::GnomeTerminal
            | TerminalName::Vte
            | TerminalName::Dumb
            | TerminalName::Unknown => false,
        }
}

pub(crate) enum Osc52Target {
    Direct,
    EscapedDcs,
    ChunkedDcs,
}

impl Osc52Target {
    pub(crate) fn detect() -> Self {
        if tmux_transport() {
            Self::EscapedDcs
        } else if screen_active() {
            Self::ChunkedDcs
        } else {
            Self::Direct
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PasteOwner {
    Terminal(&'static str),
    Application,
    Unknown,
}

pub(crate) fn paste_owner() -> PasteOwner {
    match keyboard_modes::detect_vscode_terminal() {
        keyboard_modes::VscodeDetection::VsCode => {
            PasteOwner::Terminal("VS Code handles pasting; use its terminal paste command")
        }
        keyboard_modes::VscodeDetection::Other => PasteOwner::Application,
        keyboard_modes::VscodeDetection::Unknown => PasteOwner::Unknown,
    }
}

#[cfg(target_os = "linux")]
mod primary;
#[cfg(target_os = "linux")]
pub(crate) use primary::detect_primary_selection;

#[cfg(not(target_os = "linux"))]
pub(crate) fn detect_primary_selection() -> std::sync::Arc<std::sync::atomic::AtomicBool> {
    Default::default()
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
    if !is_direct_terminal(terminal, term) {
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
    if !direct_stdout() {
        return WebLinkDisplay::WithDestination;
    }
    web_link_display(&terminal_info(), std::env::var("TERM").ok().as_deref())
}

// VTE 0.54 introduced a parser that safely ignores unknown OSC sequences.
const MIN_VTE_VERSION_WITH_SAFE_OSC: u32 = 5400;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerminalProgramStatusSupport {
    pub(crate) osc_7501: bool,
    pub(crate) osc_21337: bool,
}

pub(crate) fn detect_terminal_program_status_support() -> TerminalProgramStatusSupport {
    terminal_program_status_support(
        &terminal_info(),
        /*stdout_is_terminal*/ std::io::stdout().is_terminal(),
        /*screen_active*/ screen_active(),
    )
}

pub(crate) fn terminal_program_status_support(
    terminal: &TerminalInfo,
    stdout_is_terminal: bool,
    screen_active: bool,
) -> TerminalProgramStatusSupport {
    let osc_7501 = stdout_is_terminal
        && match terminal.backend {
            Some(TerminalBackend::Vte { version }) => {
                version.is_none_or(|version| version >= MIN_VTE_VERSION_WITH_SAFE_OSC)
            }
            None => true,
        };
    let osc_21337 = osc_7501
        && terminal.name == TerminalName::Iterm2
        && terminal.multiplexer.is_none()
        && !screen_active;
    TerminalProgramStatusSupport {
        osc_7501,
        osc_21337,
    }
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

pub(crate) fn telemetry_labels() -> (&'static str, &'static str) {
    // Use a fixed category, not the versioned or user-provided terminal identifier.
    let terminal_info = terminal_info();
    let terminal_name = match terminal_info.name {
        TerminalName::AppleTerminal => "apple_terminal",
        TerminalName::Ghostty => "ghostty",
        TerminalName::Rex => "rex",
        TerminalName::Orca => "orca",
        TerminalName::Iterm2 => "iterm2",
        TerminalName::WarpTerminal => "warp",
        TerminalName::VsCode => "vscode",
        TerminalName::WezTerm => "wezterm",
        TerminalName::Kitty => "kitty",
        TerminalName::Alacritty => "alacritty",
        TerminalName::Konsole => "konsole",
        TerminalName::GnomeTerminal => "gnome_terminal",
        TerminalName::Vte => "vte",
        TerminalName::WindowsTerminal => "windows_terminal",
        TerminalName::Dumb => "dumb",
        TerminalName::Unknown => "unknown",
    };
    let multiplexer = match terminal_info.multiplexer {
        Some(Multiplexer::Tmux { .. }) => "tmux",
        Some(Multiplexer::Zellij { .. }) => "zellij",
        None => "none",
    };
    (terminal_name, multiplexer)
}

pub(crate) struct NotificationCapabilities {
    pub(crate) osc9: bool,
    pub(crate) dcs_passthrough: bool,
    pub(crate) max_body_bytes: usize,
    pub(crate) focused_notice: Option<&'static str>,
}

impl NotificationCapabilities {
    pub(crate) fn detect() -> Self {
        Self::for_terminal(&terminal_info())
    }

    pub(crate) fn for_terminal(info: &TerminalInfo) -> Self {
        // Ghostty has a fixed notification buffer and suppresses focused banners.
        let ghostty = info.name == TerminalName::Ghostty;
        Self {
            osc9: matches!(
                info.name,
                TerminalName::Ghostty
                    | TerminalName::Iterm2
                    | TerminalName::Kitty
                    | TerminalName::WarpTerminal
                    | TerminalName::WezTerm
            ),
            dcs_passthrough: matches!(info.multiplexer, Some(Multiplexer::Tmux { .. })),
            max_body_bytes: if ghostty { 255 } else { usize::MAX },
            focused_notice: ghostty.then_some("Test sent · Ghostty hides banners while focused"),
        }
    }
}

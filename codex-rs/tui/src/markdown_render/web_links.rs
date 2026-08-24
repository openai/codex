//! Presentation policy for web-link destinations in semantic Markdown output.
//!
//! Terminal identity is a conservative heuristic, not end-to-end capability negotiation. Unknown
//! terminals and multiplexers retain the destination so a label never hides the only usable URL.

use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use codex_terminal_detection::terminal_info;
use std::io::IsTerminal;
use std::sync::LazyLock;

use crate::terminal_hyperlinks::web_destination;

#[derive(Clone, Copy, PartialEq, Eq)]
enum WebLinkDisplay {
    LabelOnly,
    WithDestination,
}

impl WebLinkDisplay {
    fn for_terminal(terminal: &TerminalInfo, term: Option<&str>) -> Self {
        if terminal.multiplexer.is_some()
            || term.is_some_and(|term| {
                term == "dumb" || term.starts_with("screen") || term.starts_with("tmux")
            })
        {
            return Self::WithDestination;
        }
        match terminal.name {
            TerminalName::Ghostty
            | TerminalName::Iterm2
            | TerminalName::WezTerm
            | TerminalName::Kitty
            | TerminalName::VsCode
            | TerminalName::Alacritty
            | TerminalName::WindowsTerminal
            | TerminalName::Konsole
            | TerminalName::GnomeTerminal
            | TerminalName::Vte => Self::LabelOnly,
            TerminalName::AppleTerminal
            | TerminalName::WarpTerminal
            | TerminalName::Dumb
            | TerminalName::Unknown => Self::WithDestination,
        }
    }

    fn hide_destination(self, destination: &str) -> bool {
        self == Self::LabelOnly && web_destination(destination).is_some()
    }
}

pub(crate) fn hide_web_link_destination(destination: &str) -> bool {
    static DISPLAY: LazyLock<WebLinkDisplay> = LazyLock::new(|| {
        if !std::io::stdout().is_terminal() || std::env::var_os("STY").is_some() {
            return WebLinkDisplay::WithDestination;
        }
        WebLinkDisplay::for_terminal(&terminal_info(), std::env::var("TERM").ok().as_deref())
    });
    DISPLAY.hide_destination(destination)
}

#[cfg(test)]
#[path = "web_links_tests.rs"]
mod tests;

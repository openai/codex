//! Terminal-specific row caps for resize reflow.
//!
//! The auto cap mirrors documented scrollback defaults for terminals we can identify. Console Host
//! does not expose its configured screen buffer through terminal metadata, so it usually lands in
//! the fallback bucket.

use codex_config::types::DEFAULT_TERMINAL_RESIZE_REFLOW_FALLBACK_MAX_ROWS;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use codex_terminal_detection::terminal_info;

use crate::legacy_core::config::TerminalResizeReflowConfig;
use crate::legacy_core::config::TerminalResizeReflowMaxRows;

const VSCODE_RESIZE_REFLOW_MAX_ROWS: usize = 1_000;
const WINDOWS_TERMINAL_RESIZE_REFLOW_MAX_ROWS: usize = 9_001;
const WEZTERM_RESIZE_REFLOW_MAX_ROWS: usize = 3_500;
const ALACRITTY_RESIZE_REFLOW_MAX_ROWS: usize = 10_000;

pub(crate) fn resize_reflow_max_rows(config: TerminalResizeReflowConfig) -> Option<usize> {
    resize_reflow_max_rows_for(
        config,
        &terminal_info(),
        crate::tui::running_in_vscode_terminal(),
    )
}

fn resize_reflow_max_rows_for(
    config: TerminalResizeReflowConfig,
    terminal: &TerminalInfo,
    running_in_vscode_terminal: bool,
) -> Option<usize> {
    match config.max_rows {
        TerminalResizeReflowMaxRows::Auto => Some(auto_resize_reflow_max_rows(
            terminal.name,
            running_in_vscode_terminal,
        )),
        TerminalResizeReflowMaxRows::Disabled => None,
        TerminalResizeReflowMaxRows::Limit(max_rows) => Some(max_rows),
    }
}

fn auto_resize_reflow_max_rows(
    terminal_name: TerminalName,
    running_in_vscode_terminal: bool,
) -> usize {
    if running_in_vscode_terminal {
        return VSCODE_RESIZE_REFLOW_MAX_ROWS;
    }

    match terminal_name {
        TerminalName::VsCode => VSCODE_RESIZE_REFLOW_MAX_ROWS,
        TerminalName::WindowsTerminal => WINDOWS_TERMINAL_RESIZE_REFLOW_MAX_ROWS,
        TerminalName::WezTerm => WEZTERM_RESIZE_REFLOW_MAX_ROWS,
        TerminalName::Alacritty => ALACRITTY_RESIZE_REFLOW_MAX_ROWS,
        TerminalName::AppleTerminal
        | TerminalName::Ghostty
        | TerminalName::Iterm2
        | TerminalName::WarpTerminal
        | TerminalName::Kitty
        | TerminalName::Konsole
        | TerminalName::GnomeTerminal
        | TerminalName::Vte
        | TerminalName::Dumb
        | TerminalName::Unknown => DEFAULT_TERMINAL_RESIZE_REFLOW_FALLBACK_MAX_ROWS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_terminal_detection::Multiplexer;

    fn test_terminal(name: TerminalName) -> TerminalInfo {
        TerminalInfo {
            name,
            term_program: None,
            version: None,
            term: None,
            multiplexer: None,
        }
    }

    #[test]
    fn auto_resize_reflow_max_rows_uses_terminal_defaults() {
        let cases = [
            (TerminalName::VsCode, VSCODE_RESIZE_REFLOW_MAX_ROWS),
            (
                TerminalName::WindowsTerminal,
                WINDOWS_TERMINAL_RESIZE_REFLOW_MAX_ROWS,
            ),
            (TerminalName::WezTerm, WEZTERM_RESIZE_REFLOW_MAX_ROWS),
            (TerminalName::Alacritty, ALACRITTY_RESIZE_REFLOW_MAX_ROWS),
            (
                TerminalName::Ghostty,
                DEFAULT_TERMINAL_RESIZE_REFLOW_FALLBACK_MAX_ROWS,
            ),
            (
                TerminalName::Unknown,
                DEFAULT_TERMINAL_RESIZE_REFLOW_FALLBACK_MAX_ROWS,
            ),
        ];

        for (terminal_name, expected_max_rows) in cases {
            assert_eq!(
                auto_resize_reflow_max_rows(
                    terminal_name,
                    /*running_in_vscode_terminal*/ false
                ),
                expected_max_rows
            );
        }
    }

    #[test]
    fn auto_resize_reflow_max_rows_prefers_vscode_probe() {
        assert_eq!(
            auto_resize_reflow_max_rows(
                TerminalName::WindowsTerminal,
                /*running_in_vscode_terminal*/ true
            ),
            VSCODE_RESIZE_REFLOW_MAX_ROWS
        );
    }

    #[test]
    fn configured_resize_reflow_max_rows_overrides_auto_detection() {
        let terminal = test_terminal(TerminalName::VsCode);
        let config = TerminalResizeReflowConfig {
            max_rows: TerminalResizeReflowMaxRows::Limit(42),
            ..Default::default()
        };

        assert_eq!(
            resize_reflow_max_rows_for(
                config, &terminal, /*running_in_vscode_terminal*/ false
            ),
            Some(42)
        );
    }

    #[test]
    fn disabled_resize_reflow_max_rows_keeps_all_rows() {
        let terminal = test_terminal(TerminalName::VsCode);
        let config = TerminalResizeReflowConfig {
            max_rows: TerminalResizeReflowMaxRows::Disabled,
            ..Default::default()
        };

        assert_eq!(
            resize_reflow_max_rows_for(
                config, &terminal, /*running_in_vscode_terminal*/ false
            ),
            None
        );
    }

    #[test]
    fn unknown_terminal_uses_fallback_even_under_multiplexer() {
        let terminal = TerminalInfo {
            name: TerminalName::Unknown,
            term_program: None,
            version: None,
            term: Some("xterm-256color".to_string()),
            multiplexer: Some(Multiplexer::Tmux { version: None }),
        };
        let config = TerminalResizeReflowConfig::default();

        assert_eq!(
            resize_reflow_max_rows_for(
                config, &terminal, /*running_in_vscode_terminal*/ false
            ),
            Some(DEFAULT_TERMINAL_RESIZE_REFLOW_FALLBACK_MAX_ROWS)
        );
    }
}

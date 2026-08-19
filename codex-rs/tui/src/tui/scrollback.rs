//! Choose terminal-safe strategies for growing the viewport and inserting history.

use crate::custom_terminal::Terminal;
use crate::insert_history::HistoryLineWrapPolicy;
use crate::insert_history::InsertHistoryMode;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use ratatui::backend::Backend;
use ratatui::layout::Position;
use ratatui::layout::Size;
use std::io;
use std::io::Write;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScrollbackStrategy {
    Standard,
    Zellij,
    FullScreen,
}

impl ScrollbackStrategy {
    pub(super) fn detect(terminal: &TerminalInfo) -> Self {
        if terminal.is_zellij() {
            Self::Zellij
        } else if terminal.name == TerminalName::WindowsTerminal
            || std::env::var_os("WT_SESSION").is_some()
        {
            Self::FullScreen
        } else {
            Self::Standard
        }
    }

    pub(super) fn history_insertion_mode(
        self,
        wrap_policy: HistoryLineWrapPolicy,
    ) -> InsertHistoryMode {
        match self {
            Self::FullScreen => InsertHistoryMode::FullScreen,
            Self::Zellij if wrap_policy == HistoryLineWrapPolicy::Terminal => {
                InsertHistoryMode::FullScreen
            }
            Self::Standard | Self::Zellij => InsertHistoryMode::Standard,
        }
    }

    pub(super) fn grow_viewport<B>(
        self,
        terminal: &mut Terminal<B>,
        viewport_top: u16,
        screen_size: Size,
        scroll_by: u16,
    ) -> io::Result<()>
    where
        B: Backend<Error = io::Error> + Write,
    {
        match self {
            Self::FullScreen => {
                // Partial DEC scroll regions can discard rows instead of moving them into Windows
                // Terminal's scrollback. Clear the stale composer, then scroll the entire screen.
                terminal.clear_after_position(Position::new(/*x*/ 0, viewport_top))?;
                let writer = terminal.backend_mut();
                queue!(
                    writer,
                    MoveTo(/*x*/ 0, screen_size.height.saturating_sub(/*rhs*/ 1))
                )?;
                for _ in 0..scroll_by {
                    queue!(writer, Print("\r\n"))?;
                }
                Ok(())
            }
            Self::Standard | Self::Zellij => terminal
                .backend_mut()
                .scroll_region_up(0..viewport_top, scroll_by),
        }
    }
}

#[cfg(test)]
#[path = "scrollback_tests.rs"]
mod tests;

//! Right-click fallback for an unchanged, editable fullscreen composer.
//! Existing selection handlers run first. Only a draw may deliver a read, so real input
//! invalidates pending paste before completion and is never replaced by clipboard text.

use super::*;
use crate::tui::VscodeDetection;
use codex_config::types::RightClickPaste;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;

pub(super) struct PendingPaste {
    thread: Option<ThreadId>,
    draft: (String, usize),
}

pub(super) struct PasteEnvironment {
    pub(super) platform_default: bool,
    pub(super) ssh: bool,
    pub(super) wsl: bool,
    pub(super) vscode: VscodeDetection,
}

impl PasteEnvironment {
    pub(super) fn detect() -> Self {
        Self {
            platform_default: cfg!(any(target_os = "windows", target_os = "linux")),
            ssh: crate::clipboard_copy::is_ssh_session(),
            wsl: crate::clipboard_copy::is_wsl_session(),
            vscode: tui::detect_vscode_terminal(),
        }
    }

    fn allows(&self, mode: RightClickPaste) -> bool {
        if self.ssh || self.vscode == VscodeDetection::VsCode {
            return false;
        }
        match mode {
            RightClickPaste::Off => false,
            RightClickPaste::On => !cfg!(target_os = "android"),
            RightClickPaste::Auto => {
                self.platform_default && !(self.wsl && self.vscode == VscodeDetection::Unknown)
            }
        }
    }
}

impl App {
    fn right_click_paste_target(&self, tui: &tui::Tui) -> Option<PendingPaste> {
        if !tui.is_owned_screen()
            || self.overlay.is_some()
            || self.transcript_view.has_selection_range()
            || self.transcript_view.is_search_active()
            || !self
                .right_click_paste_environment
                .allows(self.local_settings.tui.right_click_paste)
        {
            return None;
        }
        Some(PendingPaste {
            thread: self.current_displayed_thread_id(),
            draft: self.chat_widget.right_click_paste_target()?,
        })
    }

    pub(super) fn start_right_click_paste(&mut self, tui: &mut tui::Tui, mouse: MouseEvent) {
        if mouse.kind != MouseEventKind::Down(MouseButton::Right)
            || !mouse.modifiers.is_empty()
            || self.pending_right_click_paste.is_some()
            || tui.clipboard.is_busy()
        {
            return;
        }
        let Some(target) = self.right_click_paste_target(tui) else {
            return;
        };
        match tui.clipboard.read_text(tui.frame_requester()) {
            Ok(true) => self.pending_right_click_paste = Some(target),
            Ok(false) => {}
            Err(error) => self.chat_widget.add_error_message(error),
        }
    }

    /// Invalidate before polling, even when the worker has already completed.
    pub(super) fn invalidate_right_click_paste(&mut self, event: &TuiEvent) {
        let keep = match event {
            TuiEvent::Draw | TuiEvent::Resize(_) | TuiEvent::FocusGained => true,
            TuiEvent::Mouse(mouse) => {
                matches!(
                    mouse.kind,
                    MouseEventKind::Moved | MouseEventKind::Up(MouseButton::Right)
                ) || (mouse.kind == MouseEventKind::Down(MouseButton::Right)
                    && mouse.modifiers.is_empty())
            }
            TuiEvent::Key(_) | TuiEvent::Paste(_) | TuiEvent::FocusLost | TuiEvent::Resume => false,
        };
        if !keep {
            self.pending_right_click_paste = None;
        }
    }

    pub(super) fn finish_right_click_paste(
        &mut self,
        tui: &mut tui::Tui,
        event: TuiEvent,
    ) -> TuiEvent {
        if !matches!(event, TuiEvent::Draw) {
            return event;
        }
        let Some(result) = tui.clipboard.take_text_result() else {
            return event;
        };
        let Some(pending) = self.pending_right_click_paste.take() else {
            return event;
        };
        let Some(current) = self.right_click_paste_target(tui) else {
            return event;
        };
        if pending.thread != current.thread || pending.draft != current.draft {
            return event;
        }
        match result {
            Ok(text) if !text.is_empty() => {
                tui.frame_requester().schedule_frame();
                TuiEvent::Paste(text)
            }
            Ok(_) => event,
            Err(error) => {
                self.chat_widget.add_error_message(error);
                event
            }
        }
    }
}

#[cfg(test)]
#[path = "right_click_paste_tests.rs"]
mod tests;

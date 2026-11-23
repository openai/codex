//! Preview-local path reveal and clipboard feedback that works even when the file cannot load.

use crate::clipboard_copy::CopyFormat;
use crate::clipboard_copy::CopyStatus;
use crate::clipboard_copy::worker::CopyResult;
use crate::footer_hint::HintLine;
use crate::footer_hint::shortcut;
use crate::key_hint::plain;
use crate::keymap::ListAction;
use crate::sidebar_chooser;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use codex_utils_path_uri::PathUri;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::layout::Rect;
use ratatui::text::Line;
use std::time::Duration;
use std::time::Instant;

#[derive(Default)]
pub(in crate::app) struct PathActions {
    pub(in crate::app) scroll: Option<usize>,
    feedback: Option<CopyFeedback>,
}

enum CopyFeedback {
    Pending {
        id: u64,
        reveal: bool,
    },
    Finished {
        message: &'static str,
        expires_at: Instant,
    },
}

impl PathActions {
    pub(super) fn handle_key(&mut self, key: KeyEvent, path: &PathUri, tui: &mut Tui) -> bool {
        if key.kind != KeyEventKind::Press {
            return false;
        }
        if plain(KeyCode::Char('c')).is_press(key) {
            let result = tui.copy_transcript_selection(
                &path.inferred_native_path_string(),
                CopyFormat::PlainText,
            );
            self.copy_result(&result);
        } else if plain(KeyCode::Char('p')).is_press(key) {
            if self.scroll.is_some() {
                self.dismiss();
            } else {
                self.scroll = Some(0);
            }
        } else {
            return false;
        }
        true
    }

    pub(super) fn dismiss(&mut self) {
        self.scroll = None;
        if let Some(CopyFeedback::Pending { reveal, .. }) = &mut self.feedback {
            *reveal = false;
        }
    }

    pub(in crate::app) fn finish_copy(&mut self, completion: &(u64, CopyResult), visible: bool) {
        if matches!(self.feedback, Some(CopyFeedback::Pending { id, .. }) if id == completion.0) {
            if visible {
                self.copy_result(&completion.1);
            } else {
                self.feedback = None;
            }
        }
    }

    fn copy_result(&mut self, result: &CopyResult) {
        let (message, reveal) = match result {
            Ok(CopyStatus::Pending(id)) => {
                self.feedback = Some(CopyFeedback::Pending {
                    id: *id,
                    reveal: true,
                });
                return;
            }
            Ok(CopyStatus::Busy) if matches!(self.feedback, Some(CopyFeedback::Pending { .. })) => {
                return;
            }
            Ok(CopyStatus::Confirmed) => ("Path copied", false),
            Ok(CopyStatus::Unconfirmed) => ("Copy unconfirmed", true),
            Ok(CopyStatus::Busy) => ("Clipboard busy", true),
            Err(_) => ("Could not copy path", true),
        };
        if reveal
            && !matches!(
                self.feedback,
                Some(CopyFeedback::Pending { reveal: false, .. })
            )
        {
            self.scroll.get_or_insert(0);
        }
        self.feedback = Some(CopyFeedback::Finished {
            message,
            expires_at: Instant::now() + Duration::from_secs(/*secs*/ 5),
        });
    }

    pub(super) fn scroll_path(&mut self, area: Rect, path: &PathUri, action: ListAction) {
        if let Some(top) = &mut self.scroll {
            *top = sidebar_chooser::scroll_file_path(
                sidebar_chooser::file_path_areas(area)[0],
                &path.inferred_native_path_string(),
                *top,
                action,
            );
        }
    }

    pub(super) fn revealed_footer(&mut self, width: u16, frames: &FrameRequester) -> Line<'static> {
        let esc = plain(KeyCode::Esc).display_label();
        let hints = self.feedback_hint(frames).into_iter().chain([
            shortcut(&format!("p/{esc}"), "hide path"),
            shortcut("c", "copy path"),
        ]);
        HintLine::new(hints, width).line
    }

    pub(super) fn footer_lines(
        &mut self,
        base: Vec<Line<'static>>,
        width: u16,
        max_rows: usize,
        frames: &FrameRequester,
        available: [bool; 2],
    ) -> Vec<Line<'static>> {
        // The first base hint is close or clear highlight. Navigation can overflow.
        let mut base = base.into_iter();
        let path_hints = [shortcut("c", "copy path"), shortcut("p", "show path")]
            .into_iter()
            .zip(available)
            .filter_map(|(hint, available)| available.then_some(hint));
        let hints = base
            .next()
            .into_iter()
            .chain(path_hints)
            .chain(self.feedback_hint(frames))
            .chain(base);
        HintLine::wrapped(hints, width, max_rows)
            .into_iter()
            .map(|row| row.line)
            .collect()
    }

    fn feedback_hint(&mut self, frames: &FrameRequester) -> Option<Line<'static>> {
        let now = Instant::now();
        self.feedback.take_if(|feedback| {
            matches!(feedback, CopyFeedback::Finished { expires_at, .. } if *expires_at <= now)
        });
        match &self.feedback {
            Some(CopyFeedback::Pending { .. }) => Some("Copying path…"),
            Some(CopyFeedback::Finished {
                message,
                expires_at,
            }) => {
                frames.schedule_frame_in(expires_at.saturating_duration_since(now));
                Some(*message)
            }
            None => None,
        }
        .map(|message| Line::from(message).style(crate::style::secondary_text_style()))
    }
}

#[cfg(test)]
#[path = "path_actions_tests.rs"]
mod tests;

//! Handle clipboard feedback and image paste after the app checks worker availability.
//! Pending feedback belongs to this widget, so switching conversations discards it.

use super::*;
use crate::clipboard_copy::worker::CopyResult;

const IMAGE_PASTE_REPEAT_WINDOW: Duration = Duration::from_millis(/*millis*/ 250);

pub(super) enum PendingCopy {
    Message(u64, String),
    Selection(u64),
}

impl ChatWidget {
    /// Recognize image paste shortcuts, including presses consumed by repeat suppression.
    pub(super) fn image_paste_key_action(&mut self, key: KeyEvent) -> Option<KeyEventAction> {
        let shift_insert = cfg!(windows) && key_hint::shift(KeyCode::Insert).is_press(key);
        let image_paste = key.kind == KeyEventKind::Press
            && ((matches!(key.code, KeyCode::Char(c) if c.eq_ignore_ascii_case(&'v'))
                && key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT))
                || shift_insert);
        if !image_paste {
            if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Release) {
                self.suppress_image_paste_until = Instant::now();
            }
            return None;
        }
        if Instant::now() < self.suppress_image_paste_until {
            self.suppress_image_paste_until = Instant::now() + IMAGE_PASTE_REPEAT_WINDOW;
            return Some(KeyEventAction::None);
        }
        self.suppress_image_paste_until = Instant::now() + IMAGE_PASTE_REPEAT_WINDOW;
        let text_fallback = (cfg!(windows)
            && (key.modifiers == KeyModifiers::CONTROL || shift_insert))
            .then_some(key.code);
        if text_fallback.is_some() {
            self.bottom_pane.prepare_composer_clipboard_paste();
        }
        Some(KeyEventAction::PasteImage { text_fallback })
    }

    pub(crate) fn paste_image(&mut self) -> Result<(), crate::clipboard_paste::PasteImageError> {
        let result = paste_image_to_temp_png();
        self.suppress_image_paste_until = Instant::now() + IMAGE_PASTE_REPEAT_WINDOW;
        let (path, info) = result?;
        tracing::debug!(
            "pasted image size={}x{} format={}",
            info.width,
            info.height,
            info.encoded_format.label()
        );
        self.attach_image(path);
        Ok(())
    }

    pub(super) fn show_clipboard_flash(&mut self, result: &CopyResult) {
        let line = match result {
            Ok(status) => Line::from(status.message("selection").dim()),
            Err(error) => Line::from(format!("Copy failed: {error}").red()),
        };
        self.bottom_pane
            .show_footer_flash(line, Duration::from_secs(/*secs*/ 3));
    }

    pub(crate) fn finish_clipboard(
        &mut self,
        completion: &(u64, CopyResult),
        composer_visible: bool,
    ) -> Option<usize> {
        let pending_id = match &self.pending_clipboard {
            Some(PendingCopy::Message(id, _) | PendingCopy::Selection(id)) => Some(*id),
            None => None,
        };
        if pending_id == Some(completion.0) {
            match self.pending_clipboard.take() {
                Some(PendingCopy::Message(_, label)) => match &completion.1 {
                    Ok(status) => self.add_info_message(status.message(&label), /*hint*/ None),
                    Err(error) => self.add_error_message(format!("Copy failed: {error}")),
                },
                Some(PendingCopy::Selection(_)) => self.show_clipboard_flash(&completion.1),
                None => {}
            }
        }
        self.bottom_pane
            .finish_composer_copy(completion, composer_visible)
    }
}

//! Usage warnings and contextual discovery share the existing composer-gap row.
//! Interaction controls take priority; only discovery tips require idle input and show_tooltips.

use super::*;
use crate::history_cell::SessionInfoCell;
use crate::history_cell::UserHistoryCell;
use crate::keymap::KeymapContext;
use crate::style::secondary_text_style;
use ratatui::style::Styled;
use ratatui::text::Line;

impl App {
    pub(super) fn composer_hint(&self, width: u16) -> Option<Line<'static>> {
        if !self.chat_widget.no_modal_or_popup_active()
            || !self.transcript_view.is_following()
            || self.transcript_view.has_active_interaction()
            || self.backtrack.primed
            || self.backtrack.overlay_preview_active
        {
            return None;
        }
        if let Some(notice) = self.chat_widget.usage_notice(width) {
            return Some(notice);
        }
        if !self.local_settings.tui.show_tooltips
            || !self.chat_widget.composer_is_empty()
            || self.chat_widget.is_user_turn_pending_or_running()
        {
            return None;
        }
        // Cosmetic tip rotation needs cell types, not a scan of every prompt's text.
        let count = self
            .transcript_cells
            .iter()
            .rev()
            .take_while(|cell| !cell.as_any().is::<SessionInfoCell>())
            .filter(|cell| cell.as_any().is::<UserHistoryCell>())
            .count();
        let (prefix, keys, label) = if count == 0 {
            (
                "Tip: ",
                crate::key_hint::key_label_spans("@"),
                " mentions files, skills, and plugins",
            )
        } else {
            match count % 3 {
                0 => (
                    "Tip: drag to select text · ",
                    crate::key_hint::key_label_spans("ctrl+c"),
                    " copies",
                ),
                1 => {
                    let key = self
                        .keymap
                        .primary_hint(KeymapContext::Global, "find_transcript")?;
                    ("Tip: ", key.spans(), " searches this conversation")
                }
                _ => {
                    return Some(Line::from(
                        "Tip: /copy copies the last response".set_style(secondary_text_style()),
                    ));
                }
            }
        };
        let mut tip = Line::from(prefix.set_style(secondary_text_style()));
        tip.spans.extend(keys);
        tip.spans.push(label.set_style(secondary_text_style()));
        Some(tip)
    }
}

#[cfg(test)]
#[path = "composer_hints_tests.rs"]
mod tests;

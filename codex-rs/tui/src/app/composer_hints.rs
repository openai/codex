//! Usage warnings take priority over startup announcements and stable catalog tips in fullscreen.
//! Only complete tips that fit one row are shown; tips require idle input and show_tooltips.

use super::*;
use crate::history_cell::SessionInfoCell;
use crate::history_cell::UserHistoryCell;
use crate::style::secondary_text_style;
use crate::terminal_hyperlinks::HyperlinkLine;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;

/// Keep announcement eligibility and random tip selection stable across redraws.
pub(super) struct ComposerTips {
    seed: u64,
    boot_turn_count: Option<usize>,
    announcement_dismissed: bool,
}

impl Default for ComposerTips {
    fn default() -> Self {
        Self::new(rand::random())
    }
}

impl ComposerTips {
    pub(super) fn new(seed: u64) -> Self {
        Self {
            seed,
            boot_turn_count: None,
            announcement_dismissed: false,
        }
    }

    fn select(
        &mut self,
        turn_count: usize,
        announcement: impl FnOnce() -> Option<String>,
        keymap: &RuntimeKeymap,
        width: usize,
        cwd: &Path,
    ) -> Option<HyperlinkLine> {
        // Remember leaving startup even if /clear or backtracking later removes user cells.
        self.announcement_dismissed |=
            *self.boot_turn_count.get_or_insert(turn_count) != turn_count;
        let render = |tip: String| {
            let lines = crate::tooltips::render_tooltip_lines(&tip, width, cwd);
            match lines.as_slice() {
                [line] if line.width() <= width => Some(line.clone().style(secondary_text_style())),
                _ => None,
            }
        };
        if !self.announcement_dismissed
            && let Some(line) = announcement().and_then(&render)
        {
            return Some(line);
        }

        let mut rng = StdRng::seed_from_u64(self.seed.wrapping_add(turn_count as u64));
        // Randomize before fitting so short tips are not favored by catalog order.
        let mut tips = crate::tooltips::resolved_tooltips(Some(keymap)).collect::<Vec<_>>();
        tips.shuffle(&mut rng);
        tips.into_iter().find_map(render)
    }
}

impl App {
    pub(super) fn composer_hint(&mut self, width: u16) -> Option<HyperlinkLine> {
        let running = self.chat_widget.is_user_turn_pending_or_running();
        if running {
            self.composer_tips.announcement_dismissed = true;
        }
        if !self.chat_widget.no_modal_or_popup_active()
            || !self.transcript_view.is_following()
            || self.transcript_view.has_active_interaction()
            || self.backtrack.primed
            || self.backtrack.overlay_preview_active
        {
            return None;
        }
        let width = width.checked_sub(/*rhs*/ 2)?;
        if width == 0 {
            return None;
        }
        if let Some(notice) = self.chat_widget.usage_notice(width) {
            return Some(HyperlinkLine::new(notice));
        }
        if !self.local_settings.tui.show_tooltips
            || !self.chat_widget.composer_is_empty()
            || running
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
        self.composer_tips.select(
            count,
            || {
                crate::tooltips::announcement::fetch_announcement_tip(
                    self.chat_widget.current_plan_type(),
                )
            },
            &self.keymap,
            usize::from(width),
            self.config.cwd.as_path(),
        )
    }
}

#[cfg(test)]
#[path = "composer_hints_tests.rs"]
mod tests;

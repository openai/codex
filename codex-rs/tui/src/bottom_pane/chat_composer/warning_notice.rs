//! Fit warning counts and the configured warnings shortcut into the existing passive hint row.
//! Interactive hints retain priority; warning delivery never changes composer geometry.
//! Only the count and warning label receive an amber accent; surrounding hints stay muted.

use super::*;

impl ChatComposer {
    pub(crate) fn warning_notice_contains(&self, position: ratatui::layout::Position) -> bool {
        self.footer
            .warning_notice_area
            .get()
            .is_some_and(|area| area.contains(position))
    }

    pub(super) fn show_warning_notice(&self, options: ComposerRenderOptions<'_>) -> bool {
        options.warning_count > 0
            && !options.footer.is_some_and(|footer| footer.is_interactive)
            && matches!(self.popups.active, ActivePopup::None)
            && super::super::footer::shows_passive_footer_line(&self.footer_props())
            && self.footer.hint_override.is_none()
            && !self.footer.flash_visible()
            && self.history_search.is_none()
            && self.draft.textarea.vim_query().is_none()
            && !self.quit_shortcut_hint_visible()
    }

    pub(super) fn warning_notice_layout(
        &self,
        hint_area: Rect,
        options: ComposerRenderOptions<'_>,
    ) -> Option<(Rect, Line<'static>)> {
        if !self.show_warning_notice(options) || hint_area.is_empty() {
            return None;
        }
        // Leave space for ordinary hints, and one clear cell at the terminal's right edge.
        let available = hint_area.width.saturating_sub(/*rhs*/ 1);
        let mut budget = (available / 2).max(/*other*/ 14).min(available);
        if let Some(footer) = options.footer {
            // Navigation and loading hints must remain readable before spending spare columns
            // on warnings. A warning may shrink to its count, or wait until that hint closes.
            let hint_width = footer.text.width().min(usize::from(available)) as u16;
            budget = budget.min(available.saturating_sub(hint_width.saturating_add(/*rhs*/ 2)));
        }
        if budget < Line::from(format!("⚠ {}", options.warning_count)).width() as u16 {
            return None;
        }
        let line = self.warning_notice(options.warning_count, budget);
        let width = line.width() as u16;
        let area = Rect::new(
            hint_area.right().saturating_sub(width + 1),
            hint_area.bottom() - 1,
            width,
            /*height*/ 1,
        );
        Some((area, line))
    }

    pub(super) fn warning_notice(&self, count: usize, width: u16) -> Line<'static> {
        let plural = if count == 1 { "" } else { "s" };
        let accent = crate::style::warning_notice_style();
        let secondary = crate::style::secondary_text_style();
        let shortcut = self
            .footer
            .show_warnings_key
            .map(ShortcutHint::display_label)
            .unwrap_or_else(|| "/warnings".into());
        let mut full = Line::from(vec![
            "⚠ ".into(),
            Span::styled(format!("{count} warning{plural}"), accent),
            " · ".into(),
        ])
        .style(secondary);
        full.extend(key_hint::key_label_spans(&shortcut));
        full.push_span(" to view");
        if full.width() <= usize::from(width) {
            return full;
        }
        let mut compact = Line::from(vec![
            "⚠ ".into(),
            Span::styled(count.to_string(), accent),
            " · ".into(),
        ])
        .style(secondary);
        compact.extend(key_hint::key_label_spans(&shortcut));
        if compact.width() <= usize::from(width) {
            return compact;
        }
        truncate_line_with_ellipsis_if_overflow(
            Line::from(vec!["⚠ ".into(), Span::styled(count.to_string(), accent)]).style(secondary),
            usize::from(width),
        )
    }
}

#[cfg(test)]
#[path = "warning_notice_tests.rs"]
mod tests;

//! Share the composer gap between copy feedback, reading controls, and linked tips.
//! Feedback wins while visible; controls and tips release their pointer targets when replaced.

use super::*;
use crate::clipboard_copy::CopyStatus;
use crate::footer_hint::first_fitting_line;
use crate::terminal_hyperlinks::HyperlinkParagraph;
use ratatui::style::Style;
use std::time::Duration;
use std::time::Instant;

pub(super) struct CopyFeedback {
    result: Result<CopyStatus, ()>,
    characters: usize,
    expires_at: Instant,
}

impl TranscriptView {
    pub(crate) fn show_copy_feedback(
        &mut self,
        result: &Result<CopyStatus, String>,
        characters: usize,
    ) {
        self.copy_feedback = Some(CopyFeedback {
            result: result.as_ref().copied().map_err(|_| ()),
            characters,
            expires_at: Instant::now() + Duration::from_secs(/*secs*/ 5),
        });
    }

    pub(crate) fn render_composer_gap(
        &mut self,
        area: Option<Rect>,
        hint: Option<&HyperlinkLine>,
        buffer: &mut Buffer,
    ) -> Option<Duration> {
        self.composer_tip = None;
        if self
            .copy_feedback
            .as_ref()
            .is_some_and(|feedback| feedback.expires_at <= Instant::now())
        {
            self.copy_feedback = None;
        }
        let Some(area) = area.filter(|area| !area.is_empty()) else {
            self.render_follow_control(/*area*/ None, buffer);
            return None;
        };
        if let Some(feedback) = &self.copy_feedback {
            let line = feedback.line(area.width.saturating_sub(/*rhs*/ 1));
            let delay = feedback
                .expires_at
                .saturating_duration_since(Instant::now());
            let width = line.width().min(usize::from(area.width)) as u16;
            let target = Rect::new(
                area.right().saturating_sub(width + 1).max(area.x),
                area.y,
                width,
                /*height*/ 1,
            );
            line.render(target, buffer);
            self.render_follow_control(/*area*/ None, buffer);
            return Some(delay);
        }
        self.render_follow_control(Some(area), buffer);
        if self.is_following()
            && !self.has_active_interaction()
            && let Some(hint) = hint.filter(|hint| hint.width() + 2 <= usize::from(area.width))
        {
            let width = hint.width() as u16;
            let target = Rect::new(area.right() - width - 1, area.y, width, /*height*/ 1);
            HyperlinkParagraph::new(std::slice::from_ref(hint), Style::default())
                .render(target, buffer);
            self.composer_tip = Some((target, hint.clone()));
        }
        None
    }
}

impl CopyFeedback {
    fn line(&self, width: u16) -> Line<'static> {
        let characters = self.characters;
        let labels = match self.result {
            Ok(CopyStatus::Confirmed) => [
                format!("Copied {characters} chars to host clipboard"),
                format!("Copied {characters} chars"),
                "Copied".into(),
            ],
            Ok(CopyStatus::Unconfirmed) => [
                "Copy sent to terminal · paste to verify".into(),
                "Copy sent · verify paste".into(),
                "Copy unconfirmed".into(),
            ],
            Err(()) => [
                "Copy failed · try again".into(),
                "Copy failed".into(),
                "Copy failed".into(),
            ],
        };
        let line = first_fitting_line(labels.map(Line::from), width);
        if self.result.is_err() {
            line.red()
        } else {
            line.fg(crate::style::accent_color())
        }
    }
}

#[cfg(test)]
#[path = "composer_gap_tests.rs"]
mod tests;

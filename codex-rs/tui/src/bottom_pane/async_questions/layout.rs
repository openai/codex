//! Allocate question, options, input, and footer rows without intermediate layout plans.
//! Tight layouts reserve freeform editing space; named choices require the complete prompt.

use ratatui::layout::Rect;

use super::AsyncQuestions;
use super::DESIRED_SPACERS_BETWEEN_SECTIONS;

pub(super) struct LayoutSections {
    pub(super) progress_area: Rect,
    pub(super) question_area: Rect,
    pub(super) question_lines: Vec<String>,
    pub(super) options_area: Rect,
    pub(super) notes_area: Rect,
    pub(super) footer_lines: u16,
    pub(super) spacer_after_input: u16,
}

impl AsyncQuestions {
    pub(super) fn layout_sections(&self, area: Rect) -> LayoutSections {
        let has_options = self.has_options();
        let mut question_lines = self.wrapped_question_lines(area.width);
        let mut footer_pref = self.footer_lines(area.width, /*option_tip*/ None).len() as u16;
        if has_options
            && question_lines.len()
                + usize::from(self.options_required_height(area.width))
                + usize::from(footer_pref)
                + usize::from(self.unanswered_count() > 1)
                + usize::from(DESIRED_SPACERS_BETWEEN_SECTIONS)
                > usize::from(area.height)
        {
            footer_pref = self.footer_lines(area.width, Some(self.option_tip())).len() as u16;
        }
        let progress_pref = u16::from(self.unanswered_count() > 1);
        let min_notes = u16::from(!has_options).min(area.height);
        let available = area.height.saturating_sub(min_notes);
        let question_height = question_lines.len().min(usize::from(
            available.saturating_sub(u16::from(self.other_selected())),
        )) as u16;
        question_lines.truncate(usize::from(question_height));
        let mut remaining = available.saturating_sub(question_height);
        let mut options_height = 0;
        let mut spacer_before_question = 0;
        let spacer_after_question;
        let mut spacer_after_options = 0;
        let mut spacer_after_input = 0;
        let progress_height;
        let footer_lines;
        let notes_height;

        if has_options {
            let full_options_height = self.options_required_height(area.width);
            let min_options_height = remaining.min(1);
            // Reserve hints and spacers while retaining a row for the selected option.
            let reserved = footer_pref + progress_pref + DESIRED_SPACERS_BETWEEN_SECTIONS;
            options_height =
                full_options_height.min(remaining.saturating_sub(reserved).max(min_options_height));
            remaining -= options_height;
            progress_height = progress_pref.min(remaining);
            remaining -= progress_height;

            spacer_after_options = u16::from(remaining > footer_pref);
            remaining -= spacer_after_options;
            footer_lines = footer_pref.min(remaining);
            remaining -= footer_lines;
            spacer_after_question = u16::from(remaining > 0);
            remaining -= spacer_after_question;
            options_height += remaining.min(full_options_height.saturating_sub(options_height));
            notes_height = 0;
        } else {
            // Freeform answers take their preferred input height before hints and progress.
            let preferred_notes = self
                .composer
                .inline_input_height(area.width)
                .clamp(1, 8)
                .saturating_sub(min_notes)
                .min(remaining);
            remaining -= preferred_notes;
            footer_lines = footer_pref.min(remaining);
            remaining -= footer_lines;
            progress_height = progress_pref.min(remaining);
            remaining -= progress_height;
            spacer_after_input = u16::from(remaining > 0);
            remaining -= spacer_after_input;
            // Drop padding before input or prompt content in a constrained viewport.
            spacer_before_question = u16::from(remaining >= 2 && self.unanswered_count() > 1);
            spacer_after_question = u16::from(remaining >= 1);
            notes_height = min_notes + preferred_notes + remaining
                - spacer_before_question
                - spacer_after_question;
        }

        let mut y = area.y;
        let mut take_rows = |height| {
            let rows = Rect::new(area.x, y, area.width, height);
            y = y.saturating_add(height);
            rows
        };
        let progress_area = take_rows(progress_height);
        take_rows(spacer_before_question);
        let question_area = take_rows(question_height);
        take_rows(spacer_after_question);
        let options_area = take_rows(options_height);
        take_rows(spacer_after_options);
        let notes_area = take_rows(notes_height);
        LayoutSections {
            progress_area,
            question_area,
            question_lines,
            options_area,
            notes_area,
            footer_lines,
            spacer_after_input,
        }
    }
}

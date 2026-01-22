use ratatui::layout::Rect;

use super::RequestUserInputOverlay;

pub(super) struct LayoutSections {
    pub(super) progress_area: Rect,
    pub(super) header_area: Rect,
    pub(super) question_area: Rect,
    pub(super) answer_title_area: Rect,
    // Wrapped question text lines to render in the question area.
    pub(super) question_lines: Vec<String>,
    pub(super) options_area: Rect,
    pub(super) notes_title_area: Rect,
    pub(super) notes_area: Rect,
    // Number of footer rows (status + hints).
    pub(super) footer_lines: u16,
}

impl RequestUserInputOverlay {
    /// Compute layout sections, collapsing notes and hints as space shrinks.
    pub(super) fn layout_sections(&self, area: Rect) -> LayoutSections {
        let question_lines = self
            .current_question()
            .map(|q| {
                textwrap::wrap(&q.question, area.width.max(1) as usize)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let question_text_height = question_lines.len() as u16;
        let has_options = self.has_options();
        let mut notes_input_height = self.notes_input_height(area.width);
        // Keep the question + options visible first; notes and hints collapse as space shrinks.
        let footer_lines = if self.unanswered_count() > 0 { 2 } else { 1 };
        let mut notes_title_height = if has_options { 1 } else { 0 };

        let mut cursor_y = area.y;
        let progress_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: 1,
        };
        cursor_y = cursor_y.saturating_add(1);
        let header_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: 1,
        };
        cursor_y = cursor_y.saturating_add(1);
        let question_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: question_text_height,
        };
        cursor_y = cursor_y.saturating_add(question_text_height);
        // Remaining height after progress/header/question areas.
        let remaining = area.height.saturating_sub(cursor_y.saturating_sub(area.y));
        let mut answer_title_height = if has_options { 1 } else { 0 };
        let mut options_height = 0;
        if has_options {
            let remaining_content = remaining.saturating_sub(footer_lines);
            let options_len = self.options_len() as u16;
            if remaining_content == 0 {
                answer_title_height = 0;
                notes_title_height = 0;
                notes_input_height = 0;
                options_height = 0;
            } else {
                let min_notes = 1u16;
                let full_notes = 3u16;
                // Prefer to keep all options visible, then allocate notes height.
                if remaining_content
                    >= options_len + answer_title_height + notes_title_height + full_notes
                {
                    let max_notes = remaining_content
                        .saturating_sub(options_len)
                        .saturating_sub(answer_title_height)
                        .saturating_sub(notes_title_height);
                    notes_input_height = notes_input_height.min(max_notes).max(full_notes);
                } else if remaining_content > options_len + answer_title_height + min_notes {
                    notes_title_height = 0;
                    notes_input_height = min_notes;
                } else {
                    // Tight layout: hide section titles and shrink notes to one line.
                    answer_title_height = 0;
                    notes_title_height = 0;
                    notes_input_height = min_notes;
                }

                // Reserve notes/answer title area so options are scrollable if needed.
                let reserved = answer_title_height
                    .saturating_add(notes_title_height)
                    .saturating_add(notes_input_height);
                options_height = remaining_content.saturating_sub(reserved);
            }
        } else {
            let max_notes = remaining.saturating_sub(footer_lines);
            if max_notes == 0 {
                notes_input_height = 0;
            } else {
                // When no options exist, notes are the primary input.
                notes_input_height = notes_input_height.min(max_notes).max(3.min(max_notes));
            }
        }

        let answer_title_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: answer_title_height,
        };
        cursor_y = cursor_y.saturating_add(answer_title_height);
        let options_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: options_height,
        };
        cursor_y = cursor_y.saturating_add(options_height);

        let notes_title_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: notes_title_height,
        };
        cursor_y = cursor_y.saturating_add(notes_title_height);
        let notes_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: notes_input_height,
        };

        LayoutSections {
            progress_area,
            header_area,
            question_area,
            answer_title_area,
            question_lines,
            options_area,
            notes_title_area,
            notes_area,
            footer_lines,
        }
    }
}

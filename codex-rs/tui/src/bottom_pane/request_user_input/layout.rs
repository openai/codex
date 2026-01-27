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
        let has_options = self.has_options();
        let footer_pref = if self.unanswered_count() > 0 { 2 } else { 1 };
        let notes_pref_height = self.notes_input_height(area.width);
        let mut question_lines = self.wrapped_question_lines(area.width);
        let mut question_height = question_lines.len() as u16;

        let mut progress_height = 0;
        let mut answer_title_height = 0;
        let mut notes_title_height = 0;
        let mut notes_height = 0;
        let mut options_height = 0;
        let mut footer_lines = 0;

        if has_options {
            let options_required_height = self.options_required_height(area.width);
            let min_options_height = 1u16;
            let required = 1u16
                .saturating_add(question_height)
                .saturating_add(options_required_height);

            if required > area.height {
                // Tight layout: allocate header + question + options first and drop everything else.
                let max_question_height = area
                    .height
                    .saturating_sub(1u16.saturating_add(min_options_height));
                question_height = question_height.min(max_question_height);
                question_lines.truncate(question_height as usize);
                options_height = area
                    .height
                    .saturating_sub(1u16.saturating_add(question_height));
            } else {
                options_height = options_required_height;
                let used = 1u16
                    .saturating_add(question_height)
                    .saturating_add(options_height);
                let mut remaining = area.height.saturating_sub(used);

                // Prefer notes next, then footer, then labels, with progress last.
                notes_height = notes_pref_height.min(remaining);
                remaining = remaining.saturating_sub(notes_height);

                footer_lines = footer_pref.min(remaining);
                remaining = remaining.saturating_sub(footer_lines);

                if remaining > 0 {
                    answer_title_height = 1;
                    remaining = remaining.saturating_sub(1);
                }
                if remaining > 0 {
                    notes_title_height = 1;
                    remaining = remaining.saturating_sub(1);
                }
                if remaining > 0 {
                    progress_height = 1;
                    remaining = remaining.saturating_sub(1);
                }

                // Expand the notes composer with any leftover rows.
                notes_height = notes_height.saturating_add(remaining);
            }
        } else {
            let required = 1u16.saturating_add(question_height);
            if required > area.height {
                let max_question_height = area.height.saturating_sub(1);
                question_height = question_height.min(max_question_height);
                question_lines.truncate(question_height as usize);
            } else {
                let mut remaining = area.height.saturating_sub(required);
                notes_height = notes_pref_height.min(remaining);
                remaining = remaining.saturating_sub(notes_height);

                footer_lines = footer_pref.min(remaining);
                remaining = remaining.saturating_sub(footer_lines);

                if remaining > 0 {
                    progress_height = 1;
                    remaining = remaining.saturating_sub(1);
                }

                notes_height = notes_height.saturating_add(remaining);
            }
        }

        let mut cursor_y = area.y;
        let progress_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: progress_height,
        };
        cursor_y = cursor_y.saturating_add(progress_height);
        let header_height = area.height.saturating_sub(progress_height).min(1);
        let header_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: header_height,
        };
        cursor_y = cursor_y.saturating_add(header_height);
        let question_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: question_height,
        };
        cursor_y = cursor_y.saturating_add(question_height);

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
            height: notes_height,
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

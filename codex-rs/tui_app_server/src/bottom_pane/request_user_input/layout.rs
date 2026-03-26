use ratatui::layout::Rect;

use super::DESIRED_SPACERS_BETWEEN_SECTIONS;
use super::RequestUserInputOverlay;

pub(super) struct LayoutSections {
    pub(super) progress_area: Rect,
    pub(super) question_area: Rect,
    // Wrapped question text lines to render in the question area.
    pub(super) question_lines: Vec<String>,
    pub(super) options_area: Rect,
    pub(super) notes_area: Rect,
    // Number of footer rows (status + hints).
    pub(super) footer_lines: u16,
}

impl RequestUserInputOverlay {
    /// Compute layout sections, collapsing notes and hints as space shrinks.
    pub(super) fn layout_sections(&self, area: Rect) -> LayoutSections {
        let has_options = self.has_options();
        let notes_visible = !has_options || self.notes_ui_visible();
        let footer_pref = self.footer_required_height(area.width);
        let notes_pref_height = self.notes_input_height(area.width);
        let mut question_lines = self.wrapped_question_lines(area.width);
        let question_height = question_lines.len() as u16;

        let layout = if has_options {
            self.layout_with_options(
                OptionsLayoutArgs {
                    available_height: area.height,
                    width: area.width,
                    question_height,
                    notes_pref_height,
                    footer_pref,
                    notes_visible,
                },
                &mut question_lines,
            )
        } else {
            self.layout_without_options(
                area.height,
                question_height,
                notes_pref_height,
                footer_pref,
                &mut question_lines,
            )
        };

        let (progress_area, question_area, options_area, notes_area) =
            self.build_layout_areas(area, layout);

        LayoutSections {
            progress_area,
            question_area,
            question_lines,
            options_area,
            notes_area,
            footer_lines: layout.footer_lines,
        }
    }

    /// Layout calculation when options are present.
    fn layout_with_options(
        &self,
        args: OptionsLayoutArgs,
        question_lines: &mut Vec<String>,
    ) -> LayoutPlan {
        let OptionsLayoutArgs {
            available_height,
            width,
            mut question_height,
            notes_pref_height,
            footer_pref,
            notes_visible,
        } = args;
        let min_options_height = available_height.min(1);
        let max_question_height = available_height.saturating_sub(min_options_height);
        if question_height > max_question_height {
            question_height = max_question_height;
            question_lines.truncate(question_height as usize);
        }
        self.layout_with_options_normal(
            OptionsNormalArgs {
                available_height,
                question_height,
                notes_pref_height,
                footer_pref,
                notes_visible,
            },
            OptionsHeights {
                preferred: self.options_preferred_height(width),
                full: self.options_required_height(width),
            },
        )
    }

    /// Normal layout for options case: allocate footer + progress first, and
    /// only allocate notes (and its label) when explicitly visible.
    fn layout_with_options_normal(
        &self,
        args: OptionsNormalArgs,
        options: OptionsHeights,
    ) -> LayoutPlan {
        let OptionsNormalArgs {
            available_height,
            question_height,
            notes_pref_height,
            footer_pref,
            notes_visible,
        } = args;
        let max_options_height = available_height.saturating_sub(question_height);
        let min_options_height = max_options_height.min(1);
        let mut options_height = options
            .preferred
            .min(max_options_height)
            .max(min_options_height);
        let used = question_height.saturating_add(options_height);
        let mut remaining = available_height.saturating_sub(used);

        // When notes are hidden, prefer to reserve room for progress, footer,
        // and spacers by shrinking the options window if needed.
        let desired_spacers = if notes_visible {
            // Notes already separate options from the footer, so only keep a
            // single spacer between the question and options.
            1
        } else {
            DESIRED_SPACERS_BETWEEN_SECTIONS
        };
        let required_extra = footer_pref
            .saturating_add(1) // progress line
            .saturating_add(desired_spacers);
        if remaining < required_extra {
            let deficit = required_extra.saturating_sub(remaining);
            let reducible = options_height.saturating_sub(min_options_height);
            let reduce_by = deficit.min(reducible);
            options_height = options_height.saturating_sub(reduce_by);
            remaining = remaining.saturating_add(reduce_by);
        }

        let mut progress_height = 0;
        if remaining > 0 {
            progress_height = 1;
            remaining = remaining.saturating_sub(1);
        }

        if !notes_visible {
            let mut spacer_after_options = 0;
            if remaining > footer_pref {
                spacer_after_options = 1;
                remaining = remaining.saturating_sub(1);
            }
            let footer_lines = footer_pref.min(remaining);
            remaining = remaining.saturating_sub(footer_lines);
            let mut spacer_after_question = 0;
            if remaining > 0 {
                spacer_after_question = 1;
                remaining = remaining.saturating_sub(1);
            }
            let grow_by = remaining.min(options.full.saturating_sub(options_height));
            options_height = options_height.saturating_add(grow_by);
            return LayoutPlan {
                question_height,
                progress_height,
                spacer_after_question,
                options_height,
                spacer_after_options,
                notes_height: 0,
                footer_lines,
            };
        }

        let footer_lines = footer_pref.min(remaining);
        remaining = remaining.saturating_sub(footer_lines);

        // Prefer spacers before notes, then notes.
        let mut spacer_after_question = 0;
        if remaining > 0 {
            spacer_after_question = 1;
            remaining = remaining.saturating_sub(1);
        }
        let spacer_after_options = 0;
        let mut notes_height = notes_pref_height.min(remaining);
        remaining = remaining.saturating_sub(notes_height);

        notes_height = notes_height.saturating_add(remaining);

        LayoutPlan {
            question_height,
            progress_height,
            spacer_after_question,
            options_height,
            spacer_after_options,
            notes_height,
            footer_lines,
        }
    }

    /// Layout calculation when no options are present.
    ///
    /// Handles both tight layout (when space is constrained) and normal layout
    /// (when there's sufficient space for all elements).
    ///
    fn layout_without_options(
        &self,
        available_height: u16,
        question_height: u16,
        notes_pref_height: u16,
        footer_pref: u16,
        question_lines: &mut Vec<String>,
    ) -> LayoutPlan {
        let required = question_height;
        if required > available_height {
            self.layout_without_options_tight(available_height, question_height, question_lines)
        } else {
            self.layout_without_options_normal(
                available_height,
                question_height,
                notes_pref_height,
                footer_pref,
            )
        }
    }

    /// Tight layout for no-options case: truncate question to fit available space.
    fn layout_without_options_tight(
        &self,
        available_height: u16,
        question_height: u16,
        question_lines: &mut Vec<String>,
    ) -> LayoutPlan {
        let max_question_height = available_height;
        let adjusted_question_height = question_height.min(max_question_height);
        question_lines.truncate(adjusted_question_height as usize);

        LayoutPlan {
            question_height: adjusted_question_height,
            progress_height: 0,
            spacer_after_question: 0,
            options_height: 0,
            spacer_after_options: 0,
            notes_height: 0,
            footer_lines: 0,
        }
    }

    /// Normal layout for no-options case: allocate space for notes, footer, and progress.
    fn layout_without_options_normal(
        &self,
        available_height: u16,
        question_height: u16,
        notes_pref_height: u16,
        footer_pref: u16,
    ) -> LayoutPlan {
        let required = question_height;
        let mut remaining = available_height.saturating_sub(required);
        let mut notes_height = notes_pref_height.min(remaining);
        remaining = remaining.saturating_sub(notes_height);

        let footer_lines = footer_pref.min(remaining);
        remaining = remaining.saturating_sub(footer_lines);

        let mut progress_height = 0;
        if remaining > 0 {
            progress_height = 1;
            remaining = remaining.saturating_sub(1);
        }

        notes_height = notes_height.saturating_add(remaining);

        LayoutPlan {
            question_height,
            progress_height,
            spacer_after_question: 0,
            options_height: 0,
            spacer_after_options: 0,
            notes_height,
            footer_lines,
        }
    }

    /// Build the final layout areas from computed heights.
    fn build_layout_areas(
        &self,
        area: Rect,
        heights: LayoutPlan,
    ) -> (
        Rect, // progress_area
        Rect, // question_area
        Rect, // options_area
        Rect, // notes_area
    ) {
        let mut cursor_y = area.y;
        let progress_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: heights.progress_height,
        };
        cursor_y = cursor_y.saturating_add(heights.progress_height);
        let question_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: heights.question_height,
        };
        cursor_y = cursor_y.saturating_add(heights.question_height);
        cursor_y = cursor_y.saturating_add(heights.spacer_after_question);

        let options_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: heights.options_height,
        };
        cursor_y = cursor_y.saturating_add(heights.options_height);
        cursor_y = cursor_y.saturating_add(heights.spacer_after_options);

        let notes_area = Rect {
            x: area.x,
            y: cursor_y,
            width: area.width,
            height: heights.notes_height,
        };

        (progress_area, question_area, options_area, notes_area)
    }
}

#[derive(Clone, Copy, Debug)]
struct LayoutPlan {
    progress_height: u16,
    question_height: u16,
    spacer_after_question: u16,
    options_height: u16,
    spacer_after_options: u16,
    notes_height: u16,
    footer_lines: u16,
}

#[derive(Clone, Copy, Debug)]
struct OptionsLayoutArgs {
    available_height: u16,
    width: u16,
    question_height: u16,
    notes_pref_height: u16,
    footer_pref: u16,
    notes_visible: bool,
}

#[derive(Clone, Copy, Debug)]
struct OptionsNormalArgs {
    available_height: u16,
    question_height: u16,
    notes_pref_height: u16,
    footer_pref: u16,
    notes_visible: bool,
}

#[derive(Clone, Copy, Debug)]
struct OptionsHeights {
    preferred: u16,
    full: u16,
}

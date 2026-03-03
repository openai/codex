use std::borrow::Cow;

use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::ChatComposer;
use crate::bottom_pane::ChatComposerConfig;
use crate::bottom_pane::scroll_state::ScrollState;
use crate::bottom_pane::selection_popup_common::GenericDisplayRow;
use crate::bottom_pane::selection_popup_common::measure_rows_height;
use crate::bottom_pane::selection_popup_common::menu_surface_inset;
use crate::bottom_pane::selection_popup_common::menu_surface_padding_height;
use crate::bottom_pane::selection_popup_common::render_menu_surface;
use crate::bottom_pane::selection_popup_common::render_rows;
use crate::bottom_pane::selection_popup_common::wrap_styled_line;
use crate::render::renderable::Renderable;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

use super::ApprovalOption;
use super::ApprovalRequest;
use super::build_header;

pub(super) const PATCH_REJECT_OPTION_INDEX: usize = 2;
const PATCH_NOTES_PLACEHOLDER: &str = "Tell Codex what to do differently";
const PATCH_EMPTY_NOTES_MESSAGE: &str = "Add guidance before sending.";
const PATCH_MIN_OVERLAY_HEIGHT: u16 = 8;
const PATCH_MIN_COMPOSER_HEIGHT: u16 = 3;
const PATCH_MAX_COMPOSER_HEIGHT: u16 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PatchFocus {
    Options,
    Notes,
}

pub(super) struct PatchOverlayState {
    pub(super) focus: PatchFocus,
    pub(super) composer: ChatComposer,
    pub(super) notes_visible: bool,
    pub(super) note_submit_attempted: bool,
}

impl PatchOverlayState {
    pub(super) fn new(app_event_tx: &AppEventSender) -> Self {
        let mut composer = ChatComposer::new_with_config(
            true,
            app_event_tx.clone(),
            false,
            PATCH_NOTES_PLACEHOLDER.to_string(),
            false,
            ChatComposerConfig::plain_text(),
        );
        composer.set_footer_hint_override(Some(Vec::new()));
        Self {
            focus: PatchFocus::Options,
            composer,
            notes_visible: false,
            note_submit_attempted: false,
        }
    }

    pub(super) fn focus_is_notes(&self) -> bool {
        matches!(self.focus, PatchFocus::Notes)
    }

    pub(super) fn note_text(&self) -> String {
        self.composer.current_text_with_pending()
    }

    pub(super) fn notes_visible(&self, selected_idx: Option<usize>) -> bool {
        selected_idx == Some(PATCH_REJECT_OPTION_INDEX)
            && (self.notes_visible || !self.note_text().trim().is_empty())
    }

    pub(super) fn note_error_visible(&self, selected_idx: Option<usize>) -> bool {
        self.notes_visible(selected_idx)
            && self.note_submit_attempted
            && self.note_text().trim().is_empty()
    }

    fn notes_input_height(&self, width: u16) -> u16 {
        self.composer
            .desired_height(width.max(1))
            .clamp(PATCH_MIN_COMPOSER_HEIGHT, PATCH_MAX_COMPOSER_HEIGHT)
    }
}

pub(super) struct PatchLayout {
    title_lines: Vec<Line<'static>>,
    header: Box<dyn Renderable>,
    header_height: u16,
    rows: Vec<GenericDisplayRow>,
    options_state: ScrollState,
    options_height: u16,
    hint_lines: Vec<Line<'static>>,
    validation_lines: Vec<Line<'static>>,
    notes_height: u16,
    show_notes: bool,
}

impl PatchLayout {
    pub(super) fn new(
        request: &ApprovalRequest,
        options: &[ApprovalOption],
        options_state: ScrollState,
        state: Option<&PatchOverlayState>,
        width: u16,
    ) -> Option<Self> {
        if !matches!(request, ApprovalRequest::ApplyPatch { .. }) {
            return None;
        }

        let width = menu_surface_inset(Rect::new(0, 0, width, u16::MAX))
            .width
            .max(1);
        let title_lines = wrap_patch_title(width);
        let header = build_header(request);
        let header_height = header.desired_height(width);
        let mut options_state = options_state;
        if options_state.selected_idx.is_none() {
            options_state.selected_idx = Some(0);
        }
        let rows_width = width.saturating_add(2);
        let rows = patch_option_rows(options, options_state.selected_idx);
        let options_height =
            measure_rows_height(&rows, &options_state, rows.len().max(1), rows_width.max(1));
        let hint_lines = patch_hint_lines(request, options_state.selected_idx, state, width);
        let validation_lines = patch_validation_lines(options_state.selected_idx, state, width);
        let show_notes = state.is_some_and(|state| state.notes_visible(options_state.selected_idx));
        let notes_height = if show_notes {
            state
                .map(|state| state.notes_input_height(width))
                .unwrap_or(PATCH_MIN_COMPOSER_HEIGHT)
        } else {
            0
        };

        Some(Self {
            title_lines,
            header,
            header_height,
            rows,
            options_state,
            options_height,
            hint_lines,
            validation_lines,
            notes_height,
            show_notes,
        })
    }

    pub(super) fn total_height(&self) -> u16 {
        let height = self.title_lines.len() as u16
            + 1
            + self.header_height
            + 1
            + self.options_height
            + 1
            + self.hint_lines.len() as u16
            + self.notes_height
            + self.validation_lines.len() as u16
            + menu_surface_padding_height();
        height.max(PATCH_MIN_OVERLAY_HEIGHT)
    }

    pub(super) fn render(&self, area: Rect, state: Option<&PatchOverlayState>, buf: &mut Buffer) {
        let content_area = render_menu_surface(area, buf);
        if content_area.width == 0 || content_area.height == 0 {
            return;
        }

        let notes_area = self.notes_area(content_area);
        let mut cursor_y = content_area.y;
        for line in &self.title_lines {
            Paragraph::new(line.clone()).render(
                Rect {
                    x: content_area.x,
                    y: cursor_y,
                    width: content_area.width,
                    height: 1,
                },
                buf,
            );
            cursor_y = cursor_y.saturating_add(1);
        }
        cursor_y = cursor_y.saturating_add(1);

        self.header.render(
            Rect {
                x: content_area.x,
                y: cursor_y,
                width: content_area.width,
                height: self.header_height.min(
                    content_area
                        .height
                        .saturating_sub(cursor_y - content_area.y),
                ),
            },
            buf,
        );
        cursor_y = cursor_y
            .saturating_add(self.header_height)
            .saturating_add(1);

        render_rows(
            Rect {
                x: content_area.x.saturating_sub(2),
                y: cursor_y,
                width: content_area.width.saturating_add(2),
                height: self.options_height,
            },
            buf,
            &self.rows,
            &self.options_state,
            self.rows.len().max(1),
            "No options",
        );
        cursor_y = cursor_y.saturating_add(self.options_height);
        cursor_y = cursor_y.saturating_add(1);

        for line in &self.hint_lines {
            Paragraph::new(line.clone()).render(
                Rect {
                    x: content_area.x,
                    y: cursor_y,
                    width: content_area.width,
                    height: 1,
                },
                buf,
            );
            cursor_y = cursor_y.saturating_add(1);
        }

        if let (Some(state), Some(notes_area)) = (state, notes_area) {
            state.composer.render(notes_area, buf);
            cursor_y = cursor_y.saturating_add(notes_area.height);
        }

        for line in &self.validation_lines {
            Paragraph::new(line.clone()).render(
                Rect {
                    x: content_area.x,
                    y: cursor_y,
                    width: content_area.width,
                    height: 1,
                },
                buf,
            );
            cursor_y = cursor_y.saturating_add(1);
        }
    }

    pub(super) fn cursor_pos(&self, area: Rect, state: &PatchOverlayState) -> Option<(u16, u16)> {
        let content_area = menu_surface_inset(area);
        if content_area.width == 0 || content_area.height == 0 {
            return None;
        }
        self.notes_area(content_area)
            .and_then(|notes_area| state.composer.cursor_pos(notes_area))
    }

    fn notes_area(&self, content_area: Rect) -> Option<Rect> {
        if !self.show_notes {
            return None;
        }

        let notes_y = content_area.y
            + self.title_lines.len() as u16
            + 1
            + self.header_height
            + 1
            + self.options_height
            + 1
            + self.hint_lines.len() as u16;
        let validation_height = self.validation_lines.len() as u16;
        Some(Rect {
            x: content_area.x,
            y: notes_y,
            width: content_area.width,
            height: self.notes_height.min(
                content_area
                    .height
                    .saturating_sub(notes_y.saturating_sub(content_area.y) + validation_height),
            ),
        })
    }
}

fn wrap_patch_title(width: u16) -> Vec<Line<'static>> {
    let line = Line::from("Would you like to make the following edits?".bold());
    wrap_styled_line(&line, width.max(1))
        .into_iter()
        .map(line_to_owned)
        .collect()
}

fn patch_option_rows(
    options: &[ApprovalOption],
    selected_idx: Option<usize>,
) -> Vec<GenericDisplayRow> {
    options
        .iter()
        .enumerate()
        .map(|(idx, option)| {
            let prefix = if selected_idx == Some(idx) {
                '›'
            } else {
                ' '
            };
            let prefix_label = format!("{prefix} {}. ", idx + 1);
            GenericDisplayRow {
                name: format!("{prefix_label}{}", option.label),
                display_shortcut: option
                    .display_shortcut
                    .or_else(|| option.additional_shortcuts.first().copied()),
                wrap_indent: Some(UnicodeWidthStr::width(prefix_label.as_str())),
                ..Default::default()
            }
        })
        .collect()
}

fn patch_hint_lines(
    request: &ApprovalRequest,
    selected_idx: Option<usize>,
    state: Option<&PatchOverlayState>,
    width: u16,
) -> Vec<Line<'static>> {
    let mut hint = if state.is_some_and(|state| state.notes_visible(selected_idx)) {
        if state.is_some_and(PatchOverlayState::focus_is_notes) {
            "Press enter to send or tab to go back or esc to cancel".to_string()
        } else {
            "Press enter or tab to edit follow up or esc to cancel".to_string()
        }
    } else {
        "Press enter to confirm or esc to cancel".to_string()
    };
    if request.thread_label().is_some() {
        hint.push_str(" | o to open thread");
    }

    wrap_styled_line(&Line::from(hint).dim(), width.max(1))
        .into_iter()
        .map(line_to_owned)
        .collect()
}

fn patch_validation_lines(
    selected_idx: Option<usize>,
    state: Option<&PatchOverlayState>,
    width: u16,
) -> Vec<Line<'static>> {
    if !state.is_some_and(|state| state.note_error_visible(selected_idx)) {
        return Vec::new();
    }

    wrap_styled_line(&Line::from(PATCH_EMPTY_NOTES_MESSAGE).red(), width.max(1))
        .into_iter()
        .map(line_to_owned)
        .collect()
}

fn line_to_owned(line: Line<'_>) -> Line<'static> {
    Line {
        style: line.style,
        alignment: line.alignment,
        spans: line
            .spans
            .into_iter()
            .map(|span| Span {
                style: span.style,
                content: Cow::Owned(span.content.into_owned()),
            })
            .collect(),
    }
}

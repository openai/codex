use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Stylize as _;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Tabs;
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

use codex_protocol::ask_user_question::AskUserQuestionKind;
use crossterm::event::KeyCode;

use crate::key_hint;
use crate::render::renderable::Renderable;

use super::AskUserQuestionOverlay;
use super::CUSTOM_PLACEHOLDER_DEFAULT;
use super::Focus;
use super::SubmitFocus;

const TAB_ICON_UNANSWERED: &str = "☐";
const TAB_ICON_ANSWERED: &str = "☑";

const TAB_SELECTED_BG: Color = Color::LightBlue;
const TAB_SELECTED_FG: Color = Color::Black;

impl Renderable for AskUserQuestionOverlay {
    fn desired_height(&self, _width: u16) -> u16 {
        let header_height = 1usize; // tabs
        let spacer_height = 1usize;
        let body_spacer_height = 1usize;
        let poll_input_separator_height = 1usize;
        let input_height = 1usize;
        let controls_separator_height = 1usize;
        let controls_height = 1usize;
        let controls_bottom_separator_height = 1usize;

        let body_height = if self.is_submit_tab() {
            let list_height = self.request.questions.len().max(1);
            // title(2) + list + buttons(2)
            2usize.saturating_add(list_height).saturating_add(2)
        } else {
            let options_len = self
                .current_question()
                .map(|q| q.options.len())
                .unwrap_or(0);
            let visible_items = options_len.clamp(1, 4);
            let options_height = visible_items.saturating_mul(2); // label + description
            // question(2) + options
            2usize.saturating_add(options_height)
        };

        let height = header_height
            .saturating_add(spacer_height)
            .saturating_add(body_height)
            .saturating_add(body_spacer_height)
            .saturating_add(poll_input_separator_height)
            .saturating_add(input_height)
            .saturating_add(controls_separator_height)
            .saturating_add(controls_height)
            .saturating_add(controls_bottom_separator_height)
            .max(14);
        height.try_into().unwrap_or(u16::MAX)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.render_ui(area, buf);
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.cursor_pos_impl(area)
    }
}

impl AskUserQuestionOverlay {
    fn render_ui(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        Clear.render(area, buf);

        let [
            tabs_area,
            spacer_area,
            body_area,
            body_spacer_area,
            poll_input_separator_area,
            input_area,
            controls_top_separator_area,
            controls_area,
            controls_bottom_separator_area,
        ] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area);

        self.render_tabs(tabs_area, buf);
        Paragraph::new(Line::from("")).render(spacer_area, buf);

        if self.is_submit_tab() {
            self.render_submit(body_area, buf);
        } else {
            self.render_question(body_area, buf);
        }

        Paragraph::new(Line::from("")).render(body_spacer_area, buf);
        self.render_rule(poll_input_separator_area, buf);
        self.render_progress_hint(input_area, buf);
        self.render_rule(controls_top_separator_area, buf);
        self.render_controls(controls_area, buf);
        self.render_rule(controls_bottom_separator_area, buf);
    }

    fn render_tabs(&self, area: Rect, buf: &mut Buffer) {
        let [left_area, tabs_area, right_area] = Layout::horizontal([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .areas(area);

        Paragraph::new(Line::from("←".dim())).render(left_area, buf);
        Paragraph::new(Line::from("→".dim())).render(right_area, buf);

        let mut titles: Vec<Line<'static>> = Vec::new();
        for (idx, q) in self.request.questions.iter().enumerate() {
            let icon = if self.question_answered(idx) {
                TAB_ICON_ANSWERED
            } else {
                TAB_ICON_UNANSWERED
            };
            titles.push(Line::from(vec![
                " ".into(),
                icon.into(),
                "  ".into(),
                q.header.clone().into(),
                " ".into(),
            ]));
        }
        titles.push(Line::from(vec![
            " ".into(),
            "✓".into(),
            "  ".into(),
            "Submit".into(),
            " ".into(),
        ]));

        let tabs = Tabs::new(titles)
            .select(self.active_tab)
            .style(Style::default().dim())
            .highlight_style(
                Style::default()
                    .fg(TAB_SELECTED_FG)
                    .bg(TAB_SELECTED_BG)
                    .bold(),
            )
            .divider(" ")
            .padding("", "");
        tabs.render(tabs_area, buf);
    }

    fn render_question(&self, area: Rect, buf: &mut Buffer) {
        let [question_area, options_area] =
            Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(area);

        let Some(question) = self.current_question() else {
            Paragraph::new(Line::from("No questions".dim())).render(question_area, buf);
            return;
        };

        Paragraph::new(Line::from(question.question.clone()).bold()).render(question_area, buf);

        self.render_options(options_area, buf);
    }

    fn render_options(&self, area: Rect, buf: &mut Buffer) {
        let Some(q_idx) = self.current_question_index() else {
            return;
        };
        let question = &self.request.questions[q_idx];
        let Some(question_state) = self.current_state() else {
            return;
        };

        let options_len = question.options.len();
        let visible_items = (area.height as usize / 2).max(1).min(options_len.max(1));
        let mut state = question_state.highlight;
        state.ensure_visible(options_len, visible_items);
        let start_idx = state.scroll_top.min(options_len.saturating_sub(1));

        for (i, opt) in question
            .options
            .iter()
            .enumerate()
            .skip(start_idx)
            .take(visible_items)
        {
            let row = (i - start_idx) as u16;
            let y = area.y.saturating_add(row.saturating_mul(2));
            if y >= area.y + area.height {
                break;
            }

            let highlighted = state.selected_idx == Some(i);
            let selected_multi = question_state
                .selected_multi
                .get(i)
                .copied()
                .unwrap_or(false);
            let selected_single = question_state.selected_single == Some(i);

            let arrow = if highlighted { ">" } else { " " };
            let (prefix_string, prefix_spans): (String, Vec<Span<'static>>) = match question.kind {
                AskUserQuestionKind::SingleChoice => {
                    let prefix_string = format!("{arrow} {}. ", i + 1);
                    (
                        prefix_string.clone(),
                        vec![format!("{arrow} ").into(), format!("{}. ", i + 1).dim()],
                    )
                }
                AskUserQuestionKind::MultipleChoice => {
                    let checkbox = if selected_multi { "[x]" } else { "[ ]" };
                    let prefix_string = format!("{arrow} {checkbox} {}. ", i + 1);
                    (
                        prefix_string.clone(),
                        vec![
                            format!("{arrow} ").into(),
                            checkbox.into(),
                            " ".into(),
                            format!("{}. ", i + 1).dim(),
                        ],
                    )
                }
            };
            let prefix_width = UnicodeWidthStr::width(prefix_string.as_str());

            let (label_text, label_dim) = if opt.custom {
                let text = self
                    .custom_text(q_idx)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToString::to_string)
                    .unwrap_or_else(|| {
                        if opt.label.is_empty() {
                            CUSTOM_PLACEHOLDER_DEFAULT.to_string()
                        } else {
                            opt.label.clone()
                        }
                    });
                let dim = self
                    .custom_text(q_idx)
                    .map(str::trim)
                    .unwrap_or_default()
                    .is_empty();
                (text, dim)
            } else {
                (opt.label.clone(), false)
            };

            let is_selected = match question.kind {
                AskUserQuestionKind::SingleChoice => selected_single,
                AskUserQuestionKind::MultipleChoice => selected_multi,
            };

            let mut spans: Vec<Span<'static>> = Vec::new();
            spans.extend(prefix_spans);
            let mut label_span: Span<'static> = label_text.into();
            if highlighted || is_selected {
                label_span = label_span.cyan().bold();
            } else if label_dim {
                label_span = label_span.dim();
            }
            spans.push(label_span);

            Paragraph::new(Line::from(spans)).render(
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
                buf,
            );

            let desc_y = y.saturating_add(1);
            if desc_y >= area.y + area.height {
                break;
            }
            let desc = opt.description.as_str();
            let desc_line = if desc.is_empty() {
                Line::from("")
            } else {
                let indent = " ".repeat(prefix_width);
                Line::from(vec![indent.into(), desc.to_string().dim()])
            };
            Paragraph::new(desc_line).render(
                Rect {
                    x: area.x,
                    y: desc_y,
                    width: area.width,
                    height: 1,
                },
                buf,
            );
        }
    }

    fn render_submit(&self, area: Rect, buf: &mut Buffer) {
        let [title_area, list_area, buttons_area] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .areas(area);

        Paragraph::new(Line::from("Review your answers".bold())).render(title_area, buf);

        let mut lines: Vec<Line<'static>> = Vec::new();
        for (idx, q) in self.request.questions.iter().enumerate() {
            let answers = self.answer_for_question(idx).join(", ");
            let mut spans: Vec<Span<'static>> = vec![q.header.clone().bold(), ": ".into()];
            if !answers.is_empty() {
                spans.push(answers.cyan());
            }
            lines.push(Line::from(spans));
        }
        Paragraph::new(lines).render(list_area, buf);

        let submit_selected = matches!(self.submit_focus, SubmitFocus::Submit);
        let cancel_selected = matches!(self.submit_focus, SubmitFocus::Cancel);
        let submit = if submit_selected {
            "[ Submit ]".cyan().bold()
        } else {
            "[ Submit ]".dim()
        };
        let cancel = if cancel_selected {
            "[ Cancel ]".cyan().bold()
        } else {
            "[ Cancel ]".dim()
        };
        let buttons = Line::from(vec![submit, "  ".into(), cancel]);
        Paragraph::new(buttons).render(buttons_area, buf);
    }

    fn render_rule(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let width = usize::from(area.width);
        let line = "─".repeat(width.max(1)).dim();
        Paragraph::new(Line::from(line)).render(area, buf);
    }

    fn render_progress_hint(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let hint = if let Some(question) = self.current_question() {
            match question.kind {
                AskUserQuestionKind::SingleChoice => Line::from(vec![
                    "Select to continue. ".dim(),
                    key_hint::plain(KeyCode::Left).into(),
                    "/".into(),
                    key_hint::plain(KeyCode::Right).into(),
                    " tabs".dim(),
                ]),
                AskUserQuestionKind::MultipleChoice => Line::from(vec![
                    "Press ".dim(),
                    key_hint::plain(KeyCode::Enter).into(),
                    " for Next. ".dim(),
                    key_hint::plain(KeyCode::Left).into(),
                    "/".into(),
                    key_hint::plain(KeyCode::Right).into(),
                    " tabs".dim(),
                ]),
            }
        } else {
            Line::from(vec![
                key_hint::plain(KeyCode::Left).into(),
                "/".into(),
                key_hint::plain(KeyCode::Right).into(),
                " tabs".dim(),
            ])
        };

        Paragraph::new(hint).render(area, buf);
    }

    fn render_controls(&self, area: Rect, buf: &mut Buffer) {
        let hint = Line::from(vec![
            key_hint::plain(KeyCode::Up).into(),
            "/".into(),
            key_hint::plain(KeyCode::Down).into(),
            " move | ".into(),
            key_hint::plain(KeyCode::Enter).into(),
            " select/next | ".into(),
            key_hint::plain(KeyCode::Esc).into(),
            " cancel".into(),
        ])
        .dim();
        Paragraph::new(hint).render(area, buf);
    }

    fn cursor_pos_impl(&self, area: Rect) -> Option<(u16, u16)> {
        if self.is_submit_tab() {
            return None;
        }
        if !matches!(self.focus, Focus::Custom) {
            return None;
        }

        let [_, _, body_area, _, _, _, _, _, _] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area);
        let [_, options_area] =
            Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(body_area);

        let q_idx = self.current_question_index()?;
        let question = &self.request.questions[q_idx];
        let question_state = self.current_state()?;
        let custom_idx = question_state.custom_idx?;

        if !self.custom_is_selected(q_idx) {
            return None;
        }

        let options_len = question.options.len();
        let visible_items = (options_area.height as usize / 2)
            .max(1)
            .min(options_len.max(1));
        let mut state = question_state.highlight;
        state.ensure_visible(options_len, visible_items);
        let start_idx = state.scroll_top.min(options_len.saturating_sub(1));

        if state.selected_idx != Some(custom_idx) {
            return None;
        }

        let row = (custom_idx - start_idx) as u16;
        let y = options_area.y.saturating_add(row.saturating_mul(2));

        let selected_multi = question_state
            .selected_multi
            .get(custom_idx)
            .copied()
            .unwrap_or(false);
        let highlighted = true;
        let arrow = if highlighted { ">" } else { " " };
        let prefix_string = match question.kind {
            AskUserQuestionKind::SingleChoice => format!("{arrow} {}. ", custom_idx + 1),
            AskUserQuestionKind::MultipleChoice => {
                let checkbox = if selected_multi { "[x]" } else { "[ ]" };
                format!("{arrow} {checkbox} {}. ", custom_idx + 1)
            }
        };

        let prefix_width = UnicodeWidthStr::width(prefix_string.as_str()) as u16;
        let entry = question_state.custom_entry.as_ref()?;
        let cursor_bytes = entry.text.cursor().min(entry.text.text().len());
        let cursor_col = UnicodeWidthStr::width(&entry.text.text()[..cursor_bytes]) as u16;

        Some((
            options_area
                .x
                .saturating_add(prefix_width)
                .saturating_add(cursor_col),
            y,
        ))
    }
}

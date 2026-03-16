use std::cell::Cell;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

use crate::bottom_pane::BuiltinCommandFlags;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::popup_consts::MAX_POPUP_ROWS;
use crate::bottom_pane::selection_popup_common::render_menu_surface;
use crate::bottom_pane::visible_builtins_for_input;
use crate::key_hint;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;

const HELP_VIEW_MIN_BODY_ROWS: u16 = 6;

#[derive(Clone, Copy)]
enum HelpRowWrap {
    None,
    Note,
    Description,
    Usage,
}

#[derive(Clone)]
struct HelpRow {
    plain_text: String,
    line: Line<'static>,
    wrap: HelpRowWrap,
}

#[derive(Default)]
struct HelpSearch {
    active_query: String,
    input: Option<String>,
    selected_match: usize,
}

pub(crate) struct SlashHelpView {
    complete: bool,
    rows: Vec<HelpRow>,
    scroll_top: Cell<usize>,
    follow_selected_match: Cell<bool>,
    search: HelpSearch,
}

impl SlashHelpView {
    pub(crate) fn new(flags: BuiltinCommandFlags) -> Self {
        Self {
            complete: false,
            rows: Self::build_document(flags),
            scroll_top: Cell::new(0),
            follow_selected_match: Cell::new(false),
            search: HelpSearch::default(),
        }
    }

    fn visible_body_rows(area_height: u16) -> usize {
        area_height
            .saturating_sub(3)
            .max(HELP_VIEW_MIN_BODY_ROWS)
            .into()
    }

    fn build_document(flags: BuiltinCommandFlags) -> Vec<HelpRow> {
        let mut rows = vec![
            HelpRow {
                plain_text: "Slash Commands".to_string(),
                line: Line::from("Slash Commands".bold()),
                wrap: HelpRowWrap::None,
            },
            HelpRow {
                plain_text: String::new(),
                line: Line::from(""),
                wrap: HelpRowWrap::None,
            },
            HelpRow {
                plain_text: "Type / to open the command popup. For commands with both a picker and an arg form, bare /command opens the picker and /command ... runs directly.".to_string(),
                line: Line::from(
                    "Type / to open the command popup. For commands with both a picker and an arg form, bare /command opens the picker and /command ... runs directly."
                        .dim(),
                ),
                wrap: HelpRowWrap::Note,
            },
            HelpRow {
                plain_text: "Args use shell-style quoting; quote values with spaces.".to_string(),
                line: Line::from("Args use shell-style quoting; quote values with spaces.".dim()),
                wrap: HelpRowWrap::Note,
            },
            HelpRow {
                plain_text: String::new(),
                line: Line::from(""),
                wrap: HelpRowWrap::None,
            },
        ];

        for cmd in visible_builtins_for_input(flags) {
            rows.push(HelpRow {
                plain_text: format!("/{}", cmd.command()),
                line: Line::from(format!("/{}", cmd.command()).cyan().bold()),
                wrap: HelpRowWrap::None,
            });
            rows.push(HelpRow {
                plain_text: format!("  {}", cmd.description()),
                line: Line::from(format!("  {}", cmd.description()).dim()),
                wrap: HelpRowWrap::Description,
            });
            rows.push(HelpRow {
                plain_text: "  Usage:".to_string(),
                line: Line::from("  Usage:".dim()),
                wrap: HelpRowWrap::None,
            });
            for form in cmd.help_forms() {
                let plain_text = if form.is_empty() {
                    format!("/{}", cmd.command())
                } else {
                    format!("/{} {}", cmd.command(), form)
                };
                rows.push(HelpRow {
                    plain_text: plain_text.clone(),
                    line: Line::from(plain_text.cyan()),
                    wrap: HelpRowWrap::Usage,
                });
            }
            rows.push(HelpRow {
                plain_text: String::new(),
                line: Line::from(""),
                wrap: HelpRowWrap::None,
            });
        }

        while rows.last().is_some_and(|row| row.plain_text.is_empty()) {
            rows.pop();
        }

        rows
    }

    fn scroll_by(&mut self, delta: isize) {
        self.scroll_top
            .set(self.scroll_top.get().saturating_add_signed(delta));
        self.follow_selected_match.set(false);
    }

    fn matching_logical_rows(rows: &[HelpRow], query: &str) -> Vec<usize> {
        let query = query.to_ascii_lowercase();
        rows.iter()
            .enumerate()
            .filter_map(|(idx, row)| {
                row.plain_text
                    .to_ascii_lowercase()
                    .contains(query.as_str())
                    .then_some(idx)
            })
            .collect()
    }

    fn current_query(&self) -> Option<&str> {
        if let Some(input) = self.search.input.as_deref() {
            return (!input.is_empty()).then_some(input);
        }
        (!self.search.active_query.is_empty()).then_some(self.search.active_query.as_str())
    }

    fn search_indicator(
        &self,
        rows: &[HelpRow],
        total_rows: usize,
        visible_rows: usize,
        scroll_top: usize,
    ) -> String {
        let start_row = if total_rows == 0 { 0 } else { scroll_top + 1 };
        let end_row = (scroll_top + visible_rows).min(total_rows);
        let viewport = format!("{start_row}-{end_row}/{total_rows}");
        let Some(query) = self.current_query() else {
            return viewport;
        };
        let match_count = Self::matching_logical_rows(rows, query).len();
        if self.search.input.is_some() {
            return format!(
                "{} match{} | {viewport}",
                match_count,
                if match_count == 1 { "" } else { "es" }
            );
        }
        if match_count == 0 {
            return format!("0/0 | {viewport}");
        }
        let current_match = self.search.selected_match.min(match_count - 1) + 1;
        format!("{current_match}/{match_count} | {viewport}")
    }

    fn footer_line(&self) -> Line<'static> {
        if let Some(input) = self.search.input.as_deref() {
            return Line::from(vec![
                "Search: ".dim(),
                format!("/{input}").cyan(),
                "  |  ".dim(),
                key_hint::plain(KeyCode::Enter).into(),
                " apply  |  ".dim(),
                key_hint::plain(KeyCode::Esc).into(),
                " cancel".dim(),
            ]);
        }

        let mut spans = vec![
            key_hint::plain(KeyCode::Up).into(),
            "/".into(),
            key_hint::plain(KeyCode::Down).into(),
            " scroll  |  [".dim(),
            key_hint::ctrl(KeyCode::Char('p')).into(),
            " / ".dim(),
            key_hint::ctrl(KeyCode::Char('n')).into(),
            "] page  |  ".dim(),
            "/ search".dim(),
        ];
        if !self.search.active_query.is_empty() {
            spans.push("  |  ".dim());
            spans.push("n/p match".dim());
        }
        spans.extend([
            "  |  ".dim(),
            key_hint::plain(KeyCode::Esc).into(),
            " close".dim(),
        ]);
        Line::from(spans)
    }

    fn wrap_rows(rows: &[HelpRow], width: u16) -> (Vec<Line<'static>>, Vec<usize>, Vec<usize>) {
        let width = width.max(24);
        let note_opts = RtOptions::new(width as usize)
            .initial_indent(Line::from(""))
            .subsequent_indent(Line::from(""));
        let description_opts = RtOptions::new(width as usize)
            .initial_indent(Line::from(""))
            .subsequent_indent(Line::from("  "));
        let usage_opts = RtOptions::new(width as usize)
            .initial_indent(Line::from("      "))
            .subsequent_indent(Line::from("        "));

        let mut wrapped_rows = Vec::new();
        let mut row_starts = Vec::with_capacity(rows.len());
        let mut row_ends = Vec::with_capacity(rows.len());

        for row in rows {
            row_starts.push(wrapped_rows.len());
            let wrapped = match row.wrap {
                HelpRowWrap::None => vec![row.line.clone()],
                HelpRowWrap::Note => word_wrap_lines([row.line.clone()], note_opts.clone()),
                HelpRowWrap::Description => {
                    word_wrap_lines([row.line.clone()], description_opts.clone())
                }
                HelpRowWrap::Usage => word_wrap_lines([row.line.clone()], usage_opts.clone()),
            };
            wrapped_rows.extend(wrapped);
            row_ends.push(wrapped_rows.len());
        }

        (wrapped_rows, row_starts, row_ends)
    }

    fn move_to_match(&mut self, delta: isize) {
        if self.search.input.is_some() || self.search.active_query.is_empty() {
            return;
        }

        let matches = Self::matching_logical_rows(&self.rows, &self.search.active_query);
        if matches.is_empty() {
            return;
        }

        let next = (self.search.selected_match as isize + delta).rem_euclid(matches.len() as isize);
        self.search.selected_match = next as usize;
        self.follow_selected_match.set(true);
    }
}

impl BottomPaneView for SlashHelpView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if let Some(input) = self.search.input.as_mut() {
            match key_event {
                KeyEvent {
                    code: KeyCode::Esc, ..
                } => {
                    self.search.input = None;
                }
                KeyEvent {
                    code: KeyCode::Enter,
                    ..
                } => {
                    self.search.active_query = self.search.input.take().unwrap_or_default();
                    self.search.selected_match = 0;
                    self.follow_selected_match
                        .set(!self.search.active_query.is_empty());
                }
                KeyEvent {
                    code: KeyCode::Backspace,
                    ..
                } => {
                    input.pop();
                }
                KeyEvent {
                    code: KeyCode::Char(c),
                    modifiers,
                    ..
                } if !modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                    input.push(c);
                }
                _ => {}
            }
            return;
        }

        match key_event {
            KeyEvent {
                code: KeyCode::Char('/'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.search.active_query.clear();
                self.search.selected_match = 0;
                self.follow_selected_match.set(false);
                self.search.input = Some(String::new());
            }
            KeyEvent {
                code: KeyCode::Up, ..
            }
            | KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.scroll_by(-1),
            KeyEvent {
                code: KeyCode::Down,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.scroll_by(1),
            KeyEvent {
                code: KeyCode::PageUp,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.scroll_by(-(MAX_POPUP_ROWS as isize)),
            KeyEvent {
                code: KeyCode::PageDown,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.scroll_by(MAX_POPUP_ROWS as isize),
            KeyEvent {
                code: KeyCode::Esc, ..
            } if !self.search.active_query.is_empty() => {
                self.search.active_query.clear();
                self.search.selected_match = 0;
                self.follow_selected_match.set(false);
            }
            KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.on_ctrl_c();
            }
            KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.move_to_match(1),
            KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('N'),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => self.move_to_match(-1),
            _ => {}
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.complete = true;
        CancellationEvent::Handled
    }

    fn prefer_esc_to_handle_key_event(&self) -> bool {
        self.search.input.is_some() || !self.search.active_query.is_empty()
    }
}

impl crate::render::renderable::Renderable for SlashHelpView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let content_area = render_menu_surface(area, buf);
        let [header_area, body_area, footer_area] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Length(2),
        ])
        .areas(content_area);
        let [_footer_gap_area, footer_line_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(footer_area);

        let (lines, row_starts, row_ends) = Self::wrap_rows(&self.rows, body_area.width);
        let header_lines = lines.iter().take(2).cloned().collect::<Vec<_>>();
        let mut body_lines = lines.iter().skip(2).cloned().collect::<Vec<_>>();
        let visible_rows = Self::visible_body_rows(body_area.height);
        let max_scroll = body_lines.len().saturating_sub(visible_rows);
        let mut scroll_top = self.scroll_top.get().min(max_scroll);

        if self.search.input.is_none()
            && !self.search.active_query.is_empty()
            && let Some(selected_row_idx) =
                Self::matching_logical_rows(&self.rows, &self.search.active_query)
                    .get(self.search.selected_match)
                    .copied()
        {
            let start = row_starts[selected_row_idx].saturating_sub(2);
            let end = row_ends[selected_row_idx].saturating_sub(2);
            if self.follow_selected_match.get() || start < scroll_top {
                scroll_top = start;
            } else if end > scroll_top + visible_rows {
                scroll_top = end.saturating_sub(visible_rows);
            }
            scroll_top = scroll_top.min(max_scroll);
            self.scroll_top.set(scroll_top);
            self.follow_selected_match.set(false);
            for line in body_lines.iter_mut().take(end).skip(start) {
                *line = line.clone().patch_style(Style::new().reversed());
            }
        }

        self.scroll_top.set(scroll_top);

        Paragraph::new(header_lines).render(header_area, buf);
        Paragraph::new(body_lines.clone())
            .scroll((scroll_top as u16, 0))
            .render(body_area, buf);

        let footer_line = self.footer_line();
        Paragraph::new(footer_line.clone()).render(footer_line_area, buf);
        let indicator =
            self.search_indicator(&self.rows, body_lines.len(), visible_rows, scroll_top);
        let indicator_width = UnicodeWidthStr::width(indicator.as_str()) as u16;
        let footer_width = footer_line.width() as u16;
        if footer_width + indicator_width + 2 <= footer_line_area.width {
            Paragraph::new(indicator.dim()).render(
                Rect::new(
                    footer_line_area.x + footer_line_area.width - indicator_width,
                    footer_line_area.y,
                    indicator_width,
                    footer_line_area.height,
                ),
                buf,
            );
        }
    }

    fn desired_height(&self, width: u16) -> u16 {
        let (wrapped_rows, _, _) = Self::wrap_rows(&self.rows, width.saturating_sub(4));
        let content_rows = wrapped_rows.len() as u16;
        content_rows.max(HELP_VIEW_MIN_BODY_ROWS + 4)
    }
}

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::popup_consts::MAX_POPUP_ROWS;
use crate::bottom_pane::selection_popup_common::render_menu_surface;
use crate::key_hint;
use crate::slash_command::built_in_slash_commands;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;

const HELP_VIEW_MIN_BODY_ROWS: u16 = 6;

pub(crate) struct SlashHelpView {
    complete: bool,
    scroll_top: usize,
}

impl SlashHelpView {
    pub(crate) fn new() -> Self {
        Self {
            complete: false,
            scroll_top: 0,
        }
    }

    fn footer_hint() -> Line<'static> {
        Line::from(vec![
            "Scroll with ".into(),
            key_hint::plain(KeyCode::Up).into(),
            "/".into(),
            key_hint::plain(KeyCode::Down).into(),
            " or ".into(),
            key_hint::plain(KeyCode::PageUp).into(),
            "/".into(),
            key_hint::plain(KeyCode::PageDown).into(),
            ", close with ".into(),
            key_hint::plain(KeyCode::Esc).into(),
            ".".into(),
        ])
    }

    fn visible_body_rows(area_height: u16) -> usize {
        area_height
            .saturating_sub(3)
            .max(HELP_VIEW_MIN_BODY_ROWS)
            .into()
    }

    fn build_lines(width: u16) -> Vec<Line<'static>> {
        let width = width.max(24);
        let note_opts = RtOptions::new(width as usize)
            .initial_indent(Line::from(""))
            .subsequent_indent(Line::from(""));
        let usage_opts = RtOptions::new(width as usize)
            .initial_indent(Line::from("      "))
            .subsequent_indent(Line::from("        "));

        let mut lines = vec![Line::from("Slash Commands".bold()), Line::from("")];
        lines.extend(word_wrap_lines(
            [Line::from(
                "Type / to open the command popup. For commands with both a picker and an arg form, bare /command opens the picker and /command ... runs directly."
                    .dim(),
            )],
            note_opts.clone(),
        ));
        lines.extend(word_wrap_lines(
            [Line::from(
                "Args use shell-style quoting; quote values with spaces.".dim(),
            )],
            note_opts,
        ));
        lines.push(Line::from(""));

        for (_, cmd) in built_in_slash_commands() {
            lines.push(Line::from(format!("/{}", cmd.command()).cyan().bold()));
            lines.extend(word_wrap_lines(
                [Line::from(format!("  {}", cmd.description()).dim())],
                RtOptions::new(width as usize)
                    .initial_indent(Line::from(""))
                    .subsequent_indent(Line::from("  ")),
            ));
            lines.push(Line::from("  Usage:".dim()));
            for form in cmd.help_forms() {
                let usage = if form.is_empty() {
                    format!("/{}", cmd.command())
                } else {
                    format!("/{} {}", cmd.command(), form)
                };
                lines.extend(word_wrap_lines(
                    [Line::from(usage.cyan())],
                    usage_opts.clone(),
                ));
            }
            lines.push(Line::from(""));
        }

        while lines.last().is_some_and(|line| line.spans.is_empty()) {
            lines.pop();
        }

        lines
    }

    fn scroll_by(&mut self, delta: isize) {
        self.scroll_top = self.scroll_top.saturating_add_signed(delta);
    }
}

impl BottomPaneView for SlashHelpView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
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
            }
            | KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.on_ctrl_c();
            }
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
            Constraint::Length(1),
        ])
        .areas(content_area);

        let lines = Self::build_lines(body_area.width);
        let visible_rows = Self::visible_body_rows(body_area.height);
        let max_scroll = lines.len().saturating_sub(visible_rows);
        let scroll_top = self.scroll_top.min(max_scroll) as u16;

        Paragraph::new(lines.iter().take(2).cloned().collect::<Vec<_>>()).render(header_area, buf);
        Paragraph::new(lines.iter().skip(2).cloned().collect::<Vec<_>>())
            .scroll((scroll_top, 0))
            .render(body_area, buf);
        Paragraph::new(Self::footer_hint()).render(footer_area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        let content_rows = Self::build_lines(width.saturating_sub(4)).len() as u16;
        content_rows.max(HELP_VIEW_MIN_BODY_ROWS + 3)
    }
}

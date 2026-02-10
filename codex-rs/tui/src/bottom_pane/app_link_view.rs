use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use textwrap::wrap;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::measure_rows_height;
use super::selection_popup_common::render_rows;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::key_hint;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::style::user_message_style;
use crate::wrapping::word_wrap_lines;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppLinkScreen {
    Link,
    InstallConfirmation,
}

pub(crate) struct AppLinkView {
    title: String,
    description: Option<String>,
    instructions: String,
    url: String,
    is_installed: bool,
    app_event_tx: AppEventSender,
    screen: AppLinkScreen,
    selected_action: usize,
    complete: bool,
}

impl AppLinkView {
    pub(crate) fn new(
        title: String,
        description: Option<String>,
        instructions: String,
        url: String,
        is_installed: bool,
        app_event_tx: AppEventSender,
    ) -> Self {
        Self {
            title,
            description,
            instructions,
            url,
            is_installed,
            app_event_tx,
            screen: AppLinkScreen::Link,
            selected_action: 0,
            complete: false,
        }
    }

    fn action_labels(&self) -> [&'static str; 2] {
        match self.screen {
            AppLinkScreen::Link => {
                if self.is_installed {
                    ["Manage on ChatGPT", "Back"]
                } else {
                    ["Install on ChatGPT", "Back"]
                }
            }
            AppLinkScreen::InstallConfirmation => ["I already Installed it", "Back"],
        }
    }

    fn move_selection_prev(&mut self) {
        self.selected_action = self.selected_action.saturating_sub(1);
    }

    fn move_selection_next(&mut self) {
        self.selected_action = (self.selected_action + 1).min(self.action_labels().len() - 1);
    }

    fn handle_primary_action(&mut self) {
        match self.screen {
            AppLinkScreen::Link => {
                self.app_event_tx.send(AppEvent::OpenUrlInBrowser {
                    url: self.url.clone(),
                });
                if !self.is_installed {
                    self.screen = AppLinkScreen::InstallConfirmation;
                    self.selected_action = 0;
                }
            }
            AppLinkScreen::InstallConfirmation => {
                self.app_event_tx.send(AppEvent::RefreshConnectors {
                    force_refetch: true,
                });
                self.complete = true;
            }
        }
    }

    fn handle_secondary_action(&mut self) {
        match self.screen {
            AppLinkScreen::Link => {
                self.complete = true;
            }
            AppLinkScreen::InstallConfirmation => {
                self.screen = AppLinkScreen::Link;
                self.selected_action = 0;
            }
        }
    }

    fn activate_selected_action(&mut self) {
        match self.selected_action {
            0 => self.handle_primary_action(),
            _ => self.handle_secondary_action(),
        }
    }

    fn content_lines(&self, width: u16) -> Vec<Line<'static>> {
        match self.screen {
            AppLinkScreen::Link => self.link_content_lines(width),
            AppLinkScreen::InstallConfirmation => self.install_confirmation_lines(width),
        }
    }

    fn link_content_lines(&self, width: u16) -> Vec<Line<'static>> {
        let usable_width = width.max(1) as usize;
        let mut lines: Vec<Line<'static>> = Vec::new();

        lines.push(Line::from(self.title.clone().bold()));
        if let Some(description) = self
            .description
            .as_deref()
            .map(str::trim)
            .filter(|description| !description.is_empty())
        {
            for line in wrap(description, usable_width) {
                lines.push(Line::from(line.into_owned().dim()));
            }
        }

        lines.push(Line::from(""));
        if self.is_installed {
            for line in wrap("Use $ to insert this app into the prompt.", usable_width) {
                lines.push(Line::from(line.into_owned()));
            }
            lines.push(Line::from(""));
        }

        let instructions = self.instructions.trim();
        if !instructions.is_empty() {
            for line in wrap(instructions, usable_width) {
                lines.push(Line::from(line.into_owned()));
            }
            for line in wrap(
                "Newly installed apps can take a few minutes to appear in /apps.",
                usable_width,
            ) {
                lines.push(Line::from(line.into_owned()));
            }
            if !self.is_installed {
                for line in wrap(
                    "After installed, use $ to insert this app into the prompt.",
                    usable_width,
                ) {
                    lines.push(Line::from(line.into_owned()));
                }
            }
            lines.push(Line::from(""));
        }

        lines
    }

    fn install_confirmation_lines(&self, width: u16) -> Vec<Line<'static>> {
        let usable_width = width.max(1) as usize;
        let mut lines: Vec<Line<'static>> = Vec::new();

        lines.push(Line::from("Finish App Setup".bold()));
        lines.push(Line::from(""));

        for line in wrap(
            "Complete app setup on ChatGPT in the browser window that just opened.",
            usable_width,
        ) {
            lines.push(Line::from(line.into_owned()));
        }
        for line in wrap(
            "Sign in there if needed, then return here and select \"I already Installed it\".",
            usable_width,
        ) {
            lines.push(Line::from(line.into_owned()));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec!["Setup URL:".dim()]));
        let url_line = Line::from(vec![self.url.clone().cyan().underlined()]);
        lines.extend(word_wrap_lines(vec![url_line], usable_width));

        lines
    }

    fn action_rows(&self) -> Vec<GenericDisplayRow> {
        self.action_labels()
            .into_iter()
            .enumerate()
            .map(|(index, label)| {
                let prefix = if self.selected_action == index {
                    '›'
                } else {
                    ' '
                };
                GenericDisplayRow {
                    name: format!("{prefix} {}. {label}", index + 1),
                    ..Default::default()
                }
            })
            .collect()
    }

    fn action_state(&self) -> ScrollState {
        let mut state = ScrollState::new();
        state.selected_idx = Some(self.selected_action);
        state
    }

    fn action_rows_height(&self, width: u16) -> u16 {
        let rows = self.action_rows();
        let state = self.action_state();
        measure_rows_height(&rows, &state, rows.len().max(1), width.max(1))
    }

    fn hint_line(&self) -> Line<'static> {
        Line::from(vec![
            "Use ".into(),
            key_hint::plain(KeyCode::Tab).into(),
            " / ".into(),
            key_hint::plain(KeyCode::Up).into(),
            " ".into(),
            key_hint::plain(KeyCode::Down).into(),
            " to move, ".into(),
            key_hint::plain(KeyCode::Enter).into(),
            " to select, ".into(),
            key_hint::plain(KeyCode::Esc).into(),
            " to close".into(),
        ])
    }
}

impl BottomPaneView for AppLinkView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.on_ctrl_c();
            }
            KeyEvent {
                code: KeyCode::Up, ..
            }
            | KeyEvent {
                code: KeyCode::Left,
                ..
            }
            | KeyEvent {
                code: KeyCode::BackTab,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.move_selection_prev(),
            KeyEvent {
                code: KeyCode::Down,
                ..
            }
            | KeyEvent {
                code: KeyCode::Right,
                ..
            }
            | KeyEvent {
                code: KeyCode::Tab, ..
            }
            | KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.move_selection_next(),
            KeyEvent {
                code: KeyCode::Char('1'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.selected_action = 0;
                self.activate_selected_action();
            }
            KeyEvent {
                code: KeyCode::Char('2'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.selected_action = 1;
                self.activate_selected_action();
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.activate_selected_action(),
            _ => {}
        }
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.complete = true;
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.complete
    }
}

impl crate::render::renderable::Renderable for AppLinkView {
    fn desired_height(&self, width: u16) -> u16 {
        let content_width = width.saturating_sub(4).max(1);
        let content_lines = self.content_lines(content_width);
        let action_rows_height = self.action_rows_height(content_width);
        content_lines.len() as u16 + action_rows_height + 3
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        Block::default()
            .style(user_message_style())
            .render(area, buf);

        let actions_height = self.action_rows_height(area.width.saturating_sub(4));
        let [content_area, actions_area, hint_area] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(actions_height),
            Constraint::Length(1),
        ])
        .areas(area);

        let inner = content_area.inset(Insets::vh(1, 2));
        let content_width = inner.width.max(1);
        let lines = self.content_lines(content_width);
        Paragraph::new(lines).render(inner, buf);

        if actions_area.height > 0 {
            let actions_area = Rect {
                x: actions_area.x.saturating_add(2),
                y: actions_area.y,
                width: actions_area.width.saturating_sub(2),
                height: actions_area.height,
            };
            let action_rows = self.action_rows();
            let action_state = self.action_state();
            render_rows(
                actions_area,
                buf,
                &action_rows,
                &action_state,
                action_rows.len().max(1),
                "No actions",
            );
        }

        if hint_area.height > 0 {
            let hint_area = Rect {
                x: hint_area.x.saturating_add(2),
                y: hint_area.y,
                width: hint_area.width.saturating_sub(2),
                height: hint_area.height,
            };
            self.hint_line().dim().render(hint_area, buf);
        }
    }
}

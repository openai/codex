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
use ratatui::widgets::Wrap;
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
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_lines;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppLinkScreen {
    Link,
    InstallConfirmation,
}

pub(crate) struct AppLinkViewParams {
    pub(crate) app_id: String,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    pub(crate) instructions: String,
    pub(crate) url: String,
    pub(crate) is_installed: bool,
    pub(crate) is_enabled: bool,
}

pub(crate) struct AppLinkView {
    app_id: String,
    title: String,
    description: Option<String>,
    instructions: String,
    url: String,
    is_installed: bool,
    is_enabled: bool,
    app_event_tx: AppEventSender,
    screen: AppLinkScreen,
    selected_action: usize,
    complete: bool,
}

impl AppLinkView {
    pub(crate) fn new(params: AppLinkViewParams, app_event_tx: AppEventSender) -> Self {
        let AppLinkViewParams {
            app_id,
            title,
            description,
            instructions,
            url,
            is_installed,
            is_enabled,
        } = params;
        Self {
            app_id,
            title,
            description,
            instructions,
            url,
            is_installed,
            is_enabled,
            app_event_tx,
            screen: AppLinkScreen::Link,
            selected_action: 0,
            complete: false,
        }
    }

    fn action_labels(&self) -> Vec<&'static str> {
        match self.screen {
            AppLinkScreen::Link => {
                if self.is_installed {
                    vec![
                        "Manage on ChatGPT",
                        if self.is_enabled {
                            "Disable app"
                        } else {
                            "Enable app"
                        },
                        "Back",
                    ]
                } else {
                    vec!["Install on ChatGPT", "Back"]
                }
            }
            AppLinkScreen::InstallConfirmation => vec!["I already Installed it", "Back"],
        }
    }

    fn move_selection_prev(&mut self) {
        self.selected_action = self.selected_action.saturating_sub(1);
    }

    fn move_selection_next(&mut self) {
        self.selected_action = (self.selected_action + 1).min(self.action_labels().len() - 1);
    }

    fn open_chatgpt_link(&mut self) {
        self.app_event_tx.send(AppEvent::OpenUrlInBrowser {
            url: self.url.clone(),
        });
        if !self.is_installed {
            self.screen = AppLinkScreen::InstallConfirmation;
            self.selected_action = 0;
        }
    }

    fn refresh_connectors_and_close(&mut self) {
        self.app_event_tx.send(AppEvent::RefreshConnectors {
            force_refetch: true,
        });
        self.complete = true;
    }

    fn back_to_link_screen(&mut self) {
        self.screen = AppLinkScreen::Link;
        self.selected_action = 0;
    }

    fn toggle_enabled(&mut self) {
        self.is_enabled = !self.is_enabled;
        self.app_event_tx.send(AppEvent::SetAppEnabled {
            id: self.app_id.clone(),
            enabled: self.is_enabled,
        });
    }

    fn activate_selected_action(&mut self) {
        match self.screen {
            AppLinkScreen::Link => match self.selected_action {
                0 => self.open_chatgpt_link(),
                1 if self.is_installed => self.toggle_enabled(),
                _ => self.complete = true,
            },
            AppLinkScreen::InstallConfirmation => match self.selected_action {
                0 => self.refresh_connectors_and_close(),
                _ => self.back_to_link_screen(),
            },
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
        lines.extend(adaptive_wrap_lines(
            vec![url_line],
            RtOptions::new(usable_width),
        ));

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
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if let Some(index) = c
                    .to_digit(10)
                    .and_then(|digit| digit.checked_sub(1))
                    .map(|index| index as usize)
                    && index < self.action_labels().len()
                {
                    self.selected_action = index;
                    self.activate_selected_action();
                }
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
        let content_rows = Paragraph::new(content_lines)
            .wrap(Wrap { trim: false })
            .line_count(content_width)
            .max(1) as u16;
        let action_rows_height = self.action_rows_height(content_width);
        content_rows + action_rows_height + 3
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
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(inner, buf);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use crate::render::renderable::Renderable;
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn installed_app_has_toggle_action() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let view = AppLinkView::new(
            AppLinkViewParams {
                app_id: "connector_1".to_string(),
                title: "Notion".to_string(),
                description: None,
                instructions: "Manage app".to_string(),
                url: "https://example.test/notion".to_string(),
                is_installed: true,
                is_enabled: true,
            },
            tx,
        );

        assert_eq!(
            view.action_labels(),
            vec!["Manage on ChatGPT", "Disable app", "Back"]
        );
    }

    #[test]
    fn toggle_action_sends_set_app_enabled_and_updates_label() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut view = AppLinkView::new(
            AppLinkViewParams {
                app_id: "connector_1".to_string(),
                title: "Notion".to_string(),
                description: None,
                instructions: "Manage app".to_string(),
                url: "https://example.test/notion".to_string(),
                is_installed: true,
                is_enabled: true,
            },
            tx,
        );

        view.handle_key_event(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));

        match rx.try_recv() {
            Ok(AppEvent::SetAppEnabled { id, enabled }) => {
                assert_eq!(id, "connector_1");
                assert!(!enabled);
            }
            Ok(other) => panic!("unexpected app event: {other:?}"),
            Err(err) => panic!("missing app event: {err}"),
        }

        assert_eq!(
            view.action_labels(),
            vec!["Manage on ChatGPT", "Enable app", "Back"]
        );
    }

    #[test]
    fn install_confirmation_does_not_split_long_url_like_token_without_scheme() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let url_like =
            "example.test/api/v1/projects/alpha-team/releases/2026-02-17/builds/1234567890";
        let mut view = AppLinkView::new(
            AppLinkViewParams {
                app_id: "connector_1".to_string(),
                title: "Notion".to_string(),
                description: None,
                instructions: "Manage app".to_string(),
                url: url_like.to_string(),
                is_installed: true,
                is_enabled: true,
            },
            tx,
        );
        view.screen = AppLinkScreen::InstallConfirmation;

        let rendered: Vec<String> = view
            .content_lines(40)
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.content.into_owned())
                    .collect::<String>()
            })
            .collect();

        assert_eq!(
            rendered
                .iter()
                .filter(|line| line.contains(url_like))
                .count(),
            1,
            "expected full URL-like token in one rendered line, got: {rendered:?}"
        );
    }

    #[test]
    fn install_confirmation_render_keeps_url_tail_visible_when_narrow() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let url = "https://example.test/api/v1/projects/alpha-team/releases/2026-02-17/builds/1234567890/artifacts/reports/performance/summary/detail/with/a/very/long/path/tail42";
        let mut view = AppLinkView::new(
            AppLinkViewParams {
                app_id: "connector_1".to_string(),
                title: "Notion".to_string(),
                description: None,
                instructions: "Manage app".to_string(),
                url: url.to_string(),
                is_installed: true,
                is_enabled: true,
            },
            tx,
        );
        view.screen = AppLinkScreen::InstallConfirmation;

        let width: u16 = 36;
        let height = view.desired_height(width);
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);

        let rendered_blob = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| {
                        let symbol = buf[(x, y)].symbol();
                        if symbol.is_empty() {
                            ' '
                        } else {
                            symbol.chars().next().unwrap_or(' ')
                        }
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            rendered_blob.contains("tail42"),
            "expected wrapped setup URL tail to remain visible in narrow pane, got:\n{rendered_blob}"
        );
    }
}

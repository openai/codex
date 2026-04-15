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
use ratatui::widgets::Widget;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::key_hint;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::style::user_message_style;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::measure_rows_height;
use super::selection_popup_common::render_rows;

const MEMORIES_DOC_URL: &str = "https://developers.openai.com/codex/memories";

#[derive(Clone, Copy, PartialEq, Eq)]
enum MemoriesSetting {
    Use,
    Generate,
}

struct MemoriesSettingItem {
    setting: MemoriesSetting,
    name: &'static str,
    description: &'static str,
    enabled: bool,
}

pub(crate) struct MemoriesSettingsView {
    items: Vec<MemoriesSettingItem>,
    state: ScrollState,
    complete: bool,
    app_event_tx: AppEventSender,
    header: Box<dyn Renderable>,
    docs_link: Line<'static>,
    footer_hint: Line<'static>,
}

impl MemoriesSettingsView {
    pub(crate) fn new(
        use_memories: bool,
        generate_memories: bool,
        app_event_tx: AppEventSender,
    ) -> Self {
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Memories".bold()));
        header.push(Line::from(
            "Choose how Codex uses and creates memories. Changes are saved to config.toml".dim(),
        ));

        let mut view = Self {
            items: vec![
                MemoriesSettingItem {
                    setting: MemoriesSetting::Use,
                    name: "Use memories",
                    description: "Use memories in the following threads. Applied at next thread.",
                    enabled: use_memories,
                },
                MemoriesSettingItem {
                    setting: MemoriesSetting::Generate,
                    name: "Generate memories",
                    description: "Generate memories from the following threads. Current thread included.",
                    enabled: generate_memories,
                },
            ],
            state: ScrollState::new(),
            complete: false,
            app_event_tx,
            header: Box::new(header),
            docs_link: Line::from(vec![
                "Learn more: ".dim(),
                MEMORIES_DOC_URL.cyan().underlined(),
            ]),
            footer_hint: memories_settings_hint_line(),
        };
        view.initialize_selection();
        view
    }

    fn initialize_selection(&mut self) {
        self.state.selected_idx = (!self.items.is_empty()).then_some(0);
    }

    fn visible_len(&self) -> usize {
        self.items.len()
    }

    fn build_rows(&self) -> Vec<GenericDisplayRow> {
        let mut rows = Vec::with_capacity(self.items.len());
        let selected_idx = self.state.selected_idx;
        for (idx, item) in self.items.iter().enumerate() {
            let prefix = if selected_idx == Some(idx) {
                '›'
            } else {
                ' '
            };
            let marker = if item.enabled { 'x' } else { ' ' };
            let name = format!("{prefix} [{marker}] {}", item.name);
            rows.push(GenericDisplayRow {
                name,
                description: Some(item.description.to_string()),
                ..Default::default()
            });
        }

        rows
    }

    fn move_up(&mut self) {
        let len = self.visible_len();
        if len == 0 {
            return;
        }
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    fn move_down(&mut self) {
        let len = self.visible_len();
        if len == 0 {
            return;
        }
        self.state.move_down_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    fn toggle_selected(&mut self) {
        let Some(selected_idx) = self.state.selected_idx else {
            return;
        };

        if let Some(item) = self.items.get_mut(selected_idx) {
            item.enabled = !item.enabled;
        }
    }

    fn rows_width(total_width: u16) -> u16 {
        total_width.saturating_sub(2)
    }

    fn current_setting(&self, setting: MemoriesSetting) -> bool {
        self.items
            .iter()
            .find(|item| item.setting == setting)
            .is_some_and(|item| item.enabled)
    }
}

impl BottomPaneView for MemoriesSettingsView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Up, ..
            }
            | KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('\u{0010}'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.move_up(),
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.move_up(),
            KeyEvent {
                code: KeyCode::Down,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('\u{000e}'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.move_down(),
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.move_down(),
            KeyEvent {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.toggle_selected(),
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.save(),
            KeyEvent {
                code: KeyCode::Esc, ..
            } => self.cancel(),
            _ => {}
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.cancel();
        CancellationEvent::Handled
    }
}

impl MemoriesSettingsView {
    fn save(&mut self) {
        self.app_event_tx.send(AppEvent::UpdateMemorySettings {
            use_memories: self.current_setting(MemoriesSetting::Use),
            generate_memories: self.current_setting(MemoriesSetting::Generate),
        });
        self.complete = true;
    }

    fn cancel(&mut self) {
        self.complete = true;
    }
}

impl Renderable for MemoriesSettingsView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let [content_area, footer_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);

        Block::default()
            .style(user_message_style())
            .render(content_area, buf);

        let header_height = self
            .header
            .desired_height(content_area.width.saturating_sub(4));
        let rows = self.build_rows();
        let rows_width = Self::rows_width(content_area.width);
        let rows_height = measure_rows_height(
            &rows,
            &self.state,
            MAX_POPUP_ROWS,
            rows_width.saturating_add(1),
        );
        let [header_area, _, list_area, _, docs_area] = Layout::vertical([
            Constraint::Max(header_height),
            Constraint::Max(1),
            Constraint::Length(rows_height),
            Constraint::Max(1),
            Constraint::Length(1),
        ])
        .areas(content_area.inset(Insets::vh(/*v*/ 1, /*h*/ 2)));

        self.header.render(header_area, buf);

        if list_area.height > 0 {
            let render_area = Rect {
                x: list_area.x.saturating_sub(2),
                y: list_area.y,
                width: rows_width.max(1),
                height: list_area.height,
            };
            render_rows(
                render_area,
                buf,
                &rows,
                &self.state,
                MAX_POPUP_ROWS,
                "  No memory settings available",
            );
        }
        self.docs_link.clone().render(docs_area, buf);

        let hint_area = Rect {
            x: footer_area.x + 2,
            y: footer_area.y,
            width: footer_area.width.saturating_sub(2),
            height: footer_area.height,
        };
        self.footer_hint.clone().dim().render(hint_area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        let rows = self.build_rows();
        let rows_width = Self::rows_width(width);
        let rows_height = measure_rows_height(
            &rows,
            &self.state,
            MAX_POPUP_ROWS,
            rows_width.saturating_add(1),
        );

        let mut height = self.header.desired_height(width.saturating_sub(4));
        height = height.saturating_add(rows_height + 5);
        height.saturating_add(1)
    }
}

fn memories_settings_hint_line() -> Line<'static> {
    Line::from(vec![
        "Press ".into(),
        key_hint::plain(KeyCode::Char(' ')).into(),
        " to toggle; ".into(),
        key_hint::plain(KeyCode::Enter).into(),
        " to save".into(),
    ])
}

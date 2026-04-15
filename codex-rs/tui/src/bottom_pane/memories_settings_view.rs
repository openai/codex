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
use crate::bottom_pane::popup_consts::standard_popup_hint_line;
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum MemoriesAction {
    Reset,
}

enum MemoriesMenuItem {
    Setting {
        setting: MemoriesSetting,
        name: &'static str,
        description: &'static str,
        enabled: bool,
    },
    Action {
        action: MemoriesAction,
        name: &'static str,
        description: &'static str,
    },
}

pub(crate) struct MemoriesSettingsView {
    items: Vec<MemoriesMenuItem>,
    state: ScrollState,
    reset_confirmation: Option<ScrollState>,
    complete: bool,
    app_event_tx: AppEventSender,
    docs_link: Line<'static>,
}

impl MemoriesSettingsView {
    pub(crate) fn new(
        use_memories: bool,
        generate_memories: bool,
        app_event_tx: AppEventSender,
    ) -> Self {
        let mut view = Self {
            items: vec![
                MemoriesMenuItem::Setting {
                    setting: MemoriesSetting::Use,
                    name: "Use memories",
                    description: "Use memories in the following threads. Applied at next thread.",
                    enabled: use_memories,
                },
                MemoriesMenuItem::Setting {
                    setting: MemoriesSetting::Generate,
                    name: "Generate memories",
                    description: "Generate memories from the following threads. Current thread included.",
                    enabled: generate_memories,
                },
                MemoriesMenuItem::Action {
                    action: MemoriesAction::Reset,
                    name: "Reset all memories",
                    description: "Clear local memory files and summaries. Existing threads stay intact.",
                },
            ],
            state: ScrollState::new(),
            reset_confirmation: None,
            complete: false,
            app_event_tx,
            docs_link: Line::from(vec![
                "Learn more: ".dim(),
                MEMORIES_DOC_URL.cyan().underlined(),
            ]),
        };
        view.initialize_selection();
        view
    }

    fn initialize_selection(&mut self) {
        self.state.selected_idx = (!self.items.is_empty()).then_some(0);
    }

    fn settings_header(&self) -> ColumnRenderable<'_> {
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Memories".bold()));
        header.push(Line::from(
            "Choose how Codex uses and creates memories. Changes are saved to config.toml".dim(),
        ));
        header
    }

    fn reset_confirmation_header(&self) -> ColumnRenderable<'_> {
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Reset all memories?".bold()));
        header.push(Line::from(
            "This clears local memory files and rollout summaries for the current Codex home."
                .dim(),
        ));
        header
    }

    fn active_state(&self) -> &ScrollState {
        self.reset_confirmation.as_ref().unwrap_or(&self.state)
    }

    fn active_state_mut(&mut self) -> &mut ScrollState {
        self.reset_confirmation.as_mut().unwrap_or(&mut self.state)
    }

    fn visible_len(&self) -> usize {
        if self.reset_confirmation.is_some() {
            2
        } else {
            self.items.len()
        }
    }

    fn build_rows(&self) -> Vec<GenericDisplayRow> {
        if let Some(state) = self.reset_confirmation.as_ref() {
            return ["Reset all memories", "Go back"]
                .into_iter()
                .enumerate()
                .map(|(idx, name)| GenericDisplayRow {
                    name: if state.selected_idx == Some(idx) {
                        format!("› {name}")
                    } else {
                        format!("  {name}")
                    },
                    description: Some(match idx {
                        0 => "Delete local memory files and rollout summaries.".to_string(),
                        1 => "Return to memory settings.".to_string(),
                        _ => unreachable!("reset confirmation only renders two rows"),
                    }),
                    ..Default::default()
                })
                .collect();
        }

        let selected_idx = self.state.selected_idx;
        self.items
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let prefix = if selected_idx == Some(idx) {
                    '›'
                } else {
                    ' '
                };
                let (name, description) = match item {
                    MemoriesMenuItem::Setting {
                        name,
                        description,
                        enabled,
                        ..
                    } => (
                        format!("{prefix} [{}] {name}", if *enabled { 'x' } else { ' ' }),
                        description,
                    ),
                    MemoriesMenuItem::Action {
                        name, description, ..
                    } => (format!("{prefix} {name}"), description),
                };
                GenericDisplayRow {
                    name,
                    description: Some((*description).to_string()),
                    ..Default::default()
                }
            })
            .collect()
    }

    fn move_up(&mut self) {
        let len = self.visible_len();
        if len == 0 {
            return;
        }
        let state = self.active_state_mut();
        state.move_up_wrap(len);
        state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    fn move_down(&mut self) {
        let len = self.visible_len();
        if len == 0 {
            return;
        }
        let state = self.active_state_mut();
        state.move_down_wrap(len);
        state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    fn toggle_selected(&mut self) {
        if self.reset_confirmation.is_some() {
            return;
        }

        let Some(selected_idx) = self.state.selected_idx else {
            return;
        };

        if let Some(MemoriesMenuItem::Setting { enabled, .. }) = self.items.get_mut(selected_idx) {
            *enabled = !*enabled;
        }
    }

    fn rows_width(total_width: u16) -> u16 {
        total_width.saturating_sub(2)
    }

    fn current_setting(&self, setting: MemoriesSetting) -> bool {
        self.items
            .iter()
            .find_map(|item| match item {
                MemoriesMenuItem::Setting {
                    setting: item_setting,
                    enabled,
                    ..
                } if *item_setting == setting => Some(*enabled),
                _ => None,
            })
            .unwrap_or(false)
    }

    fn open_reset_confirmation(&mut self) {
        let mut state = ScrollState::new();
        state.selected_idx = Some(0);
        self.reset_confirmation = Some(state);
    }

    fn close_reset_confirmation(&mut self) {
        self.reset_confirmation = None;
        self.state.selected_idx = self.items.len().checked_sub(1);
    }

    fn footer_hint(&self) -> Line<'static> {
        if self.reset_confirmation.is_some() {
            standard_popup_hint_line()
        } else {
            memories_settings_hint_line()
        }
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
        if let Some(state) = self.reset_confirmation.as_ref() {
            match state.selected_idx {
                Some(0) => {
                    self.app_event_tx.send(AppEvent::ResetMemories);
                    self.complete = true;
                }
                Some(1) | None => self.close_reset_confirmation(),
                Some(other) => unreachable!("unexpected reset confirmation row: {other}"),
            }
            return;
        }

        match self.state.selected_idx.and_then(|idx| self.items.get(idx)) {
            Some(MemoriesMenuItem::Action {
                action: MemoriesAction::Reset,
                ..
            }) => self.open_reset_confirmation(),
            _ => {
                self.app_event_tx.send(AppEvent::UpdateMemorySettings {
                    use_memories: self.current_setting(MemoriesSetting::Use),
                    generate_memories: self.current_setting(MemoriesSetting::Generate),
                });
                self.complete = true;
            }
        }
    }

    fn cancel(&mut self) {
        if self.reset_confirmation.is_some() {
            self.close_reset_confirmation();
        } else {
            self.complete = true;
        }
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

        let header = if self.reset_confirmation.is_some() {
            self.reset_confirmation_header()
        } else {
            self.settings_header()
        };
        let header_height = header.desired_height(content_area.width.saturating_sub(4));
        let rows = self.build_rows();
        let rows_width = Self::rows_width(content_area.width);
        let rows_height = measure_rows_height(
            &rows,
            self.active_state(),
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

        header.render(header_area, buf);

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
                self.active_state(),
                MAX_POPUP_ROWS,
                "  No memory settings available",
            );
        }
        if self.reset_confirmation.is_none() {
            self.docs_link.clone().render(docs_area, buf);
        }

        let hint_area = Rect {
            x: footer_area.x + 2,
            y: footer_area.y,
            width: footer_area.width.saturating_sub(2),
            height: footer_area.height,
        };
        self.footer_hint().render(hint_area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        let header = if self.reset_confirmation.is_some() {
            self.reset_confirmation_header()
        } else {
            self.settings_header()
        };
        let rows = self.build_rows();
        let rows_width = Self::rows_width(width);
        let rows_height = measure_rows_height(
            &rows,
            self.active_state(),
            MAX_POPUP_ROWS,
            rows_width.saturating_add(1),
        );

        let docs_height = if self.reset_confirmation.is_some() {
            0
        } else {
            1
        };
        let mut height = header.desired_height(width.saturating_sub(4));
        height = height.saturating_add(rows_height + 4 + docs_height);
        height.saturating_add(1)
    }
}

fn memories_settings_hint_line() -> Line<'static> {
    Line::from(vec![
        "Press ".into(),
        key_hint::plain(KeyCode::Char(' ')).into(),
        " to toggle; ".into(),
        key_hint::plain(KeyCode::Enter).into(),
        " to save or select".into(),
    ])
}

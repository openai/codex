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

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::popup_consts::picker_hint_line_for_keymap;
use crate::key_hint;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::ListAction;
use crate::keymap::ListKeymap;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::style::user_message_style;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::picker_rows::measure_rows_height;
use super::picker_rows::render_rows;
use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;

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
    keymap: ListKeymap,
}

impl MemoriesSettingsView {
    pub(crate) fn new(
        use_memories: bool,
        generate_memories: bool,
        app_event_tx: AppEventSender,
        keymap: ListKeymap,
    ) -> Self {
        let mut view = Self {
            items: vec![
                MemoriesMenuItem::Setting {
                    setting: MemoriesSetting::Use,
                    name: "Use memories",
                    description: "Use memories starting with the next thread",
                    enabled: use_memories,
                },
                MemoriesMenuItem::Setting {
                    setting: MemoriesSetting::Generate,
                    name: "Generate memories",
                    description: "Generate memories from this thread and future threads",
                    enabled: generate_memories,
                },
                MemoriesMenuItem::Action {
                    action: MemoriesAction::Reset,
                    name: "Reset all memories",
                    description: "Clear local memory files and summaries without changing existing threads",
                },
            ],
            state: ScrollState::new(),
            reset_confirmation: None,
            complete: false,
            app_event_tx,
            docs_link: Line::from(vec![
                "Learn more: ".dim(),
                MEMORIES_DOC_URL
                    .fg(crate::style::accent_color())
                    .underlined(),
            ]),
            keymap,
        };
        view.initialize_selection();
        view
    }

    fn initialize_selection(&mut self) {
        self.state.selected_idx = (!self.items.is_empty()).then_some(0);
    }

    fn settings_header(&self) -> ColumnRenderable<'_> {
        let mut header = ColumnRenderable::new();
        header.push(Paragraph::new(Line::from("Memories".bold())).wrap(Wrap { trim: false }));
        header.push(
            Paragraph::new(Line::from(
                "Choose how Codex uses and creates memories. Changes are saved to config.toml"
                    .dim(),
            ))
            .wrap(Wrap { trim: false }),
        );
        header
    }

    fn reset_confirmation_header(&self) -> ColumnRenderable<'_> {
        let mut header = ColumnRenderable::new();
        header.push(
            Paragraph::new(Line::from("Reset all memories?".bold())).wrap(Wrap { trim: false }),
        );
        header.push(
            Paragraph::new(Line::from(
                "This clears local memory files and rollout summaries for the current Codex home."
                    .dim(),
            ))
            .wrap(Wrap { trim: false }),
        );
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
                    selection_style: Some(super::picker_style::selection_style()),
                    wrap_indent: Some(2),
                    name: if state.selected_idx == Some(idx) {
                        format!("› {name}")
                    } else {
                        format!("  {name}")
                    },
                    description: Some(match idx {
                        0 => "Delete local memory files and rollout summaries".to_string(),
                        1 => "Return to memory settings".to_string(),
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
                    selection_style: Some(super::picker_style::selection_style()),
                    wrap_indent: Some(if matches!(item, MemoriesMenuItem::Setting { .. }) {
                        6
                    } else {
                        2
                    }),
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

    fn page_up(&mut self) {
        let len = self.visible_len();
        let visible = MAX_POPUP_ROWS.min(len);
        self.active_state_mut().page_up_clamped(len, visible);
    }

    fn page_down(&mut self) {
        let len = self.visible_len();
        let visible = MAX_POPUP_ROWS.min(len);
        self.active_state_mut().page_down_clamped(len, visible);
    }

    fn jump_top(&mut self) {
        let len = self.visible_len();
        let visible = MAX_POPUP_ROWS.min(len);
        self.active_state_mut().jump_top(len, visible);
    }

    fn jump_bottom(&mut self) {
        let len = self.visible_len();
        let visible = MAX_POPUP_ROWS.min(len);
        self.active_state_mut().jump_bottom(len, visible);
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
            picker_hint_line_for_keymap(&self.keymap)
        } else {
            memories_settings_hint_line(&self.keymap)
        }
    }
}

impl BottomPaneView for MemoriesSettingsView {
    fn keymap_contexts(&self) -> crate::keymap::KeymapContextSet {
        crate::keymap::KeymapContextSet::new(crate::keymap::KeymapContext::List)
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
            _ if self.keymap.move_up.is_pressed(key_event) => self.move_up(),
            _ if self.keymap.move_down.is_pressed(key_event) => self.move_down(),
            _ if self.keymap.page_up.is_pressed(key_event) => self.page_up(),
            _ if self.keymap.page_down.is_pressed(key_event) => self.page_down(),
            _ if self.keymap.jump_top.is_pressed(key_event) => self.jump_top(),
            _ if self.keymap.jump_bottom.is_pressed(key_event) => self.jump_bottom(),
            KeyEvent {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.toggle_selected(),
            _ if self.keymap.accept.is_pressed(key_event) => self.save(),
            _ if self.keymap.cancel.is_pressed(key_event) => self.cancel(),
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

        let hint = self.footer_hint();
        let hint_lines = super::selection_popup_common::wrap_styled_line(
            &hint,
            area.width.saturating_sub(/*rhs*/ 2),
        );
        let [content_area, footer_area] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(hint_lines.len() as u16),
        ])
        .areas(area);

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
        let rows_height = measure_rows_height(
            &rows,
            self.active_state(),
            MAX_POPUP_ROWS,
            content_area.width,
        );
        let docs_height = if self.reset_confirmation.is_some() {
            0
        } else {
            2
        };
        let [body_area, docs_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(docs_height)])
                .areas(content_area);
        let [header_area, _, render_area] =
            super::picker_rows::layout(body_area, header_height, /*search*/ 0, rows_height);

        header.render(header_area, buf);

        if render_area.height > 0 {
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
            let docs_area = docs_area.inset(Insets::tlbr(
                /*top*/ 0, /*left*/ 2, /*bottom*/ 0, /*right*/ 2,
            ));
            self.docs_link.clone().render(docs_area, buf);
            crate::terminal_hyperlinks::mark_url_hyperlink(buf, docs_area, MEMORIES_DOC_URL);
        }

        let hint_area = Rect {
            x: footer_area.x + 2,
            y: footer_area.y,
            width: footer_area.width.saturating_sub(2),
            height: footer_area.height,
        };
        Paragraph::new(hint_lines).dim().render(hint_area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        let header = if self.reset_confirmation.is_some() {
            self.reset_confirmation_header()
        } else {
            self.settings_header()
        };
        let rows = self.build_rows();
        let rows_height = measure_rows_height(&rows, self.active_state(), MAX_POPUP_ROWS, width);

        let docs_height = if self.reset_confirmation.is_some() {
            0
        } else {
            1
        };
        let mut height = header.desired_height(width.saturating_sub(4));
        height = height.saturating_add(rows_height + 4 + docs_height);
        height.saturating_add(
            super::selection_popup_common::wrap_styled_line(
                &self.footer_hint(),
                width.saturating_sub(/*rhs*/ 2),
            )
            .len() as u16,
        )
    }
}

fn memories_settings_hint_line(keymap: &ListKeymap) -> Line<'static> {
    let mut spans = vec![key_hint::plain(KeyCode::Char(' ')).into(), " toggle".into()];
    if let Some(accept) = keymap.primary_hint(ListAction::Accept) {
        spans.push(" · ".into());
        spans.extend(accept.spans());
        spans.push(" save/select".into());
    }
    if let Some(cancel) = keymap.primary_hint(ListAction::Cancel) {
        spans.push(" · ".into());
        spans.extend(cancel.spans());
        spans.push(" cancel".into());
    }
    Line::from(spans)
}

#[cfg(test)]
#[path = "memories_settings_view_tests.rs"]
mod tests;

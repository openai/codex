use std::path::PathBuf;

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
use crate::skills_helpers::match_skill;
use crate::skills_helpers::truncate_skill_name;
use crate::style::user_message_style;
use codex_core::protocol::Op;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::render_rows_single_line;

const SEARCH_PLACEHOLDER: &str = "Type to search skills";
const SEARCH_PROMPT_PREFIX: &str = "> ";

pub(crate) struct SkillsToggleItem {
    pub name: String,
    pub skill_name: String,
    pub description: String,
    pub enabled: bool,
    pub path: PathBuf,
}

pub(crate) struct SkillsToggleView {
    items: Vec<SkillsToggleItem>,
    state: ScrollState,
    complete: bool,
    app_event_tx: AppEventSender,
    header: Box<dyn Renderable>,
    footer_hint: Line<'static>,
    search_query: String,
    filtered_indices: Vec<usize>,
}

impl SkillsToggleView {
    pub(crate) fn new(items: Vec<SkillsToggleItem>, app_event_tx: AppEventSender) -> Self {
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Enable/Disable Skills".bold()));
        header.push(Line::from(
            "Turn skills on or off. Your changes are saved automatically.".dim(),
        ));

        let mut view = Self {
            items,
            state: ScrollState::new(),
            complete: false,
            app_event_tx,
            header: Box::new(header),
            footer_hint: skills_toggle_hint_line(),
            search_query: String::new(),
            filtered_indices: Vec::new(),
        };
        view.apply_filter();
        view
    }

    fn visible_len(&self) -> usize {
        self.filtered_indices.len()
    }

    fn max_visible_rows(len: usize) -> usize {
        MAX_POPUP_ROWS.min(len.max(1))
    }

    fn apply_filter(&mut self) {
        // Filter + sort while preserving the current selection when possible.
        let previously_selected = self
            .state
            .selected_idx
            .and_then(|visible_idx| self.filtered_indices.get(visible_idx).copied());

        let filter = self.search_query.trim();
        if filter.is_empty() {
            self.filtered_indices = (0..self.items.len()).collect();
        } else {
            let mut matches: Vec<(usize, i32)> = Vec::new();
            for (idx, item) in self.items.iter().enumerate() {
                let display_name = item.name.as_str();
                if let Some((_indices, score)) = match_skill(filter, display_name, &item.skill_name)
                {
                    matches.push((idx, score));
                }
            }

            matches.sort_by(|a, b| {
                a.1.cmp(&b.1).then_with(|| {
                    let an = self.items[a.0].name.as_str();
                    let bn = self.items[b.0].name.as_str();
                    an.cmp(bn)
                })
            });

            self.filtered_indices = matches.into_iter().map(|(idx, _score)| idx).collect();
        }

        let len = self.filtered_indices.len();
        self.state.selected_idx = previously_selected
            .and_then(|actual_idx| {
                self.filtered_indices
                    .iter()
                    .position(|idx| *idx == actual_idx)
            })
            .or_else(|| (len > 0).then_some(0));

        let visible = Self::max_visible_rows(len);
        self.state.clamp_selection(len);
        self.state.ensure_visible(len, visible);
    }

    fn build_rows(&self) -> Vec<GenericDisplayRow> {
        self.filtered_indices
            .iter()
            .enumerate()
            .filter_map(|(visible_idx, actual_idx)| {
                self.items.get(*actual_idx).map(|item| {
                    let is_selected = self.state.selected_idx == Some(visible_idx);
                    let prefix = if is_selected { '›' } else { ' ' };
                    let marker = if item.enabled { 'x' } else { ' ' };
                    let item_name = truncate_skill_name(&item.name);
                    let name = format!("{prefix} [{marker}] {item_name}");
                    GenericDisplayRow {
                        name,
                        description: Some(item.description.clone()),
                        ..Default::default()
                    }
                })
            })
            .collect()
    }

    fn move_up(&mut self) {
        let len = self.visible_len();
        self.state.move_up_wrap(len);
        let visible = Self::max_visible_rows(len);
        self.state.ensure_visible(len, visible);
    }

    fn move_down(&mut self) {
        let len = self.visible_len();
        self.state.move_down_wrap(len);
        let visible = Self::max_visible_rows(len);
        self.state.ensure_visible(len, visible);
    }

    fn toggle_selected(&mut self) {
        let Some(idx) = self.state.selected_idx else {
            return;
        };
        let Some(actual_idx) = self.filtered_indices.get(idx).copied() else {
            return;
        };
        let Some(item) = self.items.get_mut(actual_idx) else {
            return;
        };

        item.enabled = !item.enabled;
        self.app_event_tx.send(AppEvent::SetSkillEnabled {
            path: item.path.clone(),
            enabled: item.enabled,
        });
    }

    fn close(&mut self) {
        if self.complete {
            return;
        }
        self.complete = true;
        self.app_event_tx.send(AppEvent::ManageSkillsClosed);
        self.app_event_tx.send(AppEvent::CodexOp(Op::ListSkills {
            cwds: Vec::new(),
            force_reload: true,
        }));
    }

    fn rows_width(total_width: u16) -> u16 {
        total_width.saturating_sub(2)
    }

    fn rows_height(&self, rows: &[GenericDisplayRow]) -> u16 {
        rows.len().clamp(1, MAX_POPUP_ROWS).try_into().unwrap_or(1)
    }
}

impl BottomPaneView for SkillsToggleView {
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
            } /* ^P */ => self.move_up(),
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
            } /* ^N */ => self.move_down(),
            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } => {
                self.search_query.pop();
                self.apply_filter();
            }
            KeyEvent {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.toggle_selected(),
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.on_ctrl_c();
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                ..
            } if !modifiers.contains(KeyModifiers::CONTROL)
                && !modifiers.contains(KeyModifiers::ALT) =>
            {
                self.search_query.push(c);
                self.apply_filter();
            }
            _ => {}
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.close();
        CancellationEvent::Handled
    }
}

impl Renderable for SkillsToggleView {
    fn desired_height(&self, width: u16) -> u16 {
        let rows = self.build_rows();
        let rows_height = self.rows_height(&rows);

        let mut height = self.header.desired_height(width.saturating_sub(4));
        height = height.saturating_add(rows_height + 3);
        height = height.saturating_add(2);
        height.saturating_add(1)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        // Reserve the footer line for the key-hint row.
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
        let rows_height = self.rows_height(&rows);
        let [header_area, _, search_area, list_area] = Layout::vertical([
            Constraint::Max(header_height),
            Constraint::Max(1),
            Constraint::Length(2),
            Constraint::Length(rows_height),
        ])
        .areas(content_area.inset(Insets::vh(1, 2)));

        self.header.render(header_area, buf);

        // Render the search prompt as two lines to mimic the composer.
        if search_area.height >= 2 {
            let [placeholder_area, input_area] =
                Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(search_area);
            Line::from(SEARCH_PLACEHOLDER.dim()).render(placeholder_area, buf);
            let line = if self.search_query.is_empty() {
                Line::from(vec![SEARCH_PROMPT_PREFIX.dim()])
            } else {
                Line::from(vec![
                    SEARCH_PROMPT_PREFIX.dim(),
                    self.search_query.clone().into(),
                ])
            };
            line.render(input_area, buf);
        } else if search_area.height > 0 {
            let query_span = if self.search_query.is_empty() {
                SEARCH_PLACEHOLDER.dim()
            } else {
                self.search_query.clone().into()
            };
            Line::from(query_span).render(search_area, buf);
        }

        if list_area.height > 0 {
            let render_area = Rect {
                x: list_area.x.saturating_sub(2),
                y: list_area.y,
                width: rows_width.max(1),
                height: list_area.height,
            };
            render_rows_single_line(
                render_area,
                buf,
                &rows,
                &self.state,
                render_area.height as usize,
                "no matches",
            );
        }

        let hint_area = Rect {
            x: footer_area.x + 2,
            y: footer_area.y,
            width: footer_area.width.saturating_sub(2),
            height: footer_area.height,
        };
        self.footer_hint.clone().dim().render(hint_area, buf);
    }
}

fn skills_toggle_hint_line() -> Line<'static> {
    Line::from(vec![
        "Press ".into(),
        key_hint::plain(KeyCode::Char(' ')).into(),
        " or ".into(),
        key_hint::plain(KeyCode::Enter).into(),
        " to toggle; ".into(),
        key_hint::plain(KeyCode::Esc).into(),
        " to close".into(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use insta::assert_snapshot;
    use ratatui::layout::Rect;
    use tokio::sync::mpsc::unbounded_channel;

    fn render_lines(view: &SkillsToggleView, width: u16) -> String {
        let height = view.desired_height(width);
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);

        let lines: Vec<String> = (0..area.height)
            .map(|row| {
                let mut line = String::new();
                for col in 0..area.width {
                    let symbol = buf[(area.x + col, area.y + row)].symbol();
                    if symbol.is_empty() {
                        line.push(' ');
                    } else {
                        line.push_str(symbol);
                    }
                }
                line
            })
            .collect();
        lines.join("\n")
    }

    #[test]
    fn renders_basic_popup() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let items = vec![
            SkillsToggleItem {
                name: "Repo Scout".to_string(),
                skill_name: "repo_scout".to_string(),
                description: "Summarize the repo layout".to_string(),
                enabled: true,
                path: PathBuf::from("/tmp/skills/repo_scout.toml"),
            },
            SkillsToggleItem {
                name: "Changelog Writer".to_string(),
                skill_name: "changelog_writer".to_string(),
                description: "Draft release notes".to_string(),
                enabled: false,
                path: PathBuf::from("/tmp/skills/changelog_writer.toml"),
            },
        ];
        let view = SkillsToggleView::new(items, tx);
        assert_snapshot!("skills_toggle_basic", render_lines(&view, 72));
    }
}

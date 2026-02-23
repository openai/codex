use std::path::Path;

use crate::key_hint;
use crate::text_formatting::truncate_text;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_app_server_protocol::build_turns_from_rollout_items;
use codex_core::RolloutRecorder;
use codex_protocol::protocol::RolloutItem;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize as _;
use ratatui::text::Line;
use tokio_stream::StreamExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ForkTurnSelection {
    Fork { nth_user_message: usize },
    Cancel,
    Exit,
}

pub(crate) async fn run_fork_turn_picker(
    tui: &mut Tui,
    rollout_path: &Path,
) -> Result<ForkTurnSelection> {
    let initial_history = RolloutRecorder::get_rollout_history(rollout_path).await?;
    let rollout_items = initial_history.get_rollout_items();
    let rows = turn_rows_from_rollout_items(&rollout_items);

    let alt = AltScreenGuard::enter(tui);
    let mut state = ForkTurnPickerState::new(alt.tui.frame_requester(), rows);
    state.request_frame();

    let mut tui_events = alt.tui.event_stream().fuse();
    loop {
        match tui_events.next().await {
            Some(TuiEvent::Key(key)) => {
                if matches!(key.kind, KeyEventKind::Release) {
                    continue;
                }
                if let Some(selection) = state.handle_key(key) {
                    return Ok(selection);
                }
            }
            Some(TuiEvent::Draw) => {
                if let Ok(size) = alt.tui.terminal.size() {
                    let list_height = size.height.saturating_sub(3) as usize;
                    state.update_view_rows(list_height);
                }
                draw_picker(alt.tui, &state)?;
            }
            Some(_) => {}
            None => break,
        }
    }

    Ok(ForkTurnSelection::Cancel)
}

struct AltScreenGuard<'a> {
    tui: &'a mut Tui,
}

impl<'a> AltScreenGuard<'a> {
    fn enter(tui: &'a mut Tui) -> Self {
        let _ = tui.enter_alt_screen();
        Self { tui }
    }
}

impl Drop for AltScreenGuard<'_> {
    fn drop(&mut self) {
        let _ = self.tui.leave_alt_screen();
    }
}

#[derive(Debug, Clone)]
struct TurnRow {
    turn_number: usize,
    user_turn_number: usize,
    nth_user_message: usize,
    status: TurnStatus,
    user_preview: String,
    agent_preview: String,
}

impl TurnRow {
    fn matches_query(&self, query: &str) -> bool {
        self.user_preview.to_lowercase().contains(query)
            || self.agent_preview.to_lowercase().contains(query)
            || self.status_label().to_lowercase().contains(query)
    }

    fn status_label(&self) -> &'static str {
        match self.status {
            TurnStatus::Completed => "completed",
            TurnStatus::Interrupted => "interrupted",
            TurnStatus::Failed => "failed",
            TurnStatus::InProgress => "in progress",
        }
    }
}

struct ForkTurnPickerState {
    requester: FrameRequester,
    all_rows: Vec<TurnRow>,
    filtered_rows: Vec<TurnRow>,
    selected: usize,
    scroll_top: usize,
    query: String,
    view_rows: Option<usize>,
}

impl ForkTurnPickerState {
    fn new(requester: FrameRequester, rows: Vec<TurnRow>) -> Self {
        let selected = rows.len().saturating_sub(1);
        let mut state = Self {
            requester,
            all_rows: rows.clone(),
            filtered_rows: rows,
            selected,
            scroll_top: 0,
            query: String::new(),
            view_rows: None,
        };
        state.ensure_selected_visible();
        state
    }

    fn request_frame(&self) {
        self.requester.schedule_frame();
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<ForkTurnSelection> {
        match key.code {
            KeyCode::Esc => return Some(ForkTurnSelection::Cancel),
            KeyCode::Char('c')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                return Some(ForkTurnSelection::Exit);
            }
            KeyCode::Enter => {
                let row = self.filtered_rows.get(self.selected)?;
                return Some(ForkTurnSelection::Fork {
                    nth_user_message: row.nth_user_message,
                });
            }
            KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                    self.ensure_selected_visible();
                }
                self.request_frame();
            }
            KeyCode::Down => {
                if self.selected + 1 < self.filtered_rows.len() {
                    self.selected += 1;
                    self.ensure_selected_visible();
                }
                self.request_frame();
            }
            KeyCode::PageUp => {
                let step = self.view_rows.unwrap_or(10).max(1);
                if self.selected > 0 {
                    self.selected = self.selected.saturating_sub(step);
                    self.ensure_selected_visible();
                    self.request_frame();
                }
            }
            KeyCode::PageDown => {
                if !self.filtered_rows.is_empty() {
                    let step = self.view_rows.unwrap_or(10).max(1);
                    let max_index = self.filtered_rows.len().saturating_sub(1);
                    self.selected = (self.selected + step).min(max_index);
                    self.ensure_selected_visible();
                    self.request_frame();
                }
            }
            KeyCode::Backspace => {
                let mut new_query = self.query.clone();
                new_query.pop();
                self.set_query(new_query);
            }
            KeyCode::Char(c) => {
                if !key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                    && !key.modifiers.contains(crossterm::event::KeyModifiers::ALT)
                {
                    let mut new_query = self.query.clone();
                    new_query.push(c);
                    self.set_query(new_query);
                }
            }
            _ => {}
        }
        None
    }

    fn set_query(&mut self, new_query: String) {
        if self.query == new_query {
            return;
        }
        self.query = new_query;
        self.apply_filter();
    }

    fn apply_filter(&mut self) {
        if self.query.is_empty() {
            self.filtered_rows = self.all_rows.clone();
        } else {
            let q = self.query.to_lowercase();
            self.filtered_rows = self
                .all_rows
                .iter()
                .filter(|row| row.matches_query(&q))
                .cloned()
                .collect();
        }
        self.selected = self.filtered_rows.len().saturating_sub(1);
        self.ensure_selected_visible();
        self.request_frame();
    }

    fn update_view_rows(&mut self, rows: usize) {
        self.view_rows = if rows == 0 { None } else { Some(rows) };
        self.ensure_selected_visible();
    }

    fn ensure_selected_visible(&mut self) {
        if self.filtered_rows.is_empty() {
            self.scroll_top = 0;
            return;
        }
        let capacity = self.view_rows.unwrap_or(self.filtered_rows.len()).max(1);

        if self.selected < self.scroll_top {
            self.scroll_top = self.selected;
        } else {
            let last_visible = self.scroll_top.saturating_add(capacity - 1);
            if self.selected > last_visible {
                self.scroll_top = self.selected.saturating_sub(capacity - 1);
            }
        }

        let max_start = self.filtered_rows.len().saturating_sub(capacity);
        if self.scroll_top > max_start {
            self.scroll_top = max_start;
        }
    }
}

fn turn_rows_from_rollout_items(items: &[RolloutItem]) -> Vec<TurnRow> {
    let turns = build_turns_from_rollout_items(items);
    turn_rows_from_turns(&turns)
}

fn turn_rows_from_turns(turns: &[Turn]) -> Vec<TurnRow> {
    let total_user_turns = turns
        .iter()
        .filter(|turn| turn_has_user_message(turn))
        .count();
    let mut user_turns_seen = 0usize;
    let mut rows = Vec::new();

    for (idx, turn) in turns.iter().enumerate() {
        let has_user_message = turn_has_user_message(turn);
        if has_user_message {
            user_turns_seen = user_turns_seen.saturating_add(1);
        }

        let has_agent_message = turn_has_agent_message(turn);
        if !has_user_message || !has_agent_message || matches!(turn.status, TurnStatus::InProgress)
        {
            continue;
        }

        let nth_user_message = if user_turns_seen == total_user_turns {
            usize::MAX
        } else {
            user_turns_seen
        };

        rows.push(TurnRow {
            turn_number: idx.saturating_add(1),
            user_turn_number: user_turns_seen,
            nth_user_message,
            status: turn.status.clone(),
            user_preview: extract_user_preview(turn),
            agent_preview: extract_agent_preview(turn),
        });
    }

    rows
}

fn turn_has_user_message(turn: &Turn) -> bool {
    turn.items
        .iter()
        .any(|item| matches!(item, ThreadItem::UserMessage { .. }))
}

fn turn_has_agent_message(turn: &Turn) -> bool {
    turn.items
        .iter()
        .any(|item| matches!(item, ThreadItem::AgentMessage { .. }))
}

fn extract_user_preview(turn: &Turn) -> String {
    for item in &turn.items {
        let ThreadItem::UserMessage { content, .. } = item else {
            continue;
        };
        let text = content
            .iter()
            .filter_map(|entry| match entry {
                UserInput::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if !text.is_empty() {
            return text;
        }
        if !content.is_empty() {
            return "(non-text input)".to_string();
        }
    }

    "(no user message)".to_string()
}

fn extract_agent_preview(turn: &Turn) -> String {
    let last_agent_text = turn.items.iter().filter_map(|item| match item {
        ThreadItem::AgentMessage { text, .. } => Some(text.as_str()),
        _ => None,
    });
    for text in last_agent_text.rev() {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    "(no agent message)".to_string()
}

fn draw_picker(tui: &mut Tui, state: &ForkTurnPickerState) -> std::io::Result<()> {
    let height = tui.terminal.size()?.height;
    tui.draw(height, |frame| {
        let area = frame.area();
        let [header, search, list, hint] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(area.height.saturating_sub(3)),
            Constraint::Length(1),
        ])
        .areas(area);

        let header_line: Line = vec![
            "Fork from a previous turn".bold().cyan(),
            "  ".into(),
            format!("({} responses)", state.all_rows.len()).dim(),
        ]
        .into();
        frame.render_widget_ref(header_line, header);

        let q = if state.query.is_empty() {
            "Type to search turns".dim().to_string()
        } else {
            format!("Search: {}", state.query)
        };
        frame.render_widget_ref(Line::from(q), search);

        render_list(frame, list, state);

        let hint_line: Line = vec![
            key_hint::plain(KeyCode::Enter).into(),
            " to fork ".dim(),
            "    ".into(),
            key_hint::plain(KeyCode::Esc).into(),
            " to cancel ".dim(),
            "    ".into(),
            key_hint::ctrl(KeyCode::Char('c')).into(),
            " to quit ".dim(),
            "    ".into(),
            key_hint::plain(KeyCode::Up).into(),
            "/".dim(),
            key_hint::plain(KeyCode::Down).into(),
            " to browse".dim(),
        ]
        .into();
        frame.render_widget_ref(hint_line, hint);
    })
}

fn render_list(frame: &mut crate::custom_terminal::Frame, area: Rect, state: &ForkTurnPickerState) {
    if area.height == 0 {
        return;
    }

    let rows = &state.filtered_rows;
    if rows.is_empty() {
        let line = if state.query.is_empty() {
            "No previous model responses in this conversation yet"
                .italic()
                .dim()
        } else {
            "No turns match your search".italic().dim()
        };
        frame.render_widget_ref(Line::from(line), area);
        return;
    }

    let capacity = area.height as usize;
    let start = state.scroll_top.min(rows.len().saturating_sub(1));
    let end = rows.len().min(start + capacity);
    let mut y = area.y;

    for (idx, row) in rows[start..end].iter().enumerate() {
        let is_selected = start + idx == state.selected;
        let marker = if is_selected {
            "> ".bold()
        } else {
            "  ".into()
        };
        let prefix = format!(
            "U{:>3} T{:>3} {:<11}",
            row.user_turn_number,
            row.turn_number,
            row.status_label()
        );
        let fixed_width = 2usize.saturating_add(prefix.len()).saturating_add(3);
        let available = area.width as usize;
        let preview_width = available.saturating_sub(fixed_width);
        let user_max = preview_width.saturating_mul(3) / 5;
        let agent_max = preview_width.saturating_sub(user_max);
        let user_preview = truncate_text(&row.user_preview, user_max.max(8));
        let agent_preview = truncate_text(&row.agent_preview, agent_max.max(8));

        let line: Line = vec![
            marker,
            prefix.cyan(),
            "  ".into(),
            user_preview.into(),
            " | ".dim(),
            agent_preview.dim(),
        ]
        .into();
        frame.render_widget_ref(line, Rect::new(area.x, y, area.width, 1));
        y = y.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_terminal::Terminal;
    use crate::test_backend::VT100Backend;
    use codex_app_server_protocol::TurnError;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use ratatui::layout::Constraint;
    use ratatui::layout::Layout;

    fn user_message(text: &str) -> ThreadItem {
        ThreadItem::UserMessage {
            id: "user-1".to_string(),
            content: vec![UserInput::Text {
                text: text.to_string(),
                text_elements: Vec::new(),
            }],
        }
    }

    fn user_non_text() -> ThreadItem {
        ThreadItem::UserMessage {
            id: "user-1".to_string(),
            content: vec![UserInput::Image {
                url: "https://example.com/image.png".to_string(),
            }],
        }
    }

    fn agent_message(text: &str) -> ThreadItem {
        ThreadItem::AgentMessage {
            id: "agent-1".to_string(),
            text: text.to_string(),
            phase: None,
        }
    }

    fn turn(status: TurnStatus, items: Vec<ThreadItem>) -> Turn {
        Turn {
            id: "turn-id".to_string(),
            items,
            status,
            error: None::<TurnError>,
        }
    }

    #[test]
    fn turn_rows_use_max_cutoff_for_last_user_turn() {
        let turns = vec![
            turn(
                TurnStatus::Completed,
                vec![user_message("first"), agent_message("reply 1")],
            ),
            turn(
                TurnStatus::Completed,
                vec![user_message("second"), agent_message("reply 2")],
            ),
        ];

        let rows = turn_rows_from_turns(&turns);

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].nth_user_message, 1);
        assert_eq!(rows[1].nth_user_message, usize::MAX);
    }

    #[test]
    fn turn_rows_count_hidden_user_turns_when_computing_cutoff() {
        let turns = vec![
            turn(
                TurnStatus::Completed,
                vec![user_message("u1"), agent_message("a1")],
            ),
            turn(TurnStatus::Failed, vec![user_message("u2")]),
            turn(
                TurnStatus::Completed,
                vec![user_message("u3"), agent_message("a3")],
            ),
        ];

        let rows = turn_rows_from_turns(&turns);

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].user_turn_number, 1);
        assert_eq!(rows[0].nth_user_message, 1);
        assert_eq!(rows[1].user_turn_number, 3);
        assert_eq!(rows[1].nth_user_message, usize::MAX);
    }

    #[test]
    fn extract_user_preview_uses_non_text_fallback() {
        let preview = extract_user_preview(&turn(
            TurnStatus::Completed,
            vec![user_non_text(), agent_message("ok")],
        ));
        assert_eq!(preview, "(non-text input)");
    }

    #[test]
    fn extract_agent_preview_uses_last_non_empty_agent_message() {
        let preview = extract_agent_preview(&turn(
            TurnStatus::Completed,
            vec![
                user_message("prompt"),
                agent_message(""),
                agent_message("final answer"),
            ],
        ));
        assert_eq!(preview, "final answer");
    }

    #[test]
    fn fork_turn_picker_list_snapshot() {
        let rows = vec![
            TurnRow {
                turn_number: 1,
                user_turn_number: 1,
                nth_user_message: 1,
                status: TurnStatus::Completed,
                user_preview: "Refactor /fork to open a picker".to_string(),
                agent_preview: "I'll inspect the existing /resume flow first.".to_string(),
            },
            TurnRow {
                turn_number: 2,
                user_turn_number: 2,
                nth_user_message: 2,
                status: TurnStatus::Interrupted,
                user_preview: "Continue after lunch".to_string(),
                agent_preview: "Resuming from where we left off.".to_string(),
            },
            TurnRow {
                turn_number: 3,
                user_turn_number: 3,
                nth_user_message: usize::MAX,
                status: TurnStatus::Completed,
                user_preview: "Now add tests".to_string(),
                agent_preview: "Added dispatch and picker unit tests.".to_string(),
            },
        ];
        let mut state = ForkTurnPickerState::new(FrameRequester::test_dummy(), rows);
        state.view_rows = Some(3);
        state.selected = 1;
        state.scroll_top = 0;
        state.update_view_rows(3);

        let width: u16 = 90;
        let height: u16 = 3;
        let backend = VT100Backend::new(width, height);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, height));

        {
            let mut frame = terminal.get_frame();
            let area = frame.area();
            let segments = Layout::vertical([Constraint::Min(1)]).split(area);
            render_list(&mut frame, segments[0], &state);
        }
        terminal.flush().expect("flush");

        let snapshot = terminal.backend().to_string();
        assert_snapshot!("fork_turn_picker_list", snapshot);
    }
}

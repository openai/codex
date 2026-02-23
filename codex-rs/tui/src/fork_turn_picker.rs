use std::path::Path;

use crate::key_hint;
use crate::text_formatting::truncate_text;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use codex_core::RolloutRecorder;
use codex_core::parse_turn_item;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::TurnItem;
use codex_protocol::items::UserMessageItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::user_input::UserInput;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize as _;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;
use tokio_stream::StreamExt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ForkTurnEntry {
    pub(crate) user_request: String,
    pub(crate) model_response: Option<String>,
}

pub(crate) async fn load_fork_turn_entries(path: &Path) -> std::io::Result<Vec<ForkTurnEntry>> {
    let history = RolloutRecorder::get_rollout_history(path).await?;
    Ok(fork_turn_entries_from_rollout_items(
        &history.get_rollout_items(),
    ))
}

pub(crate) async fn run_fork_turn_picker(
    tui: &mut Tui,
    turns: Vec<ForkTurnEntry>,
) -> Result<Option<usize>> {
    if turns.is_empty() {
        return Ok(None);
    }

    let alt = AltScreenGuard::enter(tui);
    let mut screen = ForkTurnPickerScreen::new(alt.tui.frame_requester(), turns);

    let _ = alt.tui.draw(u16::MAX, |frame| {
        frame.render_widget_ref(&screen, frame.area());
    });

    let events = alt.tui.event_stream();
    tokio::pin!(events);

    while !screen.is_done() {
        if let Some(event) = events.next().await {
            match event {
                TuiEvent::Key(key_event) => screen.handle_key(key_event),
                TuiEvent::Paste(_) => {}
                TuiEvent::Draw => {
                    let _ = alt.tui.draw(u16::MAX, |frame| {
                        frame.render_widget_ref(&screen, frame.area());
                    });
                }
            }
        } else {
            screen.cancel();
            break;
        }
    }

    Ok(screen.outcome())
}

fn fork_turn_entries_from_rollout_items(items: &[RolloutItem]) -> Vec<ForkTurnEntry> {
    let mut turns: Vec<ForkTurnEntry> = Vec::new();

    for item in items {
        match item {
            RolloutItem::ResponseItem(response_item) => match parse_turn_item(response_item) {
                Some(TurnItem::UserMessage(user)) => {
                    turns.push(ForkTurnEntry {
                        user_request: user_turn_preview(&user),
                        model_response: None,
                    });
                }
                Some(TurnItem::AgentMessage(agent)) => {
                    let response = agent_turn_preview(&agent.content);
                    if !response.is_empty()
                        && let Some(last_turn) = turns.last_mut()
                    {
                        last_turn.model_response = Some(response);
                    }
                }
                _ => {}
            },
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(rollback)) => {
                let dropped = usize::try_from(rollback.num_turns).unwrap_or(usize::MAX);
                let kept = turns.len().saturating_sub(dropped);
                turns.truncate(kept);
            }
            _ => {}
        }
    }

    turns
}

fn user_turn_preview(user: &UserMessageItem) -> String {
    let mut image_count = 0usize;
    let mut local_image_count = 0usize;
    let mut skill_count = 0usize;
    let mut mention_count = 0usize;

    for item in &user.content {
        match item {
            UserInput::Text { .. } => {}
            UserInput::Image { .. } => image_count += 1,
            UserInput::LocalImage { .. } => local_image_count += 1,
            UserInput::Skill { .. } => skill_count += 1,
            UserInput::Mention { .. } => mention_count += 1,
            _ => {}
        }
    }

    let text = user
        .message()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let mut suffix_parts = Vec::new();
    let total_images = image_count + local_image_count;
    if total_images > 0 {
        let noun = if total_images == 1 { "image" } else { "images" };
        suffix_parts.push(format!("{total_images} {noun}"));
    }
    if skill_count > 0 {
        let noun = if skill_count == 1 { "skill" } else { "skills" };
        suffix_parts.push(format!("{skill_count} {noun}"));
    }
    if mention_count > 0 {
        let noun = if mention_count == 1 {
            "mention"
        } else {
            "mentions"
        };
        suffix_parts.push(format!("{mention_count} {noun}"));
    }

    let suffix = if suffix_parts.is_empty() {
        String::new()
    } else {
        format!(" [{}]", suffix_parts.join(", "))
    };

    if text.is_empty() {
        if suffix.is_empty() {
            "[empty input]".to_string()
        } else {
            format!("[non-text input]{suffix}")
        }
    } else {
        format!("{text}{suffix}")
    }
}

fn agent_turn_preview(content: &[AgentMessageContent]) -> String {
    let text = content
        .iter()
        .map(|item| match item {
            AgentMessageContent::Text { text } => text.as_str(),
        })
        .collect::<Vec<_>>()
        .join("");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn nth_user_message_for_fork(selected_index: usize, turn_count: usize) -> usize {
    if selected_index.saturating_add(1) >= turn_count {
        usize::MAX
    } else {
        selected_index + 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForkTurnPickerOutcome {
    Cancelled,
    Selected(usize),
}

struct ForkTurnPickerScreen {
    request_frame: FrameRequester,
    turns: Vec<ForkTurnEntry>,
    selected: usize,
    scroll_top: usize,
    done: bool,
    outcome: ForkTurnPickerOutcome,
}

impl ForkTurnPickerScreen {
    fn new(request_frame: FrameRequester, turns: Vec<ForkTurnEntry>) -> Self {
        let selected = turns.len().saturating_sub(1);
        Self {
            request_frame,
            turns,
            selected,
            scroll_top: 0,
            done: false,
            outcome: ForkTurnPickerOutcome::Cancelled,
        }
    }

    fn is_done(&self) -> bool {
        self.done
    }

    fn outcome(&self) -> Option<usize> {
        match self.outcome {
            ForkTurnPickerOutcome::Cancelled => None,
            ForkTurnPickerOutcome::Selected(nth_user_message) => Some(nth_user_message),
        }
    }

    fn confirm(&mut self) {
        self.outcome = ForkTurnPickerOutcome::Selected(nth_user_message_for_fork(
            self.selected,
            self.turns.len(),
        ));
        self.done = true;
        self.request_frame.schedule_frame();
    }

    fn cancel(&mut self) {
        self.outcome = ForkTurnPickerOutcome::Cancelled;
        self.done = true;
        self.request_frame.schedule_frame();
    }

    fn handle_key(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }

        if is_ctrl_exit_combo(key_event) {
            self.cancel();
            return;
        }

        if let KeyCode::Char(ch) = key_event.code
            && key_event.modifiers == KeyModifiers::NONE
            && ch.is_ascii_digit()
            && let Some(digit) = ch.to_digit(10)
            && digit != 0
        {
            self.select_display_number(usize::try_from(digit).unwrap_or(usize::MAX));
            return;
        }

        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => self.cancel(),
            KeyCode::Enter => self.confirm(),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Home | KeyCode::Char('g') => self.select_index(0),
            KeyCode::End | KeyCode::Char('G') => {
                self.select_index(self.turns.len().saturating_sub(1))
            }
            KeyCode::PageUp => self.page_move(-1),
            KeyCode::PageDown => self.page_move(1),
            _ => {}
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let next = if delta < 0 {
            self.selected.saturating_sub(delta.unsigned_abs())
        } else {
            self.selected
                .saturating_add(usize::try_from(delta).unwrap_or(usize::MAX))
                .min(self.turns.len().saturating_sub(1))
        };
        self.select_index(next);
    }

    fn page_move(&mut self, delta_pages: isize) {
        let page = self.visible_list_entries();
        let step = if page == 0 { 1 } else { page };
        let step = isize::try_from(step).unwrap_or(isize::MAX);
        self.move_selection(step.saturating_mul(delta_pages));
    }

    fn select_index(&mut self, idx: usize) {
        if idx == self.selected {
            return;
        }
        self.selected = idx.min(self.turns.len().saturating_sub(1));
        self.ensure_selected_visible();
        self.request_frame.schedule_frame();
    }

    fn select_display_number(&mut self, display_number: usize) {
        if display_number == 0 || display_number > self.turns.len() {
            return;
        }
        let idx = self.turns.len().saturating_sub(display_number);
        self.select_index(idx);
    }

    fn visible_list_entries(&self) -> usize {
        4
    }

    fn ensure_selected_visible(&mut self) {
        let entries = self.visible_list_entries().max(1);
        if self.selected < self.scroll_top {
            self.scroll_top = self.selected;
        } else if self.selected >= self.scroll_top.saturating_add(entries) {
            self.scroll_top = self.selected.saturating_add(1).saturating_sub(entries);
        }
    }

    fn effective_scroll_top(&self, visible_entries: usize) -> usize {
        let rows = visible_entries.max(1);
        if self.selected < self.scroll_top {
            self.selected
        } else if self.selected >= self.scroll_top.saturating_add(rows) {
            self.selected.saturating_add(1).saturating_sub(rows)
        } else {
            self.scroll_top
        }
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(6),
                Constraint::Length(11),
                Constraint::Length(2),
            ])
            .split(area);

        let heading = vec![
            Line::from(vec!["/fork".magenta(), " select a turn".into()]),
            Line::from("Choose a turn from the current conversation to fork after."),
            Line::from(format!(
                "{} turns available. The newest turn is selected by default.",
                self.turns.len()
            ))
            .dim(),
        ];
        Paragraph::new(heading)
            .wrap(Wrap { trim: false })
            .render(chunks[0], buf);

        self.render_turn_list(chunks[1], buf);
        self.render_selected_preview(chunks[2], buf);

        let hints = Line::from(vec![
            "Use ".dim(),
            key_hint::plain(KeyCode::Up).into(),
            "/".dim(),
            key_hint::plain(KeyCode::Down).into(),
            " (or j/k) to choose, ".dim(),
            key_hint::plain(KeyCode::Enter).into(),
            " to fork, ".dim(),
            key_hint::plain(KeyCode::Esc).into(),
            " to cancel, digits jump (1 = latest)".dim(),
        ]);
        Paragraph::new(hints)
            .wrap(Wrap { trim: false })
            .render(chunks[3], buf);
    }

    fn render_turn_list(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Turns (oldest to newest, 1 = latest)");
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.is_empty() {
            return;
        }

        let visible_rows = usize::from(inner.height);
        let visible_entries = (visible_rows / 2).max(1);
        let mut lines = Vec::with_capacity(visible_rows);
        let scroll_top = self.effective_scroll_top(visible_entries);
        let end = self
            .effective_scroll_top(visible_entries)
            .saturating_add(visible_entries)
            .min(self.turns.len());
        for idx in scroll_top..end {
            let is_selected = idx == self.selected;
            let display_number = self.display_turn_number(idx);
            let label = format!("{display_number}:");
            let user_prefix = format!("{} {} ", if is_selected { "›" } else { " " }, label);
            let user_width = usize::from(inner.width).saturating_sub(user_prefix.len());
            let user_text = if user_width == 0 {
                String::new()
            } else {
                truncate_text(&self.turns[idx].user_request, user_width)
            };
            let user_line = if is_selected {
                Line::from(vec![user_prefix.cyan(), user_text.into()])
            } else {
                Line::from(vec![user_prefix.dim(), user_text.into()])
            };
            lines.push(user_line);

            if lines.len() >= visible_rows {
                break;
            }

            let response_prefix = " ".repeat(4 + label.len());
            let response_text = self.turns[idx]
                .model_response
                .as_deref()
                .unwrap_or("[no model response recorded]");
            let response_width = usize::from(inner.width).saturating_sub(response_prefix.len());
            let response_text = if response_width == 0 {
                String::new()
            } else {
                truncate_text(response_text, response_width)
            };
            lines.push(Line::from(vec![
                response_prefix.into(),
                response_text.dim(),
            ]));
        }

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(inner, buf);
    }

    fn render_selected_preview(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Selected turn");
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.is_empty() {
            return;
        }

        let selected = &self.turns[self.selected];
        let display_number = self.display_turn_number(self.selected);
        let newer_turns = self
            .turns
            .len()
            .saturating_sub(self.selected)
            .saturating_sub(1);
        let status_line = if newer_turns == 0 {
            "Forking from the latest turn keeps the full current conversation.".to_string()
        } else {
            let suffix = if newer_turns == 1 { "" } else { "s" };
            format!("Forking here omits {newer_turns} newer turn{suffix} from the new thread.")
        };
        let model_response = selected
            .model_response
            .as_deref()
            .unwrap_or("[no model response recorded]");

        let lines = vec![
            Line::from(vec![
                "Selected: ".dim(),
                format!("{display_number}:").cyan(),
                if display_number == 1 {
                    Span::from(" (latest)").dim()
                } else {
                    Span::from("")
                },
            ]),
            Line::from(""),
            Line::from("User request".dim()),
            Line::from(selected.user_request.clone()),
            Line::from(""),
            Line::from("Model response".dim()),
            Line::from(model_response).dim(),
            Line::from(""),
            Line::from(status_line).dim(),
        ];
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(inner, buf);
    }

    fn display_turn_number(&self, idx: usize) -> usize {
        self.turns.len().saturating_sub(idx)
    }
}

impl WidgetRef for &ForkTurnPickerScreen {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        ForkTurnPickerScreen::render(self, area, buf);
    }
}

// Render the picker on the terminal's alternate screen so cancel/confirm does
// not leave a large blank region in the main scrollback.
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

fn is_ctrl_exit_combo(key_event: KeyEvent) -> bool {
    key_event.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key_event.code, KeyCode::Char('c') | KeyCode::Char('d'))
}

#[cfg(test)]
mod tests {
    use super::ForkTurnEntry;
    use super::ForkTurnPickerScreen;
    use super::fork_turn_entries_from_rollout_items;
    use super::nth_user_message_for_fork;
    use crate::custom_terminal::Terminal;
    use crate::test_backend::VT100Backend;
    use crate::tui::FrameRequester;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::RolloutItem;
    use codex_protocol::protocol::ThreadRolledBackEvent;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use ratatui::layout::Rect;

    fn user_msg(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: text.to_string(),
            }],
            end_turn: None,
            phase: None,
        }
    }

    fn assistant_msg(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: text.to_string(),
            }],
            end_turn: None,
            phase: None,
        }
    }

    #[test]
    fn extracts_effective_user_turns_and_applies_rollbacks() {
        let items = vec![
            RolloutItem::ResponseItem(user_msg("first request")),
            RolloutItem::ResponseItem(assistant_msg("first answer")),
            RolloutItem::ResponseItem(user_msg("second request")),
            RolloutItem::ResponseItem(assistant_msg("second answer")),
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
                num_turns: 1,
            })),
            RolloutItem::ResponseItem(user_msg("replacement request")),
            RolloutItem::ResponseItem(assistant_msg("replacement answer")),
        ];

        let turns = fork_turn_entries_from_rollout_items(&items);

        assert_eq!(
            turns,
            vec![
                ForkTurnEntry {
                    user_request: "first request".to_string(),
                    model_response: Some("first answer".to_string()),
                },
                ForkTurnEntry {
                    user_request: "replacement request".to_string(),
                    model_response: Some("replacement answer".to_string()),
                },
            ]
        );
    }

    #[test]
    fn nth_user_message_mapping_keeps_selected_turn_in_fork() {
        assert_eq!(nth_user_message_for_fork(0, 3), 1);
        assert_eq!(nth_user_message_for_fork(1, 3), 2);
        assert_eq!(nth_user_message_for_fork(2, 3), usize::MAX);
    }

    #[test]
    fn picker_snapshot() {
        let mut screen = ForkTurnPickerScreen::new(
            FrameRequester::test_dummy(),
            vec![
                ForkTurnEntry {
                    user_request: "Initial bug report with stack trace and repro steps".to_string(),
                    model_response: Some(
                        "Short answer: not via the current app-server protocol.".to_string(),
                    ),
                },
                ForkTurnEntry {
                    user_request:
                        "Please also handle macOS path edge cases when filenames contain spaces"
                            .to_string(),
                    model_response: Some(
                        "Acknowledged. I will cover macOS path handling and spaces.".to_string(),
                    ),
                },
                ForkTurnEntry {
                    user_request: "One more thing: include tests for rollback + fork interaction"
                        .to_string(),
                    model_response: Some(
                        "I added coverage for rollback semantics in the turn extraction helper."
                            .to_string(),
                    ),
                },
            ],
        );
        screen.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));

        let backend = VT100Backend::new(76, 22);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, 76, 22));

        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(&screen, frame.area());
        }
        terminal.flush().expect("flush");

        assert_snapshot!("fork_turn_picker", terminal.backend());
    }
}

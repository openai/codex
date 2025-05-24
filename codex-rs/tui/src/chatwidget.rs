use std::path::PathBuf;
use std::sync::Arc;
use std::collections::HashSet;

use codex_core::codex_wrapper::init_codex;
use codex_core::config::Config;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::AgentReasoningEvent;
use codex_core::protocol::ApplyPatchApprovalRequestEvent;
use codex_core::protocol::ErrorEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecApprovalRequestEvent;
use codex_core::protocol::ExecCommandBeginEvent;
use codex_core::protocol::ExecCommandEndEvent;
use codex_core::protocol::InputItem;
use codex_core::protocol::McpToolCallBeginEvent;
use codex_core::protocol::McpToolCallEndEvent;
use codex_core::protocol::Op;
use codex_core::protocol::PatchApplyBeginEvent;
use codex_core::protocol::TaskCompleteEvent;
use crossterm::event::KeyEvent;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, WidgetRef};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::unbounded_channel;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPane;
use crate::bottom_pane::BottomPaneParams;
use crate::bottom_pane::InputResult;
use crate::conversation_history_widget::ConversationHistoryWidget;
use crate::history_cell::PatchEventType;
use crate::user_approval_widget::ApprovalRequest;

#[derive(Clone, Copy, Eq, PartialEq)]
enum InputFocus {
    HistoryPane,
    BottomPane,
}

#[derive(PartialEq, Clone, Copy)]
enum SearchDirection {
    Forward,
    Backward,
}

impl Default for SearchDirection {
    fn default() -> Self {
        SearchDirection::Backward
    }
}

#[derive(Debug, Clone)]
enum MatchSource {
    CurrentSession(usize),
    Historical(usize),
}

#[derive(Default)]
struct SearchState {
    is_active: bool,
    search_string: String,
    current_match_index: usize,
    matches: Vec<MatchSource>,
    original_input: Option<String>,
    case_sensitive: bool,
    direction: SearchDirection,
    historical_entries: Vec<String>,
    history_log_id: Option<u64>,
    history_entry_count: usize,
    fetched_entries: usize,
    command_executed: bool,
}

impl SearchState {
    fn toggle_case_sensitivity(&mut self) {
        self.case_sensitive = !self.case_sensitive;
    }

    fn set_history_metadata(&mut self, log_id: u64, entry_count: usize) {
        self.history_log_id = Some(log_id);
        self.history_entry_count = entry_count;
        self.fetched_entries = 0;
        self.historical_entries.clear();
    }

    fn add_historical_entry(&mut self, offset: usize, text: Option<String>) {
        if let Some(text) = text {
            while self.historical_entries.len() <= offset {
                self.historical_entries.push(String::new());
            }
            self.historical_entries[offset] = text;
            self.fetched_entries = self.fetched_entries.max(offset + 1);
        }
    }

    fn reset(&mut self) {
        *self = Self {
            history_log_id: self.history_log_id,
            history_entry_count: self.history_entry_count,
            historical_entries: std::mem::take(&mut self.historical_entries),
            fetched_entries: self.fetched_entries,
            ..Default::default()
        };
    }
}

pub(crate) struct ChatWidget<'a> {
    app_event_tx: AppEventSender,
    codex_op_tx: UnboundedSender<Op>,
    conversation_history: ConversationHistoryWidget,
    bottom_pane: BottomPane<'a>,
    input_focus: InputFocus,
    config: Config,
    initial_user_message: Option<UserMessage>,
    search_state: SearchState,
    current_session_commands: Vec<String>,
}

struct UserMessage {
    text: String,
    image_paths: Vec<PathBuf>,
}

impl From<String> for UserMessage {
    fn from(text: String) -> Self {
        Self {
            text,
            image_paths: Vec::new(),
        }
    }
}

fn create_initial_user_message(text: String, image_paths: Vec<PathBuf>) -> Option<UserMessage> {
    if text.is_empty() && image_paths.is_empty() {
        None
    } else {
        Some(UserMessage { text, image_paths })
    }
}

impl ChatWidget<'_> {
    pub(crate) fn new(
        config: Config,
        app_event_tx: AppEventSender,
        initial_prompt: Option<String>,
        initial_images: Vec<PathBuf>,
    ) -> Self {
        let (codex_op_tx, mut codex_op_rx) = unbounded_channel::<Op>();

        let app_event_tx_clone = app_event_tx.clone();
        // Create the Codex asynchronously so the UI loads as quickly as possible.
        let config_for_agent_loop = config.clone();
        tokio::spawn(async move {
            let (codex, session_event, _ctrl_c) = match init_codex(config_for_agent_loop).await {
                Ok(vals) => vals,
                Err(e) => {
                    // TODO: surface this error to the user.
                    tracing::error!("failed to initialize codex: {e}");
                    return;
                }
            };

            // Forward the captured `SessionInitialized` event that was consumed
            // inside `init_codex()` so it can be rendered in the UI.
            app_event_tx_clone.send(AppEvent::CodexEvent(session_event.clone()));
            let codex = Arc::new(codex);
            let codex_clone = codex.clone();
            tokio::spawn(async move {
                while let Some(op) = codex_op_rx.recv().await {
                    let id = codex_clone.submit(op).await;
                    if let Err(e) = id {
                        tracing::error!("failed to submit op: {e}");
                    }
                }
            });

            while let Ok(event) = codex.next_event().await {
                app_event_tx_clone.send(AppEvent::CodexEvent(event));
            }
        });

        Self {
            app_event_tx: app_event_tx.clone(),
            codex_op_tx,
            conversation_history: ConversationHistoryWidget::new(),
            bottom_pane: BottomPane::new(BottomPaneParams {
                app_event_tx,
                has_input_focus: true,
            }),
            input_focus: InputFocus::BottomPane,
            config,
            initial_user_message: create_initial_user_message(
                initial_prompt.unwrap_or_default(),
                initial_images,
            ),
            search_state: SearchState::default(),
            current_session_commands: Vec::new(),
        }
    }

    pub(crate) fn handle_key_event(&mut self, key_event: KeyEvent) {
        if self.search_state.is_active {
            match key_event.code {
                KeyCode::Char('r') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.search_state.toggle_case_sensitivity();
                    self.perform_search();
                }
                KeyCode::Char(c) => {
                    self.search_state.search_string.push(c);
                    self.perform_search();
                }
                KeyCode::Backspace => {
                    self.search_state.search_string.pop();
                    self.perform_search();
                }
                KeyCode::Esc => {
                    self.reset_search_state();
                }
                KeyCode::Enter => {
                    if self.search_state.matches.is_empty() {
                        self.reset_search_state();
                    } else {
                        let match_source = &self.search_state.matches[self.search_state.current_match_index];
                        let matched_text = match match_source {
                            MatchSource::CurrentSession(idx) => {
                                self.current_session_commands[*idx].clone()
                            }
                            MatchSource::Historical(offset) => {
                                self.search_state.historical_entries[*offset].clone()
                            }
                        };
                        self.submit_user_message(UserMessage::from(matched_text));
                        
                        self.bottom_pane.composer_mut().textarea_mut().select_all();
                        self.bottom_pane.composer_mut().textarea_mut().cut();
                        self.search_state.command_executed = true;
                        self.search_state.reset();
                    }
                }
                KeyCode::Up => {
                    if !self.search_state.matches.is_empty() {
                        self.search_state.direction = SearchDirection::Backward;
                        self.search_state.current_match_index = 
                            if self.search_state.current_match_index == 0 {
                                self.search_state.matches.len() - 1
                            } else {
                                self.search_state.current_match_index - 1
                            };
                        self.update_search_preview();
                    }
                }
                KeyCode::Down => {
                    if !self.search_state.matches.is_empty() {
                        self.search_state.direction = SearchDirection::Forward;
                        self.search_state.current_match_index = 
                            (self.search_state.current_match_index + 1) % self.search_state.matches.len();
                        self.update_search_preview();
                    }
                }
                _ => {}
            }
            self.request_redraw();
            return;
        }

        if key_event.code == KeyCode::Char('r') && key_event.modifiers.contains(KeyModifiers::CONTROL) {
            self.search_state.is_active = true;
            self.search_state.search_string.clear();
            self.search_state.matches.clear();
            self.search_state.current_match_index = 0;
            self.search_state.case_sensitive = false;
            self.search_state.direction = SearchDirection::Backward;
            self.search_state.original_input = Some(self.bottom_pane.composer().textarea().lines().join("\n"));

            if let Some(log_id) = self.search_state.history_log_id {
                for offset in 0..self.search_state.history_entry_count {
                    self.app_event_tx.send(AppEvent::GetHistoryEntry { log_id, offset });
                }
            }

            self.perform_search();
            self.request_redraw();
            return;
        }

        if matches!(key_event.code, crossterm::event::KeyCode::Tab)
            && !self.bottom_pane.is_command_popup_visible()
        {
            self.input_focus = match self.input_focus {
                InputFocus::HistoryPane => InputFocus::BottomPane,
                InputFocus::BottomPane => InputFocus::HistoryPane,
            };
            self.conversation_history
                .set_input_focus(self.input_focus == InputFocus::HistoryPane);
            self.bottom_pane
                .set_input_focus(self.input_focus == InputFocus::BottomPane);
            self.request_redraw();
            return;
        }

        match self.input_focus {
            InputFocus::HistoryPane => {
                let needs_redraw = self.conversation_history.handle_key_event(key_event);
                if needs_redraw {
                    self.request_redraw();
                }
            }
            InputFocus::BottomPane => match self.bottom_pane.handle_key_event(key_event) {
                InputResult::Submitted(text) => {
                    self.submit_user_message(text.into());
                }
                InputResult::None => {}
            },
        }
    }

    fn submit_user_message(&mut self, user_message: UserMessage) {
        let UserMessage { text, image_paths } = user_message;
        let mut items: Vec<InputItem> = Vec::new();

        if !text.is_empty() {
            items.push(InputItem::Text { text: text.clone() });
        }

        for path in image_paths {
            items.push(InputItem::LocalImage { path });
        }

        if items.is_empty() {
            return;
        }

        self.codex_op_tx
            .send(Op::UserInput { items })
            .unwrap_or_else(|e| {
                tracing::error!("failed to send message: {e}");
            });

        // Persist the text to cross-session message history.
        if !text.is_empty() {
            self.codex_op_tx
                .send(Op::AddToHistory { text: text.clone() })
                .unwrap_or_else(|e| {
                    tracing::error!("failed to send AddHistory op: {e}");
                });
            self.current_session_commands.push(text.clone());
            self.conversation_history.add_user_message(text);
        }
        self.conversation_history.scroll_to_bottom();
    }

    pub(crate) fn clear_conversation_history(&mut self) {
        self.conversation_history.clear();
        self.current_session_commands.clear();
        self.request_redraw();
    }

    pub(crate) fn handle_codex_event(&mut self, event: Event) {
        let Event { id, msg } = event;
        match msg {
            EventMsg::SessionConfigured(event) => {
                // Record session information at the top of the conversation.
                self.conversation_history
                    .add_session_info(&self.config, event.clone());

                // Forward history metadata to the bottom pane so the chat
                // composer can navigate through past messages.
                self.bottom_pane
                    .set_history_metadata(event.history_log_id, event.history_entry_count);
                self.search_state.set_history_metadata(event.history_log_id, event.history_entry_count);
                if let Some(user_message) = self.initial_user_message.take() {
                    // If the user provided an initial message, add it to the
                    // conversation history.
                    self.submit_user_message(user_message);
                }

                self.request_redraw();
            }
            EventMsg::AgentMessage(AgentMessageEvent { message }) => {
                self.conversation_history
                    .add_agent_message(&self.config, message);
                self.request_redraw();
            }
            EventMsg::AgentReasoning(AgentReasoningEvent { text }) => {
                self.conversation_history
                    .add_agent_reasoning(&self.config, text);
                self.request_redraw();
            }
            EventMsg::TaskStarted => {
                self.bottom_pane.set_task_running(true);
                self.request_redraw();
            }
            EventMsg::TaskComplete(TaskCompleteEvent {
                last_agent_message: _,
            }) => {
                self.bottom_pane.set_task_running(false);
                self.request_redraw();
            }
            EventMsg::Error(ErrorEvent { message }) => {
                self.conversation_history.add_error(message);
                self.bottom_pane.set_task_running(false);
            }
            EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
                command,
                cwd,
                reason,
            }) => {
                let request = ApprovalRequest::Exec {
                    id,
                    command,
                    cwd,
                    reason,
                };
                self.bottom_pane.push_approval_request(request);
            }
            EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
                changes,
                reason,
                grant_root,
            }) => {
                // ------------------------------------------------------------------
                // Before we even prompt the user for approval we surface the patch
                // summary in the main conversation so that the dialog appears in a
                // sensible chronological order:
                //   (1) codex → proposes patch (HistoryCell::PendingPatch)
                //   (2) UI → asks for approval (BottomPane)
                // This mirrors how command execution is shown (command begins →
                // approval dialog) and avoids surprising the user with a modal
                // prompt before they have seen *what* is being requested.
                // ------------------------------------------------------------------

                self.conversation_history
                    .add_patch_event(PatchEventType::ApprovalRequest, changes);

                self.conversation_history.scroll_to_bottom();

                // Now surface the approval request in the BottomPane as before.
                let request = ApprovalRequest::ApplyPatch {
                    id,
                    reason,
                    grant_root,
                };
                self.bottom_pane.push_approval_request(request);
                self.request_redraw();
            }
            EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                call_id,
                command,
                cwd: _,
            }) => {
                self.conversation_history
                    .add_active_exec_command(call_id, command);
                self.request_redraw();
            }
            EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
                call_id: _,
                auto_approved,
                changes,
            }) => {
                // Even when a patch is auto‑approved we still display the
                // summary so the user can follow along.
                self.conversation_history
                    .add_patch_event(PatchEventType::ApplyBegin { auto_approved }, changes);
                if !auto_approved {
                    self.conversation_history.scroll_to_bottom();
                }
                self.request_redraw();
            }
            EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                call_id,
                exit_code,
                stdout,
                stderr,
            }) => {
                self.conversation_history
                    .record_completed_exec_command(call_id, stdout, stderr, exit_code);
                self.request_redraw();
            }
            EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
                call_id,
                server,
                tool,
                arguments,
            }) => {
                self.conversation_history
                    .add_active_mcp_tool_call(call_id, server, tool, arguments);
                self.request_redraw();
            }
            EventMsg::McpToolCallEnd(McpToolCallEndEvent {
                call_id,
                success,
                result,
            }) => {
                self.conversation_history
                    .record_completed_mcp_tool_call(call_id, success, result);
                self.request_redraw();
            }
            EventMsg::GetHistoryEntryResponse(event) => {
                let codex_core::protocol::GetHistoryEntryResponseEvent {
                    offset,
                    log_id,
                    entry,
                } = event;
                let entry_text = entry.map(|e| e.text);
                if self.search_state.is_active && Some(log_id) == self.search_state.history_log_id {
                    self.search_state.add_historical_entry(offset, entry_text.clone());
                    self.perform_search();
                    self.request_redraw();
                }
                self.bottom_pane
                    .on_history_entry_response(log_id, offset, entry_text);
            }
            event => {
                self.conversation_history
                    .add_background_event(format!("{event:?}"));
                self.request_redraw();
            }
        }
    }

    /// Update the live log preview while a task is running.
    pub(crate) fn update_latest_log(&mut self, line: String) {
        // Forward only if we are currently showing the status indicator.
        self.bottom_pane.update_status_text(line);
    }

    pub(crate) fn request_redraw(&mut self) {
        self.app_event_tx.send(AppEvent::Redraw);
    }

    pub(crate) fn handle_scroll_delta(&mut self, scroll_delta: i32) {
        // If the user is trying to scroll exactly one line, we let them, but
        // otherwise we assume they are trying to scroll in larger increments.
        let magnified_scroll_delta = if scroll_delta == 1 {
            1
        } else {
            // Play with this: perhaps it should be non-linear?
            scroll_delta * 2
        };
        self.conversation_history.scroll(magnified_scroll_delta);
        self.request_redraw();
    }

    /// Forward an `Op` directly to codex.
    pub(crate) fn submit_op(&self, op: Op) {
        if let Err(e) = self.codex_op_tx.send(op) {
            tracing::error!("failed to submit op: {e}");
        }
    }

    fn perform_search(&mut self) {
        self.search_state.matches.clear();
        self.search_state.current_match_index = 0;

        let search_str = if self.search_state.case_sensitive {
            self.search_state.search_string.clone()
        } else {
            self.search_state.search_string.to_lowercase()
        };

        if search_str.is_empty() {
            self.update_search_preview();
            return;
        }

        let mut seen_commands: HashSet<String> = HashSet::new();

        for (i, command) in self.current_session_commands.iter().enumerate() {
            let command_to_compare = if self.search_state.case_sensitive {
                command.to_string()
            } else {
                command.to_lowercase()
            };
            if command_to_compare.contains(&search_str) {
                let original_command = command.to_string();
                if seen_commands.insert(original_command) {
                    self.search_state.matches.push(MatchSource::CurrentSession(i));
                }
            }
        }

        for offset in (0..self.search_state.historical_entries.len()).rev() {
            let entry = &self.search_state.historical_entries[offset];
            let entry_to_compare = if self.search_state.case_sensitive {
                entry.to_string()
            } else {
                entry.to_lowercase()
            };
            if entry_to_compare.contains(&search_str) {
                let original_entry = entry.to_string();
                if seen_commands.insert(original_entry) {
                    self.search_state.matches.push(MatchSource::Historical(offset));
                }
            }
        }

        self.update_search_preview();
    }

    fn update_search_preview(&mut self) {
        if self.search_state.matches.is_empty() {
            if let Some(ref input) = self.search_state.original_input {
                self.bottom_pane.composer_mut().textarea_mut().select_all();
                self.bottom_pane.composer_mut().textarea_mut().cut();
                let _ = self.bottom_pane.composer_mut().textarea_mut().insert_str(input);
            }
        } else {
            let match_source = &self.search_state.matches[self.search_state.current_match_index];
            match match_source {
                MatchSource::CurrentSession(idx) => {
                    let command = &self.current_session_commands[*idx];
                    self.bottom_pane.composer_mut().textarea_mut().select_all();
                    self.bottom_pane.composer_mut().textarea_mut().cut();
                    let _ = self.bottom_pane.composer_mut().textarea_mut().insert_str(command);
                    // Scroll to the bottom since we don't have a direct line index in conversation history
                    self.conversation_history.scroll_to_bottom();
                }
                MatchSource::Historical(offset) => {
                    if let Some(line) = self.search_state.historical_entries.get(*offset) {
                        self.bottom_pane.composer_mut().textarea_mut().select_all();
                        self.bottom_pane.composer_mut().textarea_mut().cut();
                        let _ = self.bottom_pane.composer_mut().textarea_mut().insert_str(line);
                        self.conversation_history.scroll_to_bottom();
                    }
                }
            }
        }
    }

    fn reset_search_state(&mut self) {
        if !self.search_state.command_executed {
            if let Some(ref input) = self.search_state.original_input {
                self.bottom_pane.composer_mut().textarea_mut().select_all();
                self.bottom_pane.composer_mut().textarea_mut().cut();
                let _ = self.bottom_pane.composer_mut().textarea_mut().insert_str(input);
            }
        }
        self.search_state.reset();
        self.request_redraw();
    }
}

impl<'a> WidgetRef for &ChatWidget<'a> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let mut constraints = vec![Constraint::Min(0)];
        if self.search_state.is_active {
            constraints.push(Constraint::Length(1));
        }
        let bottom_pane_height = self.bottom_pane.calculate_required_height(&area);
        constraints.push(Constraint::Length(bottom_pane_height));

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        let mut idx = 0;
        self.conversation_history.render_ref(chunks[idx], buf);
        idx += 1;
        if self.search_state.is_active {
            let search_text = format!(
                "{}search)`{}'",
                if self.search_state.direction == SearchDirection::Backward { "(bck-i-" } else { "(fwd-i-" },
                self.search_state.search_string
            );
            let search_style = Style::default().fg(if self.search_state.matches.is_empty() { Color::Red } else { Color::White });
            let current = if !self.search_state.matches.is_empty() {
                self.search_state.current_match_index + 1
            } else {
                0
            };
            let total = self.search_state.matches.len();
            let search_span = {
                let matches_text = format!(" {}/{} matches", current, total);
                let nav_hints = format!("▲ to backward ▼ to forward ESC to quit");
                let total_width = chunks[idx].width as usize;
                let search_text_len = search_text.len() + matches_text.len();
                let padding_len = total_width.saturating_sub(search_text_len + nav_hints.len());
                let padding = " ".repeat(padding_len);
            
                Line::from(vec![
                    Span::styled(search_text, search_style),
                    Span::raw(matches_text),
                    Span::raw(padding),
                    Span::styled("▲", Style::default().fg(Color::Yellow)),
                    Span::raw(" to backward | "),
                    Span::styled("▼", Style::default().fg(Color::Yellow)),
                    Span::raw(" to forward | "),
                    Span::styled("ESC", Style::default().fg(Color::Yellow)),
                    Span::raw(" to quit"),
                ])
            };
            let search_paragraph = Paragraph::new(search_span)
                .style(Style::default().bg(if self.search_state.matches.is_empty() { Color::Black } else { Color::Black }));
            search_paragraph.render(chunks[idx], buf);
            idx += 1;
        }
        self.bottom_pane.render_ref(chunks[idx], buf);
    }
}

use std::path::PathBuf;
use std::sync::Arc;

use codex_core::codex_wrapper::CodexConversation;
use codex_core::codex_wrapper::init_codex;
use codex_core::config::Config;
use codex_core::protocol::AgentMessageDeltaEvent;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::AgentReasoningDeltaEvent;
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
use codex_core::protocol::TokenUsage;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
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
use codex_file_search::FileMatch;

pub(crate) struct ChatWidget<'a> {
    app_event_tx: AppEventSender,
    codex_op_tx: UnboundedSender<Op>,
    conversation_history: ConversationHistoryWidget,
    bottom_pane: BottomPane<'a>,
    config: Config,
    initial_user_message: Option<UserMessage>,
    token_usage: TokenUsage,
    reasoning_buffer: String,
    // Buffer for streaming assistant answer text; we do not surface partial
    // We wait for the final AgentMessage event and then emit the full text
    // at once into scrollback so the history contains a single message.
    answer_buffer: String,
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
            let CodexConversation {
                codex,
                session_configured,
                ..
            } = match init_codex(config_for_agent_loop).await {
                Ok(vals) => vals,
                Err(e) => {
                    // TODO: surface this error to the user.
                    tracing::error!("failed to initialize codex: {e}");
                    return;
                }
            };

            // Forward the captured `SessionInitialized` event that was consumed
            // inside `init_codex()` so it can be rendered in the UI.
            app_event_tx_clone.send(AppEvent::CodexEvent(session_configured.clone()));
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
            config,
            initial_user_message: create_initial_user_message(
                initial_prompt.unwrap_or_default(),
                initial_images,
            ),
            token_usage: TokenUsage::default(),
            reasoning_buffer: String::new(),
            answer_buffer: String::new(),
        }
    }

    pub(crate) fn handle_key_event(&mut self, key_event: KeyEvent) {
        self.bottom_pane.clear_ctrl_c_quit_hint();

        match self.bottom_pane.handle_key_event(key_event) {
            InputResult::Submitted(text) => {
                self.submit_user_message(text.into());
            }
            InputResult::None => {}
        }
    }

    pub(crate) fn handle_paste(&mut self, text: String) {
        self.bottom_pane.handle_paste(text);
    }

    /// Emits the last entry's plain lines from conversation_history, if any.
    fn emit_last_history_entry(&mut self) {
        if let Some(lines) = self.conversation_history.last_entry_plain_lines() {
            self.app_event_tx.send(AppEvent::InsertHistory(lines));
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
        }

        // Only show text portion in conversation history for now.
        if !text.is_empty() {
            self.conversation_history.add_user_message(text.clone());
            self.emit_last_history_entry();
        }
        self.conversation_history.scroll_to_bottom();
    }

    pub(crate) fn handle_codex_event(&mut self, event: Event) {
        let Event { id, msg } = event;
        match msg {
            EventMsg::SessionConfigured(event) => {
                // Record session information at the top of the conversation.
                self.conversation_history
                    .add_session_info(&self.config, event.clone());
                // Immediately surface the session banner / settings summary in
                // scrollback so the user can review configuration (model,
                // sandbox, approvals, etc.) before interacting.
                self.emit_last_history_entry();

                // Forward history metadata to the bottom pane so the chat
                // composer can navigate through past messages.
                self.bottom_pane
                    .set_history_metadata(event.history_log_id, event.history_entry_count);

                if let Some(user_message) = self.initial_user_message.take() {
                    // If the user provided an initial message, add it to the
                    // conversation history.
                    self.submit_user_message(user_message);
                }

                self.request_redraw();
            }
            EventMsg::AgentMessage(AgentMessageEvent { message }) => {
                // Final assistant answer. Prefer the fully provided message
                // from the event; if it is empty fall back to any accumulated
                // delta buffer (some providers may only stream deltas and send
                // an empty final message).
                let full = if message.is_empty() {
                    std::mem::take(&mut self.answer_buffer)
                } else {
                    self.answer_buffer.clear();
                    message
                };
                if !full.is_empty() {
                    self.conversation_history
                        .add_agent_message(&self.config, full);
                    self.emit_last_history_entry();
                }
                self.request_redraw();
            }
            EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta }) => {
                // Buffer only – do not emit partial lines. This avoids cases
                // where long responses appear truncated if the terminal
                // wrapped early. The full message is emitted on
                // AgentMessage.
                self.answer_buffer.push_str(&delta);
            }
            EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent { delta }) => {
                // Buffer only – disable incremental reasoning streaming so we
                // avoid truncated intermediate lines. Full text emitted on
                // AgentReasoning.
                self.reasoning_buffer.push_str(&delta);
            }
            EventMsg::AgentReasoning(AgentReasoningEvent { text }) => {
                // Emit full reasoning text once. Some providers might send
                // final event with empty text if only deltas were used.
                let full = if text.is_empty() {
                    std::mem::take(&mut self.reasoning_buffer)
                } else {
                    self.reasoning_buffer.clear();
                    text
                };
                if !full.is_empty() {
                    self.conversation_history
                        .add_agent_reasoning(&self.config, full);
                    self.emit_last_history_entry();
                }
                self.request_redraw();
            }
            EventMsg::TaskStarted => {
                self.bottom_pane.clear_ctrl_c_quit_hint();
                self.bottom_pane.set_task_running(true);
                self.request_redraw();
            }
            EventMsg::TaskComplete(TaskCompleteEvent {
                last_agent_message: _,
            }) => {
                self.bottom_pane.set_task_running(false);
                self.request_redraw();
            }
            EventMsg::TokenCount(token_usage) => {
                self.token_usage = add_token_usage(&self.token_usage, &token_usage);
                self.bottom_pane
                    .set_token_usage(self.token_usage.clone(), self.config.model_context_window);
            }
            EventMsg::Error(ErrorEvent { message }) => {
                self.conversation_history.add_error(message.clone());
                self.emit_last_history_entry();
                self.bottom_pane.set_task_running(false);
            }
            EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
                call_id: _,
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
                call_id: _,
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
                self.emit_last_history_entry();

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
                self.emit_last_history_entry();
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
                self.emit_last_history_entry();
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
                self.emit_last_history_entry();
                self.request_redraw();
            }
            EventMsg::McpToolCallEnd(mcp_tool_call_end_event) => {
                let success = mcp_tool_call_end_event.is_success();
                let McpToolCallEndEvent { call_id, result } = mcp_tool_call_end_event;
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

                // Inform bottom pane / composer.
                self.bottom_pane
                    .on_history_entry_response(log_id, offset, entry.map(|e| e.text));
            }
            EventMsg::ShutdownComplete => {
                self.app_event_tx.send(AppEvent::ExitRequest);
            }
            event => {
                self.conversation_history
                    .add_background_event(format!("{event:?}"));
                self.emit_last_history_entry();
                self.request_redraw();
            }
        }
    }

    /// Update the live log preview while a task is running.
    pub(crate) fn update_latest_log(&mut self, line: String) {
        // Forward only if we are currently showing the status indicator.
        self.bottom_pane.update_status_text(line);
    }

    fn request_redraw(&mut self) {
        self.app_event_tx.send(AppEvent::RequestRedraw);
    }

    pub(crate) fn add_diff_output(&mut self, diff_output: String) {
        self.conversation_history
            .add_diff_output(diff_output.clone());
        self.emit_last_history_entry();
        self.request_redraw();
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

    /// Forward file-search results to the bottom pane.
    pub(crate) fn apply_file_search_result(&mut self, query: String, matches: Vec<FileMatch>) {
        self.bottom_pane.on_file_search_result(query, matches);
    }

    /// Handle Ctrl-C key press.
    /// Returns true if the key press was handled, false if it was not.
    /// If the key press was not handled, the caller should handle it (likely by exiting the process).
    pub(crate) fn on_ctrl_c(&mut self) -> bool {
        if self.bottom_pane.is_task_running() {
            self.bottom_pane.clear_ctrl_c_quit_hint();
            self.submit_op(Op::Interrupt);
            self.answer_buffer.clear();
            self.reasoning_buffer.clear();
            false
        } else if self.bottom_pane.ctrl_c_quit_hint_visible() {
            self.submit_op(Op::Shutdown);
            true
        } else {
            self.bottom_pane.show_ctrl_c_quit_hint();
            false
        }
    }

    pub(crate) fn composer_is_empty(&self) -> bool {
        self.bottom_pane.composer_is_empty()
    }

    /// Forward an `Op` directly to codex.
    pub(crate) fn submit_op(&self, op: Op) {
        if let Err(e) = self.codex_op_tx.send(op) {
            tracing::error!("failed to submit op: {e}");
        }
    }

    pub(crate) fn token_usage(&self) -> &TokenUsage {
        &self.token_usage
    }
}

impl WidgetRef for &ChatWidget<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        // In the hybrid inline viewport mode we only draw the interactive
        // bottom pane; history entries are injected directly into scrollback
        // via `Terminal::insert_before`.
        (&self.bottom_pane).render(area, buf);
    }
}

fn add_token_usage(current_usage: &TokenUsage, new_usage: &TokenUsage) -> TokenUsage {
    let cached_input_tokens = match (
        current_usage.cached_input_tokens,
        new_usage.cached_input_tokens,
    ) {
        (Some(current), Some(new)) => Some(current + new),
        (Some(current), None) => Some(current),
        (None, Some(new)) => Some(new),
        (None, None) => None,
    };
    let reasoning_output_tokens = match (
        current_usage.reasoning_output_tokens,
        new_usage.reasoning_output_tokens,
    ) {
        (Some(current), Some(new)) => Some(current + new),
        (Some(current), None) => Some(current),
        (None, Some(new)) => Some(new),
        (None, None) => None,
    };
    TokenUsage {
        input_tokens: current_usage.input_tokens + new_usage.input_tokens,
        cached_input_tokens,
        output_tokens: current_usage.output_tokens + new_usage.output_tokens,
        reasoning_output_tokens,
        total_tokens: current_usage.total_tokens + new_usage.total_tokens,
    }
}

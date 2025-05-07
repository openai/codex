use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::SendError;
use std::sync::mpsc::Sender;

use codex_core::codex_wrapper::init_codex;
use codex_core::config::Config;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::unbounded_channel;

use crate::app_event::AppEvent;
use crate::bottom_pane::BottomPane;
use crate::bottom_pane::BottomPaneParams;
use crate::bottom_pane::InputResult;
use crate::conversation_history_widget::ConversationHistoryWidget;
use crate::history_cell::PatchEventType;
use crate::user_approval_widget::ApprovalRequest;

pub(crate) struct ChatWidget<'a> {
    app_event_tx: Sender<AppEvent>,
    codex_op_tx: UnboundedSender<Op>,
    conversation_history: ConversationHistoryWidget,
    bottom_pane: BottomPane<'a>,
    input_focus: InputFocus,
    config: Config,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum InputFocus {
    HistoryPane,
    BottomPane,
}

impl ChatWidget<'_> {
    pub(crate) fn new(
        config: Config,
        app_event_tx: Sender<AppEvent>,
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
            if let Err(e) = app_event_tx_clone.send(AppEvent::CodexEvent(session_event.clone())) {
                tracing::error!("failed to send SessionInitialized event: {e}");
            }
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
                app_event_tx_clone
                    .send(AppEvent::CodexEvent(event))
                    .unwrap_or_else(|e| {
                        tracing::error!("failed to send event: {e}");
                    });
            }
        });

        let mut chat_widget = Self {
            app_event_tx: app_event_tx.clone(),
            codex_op_tx,
            conversation_history: ConversationHistoryWidget::new(),
            bottom_pane: BottomPane::new(BottomPaneParams {
                app_event_tx,
                has_input_focus: true,
            }),
            input_focus: InputFocus::BottomPane,
            config,
        };

        let _ = chat_widget.submit_welcome_message();

        if initial_prompt.is_some() || !initial_images.is_empty() {
            let text = initial_prompt.unwrap_or_default();
            let _ = chat_widget.submit_user_message_with_images(text, initial_images);
        }

        chat_widget
    }

    pub(crate) fn handle_key_event(
        &mut self,
        key_event: KeyEvent,
    ) -> std::result::Result<(), SendError<AppEvent>> {
        // Special-case <tab>: does not get dispatched to child components.
        if matches!(key_event.code, crossterm::event::KeyCode::Tab) {
            self.input_focus = match self.input_focus {
                InputFocus::HistoryPane => InputFocus::BottomPane,
                InputFocus::BottomPane => InputFocus::HistoryPane,
            };
            self.conversation_history
                .set_input_focus(self.input_focus == InputFocus::HistoryPane);
            self.bottom_pane
                .set_input_focus(self.input_focus == InputFocus::BottomPane);
            self.request_redraw()?;
            return Ok(());
        }

        match self.input_focus {
            InputFocus::HistoryPane => {
                let needs_redraw = self.conversation_history.handle_key_event(key_event);
                if needs_redraw {
                    self.request_redraw()?;
                }
                Ok(())
            }
            InputFocus::BottomPane => {
                match self.bottom_pane.handle_key_event(key_event)? {
                    InputResult::Submitted(text) => {
                        // Special client‑side commands start with a leading slash.
                        let trimmed = text.trim();
                        match trimmed {
                            "/clear" => {
                                // Clear the current conversation history without exiting.
                                self.conversation_history.clear();
                                self.request_redraw()?;
                            }
                            _ => {
                                self.submit_user_message(text)?;
                            }
                        }
                    }
                    InputResult::None => {}
                }
                Ok(())
            }
        }
    }

    fn submit_welcome_message(&mut self) -> std::result::Result<(), SendError<AppEvent>> {
        self.handle_codex_event(Event {
            id: "welcome".to_string(),
            msg: EventMsg::AgentMessage {
                message: "Welcome to codex!".to_string(),
            },
        })?;
        Ok(())
    }

    fn submit_user_message(
        &mut self,
        text: String,
    ) -> std::result::Result<(), SendError<AppEvent>> {
        // Forward to codex and update conversation history.
        self.submit_user_message_with_images(text, vec![])
    }

    fn submit_user_message_with_images(
        &mut self,
        text: String,
        image_paths: Vec<PathBuf>,
    ) -> std::result::Result<(), SendError<AppEvent>> {
        let mut items: Vec<InputItem> = Vec::new();

        if !text.is_empty() {
            items.push(InputItem::Text { text: text.clone() });
        }

        for path in image_paths {
            items.push(InputItem::LocalImage { path });
        }

        if items.is_empty() {
            return Ok(());
        }

        self.codex_op_tx
            .send(Op::UserInput { items })
            .unwrap_or_else(|e| {
                tracing::error!("failed to send message: {e}");
            });

        // Only show text portion in conversation history for now.
        if !text.is_empty() {
            self.conversation_history.add_user_message(text);
        }
        self.conversation_history.scroll_to_bottom();

        Ok(())
    }

    pub(crate) fn handle_codex_event(
        &mut self,
        event: Event,
    ) -> std::result::Result<(), SendError<AppEvent>> {
        let Event { id, msg } = event;
        match msg {
            EventMsg::SessionConfigured { model } => {
                // Record session information at the top of the conversation.
                self.conversation_history
                    .add_session_info(&self.config, model);
                self.request_redraw()?;
            }
            EventMsg::AgentMessage { message } => {
                self.conversation_history.add_agent_message(message);
                self.request_redraw()?;
            }
            EventMsg::TaskStarted => {
                self.bottom_pane.set_task_running(true)?;
                self.conversation_history
                    .add_background_event(format!("task {id} started"));
                self.request_redraw()?;
            }
            EventMsg::TaskComplete => {
                self.bottom_pane.set_task_running(false)?;
                self.request_redraw()?;
            }
            EventMsg::Error { message } => {
                self.conversation_history
                    .add_background_event(format!("Error: {message}"));
                self.bottom_pane.set_task_running(false)?;
            }
            EventMsg::ExecApprovalRequest {
                command,
                cwd,
                reason,
            } => {
                let request = ApprovalRequest::Exec {
                    id,
                    command,
                    cwd,
                    reason,
                };
                let needs_redraw = self.bottom_pane.push_approval_request(request);
                if needs_redraw {
                    self.request_redraw()?;
                }
            }
            EventMsg::ApplyPatchApprovalRequest {
                changes,
                reason,
                grant_root,
            } => {
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
                let _needs_redraw = self.bottom_pane.push_approval_request(request);
                // Redraw is always need because the history has changed.
                self.request_redraw()?;
            }
            EventMsg::ExecCommandBegin {
                call_id, command, ..
            } => {
                self.conversation_history
                    .add_active_exec_command(call_id, command);
                self.request_redraw()?;
            }
            EventMsg::PatchApplyBegin {
                call_id: _,
                auto_approved,
                changes,
            } => {
                // Even when a patch is auto‑approved we still display the
                // summary so the user can follow along.
                self.conversation_history
                    .add_patch_event(PatchEventType::ApplyBegin { auto_approved }, changes);
                if !auto_approved {
                    self.conversation_history.scroll_to_bottom();
                }
                self.request_redraw()?;
            }
            EventMsg::ExecCommandEnd {
                call_id,
                exit_code,
                stdout,
                stderr,
                ..
            } => {
                self.conversation_history
                    .record_completed_exec_command(call_id, stdout, stderr, exit_code);
                self.request_redraw()?;
            }
            EventMsg::McpToolCallBegin {
                call_id,
                server,
                tool,
                arguments,
            } => {
                self.conversation_history
                    .add_active_mcp_tool_call(call_id, server, tool, arguments);
                self.request_redraw()?;
            }
            EventMsg::McpToolCallEnd {
                call_id,
                success,
                result,
            } => {
                self.conversation_history
                    .record_completed_mcp_tool_call(call_id, success, result);
                self.request_redraw()?;
            }
            event => {
                self.conversation_history
                    .add_background_event(format!("{event:?}"));
                self.request_redraw()?;
            }
        }
        Ok(())
    }

    /// Update the live log preview while a task is running.
    pub(crate) fn update_latest_log(
        &mut self,
        line: String,
    ) -> std::result::Result<(), std::sync::mpsc::SendError<AppEvent>> {
        // Forward only if we are currently showing the status indicator.
        self.bottom_pane.update_status_text(line)?;
        Ok(())
    }

    fn request_redraw(&mut self) -> std::result::Result<(), SendError<AppEvent>> {
        self.app_event_tx.send(AppEvent::Redraw)?;
        Ok(())
    }

    pub(crate) fn handle_scroll_delta(
        &mut self,
        scroll_delta: i32,
    ) -> std::result::Result<(), std::sync::mpsc::SendError<AppEvent>> {
        // If the user is trying to scroll exactly one line, we let them, but
        // otherwise we assume they are trying to scroll in larger increments.
        let magnified_scroll_delta = if scroll_delta == 1 {
            1
        } else {
            // Play with this: perhaps it should be non-linear?
            scroll_delta * 2
        };
        self.conversation_history.scroll(magnified_scroll_delta);
        self.request_redraw()?;
        Ok(())
    }

    /// Forward an `Op` directly to codex.
    pub(crate) fn submit_op(&self, op: Op) {
        if let Err(e) = self.codex_op_tx.send(op) {
            tracing::error!("failed to submit op: {e}");
        }
    }
}

impl WidgetRef for &ChatWidget<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let bottom_height = self.bottom_pane.required_height(&area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(bottom_height)])
            .split(area);

        self.conversation_history.render(chunks[0], buf);
        (&self.bottom_pane).render(chunks[1], buf);
    }
}

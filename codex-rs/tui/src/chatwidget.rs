use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use codex_core::codex_wrapper::CodexConversation;
use codex_core::codex_wrapper::init_codex;
use codex_core::config::Config;
use codex_core::protocol::AgentMessageDeltaEvent;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::AgentReasoningDeltaEvent;
use codex_core::protocol::AgentReasoningEvent;
use codex_core::protocol::AgentReasoningRawContentDeltaEvent;
use codex_core::protocol::AgentReasoningRawContentEvent;
use codex_core::protocol::ApplyPatchApprovalRequestEvent;
use codex_core::protocol::BackgroundEventEvent;
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
use codex_core::protocol::TurnDiffEvent;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::unbounded_channel;
use tracing::info;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPane;
use crate::bottom_pane::BottomPaneParams;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::InputResult;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::history_cell::CommandOutput;
use crate::history_cell::HistoryCell;
use crate::history_cell::PatchEventType;
use crate::markdown_stream::MarkdownNewlineCollector;
use crate::markdown_stream::RenderedLineStreamer;
use crate::user_approval_widget::ApprovalRequest;
use codex_file_search::FileMatch;

struct RunningCommand {
    command: Vec<String>,
    #[allow(dead_code)]
    cwd: PathBuf,
}

pub(crate) struct ChatWidget<'a> {
    app_event_tx: AppEventSender,
    codex_op_tx: UnboundedSender<Op>,
    bottom_pane: BottomPane<'a>,
    active_history_cell: Option<HistoryCell>,
    config: Config,
    initial_user_message: Option<UserMessage>,
    total_token_usage: TokenUsage,
    last_token_usage: TokenUsage,
    // Newline-gated markdown streaming state
    reasoning_collector: MarkdownNewlineCollector,
    answer_collector: MarkdownNewlineCollector,
    reasoning_streamer: RenderedLineStreamer,
    answer_streamer: RenderedLineStreamer,
    running_commands: HashMap<String, RunningCommand>,
    current_stream: Option<StreamKind>,
    // Track header emission per stream kind to avoid cross-stream duplication
    answer_header_emitted: bool,
    reasoning_header_emitted: bool,
    live_max_rows: u16,
    task_complete_pending: bool,
    finishing_after_drain: bool,
    // Queue of interruptive UI events deferred during an active write cycle
    interrupt_queue: VecDeque<QueuedInterrupt>,
}

struct UserMessage {
    text: String,
    image_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamKind {
    Answer,
    Reasoning,
}

#[derive(Debug)]
enum QueuedInterrupt {
    ExecApproval(String, ExecApprovalRequestEvent),
    ApplyPatchApproval(String, ApplyPatchApprovalRequestEvent),
    ExecBegin(ExecCommandBeginEvent),
    McpBegin(McpToolCallBeginEvent),
    McpEnd(McpToolCallEndEvent),
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
    fn header_line(kind: StreamKind) -> ratatui::text::Line<'static> {
        use ratatui::style::Stylize;
        match kind {
            StreamKind::Reasoning => ratatui::text::Line::from("thinking".magenta().italic()),
            StreamKind::Answer => ratatui::text::Line::from("codex".magenta().bold()),
        }
    }
    fn line_is_blank(line: &ratatui::text::Line<'_>) -> bool {
        if line.spans.is_empty() {
            return true;
        }
        line.spans.iter().all(|s| s.content.trim().is_empty())
    }
    /// Periodic tick to commit at most one queued line to history with a small delay,
    /// animating the output.
    pub(crate) fn on_commit_tick(&mut self) {
        // Choose the active streamer
        let (streamer, kind_opt) = match self.current_stream {
            Some(StreamKind::Reasoning) => {
                (&mut self.reasoning_streamer, Some(StreamKind::Reasoning))
            }
            Some(StreamKind::Answer) => (&mut self.answer_streamer, Some(StreamKind::Answer)),
            None => {
                // No active stream. Nothing to animate.
                return;
            }
        };

        // Prepare header if needed
        let mut lines: Vec<ratatui::text::Line<'static>> = Vec::new();
        if let Some(k) = kind_opt {
            let header_needed = match k {
                StreamKind::Reasoning => !self.reasoning_header_emitted,
                StreamKind::Answer => !self.answer_header_emitted,
            };
            if header_needed {
                lines.push(Self::header_line(k));
                match k {
                    StreamKind::Reasoning => self.reasoning_header_emitted = true,
                    StreamKind::Answer => self.answer_header_emitted = true,
                }
            }
        }

        let step = streamer.step(self.live_max_rows as usize);
        if !step.history.is_empty() || !lines.is_empty() {
            lines.extend(step.history);
            self.app_event_tx.send(AppEvent::InsertHistory(lines));
        }

        // If streamer is now idle and there is no more active stream data, finalize state.
        let is_idle = streamer.is_idle();
        if is_idle {
            // Stop animation ticks between bursts.
            self.app_event_tx.send(AppEvent::StopCommitAnimation);
            if self.finishing_after_drain {
                // Final cleanup once fully drained at end-of-stream.
                self.current_stream = None;
                self.finishing_after_drain = false;
                if self.task_complete_pending {
                    self.bottom_pane.set_task_running(false);
                    self.task_complete_pending = false;
                }
                // After the write cycle completes, release any queued interrupts.
                self.flush_interrupt_queue();
            }
        }
    }
    fn is_write_cycle_active(&self) -> bool {
        self.current_stream.is_some()
    }

    fn flush_interrupt_queue(&mut self) {
        while let Some(q) = self.interrupt_queue.pop_front() {
            match q {
                QueuedInterrupt::ExecApproval(id, ev) => self.handle_exec_approval_now(id, ev),
                QueuedInterrupt::ApplyPatchApproval(id, ev) => {
                    self.handle_apply_patch_approval_now(id, ev)
                }
                QueuedInterrupt::ExecBegin(ev) => self.handle_exec_begin_now(ev),
                QueuedInterrupt::McpBegin(ev) => self.handle_mcp_begin_now(ev),
                QueuedInterrupt::McpEnd(ev) => self.handle_mcp_end_now(ev),
            }
        }
    }

    fn handle_exec_approval_now(&mut self, id: String, ev: ExecApprovalRequestEvent) {
        // Log a background summary immediately so the history is chronological.
        let cmdline = strip_bash_lc_and_escape(&ev.command);
        let text = format!(
            "command requires approval:\n$ {cmdline}{reason}",
            reason = ev
                .reason
                .as_ref()
                .map(|r| format!("\n{r}"))
                .unwrap_or_default()
        );
        self.add_to_history(HistoryCell::new_background_event(text));

        let request = ApprovalRequest::Exec {
            id,
            command: ev.command,
            cwd: ev.cwd,
            reason: ev.reason,
        };
        self.bottom_pane.push_approval_request(request);
        self.request_redraw();
    }

    fn handle_apply_patch_approval_now(&mut self, id: String, ev: ApplyPatchApprovalRequestEvent) {
        self.add_to_history(HistoryCell::new_patch_event(
            PatchEventType::ApprovalRequest,
            ev.changes.clone(),
        ));

        let request = ApprovalRequest::ApplyPatch {
            id,
            reason: ev.reason,
            grant_root: ev.grant_root,
        };
        self.bottom_pane.push_approval_request(request);
        self.request_redraw();
    }

    fn handle_exec_begin_now(&mut self, ev: ExecCommandBeginEvent) {
        // Ensure the status indicator is visible while the command runs.
        self.bottom_pane
            .update_status_text("running command".to_string());
        self.running_commands.insert(
            ev.call_id.clone(),
            RunningCommand {
                command: ev.command.clone(),
                cwd: ev.cwd.clone(),
            },
        );
        self.active_history_cell = Some(HistoryCell::new_active_exec_command(ev.command));
    }

    fn handle_mcp_begin_now(&mut self, ev: McpToolCallBeginEvent) {
        self.add_to_history(HistoryCell::new_active_mcp_tool_call(ev.invocation));
    }

    fn handle_mcp_end_now(&mut self, ev: McpToolCallEndEvent) {
        self.add_to_history(HistoryCell::new_completed_mcp_tool_call(
            80,
            ev.invocation,
            ev.duration,
            ev.result
                .as_ref()
                .map(|r| r.is_error.unwrap_or(false))
                .unwrap_or(false),
            ev.result,
        ));
    }
    fn interrupt_running_task(&mut self) {
        if self.bottom_pane.is_task_running() {
            self.active_history_cell = None;
            self.bottom_pane.clear_ctrl_c_quit_hint();
            self.submit_op(Op::Interrupt);
            self.bottom_pane.set_task_running(false);
            self.reasoning_collector.clear();
            self.answer_collector.clear();
            self.reasoning_streamer.clear();
            self.answer_streamer.clear();
            self.current_stream = None;
            self.answer_header_emitted = false;
            self.reasoning_header_emitted = false;
            self.request_redraw();
        }
    }
    fn layout_areas(&self, area: Rect) -> [Rect; 2] {
        Layout::vertical([
            Constraint::Max(
                self.active_history_cell
                    .as_ref()
                    .map_or(0, |c| c.desired_height(area.width)),
            ),
            Constraint::Min(self.bottom_pane.desired_height(area.width)),
        ])
        .areas(area)
    }

    pub(crate) fn new(
        config: Config,
        app_event_tx: AppEventSender,
        initial_prompt: Option<String>,
        initial_images: Vec<PathBuf>,
        enhanced_keys_supported: bool,
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
            bottom_pane: BottomPane::new(BottomPaneParams {
                app_event_tx,
                has_input_focus: true,
                enhanced_keys_supported,
            }),
            active_history_cell: None,
            config,
            initial_user_message: create_initial_user_message(
                initial_prompt.unwrap_or_default(),
                initial_images,
            ),
            total_token_usage: TokenUsage::default(),
            last_token_usage: TokenUsage::default(),
            reasoning_collector: MarkdownNewlineCollector::new(),
            answer_collector: MarkdownNewlineCollector::new(),
            reasoning_streamer: RenderedLineStreamer::new(),
            answer_streamer: RenderedLineStreamer::new(),
            running_commands: HashMap::new(),
            current_stream: None,
            answer_header_emitted: false,
            reasoning_header_emitted: false,
            live_max_rows: 3,
            task_complete_pending: false,
            finishing_after_drain: false,
            interrupt_queue: VecDeque::new(),
        }
    }

    pub fn desired_height(&self, width: u16) -> u16 {
        self.bottom_pane.desired_height(width)
            + self
                .active_history_cell
                .as_ref()
                .map_or(0, |c| c.desired_height(width))
    }

    pub(crate) fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Press {
            self.bottom_pane.clear_ctrl_c_quit_hint();
        }

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

    fn add_to_history(&mut self, cell: HistoryCell) {
        self.app_event_tx
            .send(AppEvent::InsertHistory(cell.plain_lines()));
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
            self.add_to_history(HistoryCell::new_user_prompt(text.clone()));
        }
    }

    pub(crate) fn handle_codex_event(&mut self, event: Event) {
        let Event { id, msg } = event;
        match msg {
            EventMsg::SessionConfigured(event) => {
                self.bottom_pane
                    .set_history_metadata(event.history_log_id, event.history_entry_count);
                // Record session information at the top of the conversation.
                self.add_to_history(HistoryCell::new_session_info(&self.config, event, true));

                if let Some(user_message) = self.initial_user_message.take() {
                    // If the user provided an initial message, add it to the
                    // conversation history.
                    self.submit_user_message(user_message);
                }

                self.request_redraw();
            }
            EventMsg::AgentMessage(AgentMessageEvent { message: _ }) => {
                // Final assistant answer: commit all remaining rows and close with
                // a blank line. Use the final text if provided, otherwise rely on
                // streamed deltas already in the builder.
                self.finalize_stream(StreamKind::Answer);
                self.request_redraw();
            }
            EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta }) => {
                self.begin_stream(StreamKind::Answer);
                self.stream_push_and_maybe_commit(&delta);
                self.request_redraw();
            }
            EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent { delta }) => {
                // Stream CoT into the live pane; keep input visible and commit
                // overflow rows incrementally to scrollback.
                self.begin_stream(StreamKind::Reasoning);
                self.stream_push_and_maybe_commit(&delta);
                self.request_redraw();
            }
            EventMsg::AgentReasoning(AgentReasoningEvent { text: _ }) => {
                // Final reasoning: commit remaining rows and close with a blank.
                self.finalize_stream(StreamKind::Reasoning);
                self.request_redraw();
            }
            EventMsg::AgentReasoningRawContentDelta(AgentReasoningRawContentDeltaEvent {
                delta,
            }) => {
                // Treat raw reasoning content the same as summarized reasoning for UI flow.
                self.begin_stream(StreamKind::Reasoning);
                self.stream_push_and_maybe_commit(&delta);
                self.request_redraw();
            }
            EventMsg::AgentReasoningRawContent(AgentReasoningRawContentEvent { text: _ }) => {
                // Finalize the raw reasoning stream just like the summarized reasoning event.
                self.finalize_stream(StreamKind::Reasoning);
                self.request_redraw();
            }
            EventMsg::TaskStarted => {
                self.bottom_pane.clear_ctrl_c_quit_hint();
                self.bottom_pane.set_task_running(true);
                // Replace composer with single-line spinner while waiting.
                self.bottom_pane
                    .update_status_text("waiting for model".to_string());
                self.request_redraw();
            }
            EventMsg::TaskComplete(TaskCompleteEvent {
                last_agent_message: _,
            }) => {
                // Defer clearing status/live ring until streaming fully completes.
                let streaming_active = match self.current_stream {
                    Some(StreamKind::Reasoning) => !self.reasoning_streamer.is_idle(),
                    Some(StreamKind::Answer) => !self.answer_streamer.is_idle(),
                    None => false,
                };
                if streaming_active {
                    self.task_complete_pending = true;
                } else {
                    self.bottom_pane.set_task_running(false);
                    self.request_redraw();
                }
            }
            EventMsg::TokenCount(token_usage) => {
                self.total_token_usage = add_token_usage(&self.total_token_usage, &token_usage);
                self.last_token_usage = token_usage;
                self.bottom_pane.set_token_usage(
                    self.total_token_usage.clone(),
                    self.last_token_usage.clone(),
                    self.config.model_context_window,
                );
            }
            EventMsg::Error(ErrorEvent { message }) => {
                self.add_to_history(HistoryCell::new_error_event(message.clone()));
                self.bottom_pane.set_task_running(false);
                self.reasoning_collector.clear();
                self.answer_collector.clear();
                self.reasoning_streamer.clear();
                self.answer_streamer.clear();
                self.current_stream = None;
                self.answer_header_emitted = false;
                self.reasoning_header_emitted = false;
                self.request_redraw();
            }
            EventMsg::PlanUpdate(update) => {
                // Commit plan updates directly to history (no status-line preview).
                self.add_to_history(HistoryCell::new_plan_update(update));
            }
            EventMsg::ExecApprovalRequest(ev) => {
                if self.is_write_cycle_active() {
                    self.interrupt_queue
                        .push_back(QueuedInterrupt::ExecApproval(id, ev));
                } else {
                    self.handle_exec_approval_now(id, ev);
                }
            }
            EventMsg::ApplyPatchApprovalRequest(ev) => {
                if self.is_write_cycle_active() {
                    self.interrupt_queue
                        .push_back(QueuedInterrupt::ApplyPatchApproval(id, ev));
                } else {
                    self.handle_apply_patch_approval_now(id, ev);
                }
            }
            EventMsg::ExecCommandBegin(ev) => {
                if self.is_write_cycle_active() {
                    self.interrupt_queue
                        .push_back(QueuedInterrupt::ExecBegin(ev));
                } else {
                    self.handle_exec_begin_now(ev);
                }
            }
            EventMsg::ExecCommandOutputDelta(_) => {
                // TODO
            }
            EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
                call_id: _,
                auto_approved,
                changes,
            }) => {
                self.add_to_history(HistoryCell::new_patch_event(
                    PatchEventType::ApplyBegin { auto_approved },
                    changes,
                ));
            }
            EventMsg::PatchApplyEnd(event) => {
                if !event.success {
                    self.add_to_history(HistoryCell::new_patch_apply_failure(event.stderr));
                }
            }
            EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                call_id,
                exit_code,
                duration: _,
                stdout,
                stderr,
            }) => {
                // Compute summary before moving stdout into the history cell.
                let cmd = self.running_commands.remove(&call_id);
                self.active_history_cell = None;
                self.add_to_history(HistoryCell::new_completed_exec_command(
                    cmd.map(|cmd| cmd.command).unwrap_or_else(|| vec![call_id]),
                    CommandOutput {
                        exit_code,
                        stdout,
                        stderr,
                    },
                ));
            }
            EventMsg::McpToolCallBegin(ev) => {
                if self.is_write_cycle_active() {
                    self.interrupt_queue
                        .push_back(QueuedInterrupt::McpBegin(ev));
                } else {
                    self.handle_mcp_begin_now(ev);
                }
            }
            EventMsg::McpToolCallEnd(ev) => {
                if self.is_write_cycle_active() {
                    self.interrupt_queue.push_back(QueuedInterrupt::McpEnd(ev));
                } else {
                    self.handle_mcp_end_now(ev);
                }
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
            EventMsg::TurnDiff(TurnDiffEvent { unified_diff }) => {
                info!("TurnDiffEvent: {unified_diff}");
            }
            EventMsg::BackgroundEvent(BackgroundEventEvent { message }) => {
                info!("BackgroundEvent: {message}");
            }
        }
    }

    /// Update the live log preview while a task is running.
    pub(crate) fn update_latest_log(&mut self, line: String) {
        if self.bottom_pane.is_task_running() {
            self.bottom_pane.update_status_text(line);
        }
    }

    fn request_redraw(&mut self) {
        self.app_event_tx.send(AppEvent::RequestRedraw);
    }

    pub(crate) fn add_diff_output(&mut self, diff_output: String) {
        self.add_to_history(HistoryCell::new_diff_output(diff_output.clone()));
    }

    pub(crate) fn add_status_output(&mut self) {
        self.add_to_history(HistoryCell::new_status_output(
            &self.config,
            &self.total_token_usage,
        ));
    }

    pub(crate) fn add_prompts_output(&mut self) {
        self.add_to_history(HistoryCell::new_prompts_output());
    }

    /// Forward file-search results to the bottom pane.
    pub(crate) fn apply_file_search_result(&mut self, query: String, matches: Vec<FileMatch>) {
        self.bottom_pane.on_file_search_result(query, matches);
    }

    /// Handle Ctrl-C key press.
    /// Returns CancellationEvent::Handled if the event was consumed by the UI, or
    /// CancellationEvent::Ignored if the caller should handle it (e.g. exit).
    pub(crate) fn on_ctrl_c(&mut self) -> CancellationEvent {
        match self.bottom_pane.on_ctrl_c() {
            CancellationEvent::Handled => return CancellationEvent::Handled,
            CancellationEvent::Ignored => {}
        }
        if self.bottom_pane.is_task_running() {
            self.interrupt_running_task();
            CancellationEvent::Ignored
        } else if self.bottom_pane.ctrl_c_quit_hint_visible() {
            self.submit_op(Op::Shutdown);
            CancellationEvent::Handled
        } else {
            self.bottom_pane.show_ctrl_c_quit_hint();
            CancellationEvent::Ignored
        }
    }

    pub(crate) fn on_ctrl_z(&mut self) {
        self.interrupt_running_task();
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

    /// Programmatically submit a user text message as if typed in the
    /// composer. The text will be added to conversation history and sent to
    /// the agent.
    pub(crate) fn submit_text_message(&mut self, text: String) {
        if text.is_empty() {
            return;
        }
        self.submit_user_message(text.into());
    }

    pub(crate) fn token_usage(&self) -> &TokenUsage {
        &self.total_token_usage
    }

    pub(crate) fn clear_token_usage(&mut self) {
        self.total_token_usage = TokenUsage::default();
        self.bottom_pane.set_token_usage(
            self.total_token_usage.clone(),
            self.last_token_usage.clone(),
            self.config.model_context_window,
        );
    }

    pub fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let [_, bottom_pane_area] = self.layout_areas(area);
        self.bottom_pane.cursor_pos(bottom_pane_area)
    }
}

#[cfg(test)]
impl ChatWidget<'_> {
    /// Test-only control to tune the maximum rows shown in the live overlay.
    /// Useful for verifying queue-head behavior without changing production defaults.
    pub fn test_set_live_max_rows(&mut self, n: u16) {
        self.live_max_rows = n;
    }
}

impl ChatWidget<'_> {
    fn begin_stream(&mut self, kind: StreamKind) {
        if let Some(current) = self.current_stream {
            if current != kind {
                // Synchronously flush the previous stream to keep ordering sane.
                let (collector, streamer) = match current {
                    StreamKind::Reasoning => {
                        (&mut self.reasoning_collector, &mut self.reasoning_streamer)
                    }
                    StreamKind::Answer => (&mut self.answer_collector, &mut self.answer_streamer),
                };
                let remaining = collector.finalize_and_drain(&self.config);
                if !remaining.is_empty() {
                    streamer.enqueue(remaining);
                }
                let step = streamer.drain_all(self.live_max_rows as usize);
                let prev_header_emitted = match current {
                    StreamKind::Reasoning => self.reasoning_header_emitted,
                    StreamKind::Answer => self.answer_header_emitted,
                };
                if !step.history.is_empty() || !prev_header_emitted {
                    let mut lines: Vec<ratatui::text::Line<'static>> = Vec::new();
                    if !prev_header_emitted {
                        lines.push(Self::header_line(current));
                        match current {
                            StreamKind::Reasoning => self.reasoning_header_emitted = true,
                            StreamKind::Answer => self.answer_header_emitted = true,
                        }
                    }
                    lines.extend(step.history);
                    // Ensure at most one blank separator after the flushed block.
                    if let Some(last) = lines.last() {
                        if !Self::line_is_blank(last) {
                            lines.push(ratatui::text::Line::from(""));
                        }
                    } else {
                        lines.push(ratatui::text::Line::from(""));
                    }
                    self.app_event_tx.send(AppEvent::InsertHistory(lines));
                }
                // Reset for new stream
                self.current_stream = None;
            }
        }

        if self.current_stream != Some(kind) {
            // Only reset the header flag when switching FROM a different stream kind.
            // If current_stream is None (e.g., transient idle), preserve header flags
            // to avoid duplicate headers on re-entry into the same stream.
            let prev = self.current_stream;
            self.current_stream = Some(kind);
            if prev.is_some() {
                match kind {
                    StreamKind::Reasoning => self.reasoning_header_emitted = false,
                    StreamKind::Answer => self.answer_header_emitted = false,
                }
            }
            // Ensure the waiting status is visible (composer replaced).
            self.bottom_pane
                .update_status_text("waiting for model".to_string());
            // No live ring overlay; headers will be inserted with the first commit.
        }
    }

    fn stream_push_and_maybe_commit(&mut self, delta: &str) {
        // Newline-gated: only consider committing when a newline is present.
        let (collector, streamer) = match self.current_stream {
            Some(StreamKind::Reasoning) => {
                (&mut self.reasoning_collector, &mut self.reasoning_streamer)
            }
            Some(StreamKind::Answer) => (&mut self.answer_collector, &mut self.answer_streamer),
            None => return,
        };

        collector.push_delta(delta);
        if delta.contains('\n') {
            let newly_completed = collector.commit_complete_lines(&self.config);
            if !newly_completed.is_empty() {
                streamer.enqueue(newly_completed);
                // Start or continue commit animation.
                self.app_event_tx.send(AppEvent::StartCommitAnimation);
            }
        }
    }

    fn finalize_stream(&mut self, kind: StreamKind) {
        if self.current_stream != Some(kind) {
            // Nothing to do; either already finalized or not the active stream.
            return;
        }
        let (collector, streamer) = match kind {
            StreamKind::Reasoning => (&mut self.reasoning_collector, &mut self.reasoning_streamer),
            StreamKind::Answer => (&mut self.answer_collector, &mut self.answer_streamer),
        };

        let remaining = collector.finalize_and_drain(&self.config);
        if !remaining.is_empty() {
            streamer.enqueue(remaining);
        }
        // Trailing blank spacer
        streamer.enqueue(vec![ratatui::text::Line::from("")]);
        // Mark that we should clear state after draining.
        self.finishing_after_drain = true;
        // Start animation to drain remaining lines. Final cleanup will occur when drained.
        self.app_event_tx.send(AppEvent::StartCommitAnimation);
    }
}

impl WidgetRef for &ChatWidget<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let [active_cell_area, bottom_pane_area] = self.layout_areas(area);
        (&self.bottom_pane).render(bottom_pane_area, buf);
        if let Some(cell) = &self.active_history_cell {
            cell.render_ref(active_cell_area, buf);
        }
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

#[cfg(test)]
mod chatwidget_helper_tests {
    use super::*;
    use crate::app_event::AppEvent;
    use crate::app_event_sender::AppEventSender;
    use codex_core::config::ConfigOverrides;
    use std::sync::mpsc::channel;

    fn test_config() -> Config {
        let overrides = ConfigOverrides {
            cwd: std::env::current_dir().ok(),
            ..Default::default()
        };
        match Config::load_with_cli_overrides(vec![], overrides) {
            Ok(c) => c,
            Err(e) => panic!("load test config: {e}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn helpers_are_available_and_do_not_panic() {
        let (tx_raw, _rx) = channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let cfg = test_config();
        let mut w = ChatWidget::new(cfg, tx, None, Vec::new(), false);

        // Adjust the live ring capacity (no-op for rendering) and ensure no panic.
        w.test_set_live_max_rows(4);
    }
}

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

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
use codex_protocol::parse_command::ParsedCommand;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use rand::Rng;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use tokio::sync::mpsc::UnboundedSender;
use tracing::debug;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPane;
use crate::bottom_pane::BottomPaneParams;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::InputResult;
use crate::history_cell;
use crate::history_cell::CommandOutput;
use crate::history_cell::ExecCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::PatchEventType;
// streaming internals are provided by crate::streaming and crate::markdown_stream
use crate::user_approval_widget::ApprovalRequest;
mod interrupts;
use self::interrupts::InterruptManager;
mod agent;
use self::agent::spawn_agent;
use crate::streaming::controller::AppEventHistorySink;
use crate::streaming::controller::StreamController;
use codex_core::ConversationManager;
use codex_file_search::FileMatch;
use uuid::Uuid;

// Track information about an in-flight exec command.
struct RunningCommand {
    command: Vec<String>,
    parsed_cmd: Vec<ParsedCommand>,
}

pub(crate) struct ChatWidget<'a> {
    app_event_tx: AppEventSender,
    codex_op_tx: UnboundedSender<Op>,
    bottom_pane: BottomPane<'a>,
    active_exec_cell: Option<ExecCell>,
    config: Config,
    initial_user_message: Option<UserMessage>,
    total_token_usage: TokenUsage,
    last_token_usage: TokenUsage,
    // Stream lifecycle controller
    stream: StreamController,
    // Track the most recently active stream kind in the current turn
    last_stream_kind: Option<StreamKind>,
    running_commands: HashMap<String, RunningCommand>,
    pending_exec_completions: Vec<(Vec<String>, Vec<ParsedCommand>, CommandOutput)>,
    task_complete_pending: bool,
    // Queue of interruptive UI events deferred during an active write cycle
    interrupts: InterruptManager,
    // Whether a redraw is needed after handling the current event
    needs_redraw: bool,
    session_id: Option<Uuid>,
}

struct UserMessage {
    text: String,
    image_paths: Vec<PathBuf>,
}

use crate::streaming::StreamKind;

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
    #[inline]
    fn mark_needs_redraw(&mut self) {
        self.needs_redraw = true;
    }
    fn flush_answer_stream_with_separator(&mut self) {
        let sink = AppEventHistorySink(self.app_event_tx.clone());
        let _ = self.stream.finalize(StreamKind::Answer, true, &sink);
    }
    // --- Small event handlers ---
    fn on_session_configured(&mut self, event: codex_core::protocol::SessionConfiguredEvent) {
        self.bottom_pane
            .set_history_metadata(event.history_log_id, event.history_entry_count);
        self.session_id = Some(event.session_id);
        self.add_to_history(&history_cell::new_session_info(&self.config, event, true));
        if let Some(user_message) = self.initial_user_message.take() {
            self.submit_user_message(user_message);
        }
        self.mark_needs_redraw();
    }

    fn on_agent_message(&mut self, message: String) {
        let sink = AppEventHistorySink(self.app_event_tx.clone());
        let finished = self.stream.apply_final_answer(&message, &sink);
        self.last_stream_kind = Some(StreamKind::Answer);
        self.handle_if_stream_finished(finished);
        self.mark_needs_redraw();
    }

    fn on_agent_message_delta(&mut self, delta: String) {
        self.handle_streaming_delta(StreamKind::Answer, delta);
    }

    fn on_agent_reasoning_delta(&mut self, delta: String) {
        self.handle_streaming_delta(StreamKind::Reasoning, delta);
    }

    fn on_agent_reasoning_final(&mut self, text: String) {
        let sink = AppEventHistorySink(self.app_event_tx.clone());
        let finished = self.stream.apply_final_reasoning(&text, &sink);
        self.last_stream_kind = Some(StreamKind::Reasoning);
        self.handle_if_stream_finished(finished);
        self.mark_needs_redraw();
    }

    fn on_reasoning_section_break(&mut self) {
        let sink = AppEventHistorySink(self.app_event_tx.clone());
        self.stream.insert_reasoning_section_break(&sink);
    }

    // Raw reasoning uses the same flow as summarized reasoning

    fn on_task_started(&mut self) {
        self.bottom_pane.clear_ctrl_c_quit_hint();
        self.bottom_pane.set_task_running(true);
        self.stream.reset_headers_for_new_turn();
        self.last_stream_kind = None;
        self.mark_needs_redraw();
    }

    fn on_task_complete(&mut self) {
        // If a stream is currently active, finalize only that stream to flush any tail
        // without emitting stray headers for other streams.
        if self.stream.is_write_cycle_active() {
            let sink = AppEventHistorySink(self.app_event_tx.clone());
            if let Some(kind) = self.last_stream_kind {
                let _ = self.stream.finalize(kind, true, &sink);
            }
        }
        // Mark task stopped and request redraw now that all content is in history.
        self.bottom_pane.set_task_running(false);
        self.running_commands.clear();
        self.mark_needs_redraw();
    }

    fn on_token_count(&mut self, token_usage: TokenUsage) {
        self.total_token_usage = add_token_usage(&self.total_token_usage, &token_usage);
        self.last_token_usage = token_usage;
        self.bottom_pane.set_token_usage(
            self.total_token_usage.clone(),
            self.last_token_usage.clone(),
            self.config.model_context_window,
        );
    }

    fn on_error(&mut self, message: String) {
        self.add_to_history(&history_cell::new_error_event(message));
        self.bottom_pane.set_task_running(false);
        self.running_commands.clear();
        self.stream.clear_all();
        self.mark_needs_redraw();
    }

    fn on_plan_update(&mut self, update: codex_core::plan_tool::UpdatePlanArgs) {
        self.add_to_history(&history_cell::new_plan_update(update));
    }

    fn on_exec_approval_request(&mut self, id: String, ev: ExecApprovalRequestEvent) {
        let id2 = id.clone();
        let ev2 = ev.clone();
        self.defer_or_handle(
            |q| q.push_exec_approval(id, ev),
            |s| s.handle_exec_approval_now(id2, ev2),
        );
    }

    fn on_apply_patch_approval_request(&mut self, id: String, ev: ApplyPatchApprovalRequestEvent) {
        let id2 = id.clone();
        let ev2 = ev.clone();
        self.defer_or_handle(
            |q| q.push_apply_patch_approval(id, ev),
            |s| s.handle_apply_patch_approval_now(id2, ev2),
        );
    }

    fn on_exec_command_begin(&mut self, ev: ExecCommandBeginEvent) {
        self.flush_answer_stream_with_separator();
        let ev2 = ev.clone();
        self.defer_or_handle(|q| q.push_exec_begin(ev), |s| s.handle_exec_begin_now(ev2));
    }

    fn on_exec_command_output_delta(
        &mut self,
        _ev: codex_core::protocol::ExecCommandOutputDeltaEvent,
    ) {
        // TODO: Handle streaming exec output if/when implemented
    }

    fn on_patch_apply_begin(&mut self, event: PatchApplyBeginEvent) {
        self.add_to_history(&history_cell::new_patch_event(
            PatchEventType::ApplyBegin {
                auto_approved: event.auto_approved,
            },
            event.changes,
        ));
    }

    fn on_patch_apply_end(&mut self, event: codex_core::protocol::PatchApplyEndEvent) {
        let ev2 = event.clone();
        self.defer_or_handle(
            |q| q.push_patch_end(event),
            |s| s.handle_patch_apply_end_now(ev2),
        );
    }

    fn on_exec_command_end(&mut self, ev: ExecCommandEndEvent) {
        let ev2 = ev.clone();
        self.defer_or_handle(|q| q.push_exec_end(ev), |s| s.handle_exec_end_now(ev2));
    }

    fn on_mcp_tool_call_begin(&mut self, ev: McpToolCallBeginEvent) {
        let ev2 = ev.clone();
        self.defer_or_handle(|q| q.push_mcp_begin(ev), |s| s.handle_mcp_begin_now(ev2));
    }

    fn on_mcp_tool_call_end(&mut self, ev: McpToolCallEndEvent) {
        let ev2 = ev.clone();
        self.defer_or_handle(|q| q.push_mcp_end(ev), |s| s.handle_mcp_end_now(ev2));
    }

    fn on_get_history_entry_response(
        &mut self,
        event: codex_core::protocol::GetHistoryEntryResponseEvent,
    ) {
        let codex_core::protocol::GetHistoryEntryResponseEvent {
            offset,
            log_id,
            entry,
        } = event;
        self.bottom_pane
            .on_history_entry_response(log_id, offset, entry.map(|e| e.text));
    }

    fn on_shutdown_complete(&mut self) {
        self.app_event_tx.send(AppEvent::ExitRequest);
    }

    fn on_turn_diff(&mut self, unified_diff: String) {
        debug!("TurnDiffEvent: {unified_diff}");
    }

    fn on_background_event(&mut self, message: String) {
        debug!("BackgroundEvent: {message}");
    }
    /// Periodic tick to commit at most one queued line to history with a small delay,
    /// animating the output.
    pub(crate) fn on_commit_tick(&mut self) {
        let sink = AppEventHistorySink(self.app_event_tx.clone());
        let finished = self.stream.on_commit_tick(&sink);
        self.handle_if_stream_finished(finished);
    }
    fn is_write_cycle_active(&self) -> bool {
        self.stream.is_write_cycle_active()
    }

    fn flush_interrupt_queue(&mut self) {
        let mut mgr = std::mem::take(&mut self.interrupts);
        mgr.flush_all(self);
        self.interrupts = mgr;
    }

    #[inline]
    fn defer_or_handle(
        &mut self,
        push: impl FnOnce(&mut InterruptManager),
        handle: impl FnOnce(&mut Self),
    ) {
        // Preserve deterministic FIFO across queued interrupts: once anything
        // is queued due to an active write cycle, continue queueing until the
        // queue is flushed to avoid reordering (e.g., ExecEnd before ExecBegin).
        if self.is_write_cycle_active() || !self.interrupts.is_empty() {
            push(&mut self.interrupts);
        } else {
            handle(self);
        }
    }

    #[inline]
    fn handle_if_stream_finished(&mut self, finished: bool) {
        if finished {
            if self.task_complete_pending {
                self.bottom_pane.set_task_running(false);
                self.task_complete_pending = false;
            }
            self.flush_interrupt_queue();
        }
    }

    #[inline]
    fn handle_streaming_delta(&mut self, kind: StreamKind, delta: String) {
        let sink = AppEventHistorySink(self.app_event_tx.clone());
        self.stream.begin(kind, &sink);
        self.last_stream_kind = Some(kind);
        self.stream.push_and_maybe_commit(&delta, &sink);
        self.mark_needs_redraw();
    }

    pub(crate) fn handle_exec_end_now(&mut self, ev: ExecCommandEndEvent) {
        let running = self.running_commands.remove(&ev.call_id);
        let (command, parsed) = match running {
            Some(rc) => (rc.command, rc.parsed_cmd),
            None => (vec![ev.call_id.clone()], Vec::new()),
        };
        self.pending_exec_completions.push((
            command,
            parsed,
            CommandOutput {
                exit_code: ev.exit_code,
                stdout: ev.stdout.clone(),
                stderr: ev.stderr.clone(),
            },
        ));

        if self.running_commands.is_empty() {
            self.active_exec_cell = None;
            let pending = std::mem::take(&mut self.pending_exec_completions);
            for (command, parsed, output) in pending {
                self.add_to_history(&history_cell::new_completed_exec_command(
                    command, parsed, output,
                ));
            }
        }
    }

    pub(crate) fn handle_patch_apply_end_now(
        &mut self,
        event: codex_core::protocol::PatchApplyEndEvent,
    ) {
        if event.success {
            self.add_to_history(&history_cell::new_patch_apply_success(event.stdout));
        } else {
            self.add_to_history(&history_cell::new_patch_apply_failure(event.stderr));
        }
    }

    pub(crate) fn handle_exec_approval_now(&mut self, id: String, ev: ExecApprovalRequestEvent) {
        self.flush_answer_stream_with_separator();

        let request = ApprovalRequest::Exec {
            id,
            command: ev.command,
            reason: ev.reason,
        };
        self.bottom_pane.push_approval_request(request);
        self.mark_needs_redraw();
    }

    pub(crate) fn handle_apply_patch_approval_now(
        &mut self,
        id: String,
        ev: ApplyPatchApprovalRequestEvent,
    ) {
        self.flush_answer_stream_with_separator();
        self.add_to_history(&history_cell::new_patch_event(
            PatchEventType::ApprovalRequest,
            ev.changes.clone(),
        ));

        let request = ApprovalRequest::ApplyPatch {
            id,
            reason: ev.reason,
            grant_root: ev.grant_root,
        };
        self.bottom_pane.push_approval_request(request);
        self.mark_needs_redraw();
    }

    pub(crate) fn handle_exec_begin_now(&mut self, ev: ExecCommandBeginEvent) {
        // Ensure the status indicator is visible while the command runs.
        self.running_commands.insert(
            ev.call_id.clone(),
            RunningCommand {
                command: ev.command.clone(),
                parsed_cmd: ev.parsed_cmd.clone(),
            },
        );
        // Accumulate parsed commands into a single active Exec cell so they stack
        match self.active_exec_cell.as_mut() {
            Some(exec) => {
                exec.parsed.extend(ev.parsed_cmd);
            }
            _ => {
                self.active_exec_cell = Some(history_cell::new_active_exec_command(
                    ev.command,
                    ev.parsed_cmd,
                ));
            }
        }

        // Request a redraw so the working header and command list are visible immediately.
        self.mark_needs_redraw();
    }

    pub(crate) fn handle_mcp_begin_now(&mut self, ev: McpToolCallBeginEvent) {
        self.flush_answer_stream_with_separator();
        self.add_to_history(&history_cell::new_active_mcp_tool_call(ev.invocation));
    }
    pub(crate) fn handle_mcp_end_now(&mut self, ev: McpToolCallEndEvent) {
        self.flush_answer_stream_with_separator();
        self.add_to_history(&*history_cell::new_completed_mcp_tool_call(
            80,
            ev.invocation,
            ev.duration,
            ev.result
                .as_ref()
                .map(|r| !r.is_error.unwrap_or(false))
                .unwrap_or(false),
            ev.result,
        ));
    }
    fn interrupt_running_task(&mut self) {
        if self.bottom_pane.is_task_running() {
            self.active_exec_cell = None;
            self.running_commands.clear();
            self.bottom_pane.clear_ctrl_c_quit_hint();
            self.submit_op(Op::Interrupt);
            self.bottom_pane.set_task_running(false);
            self.stream.clear_all();
            self.request_redraw();
        }
    }
    fn layout_areas(&self, area: Rect) -> [Rect; 2] {
        Layout::vertical([
            Constraint::Max(
                self.active_exec_cell
                    .as_ref()
                    .map_or(0, |c| c.desired_height(area.width)),
            ),
            Constraint::Min(self.bottom_pane.desired_height(area.width)),
        ])
        .areas(area)
    }

    pub(crate) fn new(
        config: Config,
        conversation_manager: Arc<ConversationManager>,
        app_event_tx: AppEventSender,
        initial_prompt: Option<String>,
        initial_images: Vec<PathBuf>,
        enhanced_keys_supported: bool,
    ) -> Self {
        let mut rng = rand::rng();
        let placeholder = EXAMPLE_PROMPTS[rng.random_range(0..EXAMPLE_PROMPTS.len())].to_string();
        let codex_op_tx = spawn_agent(config.clone(), app_event_tx.clone(), conversation_manager);

        Self {
            app_event_tx: app_event_tx.clone(),
            codex_op_tx,
            bottom_pane: BottomPane::new(BottomPaneParams {
                app_event_tx,
                has_input_focus: true,
                enhanced_keys_supported,
                placeholder_text: placeholder,
            }),
            active_exec_cell: None,
            config: config.clone(),
            initial_user_message: create_initial_user_message(
                initial_prompt.unwrap_or_default(),
                initial_images,
            ),
            total_token_usage: TokenUsage::default(),
            last_token_usage: TokenUsage::default(),
            stream: StreamController::new(config),
            last_stream_kind: None,
            running_commands: HashMap::new(),
            pending_exec_completions: Vec::new(),
            task_complete_pending: false,
            interrupts: InterruptManager::new(),
            needs_redraw: false,
            session_id: None,
        }
    }

    pub fn desired_height(&self, width: u16) -> u16 {
        self.bottom_pane.desired_height(width)
            + self
                .active_exec_cell
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

    fn flush_active_exec_cell(&mut self) {
        if let Some(active) = self.active_exec_cell.take() {
            self.app_event_tx
                .send(AppEvent::InsertHistory(active.display_lines()));
        }
    }

    fn add_to_history(&mut self, cell: &dyn HistoryCell) {
        self.flush_active_exec_cell();
        self.app_event_tx
            .send(AppEvent::InsertHistory(cell.display_lines()));
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

        // Only show the text portion in conversation history.
        if !text.is_empty() {
            self.add_to_history(&history_cell::new_user_prompt(text.clone()));
        }
    }

    pub(crate) fn handle_codex_event(&mut self, event: Event) {
        // Reset redraw flag for this dispatch
        self.needs_redraw = false;
        let Event { id, msg } = event;

        match msg {
            EventMsg::AgentMessageDelta(_)
            | EventMsg::AgentReasoningDelta(_)
            | EventMsg::ExecCommandOutputDelta(_) => {}
            _ => {
                tracing::trace!("handle_codex_event: {:?}", msg);
            }
        }

        match msg {
            EventMsg::SessionConfigured(e) => self.on_session_configured(e),
            EventMsg::AgentMessage(AgentMessageEvent { message }) => self.on_agent_message(message),
            EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta }) => {
                self.on_agent_message_delta(delta)
            }
            EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent { delta })
            | EventMsg::AgentReasoningRawContentDelta(AgentReasoningRawContentDeltaEvent {
                delta,
            }) => self.on_agent_reasoning_delta(delta),
            EventMsg::AgentReasoning(AgentReasoningEvent { text })
            | EventMsg::AgentReasoningRawContent(AgentReasoningRawContentEvent { text }) => {
                self.on_agent_reasoning_final(text)
            }
            EventMsg::AgentReasoningSectionBreak(_) => self.on_reasoning_section_break(),
            EventMsg::TaskStarted => self.on_task_started(),
            EventMsg::TaskComplete(TaskCompleteEvent { .. }) => self.on_task_complete(),
            EventMsg::TokenCount(token_usage) => self.on_token_count(token_usage),
            EventMsg::Error(ErrorEvent { message }) => self.on_error(message),
            EventMsg::TurnAborted(_) => self.on_error("Turn interrupted".to_owned()),
            EventMsg::PlanUpdate(update) => self.on_plan_update(update),
            EventMsg::ExecApprovalRequest(ev) => self.on_exec_approval_request(id, ev),
            EventMsg::ApplyPatchApprovalRequest(ev) => self.on_apply_patch_approval_request(id, ev),
            EventMsg::ExecCommandBegin(ev) => self.on_exec_command_begin(ev),
            EventMsg::ExecCommandOutputDelta(delta) => self.on_exec_command_output_delta(delta),
            EventMsg::PatchApplyBegin(ev) => self.on_patch_apply_begin(ev),
            EventMsg::PatchApplyEnd(ev) => self.on_patch_apply_end(ev),
            EventMsg::ExecCommandEnd(ev) => self.on_exec_command_end(ev),
            EventMsg::McpToolCallBegin(ev) => self.on_mcp_tool_call_begin(ev),
            EventMsg::McpToolCallEnd(ev) => self.on_mcp_tool_call_end(ev),
            EventMsg::GetHistoryEntryResponse(ev) => self.on_get_history_entry_response(ev),
            EventMsg::ShutdownComplete => self.on_shutdown_complete(),
            EventMsg::TurnDiff(TurnDiffEvent { unified_diff }) => self.on_turn_diff(unified_diff),
            EventMsg::BackgroundEvent(BackgroundEventEvent { message }) => {
                self.on_background_event(message)
            }
        }
        // Coalesce redraws: issue at most one after handling the event
        if self.needs_redraw {
            self.request_redraw();
            self.needs_redraw = false;
        }
    }

    fn request_redraw(&mut self) {
        self.app_event_tx.send(AppEvent::RequestRedraw);
    }

    pub(crate) fn add_diff_in_progress(&mut self) {
        self.bottom_pane.set_task_running(true);
        self.bottom_pane
            .update_status_text("computing diff".to_string());
        self.request_redraw();
    }

    pub(crate) fn add_diff_output(&mut self, diff_output: String) {
        self.bottom_pane.set_task_running(false);
        self.add_to_history(&history_cell::new_diff_output(diff_output));
        self.mark_needs_redraw();
    }

    pub(crate) fn add_status_output(&mut self) {
        self.add_to_history(&history_cell::new_status_output(
            &self.config,
            &self.total_token_usage,
            &self.session_id,
        ));
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

    pub(crate) fn composer_is_empty(&self) -> bool {
        self.bottom_pane.composer_is_empty()
    }

    pub(crate) fn insert_str(&mut self, text: &str) {
        self.bottom_pane.insert_str(text);
    }
    /// Forward an `Op` directly to codex.
    pub(crate) fn submit_op(&self, op: Op) {
        // Record outbound operation for session replay fidelity.
        crate::session_log::log_outbound_op(&op);
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

impl WidgetRef for &ChatWidget<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let [active_cell_area, bottom_pane_area] = self.layout_areas(area);
        (&self.bottom_pane).render(bottom_pane_area, buf);
        if let Some(cell) = &self.active_exec_cell {
            cell.render_ref(active_cell_area, buf);
        }
    }
}

const EXAMPLE_PROMPTS: [&str; 6] = [
    "Explain this codebase",
    "Summarize recent commits",
    "Implement {feature}",
    "Find and fix a bug in @filename",
    "Write tests for @filename",
    "Improve documentation in @filename",
];

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
mod tests;

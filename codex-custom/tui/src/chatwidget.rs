use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use codex_core::codex_wrapper::CodexConversation;
use codex_core::codex_wrapper::init_codex;
use codex_core::config::Config;
use codex_core::parse_command::ParsedCommand;
use codex_core::parse_command::parse_command;
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
use codex_core::protocol::McpInvocation;
use codex_core::protocol::McpToolCallBeginEvent;
use codex_core::protocol::McpToolCallEndEvent;
use codex_core::protocol::Op;
use codex_core::protocol::PatchApplyBeginEvent;
use codex_core::protocol::TaskCompleteEvent;
use codex_core::protocol::TokenUsage;
use codex_core::protocol::TurnDiffEvent;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
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
use crate::history_cell::CommandOutput;
use crate::history_cell::ExecCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::PatchEventType;
use crate::live_wrap::RowBuilder;
use crate::user_approval_widget::ApprovalRequest;
use codex_file_search::FileMatch;
use ratatui::style::Stylize;
use std::time::Instant;
use uuid::Uuid;

struct RunningCommand {
    command: Vec<String>,
    #[allow(dead_code)]
    cwd: PathBuf,
    parsed_cmd: Vec<ParsedCommand>,
}

pub(crate) struct ChatWidget<'a> {
    app_event_tx: AppEventSender,
    codex_op_tx: UnboundedSender<Op>,
    bottom_pane: BottomPane<'a>,
    active_exec_cell: Option<HistoryCell>,
    config: Config,
    initial_user_message: Option<UserMessage>,
    total_token_usage: TokenUsage,
    last_token_usage: TokenUsage,
    reasoning_buffer: String,
    content_buffer: String,
    // Buffer for streaming assistant answer text; we do not surface partial
    // We wait for the final AgentMessage event and then emit the full text
    // at once into scrollback so the history contains a single message.
    answer_buffer: String,
    running_commands: HashMap<String, RunningCommand>,
    live_builder: RowBuilder,
    current_stream: Option<StreamKind>,
    stream_header_emitted: bool,
    live_max_rows: u16,
    queued_text: Option<String>,
    queued_images: Vec<InputItem>,
    session_id: Option<Uuid>,
    last_esc_time: Option<Instant>,

    // Sub-agent (delegate_task) UI state
    subagent_logs_collapsed: bool,
    subagent_active: bool,
    subagent_buffer: Vec<String>,
    subagent_placeholder_shown: bool,
}

struct UserMessage {
    text: String,
    images: Vec<InputItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamKind {
    Answer,
    Reasoning,
    SubAgent,
}

impl From<String> for UserMessage {
    fn from(text: String) -> Self {
        Self {
            text,
            images: Vec::new(),
        }
    }
}

fn create_initial_user_message(text: String, image_paths: Vec<PathBuf>) -> Option<UserMessage> {
    if text.is_empty() && image_paths.is_empty() {
        None
    } else {
        let mut images: Vec<InputItem> = Vec::new();
        for path in image_paths {
            images.push(InputItem::LocalImage { path });
        }
        Some(UserMessage { text, images })
    }
}

impl ChatWidget<'_> {
    fn interrupt_running_task(&mut self) {
        if self.bottom_pane.is_task_running() {
            self.active_exec_cell = None;
            self.bottom_pane.clear_ctrl_c_quit_hint();
            self.submit_op(Op::Interrupt);
            self.bottom_pane.set_task_running(false);
            self.bottom_pane.clear_live_ring();
            self.live_builder = RowBuilder::new(self.live_builder.width());
            self.current_stream = None;
            self.stream_header_emitted = false;
            self.answer_buffer.clear();
            self.reasoning_buffer.clear();
            self.content_buffer.clear();
            self.request_redraw();
        }
    }

    /// Attach an image from the system clipboard to the composer.
    pub fn attach_image_from_clipboard(&mut self) -> bool {
        self.bottom_pane.attach_image_from_clipboard()
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
    fn emit_stream_header(&mut self, kind: StreamKind) {
        use ratatui::text::Line as RLine;
        if self.stream_header_emitted {
            return;
        }
        let header = match kind {
            StreamKind::Reasoning => RLine::from("thinking".magenta().italic()),
            StreamKind::Answer => RLine::from("codex".magenta().bold()),
            StreamKind::SubAgent => RLine::from("sub-agent".blue().bold()),
        };
        self.app_event_tx
            .send(AppEvent::InsertHistory(vec![header]));
        self.stream_header_emitted = true;
    }
    fn finalize_active_stream(&mut self) {
        if let Some(kind) = self.current_stream {
            self.finalize_stream(kind);
        }
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
                approval_policy: config.approval_policy,
                sandbox_policy: config.sandbox_policy.clone(),
            }),
            active_exec_cell: None,
            config,
            initial_user_message: create_initial_user_message(
                initial_prompt.unwrap_or_default(),
                initial_images,
            ),
            total_token_usage: TokenUsage::default(),
            last_token_usage: TokenUsage::default(),
            reasoning_buffer: String::new(),
            content_buffer: String::new(),
            answer_buffer: String::new(),
            running_commands: HashMap::new(),
            live_builder: RowBuilder::new(80),
            current_stream: None,
            stream_header_emitted: false,
            live_max_rows: 3,
            queued_text: None,
            queued_images: Vec::new(),
            session_id: None,
            last_esc_time: None,

            subagent_logs_collapsed: false,
            subagent_active: false,
            subagent_buffer: Vec::new(),
            subagent_placeholder_shown: false,
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
        // Global hotkey: Shift+Tab cycles approval policy (workflow) and
        // reconfigures the active session without leaving the TUI.
        if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            let is_shift_tab = match key_event.code {
                KeyCode::BackTab => true,
                KeyCode::Tab if key_event.modifiers.contains(KeyModifiers::SHIFT) => true,
                _ => false,
            };
            if is_shift_tab {
                self.cycle_approval_policy();
                return;
            }
        }

        // Global hotkey: Ctrl+R toggles sub-agent logs collapse/expand.
        if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            && matches!(key_event.code, KeyCode::Char('r'))
            && key_event.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.toggle_subagent_logs();
            return;
        }

        match self.bottom_pane.handle_key_event(key_event) {
            InputResult::Submitted(text) => {
                if self.bottom_pane.is_task_running() {
                    // Capture any image attachments referenced by the submitted text
                    let attached = self.bottom_pane.take_image_attachments_in_text_order(&text);
                    let mut new_images: Vec<InputItem> = Vec::new();
                    for a in attached {
                        match a.source {
                            crate::bottom_pane::ImageSource::Local(path) => {
                                new_images.push(InputItem::LocalImage { path })
                            }
                            crate::bottom_pane::ImageSource::Url(url)
                            | crate::bottom_pane::ImageSource::DataUrl(url) => {
                                new_images.push(InputItem::Image { image_url: url })
                            }
                        }
                    }
                    // Queue the text to be sent automatically when the
                    // current task completes instead of injecting into the
                    // in-flight task.
                    if let Some(existing) = self.queued_text.take() {
                        let mut combined = existing;
                        if !combined.ends_with('\n') {
                            combined.push_str("\n\n");
                        }
                        combined.push_str(&text);
                        self.queued_text = Some(combined.clone());
                        self.bottom_pane
                            .set_queued_message_preview(Some(preview_for(&combined)));
                        // Append images for this queued fragment
                        self.queued_images.extend(new_images);
                    } else {
                        self.queued_text = Some(text.clone());
                        self.bottom_pane
                            .set_queued_message_preview(Some(preview_for(&text)));
                        self.queued_images = new_images;
                    }
                    // Hint in the status that a message is queued.
                    self.bottom_pane.set_queued_indicator(true);
                    self.bottom_pane
                        .update_status_text("waiting for model — queued message".to_string());
                } else {
                    // Consume image attachments referenced by placeholders in the submitted text.
                    let attached = self.bottom_pane.take_image_attachments_in_text_order(&text);
                    let mut images: Vec<InputItem> = Vec::new();
                    for a in attached {
                        match a.source {
                            crate::bottom_pane::ImageSource::Local(path) => {
                                images.push(InputItem::LocalImage { path })
                            }
                            crate::bottom_pane::ImageSource::Url(url)
                            | crate::bottom_pane::ImageSource::DataUrl(url) => {
                                images.push(InputItem::Image { image_url: url })
                            }
                        }
                    }
                    self.submit_user_message(UserMessage { text, images });
                }
            }
            InputResult::CancelQueued => {
                // User cancelled the queued message before it was sent.
                self.queued_text = None;
                self.queued_images.clear();
                self.bottom_pane.set_queued_indicator(false);
                self.bottom_pane.set_queued_message_preview(None);
                self.bottom_pane
                    .update_status_text("waiting for model".to_string());
                self.request_redraw();
            }
            InputResult::None => {}
        }
    }

    fn toggle_subagent_logs(&mut self) {
        self.subagent_logs_collapsed = !self.subagent_logs_collapsed;
        if self.subagent_logs_collapsed {
            // Collapsing: insert a single hint.
            self.add_to_history(HistoryCell::new_background_event(
                "sub-agent logs collapsed — press Ctrl+R to expand".to_string(),
            ));
        } else {
            // Expanding: flush any buffered sub-agent messages, then insert a hint.
            if !self.subagent_buffer.is_empty() {
                let buffered = std::mem::take(&mut self.subagent_buffer);
                for msg in buffered {
                    self.add_to_history(HistoryCell::new_background_event(msg));
                }
                self.subagent_placeholder_shown = false;
            }
            self.add_to_history(HistoryCell::new_background_event(
                "sub-agent logs expanded — press Ctrl+R to collapse".to_string(),
            ));
        }
        self.request_redraw();
    }

    fn cycle_approval_policy(&mut self) {
        use codex_core::protocol::AskForApproval;
        use codex_core::protocol::SandboxPolicy;

        // Derive next workflow preset based on current policy + sandbox.
        // Order:
        // 1) untrusted | read-only
        // 2) on-request | workspace-write
        // 3) full-auto (on-failure | workspace-write)
        // 4) bypass approvals on (never | danger-full-access)
        let (next_approval, next_sandbox) =
            match (self.config.approval_policy, &self.config.sandbox_policy) {
                (AskForApproval::UnlessTrusted, SandboxPolicy::ReadOnly) => (
                    AskForApproval::OnRequest,
                    SandboxPolicy::new_workspace_write_policy(),
                ),
                (AskForApproval::OnRequest, SandboxPolicy::WorkspaceWrite { .. }) => (
                    AskForApproval::OnFailure,
                    SandboxPolicy::new_workspace_write_policy(),
                ),
                (AskForApproval::OnFailure, SandboxPolicy::WorkspaceWrite { .. }) => {
                    (AskForApproval::Never, SandboxPolicy::DangerFullAccess)
                }
                (AskForApproval::Never, SandboxPolicy::DangerFullAccess) => {
                    (AskForApproval::UnlessTrusted, SandboxPolicy::ReadOnly)
                }
                // Any other combo: normalize to the first preset.
                _ => (AskForApproval::UnlessTrusted, SandboxPolicy::ReadOnly),
            };

        self.config.approval_policy = next_approval;
        self.config.sandbox_policy = next_sandbox.clone();

        // Update footer status.
        self.bottom_pane.set_approval_policy(next_approval);
        self.bottom_pane.set_sandbox_policy(next_sandbox.clone());

        // Reconfigure the active Codex session with the updated approval + sandbox policy.
        let _ = self.codex_op_tx.send(Op::ConfigureSession {
            provider: self.config.model_provider.clone(),
            model: self.config.model.clone(),
            model_reasoning_effort: self.config.model_reasoning_effort,
            model_reasoning_summary: self.config.model_reasoning_summary,
            user_instructions: self.config.user_instructions.clone(),
            base_instructions: self.config.base_instructions.clone(),
            approval_policy: self.config.approval_policy,
            sandbox_policy: next_sandbox,
            disable_response_storage: self.config.disable_response_storage,
            notify: self.config.notify.clone(),
            cwd: self.config.cwd.clone(),
            resume_path: None,
        });
        // Trigger a redraw so the footer reflects the change immediately.
        self.request_redraw();
    }

    pub(crate) fn handle_paste(&mut self, text: String) {
        self.bottom_pane.handle_paste(text);
    }

    fn add_to_history(&mut self, cell: HistoryCell) {
        self.flush_active_exec_cell();
        self.app_event_tx
            .send(AppEvent::InsertHistory(cell.plain_lines()));
    }

    fn submit_user_message(&mut self, user_message: UserMessage) {
        let UserMessage { text, images } = user_message;
        let mut items: Vec<InputItem> = Vec::new();

        if !text.is_empty() {
            items.push(InputItem::Text { text: text.clone() });
        }

        for img in images {
            items.push(img);
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
            EventMsg::AgentMessageDelta(_)
            | EventMsg::AgentReasoningDelta(_)
            | EventMsg::ExecCommandOutputDelta(_) => {}
            _ => {
                tracing::info!("handle_codex_event: {:?}", msg);
            }
        }

        match msg {
            EventMsg::SessionConfigured(event) => {
                // Only show the full welcome banner on the first session
                // configuration. Subsequent reconfigures (e.g. resume/branch)
                // should not look like a brand new session.
                let is_first_event = self.session_id.is_none();

                self.bottom_pane
                    .set_history_metadata(event.history_log_id, event.history_entry_count);
                self.session_id = Some(event.session_id);
                // Record session information; suppress welcome banner after first.
                self.add_to_history(HistoryCell::new_session_info(
                    &self.config,
                    event,
                    is_first_event,
                ));

                if let Some(user_message) = self.initial_user_message.take() {
                    // If the user provided an initial message, add it to the
                    // conversation history.
                    self.submit_user_message(user_message);
                }

                self.request_redraw();
            }
            EventMsg::AgentMessage(AgentMessageEvent { message }) => {
                // AgentMessage: if no deltas were streamed, render the final text.
                if self.current_stream != Some(StreamKind::Answer) && !message.is_empty() {
                    self.begin_stream(StreamKind::Answer);
                    self.stream_push_and_maybe_commit(&message);
                }
                self.finalize_stream(StreamKind::Answer);
                self.request_redraw();
            }
            EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta }) => {
                self.begin_stream(StreamKind::Answer);
                self.answer_buffer.push_str(&delta);
                self.stream_push_and_maybe_commit(&delta);
                self.request_redraw();
            }
            EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent { delta }) => {
                // Stream CoT into the live pane; keep input visible and commit
                // overflow rows incrementally to scrollback.
                self.begin_stream(StreamKind::Reasoning);
                self.reasoning_buffer.push_str(&delta);
                self.stream_push_and_maybe_commit(&delta);
                self.request_redraw();
            }
            EventMsg::AgentReasoning(AgentReasoningEvent { text }) => {
                // Final reasoning: if no deltas were streamed, render the final text.
                if self.current_stream != Some(StreamKind::Reasoning) && !text.is_empty() {
                    self.begin_stream(StreamKind::Reasoning);
                    self.stream_push_and_maybe_commit(&text);
                }
                self.finalize_stream(StreamKind::Reasoning);
                self.request_redraw();
            }
            EventMsg::AgentReasoningRawContentDelta(AgentReasoningRawContentDeltaEvent {
                delta,
            }) => {
                // Treat raw reasoning content the same as summarized reasoning for UI flow.
                self.begin_stream(StreamKind::Reasoning);
                self.reasoning_buffer.push_str(&delta);
                self.stream_push_and_maybe_commit(&delta);
                self.request_redraw();
            }
            EventMsg::AgentReasoningRawContent(AgentReasoningRawContentEvent { text }) => {
                // Final raw reasoning content: if no deltas were streamed, render the final text.
                if self.current_stream != Some(StreamKind::Reasoning) && !text.is_empty() {
                    self.begin_stream(StreamKind::Reasoning);
                    self.stream_push_and_maybe_commit(&text);
                }
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
                self.bottom_pane.set_task_running(false);
                self.bottom_pane.clear_live_ring();
                self.request_redraw();
                // If the user queued a message while the agent was working,
                // submit it now to start the next turn immediately.
                if let Some(queued) = self.queued_text.take() {
                    if !queued.trim().is_empty() {
                        let images = std::mem::take(&mut self.queued_images);
                        self.submit_user_message(UserMessage {
                            text: queued,
                            images,
                        });
                    }
                    self.bottom_pane.set_queued_indicator(false);
                    self.bottom_pane.set_queued_message_preview(None);
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
                self.bottom_pane.clear_live_ring();
                self.live_builder = RowBuilder::new(self.live_builder.width());
                self.current_stream = None;
                self.stream_header_emitted = false;
                self.answer_buffer.clear();
                self.reasoning_buffer.clear();
                self.content_buffer.clear();
                self.request_redraw();
            }
            EventMsg::PlanUpdate(update) => {
                // Commit plan updates directly to history (no status-line preview).
                self.add_to_history(HistoryCell::new_plan_update(update));
            }
            EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
                call_id: _,
                command,
                cwd,
                reason,
            }) => {
                self.finalize_active_stream();
                let request = ApprovalRequest::Exec {
                    id,
                    command,
                    cwd,
                    reason,
                };
                self.bottom_pane.push_approval_request(request);
                self.request_redraw();
            }
            EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
                call_id: _,
                changes,
                reason,
                grant_root,
            }) => {
                self.finalize_active_stream();
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
                self.add_to_history(HistoryCell::new_patch_event(
                    PatchEventType::ApprovalRequest,
                    changes,
                ));

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
                cwd,
                parsed_cmd,
            }) => {
                self.finalize_active_stream();
                // Ensure the status indicator is visible while the command runs.
                self.bottom_pane
                    .update_status_text("running command".to_string());
                self.running_commands.insert(
                    call_id,
                    RunningCommand {
                        command: command.clone(),
                        cwd: cwd.clone(),
                        parsed_cmd: parsed_cmd.clone(),
                    },
                );
                let active_exec_cell = self.active_exec_cell.take();
                let merge_result = merge_cells(&command, &parsed_cmd, &active_exec_cell);
                self.active_exec_cell = match merge_result {
                    MergeResult::Merge(cell) => Some(cell),
                    MergeResult::Drop => active_exec_cell,
                    MergeResult::NewCell(cell) => {
                        if let Some(active) = active_exec_cell {
                            self.app_event_tx
                                .send(AppEvent::InsertHistory(active.plain_lines()));
                        }
                        Some(cell)
                    }
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
                if let Some(cmd) = cmd {
                    // Preserve any merged parsed commands already present on the
                    // active cell; otherwise, fall back to this command's parsed.
                    let parsed_cmd = match &self.active_exec_cell {
                        Some(HistoryCell::Exec(ExecCell { parsed, .. })) if !parsed.is_empty() => {
                            parsed.clone()
                        }
                        _ => cmd.parsed_cmd.clone(),
                    };
                    // Replace the active running cell with the finalized result,
                    // but keep it as the active cell so it can be merged with
                    // subsequent commands before being committed.
                    self.active_exec_cell = Some(HistoryCell::new_completed_exec_command(
                        cmd.command,
                        parsed_cmd,
                        CommandOutput {
                            exit_code,
                            stdout,
                            stderr,
                        },
                    ));
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
            EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
                call_id: _,
                invocation,
            }) => {
                self.finalize_active_stream();
                self.active_exec_cell = Some(HistoryCell::new_active_mcp_tool_call(invocation));
            }
            EventMsg::McpToolCallEnd(McpToolCallEndEvent {
                call_id: _,
                duration,
                invocation,
                result,
            }) => {
                let completed = HistoryCell::new_completed_mcp_tool_call(
                    80,
                    invocation,
                    duration,
                    result
                        .as_ref()
                        .map(|r| r.is_error.unwrap_or(false))
                        .unwrap_or(false),
                    result,
                );
                self.active_exec_cell = Some(completed);
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
                // Surface sub-agent (delegate) logs with optional collapse/expand handling.
                if message.starts_with("sub-agent started") {
                    // New sub-agent run: reset per-run state and always show start + hint.
                    self.subagent_active = true;
                    self.subagent_buffer.clear();
                    self.subagent_placeholder_shown = false;
                    self.add_to_history(HistoryCell::new_background_event(message.clone()));
                    self.add_to_history(HistoryCell::new_background_event(
                        "hint: press Ctrl+R to collapse/expand sub-agent logs".to_string(),
                    ));
                    // Begin a live streaming block for sub-agent output.
                    self.begin_stream(StreamKind::SubAgent);
                } else if message.starts_with("sub-agent completed") {
                    // Finalize any active streaming block first, then show a completion line.
                    self.finalize_stream(StreamKind::SubAgent);
                    self.subagent_active = false;
                    self.add_to_history(HistoryCell::new_background_event(message.clone()));
                    if self.subagent_logs_collapsed && !self.subagent_buffer.is_empty() {
                        let hidden = self.subagent_buffer.len();
                        self.add_to_history(HistoryCell::new_background_event(format!(
                            "sub-agent produced {hidden} hidden messages — press Ctrl+R to expand"
                        )));
                    }
                } else if message.starts_with("sub-agent") {
                    // Stream sub-agent messages when not collapsed; buffer when collapsed.
                    if self.subagent_logs_collapsed {
                        self.subagent_buffer.push(message.clone());
                        if !self.subagent_placeholder_shown {
                            self.subagent_placeholder_shown = true;
                            self.add_to_history(HistoryCell::new_background_event(
                                "sub-agent logs hidden — press Ctrl+R to expand".to_string(),
                            ));
                        }
                    } else {
                        // Treat as a delta into the live sub-agent stream.
                        self.begin_stream(StreamKind::SubAgent);
                        self.stream_push_and_maybe_commit(&message);
                    }
                } else {
                    // Non sub-agent background events: show as-is.
                    self.add_to_history(HistoryCell::new_background_event(message));
                }
                self.request_redraw();
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

    /// Open the prompts manager popup to add/insert saved prompts.
    pub(crate) fn open_prompts_popup(&mut self) {
        let codex_home = self.config.codex_home.clone();
        self.bottom_pane.show_prompts_popup(codex_home);
    }

    /// Insert a small banner indicating summarization is running, optionally
    /// echoing user-provided focus text.
    pub(crate) fn add_compact_note(&mut self, focus: Option<String>) {
        use ratatui::text::Line as RLine;
        let mut lines: Vec<RLine<'static>> = Vec::new();
        lines.push(RLine::from("summarizing conversation".magenta().bold()));
        if let Some(f) = focus.and_then(|s| {
            let t = s.trim().to_string();
            if t.is_empty() { None } else { Some(t) }
        }) {
            lines.push(RLine::from("focus".gray().italic()));
            for l in f.lines() {
                lines.push(RLine::from(l.to_string()).gray());
            }
        }
        lines.push(RLine::from(""));
        self.app_event_tx.send(AppEvent::InsertHistory(lines));
    }

    /// Forward file-search results to the bottom pane.
    pub(crate) fn apply_file_search_result(&mut self, query: String, matches: Vec<FileMatch>) {
        self.bottom_pane.on_file_search_result(query, matches);
    }

    pub(crate) fn on_esc(&mut self) -> bool {
        if self.bottom_pane.is_task_running() {
            self.interrupt_running_task();
            return true;
        }
        // If a modal/overlay is active, let it consume ESC.
        if self.bottom_pane.is_modal_active() {
            return false;
        }
        let now = Instant::now();
        let within = self
            .last_esc_time
            .map(|prev| now.duration_since(prev) <= std::time::Duration::from_millis(400))
            .unwrap_or(false);
        if within {
            self.last_esc_time = None;
            if let Some(sess_id) = self.session_id {
                self.bottom_pane
                    .show_past_inputs_popup(self.config.codex_home.clone(), sess_id);
                return true;
            }
            return false;
        }
        // First ESC: record it and allow normal handling (e.g., deselect queued banner).
        self.last_esc_time = Some(now);
        false
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

    pub(crate) fn set_composer_text(&mut self, text: String) {
        self.bottom_pane.set_composer_text(text);
    }

    /// Open the resume popup to select a previous session.
    pub(crate) fn open_resume_popup(&mut self) {
        let codex_home = self.config.codex_home.clone();
        let cwd = self.config.cwd.clone();
        self.bottom_pane.show_resume_popup(codex_home, cwd);
    }

    /// Backfill transcript from a rollout file and reconfigure the session to resume.
    pub(crate) fn resume_from_rollout(&mut self, path: std::path::PathBuf) {
        // Visually insert a small header indicating resume.
        let mut header = Vec::new();
        header.push(ratatui::text::Line::from(
            "resumed session".magenta().bold(),
        ));
        header.push(ratatui::text::Line::from(path.display().to_string()));
        header.push(ratatui::text::Line::from(""));
        self.app_event_tx
            .send(crate::app_event::AppEvent::InsertHistory(header));

        // Parse JSONL and inject user/assistant messages.
        if let Err(e) = self.backfill_transcript_lines(&path) {
            let mut lines = Vec::new();
            lines.push(ratatui::text::Line::from(format!(
                "failed to read transcript: {}",
                e
            )));
            lines.push(ratatui::text::Line::from(""));
            self.app_event_tx
                .send(crate::app_event::AppEvent::InsertHistory(lines));
        }

        // Reconfigure Codex to resume using this rollout file so the core state matches the UI.
        let op = codex_core::protocol::Op::ConfigureSession {
            provider: self.config.model_provider.clone(),
            model: self.config.model.clone(),
            model_reasoning_effort: self.config.model_reasoning_effort,
            model_reasoning_summary: self.config.model_reasoning_summary,
            user_instructions: self.config.user_instructions.clone(),
            base_instructions: self.config.base_instructions.clone(),
            approval_policy: self.config.approval_policy,
            sandbox_policy: self.config.sandbox_policy.clone(),
            disable_response_storage: self.config.disable_response_storage,
            notify: self.config.notify.clone(),
            cwd: self.config.cwd.clone(),
            resume_path: Some(path),
        };
        self.submit_op(op);
    }

    fn backfill_transcript_lines(&mut self, path: &std::path::Path) -> std::io::Result<()> {
        use codex_core::plan_tool::UpdatePlanArgs;
        use serde::Deserialize;
        use std::collections::HashMap as StdHashMap;
        use std::io::BufRead;
        use std::io::BufReader;

        // Local mirror of serialized rollout item types to avoid depending on private `codex_core::models`.
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum RolloutContentItem {
            InputText {
                text: String,
            },
            OutputText {
                text: String,
            },
            #[serde(other)]
            Other,
        }

        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum ReasoningSummaryItem {
            SummaryText { text: String },
        }

        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum ReasoningContentItem {
            ReasoningText { text: String },
        }

        #[derive(Deserialize)]
        struct LocalShellExecAction {
            command: Vec<String>,
            #[allow(dead_code)]
            timeout_ms: Option<u64>,
            #[allow(dead_code)]
            working_directory: Option<String>,
            #[allow(dead_code)]
            env: Option<std::collections::HashMap<String, String>>,
            #[allow(dead_code)]
            user: Option<String>,
        }

        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum LocalShellAction {
            Exec(LocalShellExecAction),
        }

        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum RolloutResponseItem {
            Message {
                #[allow(dead_code)]
                id: Option<String>,
                role: String,
                content: Vec<RolloutContentItem>,
            },
            Reasoning {
                #[allow(dead_code)]
                id: Option<String>,
                summary: Vec<ReasoningSummaryItem>,
                #[serde(default, skip_serializing_if = "Option::is_none")]
                content: Option<Vec<ReasoningContentItem>>,
                #[allow(dead_code)]
                encrypted_content: Option<String>,
            },
            LocalShellCall {
                id: Option<String>,
                call_id: Option<String>,
                #[allow(dead_code)]
                status: String,
                action: LocalShellAction,
            },
            FunctionCall {
                #[allow(dead_code)]
                id: Option<String>,
                name: String,
                arguments: String,
                call_id: String,
            },
            FunctionCallOutput {
                call_id: String,
                output: FunctionCallOutputPayloadLocal,
            },
            #[serde(other)]
            Other,
        }

        #[derive(Deserialize, Clone)]
        struct FunctionCallOutputPayloadLocal {
            content: String,
            #[allow(dead_code)]
            success: Option<bool>,
        }

        let f = std::fs::File::open(path)?;
        let reader = BufReader::new(f);
        let mut lines_iter = reader.lines();
        // Skip the first meta line.
        let _ = lines_iter.next();

        // Track pending calls so we can pair outputs with invocations.
        let mut pending_exec: StdHashMap<String, (Vec<String>, Vec<ParsedCommand>)> =
            StdHashMap::new();
        let mut pending_mcp: StdHashMap<String, McpInvocation> = StdHashMap::new();

        for line_res in lines_iter {
            let line = match line_res {
                Ok(s) => s,
                Err(_) => continue,
            };
            if line.trim().is_empty() {
                continue;
            }
            // Skip state snapshots from the rollout (they are not display items).
            let maybe_obj: Result<serde_json::Value, _> = serde_json::from_str(&line);
            if let Ok(v) = &maybe_obj {
                if v.get("record_type").and_then(|rt| rt.as_str()) == Some("state") {
                    continue;
                }
            }

            let item: Result<RolloutResponseItem, _> = serde_json::from_str(&line);
            let Ok(item) = item else { continue };
            match item {
                RolloutResponseItem::Message { role, content, .. } => {
                    let mut text = String::new();
                    for c in content {
                        match c {
                            RolloutContentItem::InputText { text: t }
                            | RolloutContentItem::OutputText { text: t } => {
                                if !text.is_empty() {
                                    text.push('\n');
                                }
                                text.push_str(&t);
                            }
                            _ => {}
                        }
                    }
                    if text.trim().is_empty() {
                        continue;
                    }
                    if role == "user" {
                        self.add_to_history(HistoryCell::new_user_prompt(text));
                    } else if role == "assistant" {
                        let mut lines: Vec<ratatui::text::Line<'static>> = Vec::new();
                        lines.push(ratatui::text::Line::from("codex".magenta().bold()));
                        for l in text.lines() {
                            lines.push(ratatui::text::Line::from(l.to_string()))
                        }
                        lines.push(ratatui::text::Line::from(""));
                        self.app_event_tx
                            .send(crate::app_event::AppEvent::InsertHistory(lines));
                    }
                }
                RolloutResponseItem::Reasoning {
                    summary, content, ..
                } => {
                    // Render summary and (optionally) raw content as a thinking block.
                    let mut lines: Vec<ratatui::text::Line<'static>> = Vec::new();
                    lines.push(ratatui::text::Line::from("thinking".magenta().italic()));
                    for s in summary {
                        let ReasoningSummaryItem::SummaryText { text } = s;
                        for l in text.lines() {
                            lines.push(ratatui::text::Line::from(l.to_string()));
                        }
                    }
                    if self.config.show_raw_agent_reasoning {
                        if let Some(items) = content {
                            for c in items {
                                let ReasoningContentItem::ReasoningText { text } = c;
                                for l in text.lines() {
                                    lines.push(ratatui::text::Line::from(l.to_string()));
                                }
                            }
                        }
                    }
                    lines.push(ratatui::text::Line::from(""));
                    self.app_event_tx
                        .send(crate::app_event::AppEvent::InsertHistory(lines));
                }
                RolloutResponseItem::LocalShellCall {
                    id,
                    call_id,
                    action,
                    ..
                } => {
                    let LocalShellAction::Exec(exec) = action;
                    let call = call_id.or(id).unwrap_or_default();
                    let command = exec.command;
                    let parsed = parse_command(&command);
                    // Show an active exec cell, then pair with output later when available.
                    self.add_to_history(HistoryCell::new_active_exec_command(
                        command.clone(),
                        parsed.clone(),
                    ));
                    if !call.is_empty() {
                        pending_exec.insert(call, (command, parsed));
                    }
                }
                RolloutResponseItem::FunctionCall {
                    name,
                    arguments,
                    call_id,
                    ..
                } => {
                    match name.as_str() {
                        "container.exec" | "shell" => {
                            #[derive(Deserialize)]
                            struct ShellParams {
                                command: Vec<String>,
                            }
                            if let Ok(params) = serde_json::from_str::<ShellParams>(&arguments) {
                                let command = params.command;
                                let parsed = parse_command(&command);
                                self.add_to_history(HistoryCell::new_active_exec_command(
                                    command.clone(),
                                    parsed.clone(),
                                ));
                                pending_exec.insert(call_id, (command, parsed));
                            }
                        }
                        "update_plan" => {
                            if let Ok(args) = serde_json::from_str::<UpdatePlanArgs>(&arguments) {
                                self.add_to_history(HistoryCell::new_plan_update(args));
                            }
                        }
                        _ => {
                            // Treat as MCP tool call if it looks like server.tool
                            let (server, tool) = match name.split_once('.') {
                                Some((s, t)) => (s.to_string(), t.to_string()),
                                None => ("tool".to_string(), name.clone()),
                            };
                            let args_value: Option<serde_json::Value> =
                                if arguments.trim().is_empty() {
                                    None
                                } else {
                                    serde_json::from_str(&arguments).ok()
                                };
                            let invocation = McpInvocation {
                                server,
                                tool,
                                arguments: args_value,
                            };
                            self.add_to_history(HistoryCell::new_active_mcp_tool_call(
                                invocation.clone(),
                            ));
                            pending_mcp.insert(call_id, invocation);
                        }
                    }
                }
                RolloutResponseItem::FunctionCallOutput { call_id, output } => {
                    // Attempt to parse as exec output payload first.
                    #[derive(Deserialize)]
                    struct ExecOutputMeta {
                        exit_code: i32,
                        #[allow(dead_code)]
                        duration_seconds: f32,
                    }
                    #[derive(Deserialize)]
                    struct ExecOutputPayload {
                        output: String,
                        metadata: ExecOutputMeta,
                    }

                    if let Ok(exec_payload) =
                        serde_json::from_str::<ExecOutputPayload>(&output.content)
                    {
                        // Pair with pending exec call; fall back to a generic command if unknown.
                        let (command, parsed) = pending_exec
                            .remove(&call_id)
                            .unwrap_or_else(|| (vec!["(exec)".to_string()], Vec::new()));

                        let cmd_output = CommandOutput {
                            exit_code: exec_payload.metadata.exit_code,
                            stdout: if exec_payload.metadata.exit_code == 0 {
                                exec_payload.output.clone()
                            } else {
                                String::new()
                            },
                            stderr: if exec_payload.metadata.exit_code != 0 {
                                exec_payload.output.clone()
                            } else {
                                String::new()
                            },
                        };
                        self.add_to_history(HistoryCell::new_completed_exec_command(
                            command, parsed, cmd_output,
                        ));
                        continue;
                    }

                    // Otherwise: treat as MCP tool result (JSON) or error string.
                    let invocation = pending_mcp.remove(&call_id).unwrap_or(McpInvocation {
                        server: "tool".to_string(),
                        tool: call_id.clone(),
                        arguments: None,
                    });

                    // Parse result JSON; if it fails, treat as error string.
                    let result: Result<mcp_types::CallToolResult, String> =
                        match serde_json::from_str(&output.content) {
                            Ok(ok) => Ok(ok),
                            Err(_) => Err(output.content.clone()),
                        };
                    let success = result
                        .as_ref()
                        .map(|r| !r.is_error.unwrap_or(false))
                        .unwrap_or(false);
                    let duration = std::time::Duration::from_millis(0);
                    let width = self.live_builder.width() as u16;
                    self.add_to_history(HistoryCell::new_completed_mcp_tool_call(
                        width, invocation, duration, success, result,
                    ));
                }
                RolloutResponseItem::Other => {}
            }
        }
        Ok(())
    }
}

fn preview_for(text: &str) -> String {
    // Use the first non-empty line; fall back to the first line.
    let mut line = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or_else(|| text.lines().next().unwrap_or(""))
        .trim()
        .to_string();
    // Truncate to a reasonable length for a single-line banner.
    const MAX_PREVIEW_CHARS: usize = 80;
    if line.chars().count() > MAX_PREVIEW_CHARS {
        let mut s = String::new();
        for ch in line.chars().take(MAX_PREVIEW_CHARS - 1) {
            s.push(ch);
        }
        s.push('…');
        line = s;
    }
    line
}

impl ChatWidget<'_> {
    fn begin_stream(&mut self, kind: StreamKind) {
        if let Some(current) = self.current_stream {
            if current != kind {
                self.finalize_stream(current);
            }
        }

        if self.current_stream != Some(kind) {
            self.current_stream = Some(kind);
            self.stream_header_emitted = false;
            // Clear any previous live content; we're starting a new stream.
            self.live_builder = RowBuilder::new(self.live_builder.width());
            // Ensure the waiting status is visible (composer replaced).
            self.bottom_pane
                .update_status_text("waiting for model".to_string());
            self.flush_active_exec_cell();
            self.emit_stream_header(kind);
        }
    }

    fn flush_active_exec_cell(&mut self) {
        if let Some(active) = self.active_exec_cell.take() {
            self.app_event_tx
                .send(AppEvent::InsertHistory(active.plain_lines()));
        }
    }

    fn stream_push_and_maybe_commit(&mut self, delta: &str) {
        self.flush_active_exec_cell();

        self.live_builder.push_fragment(delta);

        // Commit overflow rows (small batches) while keeping the last N rows visible.
        let drained = self
            .live_builder
            .drain_commit_ready(self.live_max_rows as usize);
        if !drained.is_empty() {
            let mut lines: Vec<ratatui::text::Line<'static>> = Vec::new();
            if !self.stream_header_emitted {
                match self.current_stream {
                    Some(StreamKind::Reasoning) => {
                        lines.push(ratatui::text::Line::from("thinking".magenta().italic()));
                    }
                    Some(StreamKind::Answer) => {
                        lines.push(ratatui::text::Line::from("codex".magenta().bold()));
                    }
                    Some(StreamKind::SubAgent) => {
                        lines.push(ratatui::text::Line::from("sub-agent".blue().bold()));
                    }
                    None => {}
                }
                self.stream_header_emitted = true;
            }
            for r in drained {
                lines.push(ratatui::text::Line::from(r.text));
            }
            self.app_event_tx.send(AppEvent::InsertHistory(lines));
        }

        // Update the live ring overlay lines (text-only, newest at bottom).
        let rows = self
            .live_builder
            .display_rows()
            .into_iter()
            .map(|r| ratatui::text::Line::from(r.text))
            .collect::<Vec<_>>();
        self.bottom_pane
            .set_live_ring_rows(self.live_max_rows, rows);
    }

    fn finalize_stream(&mut self, kind: StreamKind) {
        if self.current_stream != Some(kind) {
            // Nothing to do; either already finalized or not the active stream.
            return;
        }
        // Flush any partial line as a full row, then drain all remaining rows.
        self.live_builder.end_line();
        let remaining = self.live_builder.drain_rows();
        // TODO: Re-add markdown rendering for assistant answers and reasoning.
        // When finalizing, pass the accumulated text through `markdown::append_markdown`
        // to build styled `Line<'static>` entries instead of raw plain text lines.
        if !remaining.is_empty() || !self.stream_header_emitted {
            let mut lines: Vec<ratatui::text::Line<'static>> = Vec::new();
            if !self.stream_header_emitted {
                match kind {
                    StreamKind::Reasoning => {
                        lines.push(ratatui::text::Line::from("thinking".magenta().italic()));
                    }
                    StreamKind::Answer => {
                        lines.push(ratatui::text::Line::from("codex".magenta().bold()));
                    }
                    StreamKind::SubAgent => {
                        lines.push(ratatui::text::Line::from("sub-agent".blue().bold()));
                    }
                }
                self.stream_header_emitted = true;
            }
            for r in remaining {
                lines.push(ratatui::text::Line::from(r.text));
            }
            // Close the block with a blank line for readability.
            lines.push(ratatui::text::Line::from(""));
            self.app_event_tx.send(AppEvent::InsertHistory(lines));
        }

        // Clear the live overlay and reset state for the next stream.
        self.live_builder = RowBuilder::new(self.live_builder.width());
        self.bottom_pane.clear_live_ring();
        self.current_stream = None;
        self.stream_header_emitted = false;
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

enum MergeResult {
    Merge(HistoryCell),
    Drop,
    NewCell(HistoryCell),
}

// Determine whether to and how to merge two consecutive exec cells.
fn merge_cells(
    new_command: &[String],
    new_parsed: &[ParsedCommand],
    active_exec_cell: &Option<HistoryCell>,
) -> MergeResult {
    let ExecCell {
        command: _existing_command,
        parsed: existing_parsed,
        output: existing_output,
    } = match active_exec_cell {
        Some(HistoryCell::Exec(cell)) => cell,
        _ => {
            // There is no existing exec cell.
            return MergeResult::NewCell(HistoryCell::new_active_exec_command(
                new_command.to_vec(),
                new_parsed.to_vec(),
            ));
        }
    };
    let existing_last = existing_parsed.last();
    let new_last = new_parsed.last();

    // Drop the first command if it is a read and matches the last command.
    // This is a common pattern the model does and it simplifies the output to dedupe.
    let drop_first = if let (
        Some(ParsedCommand::Read {
            name: existing_name,
            ..
        }),
        Some(ParsedCommand::Read { name: new_name, .. }),
    ) = (existing_last, new_last)
    {
        existing_name == new_name
    } else {
        false
    };

    if drop_first && new_parsed.len() == 1 {
        // There is only one command and it was deduped.
        return MergeResult::Drop;
    }
    let existing_exit_code = existing_output.as_ref().map(|o| o.exit_code);
    if let Some(code) = existing_exit_code {
        if code != 0 {
            // If the previous command failed, don't merge so the user can see stderr.
            // Start a fresh cell for the new command instead of duplicating the old one.
            return MergeResult::NewCell(HistoryCell::new_active_exec_command(
                new_command.to_vec(),
                new_parsed.to_vec(),
            ));
        }
    }

    let mut merged_parsed = existing_parsed.to_vec();
    if drop_first {
        merged_parsed.extend(new_parsed[1..].to_vec());
    } else {
        merged_parsed.extend(new_parsed.to_vec());
    }

    MergeResult::Merge(HistoryCell::new_active_exec_command(
        new_command.to_vec(),
        merged_parsed,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history_cell::CommandOutput;

    fn read_cmd(name: &str) -> ParsedCommand {
        ParsedCommand::Read {
            cmd: vec!["cat".to_string(), name.to_string()],
            name: name.to_string(),
        }
    }

    fn unknown_cmd(cmd: &str) -> ParsedCommand {
        ParsedCommand::Unknown {
            cmd: cmd.split_whitespace().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn when_no_active_exec_cell_creates_new_cell() {
        let new_command = vec!["echo".to_string(), "hi".to_string()];
        let new_parsed = vec![read_cmd("a")];

        let result = merge_cells(&new_command, &new_parsed, &None);

        match result {
            MergeResult::NewCell(cell) => match cell {
                HistoryCell::Exec(ExecCell {
                    command,
                    parsed,
                    output,
                }) => {
                    assert_eq!(command, new_command);
                    assert_eq!(parsed, new_parsed);
                    assert!(output.is_none());
                }
                _ => panic!("expected Exec cell"),
            },
            _ => panic!("expected NewCell"),
        }
    }

    #[test]
    fn drops_duplicate_trailing_read_when_new_has_only_one_read() {
        // existing last = Read("foo"), new last = Read("foo"), new_parsed.len() == 1
        let active = Some(HistoryCell::new_active_exec_command(
            vec!["bash".into(), "-lc".into(), "cat foo".into()],
            vec![read_cmd("foo")],
        ));
        let new_command = vec!["cat".into(), "foo".into()];
        let new_parsed = vec![read_cmd("foo")];

        let result = merge_cells(&new_command, &new_parsed, &active);
        match result {
            MergeResult::Drop => {}
            _ => panic!("expected Drop"),
        }
    }

    #[test]
    fn does_not_merge_when_previous_command_failed() {
        // existing exit_code != 0 forces starting a fresh cell
        let active = Some(HistoryCell::new_completed_exec_command(
            vec!["bash".into(), "-lc".into(), "cat bar".into()],
            vec![read_cmd("bar")],
            CommandOutput {
                exit_code: 1,
                stdout: String::new(),
                stderr: "err".into(),
            },
        ));
        // Ensure drop_first condition is false (different name)
        let new_command = vec!["cat".into(), "baz".into()];
        let new_parsed = vec![read_cmd("baz")];

        let result = merge_cells(&new_command, &new_parsed, &active);
        match result {
            MergeResult::NewCell(cell) => match cell {
                HistoryCell::Exec(ExecCell {
                    command, parsed, ..
                }) => {
                    assert_eq!(command, new_command);
                    assert_eq!(parsed, new_parsed);
                }
                _ => panic!("expected Exec cell"),
            },
            _ => panic!("expected NewCell"),
        }
    }

    #[test]
    fn merges_with_drop_first_true_when_new_len_gt_one() {
        // existing last Read("file.txt"), new starts with same Read then more
        let active = Some(HistoryCell::new_active_exec_command(
            vec!["cat".into(), "file.txt".into()],
            vec![read_cmd("file.txt")],
        ));
        let new_command = vec!["bash".into(), "-lc".into(), "sed -n 1,20p file.txt".into()];
        // Place the duplicate Read as the LAST element to satisfy drop_first condition
        let leading = unknown_cmd("tail -n 20");
        let new_parsed = vec![leading.clone(), read_cmd("file.txt")];

        let result = merge_cells(&new_command, &new_parsed, &active);
        match result {
            MergeResult::Merge(cell) => match cell {
                HistoryCell::Exec(ExecCell {
                    command, parsed, ..
                }) => {
                    assert_eq!(command, new_command);
                    // Expect existing parsed + new_parsed[1..]
                    assert_eq!(parsed.len(), 2);
                    match (&parsed[0], &parsed[1]) {
                        (
                            ParsedCommand::Read { name, .. },
                            ParsedCommand::Read { name: n2, .. },
                        ) => {
                            assert_eq!(name, "file.txt");
                            assert_eq!(n2, "file.txt");
                        }
                        _ => panic!("unexpected parsed commands"),
                    }
                }
                _ => panic!("expected Exec cell"),
            },
            _ => panic!("expected Merge"),
        }
    }

    #[test]
    fn merges_without_drop_first_when_last_commands_differ() {
        // existing last Read("file1.txt"), new last Read("file2.txt"); should concatenate
        let active = Some(HistoryCell::new_active_exec_command(
            vec!["cat".into(), "file1.txt".into()],
            vec![read_cmd("file1.txt")],
        ));
        let new_command = vec!["bash".into(), "-lc".into(), "cat file2.txt".into()];
        let t2 = read_cmd("file2.txt");
        let extra = unknown_cmd("echo done");
        let new_parsed = vec![t2.clone(), extra.clone()];

        let result = merge_cells(&new_command, &new_parsed, &active);
        match result {
            MergeResult::Merge(cell) => match cell {
                HistoryCell::Exec(ExecCell {
                    command, parsed, ..
                }) => {
                    assert_eq!(command, new_command);
                    assert_eq!(parsed.len(), 3);
                    match (&parsed[0], &parsed[1], &parsed[2]) {
                        (ParsedCommand::Read { name: n1, .. }, p2, p3) => {
                            assert_eq!(n1, "file1.txt");
                            assert_eq!(p2, &t2);
                            assert_eq!(p3, &extra);
                        }
                        _ => panic!("unexpected parsed commands"),
                    }
                }
                _ => panic!("expected Exec cell"),
            },
            _ => panic!("expected Merge"),
        }
    }
}

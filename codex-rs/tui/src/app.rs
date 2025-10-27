use crate::UpdateAction;
use crate::app_backtrack::BacktrackState;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::ApprovalRequest;
use crate::chatwidget::ChatWidget;
use crate::chatwidget::ChatWidgetInit;
use crate::chatwidget::ChatWidgetSession;
use crate::chatwidget::DelegateDisplayLabel;
use crate::diff_render::DiffSummary;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::file_search::FileSearchManager;
use crate::history_cell::HistoryCell;
use crate::history_cell::UserHistoryCell;
use crate::pager_overlay::Overlay;
use crate::render::highlight::highlight_bash_to_lines;
use crate::resume_picker::ResumeSelection;
use crate::status::StatusShadowData;
use crate::tui;
use crate::tui::TuiEvent;
use codex_ansi_escape::ansi_escape_line;
use codex_core::AuthManager;
use codex_core::ConversationManager;
use codex_core::config::Config;
use codex_core::config::persist_model_selection;
use codex_core::config::set_hide_full_access_warning;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::Event;
use codex_core::protocol::SessionConfiguredEvent;
use codex_core::protocol::SessionSource;
use codex_core::protocol::TokenUsage;
use codex_core::protocol_config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_multi_agent::ActiveDelegateSession;
use codex_multi_agent::AgentId;
use codex_multi_agent::AgentOrchestrator;
use codex_multi_agent::DelegateEvent;
use codex_multi_agent::DelegateSessionMode;
use codex_multi_agent::DelegateSessionSummary;
use codex_multi_agent::DetachedRunSummary;
use codex_multi_agent::delegate_tool_adapter;
#[cfg(test)]
use codex_multi_agent::shadow::ShadowConfig;
use codex_multi_agent::shadow::ShadowSessionSummary;
use codex_protocol::ConversationId;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::collections::hash_map::Entry;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use std::time::SystemTime;
use tokio::select;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::unbounded_channel;
// use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AppExitInfo {
    pub token_usage: TokenUsage,
    pub conversation_id: Option<ConversationId>,
    pub update_action: Option<UpdateAction>,
}

fn spawn_event_forwarder(
    app_event_tx: AppEventSender,
    conversation_id: ConversationId,
    mut event_rx: UnboundedReceiver<Event>,
) {
    tokio::spawn(async move {
        let conversation_id = conversation_id.to_string();
        while let Some(event) = event_rx.recv().await {
            app_event_tx.send(AppEvent::CodexEvent {
                conversation_id: conversation_id.clone(),
                event,
            });
        }
    });
}

pub(crate) struct App {
    pub(crate) server: Arc<ConversationManager>,
    pub(crate) app_event_tx: AppEventSender,
    pub(crate) sessions: HashMap<String, SessionHandle>,
    pub(crate) active_session_id: String,
    pub(crate) primary_session_id: String,
    pub(crate) auth_manager: Arc<AuthManager>,
    pub(crate) delegate_orchestrator: Arc<AgentOrchestrator>,

    /// Config is stored here so we can recreate ChatWidgets as needed.
    pub(crate) config: Config,
    pub(crate) active_profile: Option<String>,

    pub(crate) file_search: FileSearchManager,

    pub(crate) transcript_cells: Vec<Arc<dyn HistoryCell>>,

    // Pager overlay state (Transcript or Static like Diff)
    pub(crate) overlay: Option<Overlay>,
    pub(crate) deferred_history_lines: Vec<Line<'static>>,
    has_emitted_history_lines: bool,

    pub(crate) enhanced_keys_supported: bool,

    /// Controls the animation thread that sends CommitTick events.
    pub(crate) commit_anim_running: Arc<AtomicBool>,

    // Esc-backtracking state grouped
    pub(crate) backtrack: crate::app_backtrack::BacktrackState,
    pub(crate) feedback: codex_feedback::CodexFeedback,
    /// Set when the user confirms an update; propagated on exit.
    pub(crate) pending_update_action: Option<UpdateAction>,
    run_parent_map: HashMap<String, String>,
}

#[derive(Default)]
struct DelegateTree {
    nodes: HashMap<String, DelegateNode>,
    roots: Vec<String>,
}

struct DelegateNode {
    agent_id: AgentId,
    parent: Option<String>,
    children: Vec<String>,
}

#[derive(Clone)]
struct DelegateDisplay {
    depth: usize,
    label: DelegateDisplayLabel,
}

#[derive(Clone)]
struct ChildCompletionSummary {
    #[allow(dead_code)]
    child_conversation_id: String,
    label: DelegateDisplayLabel,
    hint: Option<String>,
    output: Option<String>,
    #[allow(dead_code)]
    mode: DelegateSessionMode,
}

#[derive(Clone)]
enum ChildSummary {
    Completion(ChildCompletionSummary),
    Failure {
        #[allow(dead_code)]
        child_conversation_id: String,
        label: DelegateDisplayLabel,
        error: String,
        #[allow(dead_code)]
        mode: DelegateSessionMode,
    },
}

impl DelegateTree {
    fn insert(
        &mut self,
        run_id: String,
        agent_id: AgentId,
        parent: Option<String>,
    ) -> DelegateDisplay {
        if let Some(parent_id) = parent.as_ref() {
            if let Some(parent_node) = self.nodes.get_mut(parent_id) {
                parent_node.children.push(run_id.clone());
            }
        } else {
            self.roots.push(run_id.clone());
        }

        self.nodes.insert(
            run_id.clone(),
            DelegateNode {
                agent_id: agent_id.clone(),
                parent: parent.clone(),
                children: Vec::new(),
            },
        );

        self.display_for(&run_id, &agent_id)
    }

    fn display_for(&self, run_id: &str, agent_id: &AgentId) -> DelegateDisplay {
        let depth = self.depth_of(run_id).unwrap_or(0);
        let base_label = if depth == 0 {
            format!("↳ #{}", agent_id.as_str())
        } else {
            let indent = "  ".repeat(depth);
            format!("{indent}↳ #{}", agent_id.as_str())
        };
        DelegateDisplay {
            depth,
            label: DelegateDisplayLabel { depth, base_label },
        }
    }

    fn depth_of(&self, run_id: &str) -> Option<usize> {
        let mut depth = 0;
        let mut current = run_id;
        while let Some(node) = self.nodes.get(current) {
            if let Some(parent) = node.parent.as_ref() {
                depth += 1;
                current = parent;
            } else {
                break;
            }
        }
        if self.nodes.contains_key(run_id) || self.roots.iter().any(|r| r == run_id) {
            Some(depth)
        } else {
            None
        }
    }

    fn remove(&mut self, run_id: &str) {
        if let Some(node) = self.nodes.remove(run_id) {
            if let Some(parent_id) = node.parent {
                if let Some(parent_node) = self.nodes.get_mut(&parent_id) {
                    parent_node.children.retain(|child| child != run_id);
                }
            } else {
                self.roots.retain(|root| root != run_id);
            }
        }
    }

    fn first_active_root(&self) -> Option<(String, AgentId)> {
        for run_id in &self.roots {
            if let Some(node) = self.nodes.get(run_id) {
                return Some((run_id.clone(), node.agent_id.clone()));
            }
        }
        None
    }
}

impl App {
    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        tui: &mut tui::Tui,
        auth_manager: Arc<AuthManager>,
        delegate_orchestrator: Arc<AgentOrchestrator>,
        config: Config,
        active_profile: Option<String>,
        initial_prompt: Option<String>,
        initial_images: Vec<PathBuf>,
        resume_selection: ResumeSelection,
        feedback: codex_feedback::CodexFeedback,
    ) -> Result<AppExitInfo> {
        use tokio_stream::StreamExt;
        let (app_event_tx, mut app_event_rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(app_event_tx);

        let delegate_adapter = delegate_tool_adapter(delegate_orchestrator.clone());
        let mut delegate_event_rx = delegate_orchestrator.subscribe().await;
        let delegate_app_event_tx = app_event_tx.clone();
        tokio::spawn(async move {
            while let Some(event) = delegate_event_rx.recv().await {
                delegate_app_event_tx.send(AppEvent::DelegateUpdate(event));
            }
        });

        let conversation_manager = Arc::new(ConversationManager::with_delegate(
            auth_manager.clone(),
            SessionSource::Cli,
            Some(delegate_adapter.clone()),
        ));

        let enhanced_keys_supported = tui.enhanced_keys_supported();

        let ChatWidgetSession {
            widget: mut chat_widget,
            conversation_id: primary_conversation_id,
            event_rx: primary_event_rx,
        } = match resume_selection {
            ResumeSelection::StartFresh | ResumeSelection::Exit => {
                let init = crate::chatwidget::ChatWidgetInit {
                    config: config.clone(),
                    frame_requester: tui.frame_requester(),
                    app_event_tx: app_event_tx.scoped(),
                    initial_prompt: initial_prompt.clone(),
                    initial_images: initial_images.clone(),
                    enhanced_keys_supported,
                    auth_manager: auth_manager.clone(),
                    feedback: feedback.clone(),
                };
                ChatWidget::new_session(init, conversation_manager.clone()).await?
            }
            ResumeSelection::Resume(path) => {
                let resumed = conversation_manager
                    .resume_conversation_from_rollout(
                        config.clone(),
                        path.clone(),
                        auth_manager.clone(),
                    )
                    .await
                    .wrap_err_with(|| {
                        format!("Failed to resume session from {}", path.display())
                    })?;
                let init = crate::chatwidget::ChatWidgetInit {
                    config: config.clone(),
                    frame_requester: tui.frame_requester(),
                    app_event_tx: app_event_tx.scoped(),
                    initial_prompt: initial_prompt.clone(),
                    initial_images: initial_images.clone(),
                    enhanced_keys_supported,
                    auth_manager: auth_manager.clone(),
                    feedback: feedback.clone(),
                };
                ChatWidget::new_session_from_existing(
                    init,
                    resumed.conversation,
                    resumed.session_configured,
                )
            }
        };

        let primary_session_id = primary_conversation_id.to_string();
        chat_widget.ensure_conversation_id(&primary_session_id);

        spawn_event_forwarder(
            app_event_tx.clone(),
            primary_conversation_id,
            primary_event_rx,
        );

        let mut sessions = HashMap::new();
        sessions.insert(
            primary_session_id.clone(),
            SessionHandle::new(chat_widget, None, DelegateSessionMode::Standard, None),
        );

        let file_search = FileSearchManager::new(config.cwd.clone(), app_event_tx.clone());

        let mut app = Self {
            server: conversation_manager,
            app_event_tx,
            sessions,
            active_session_id: primary_session_id.clone(),
            primary_session_id,
            auth_manager: auth_manager.clone(),
            delegate_orchestrator,
            config,
            active_profile,
            file_search,
            enhanced_keys_supported,
            transcript_cells: Vec::new(),
            overlay: None,
            deferred_history_lines: Vec::new(),
            has_emitted_history_lines: false,
            commit_anim_running: Arc::new(AtomicBool::new(false)),
            backtrack: BacktrackState::default(),
            feedback: feedback.clone(),
            pending_update_action: None,
            run_parent_map: HashMap::new(),
        };

        let tui_events = tui.event_stream();
        tokio::pin!(tui_events);

        tui.frame_requester().schedule_frame();

        while select! {
            Some(event) = app_event_rx.recv() => {
                app.handle_event(tui, event).await?
            }
            Some(event) = tui_events.next() => {
                app.handle_tui_event(tui, event).await?
            }
        } {}
        tui.terminal.clear()?;
        Ok(AppExitInfo {
            token_usage: app.token_usage(),
            conversation_id: app.active_widget().and_then(ChatWidget::conversation_id),
            update_action: app.pending_update_action,
        })
    }

    pub(crate) async fn handle_tui_event(
        &mut self,
        tui: &mut tui::Tui,
        event: TuiEvent,
    ) -> Result<bool> {
        if self.overlay.is_some() {
            let _ = self.handle_backtrack_overlay_event(tui, event).await?;
        } else {
            match event {
                TuiEvent::Key(key_event) => {
                    self.handle_key_event(tui, key_event).await;
                }
                TuiEvent::Paste(pasted) => {
                    // Many terminals convert newlines to \r when pasting (e.g., iTerm2),
                    // but tui-textarea expects \n. Normalize CR to LF.
                    // [tui-textarea]: https://github.com/rhysd/tui-textarea/blob/4d18622eeac13b309e0ff6a55a46ac6706da68cf/src/textarea.rs#L782-L783
                    // [iTerm2]: https://github.com/gnachman/iTerm2/blob/5d0c0d9f68523cbd0494dad5422998964a2ecd8d/sources/iTermPasteHelper.m#L206-L216
                    let pasted = pasted.replace("\r", "\n");
                    self.active_widget_mut().handle_paste(pasted);
                }
                TuiEvent::Draw => {
                    self.active_widget_mut()
                        .maybe_post_pending_notification(tui);
                    if self
                        .active_widget_mut()
                        .handle_paste_burst_tick(tui.frame_requester())
                    {
                        return Ok(true);
                    }
                    tui.draw(
                        self.active_widget()
                            .expect("active widget")
                            .desired_height(tui.terminal.size()?.width),
                        |frame| {
                            let widget = self.active_widget().expect("active widget");
                            frame.render_widget_ref(widget, frame.area());
                            if let Some((x, y)) = widget.cursor_pos(frame.area()) {
                                frame.set_cursor_position((x, y));
                            }
                        },
                    )?;
                }
            }
        }
        Ok(true)
    }

    pub(crate) fn active_widget(&self) -> Option<&ChatWidget> {
        self.sessions
            .get(&self.active_session_id)
            .map(SessionHandle::widget)
    }

    pub(crate) fn active_widget_mut(&mut self) -> &mut ChatWidget {
        self.sessions
            .get_mut(&self.active_session_id)
            .expect("active session handle")
            .widget_mut()
    }

    fn render_history_cell(&mut self, cell: Arc<dyn HistoryCell>, tui: &mut tui::Tui) {
        if let Some(Overlay::Transcript(t)) = &mut self.overlay {
            t.insert_cell(cell.clone());
            tui.frame_requester().schedule_frame();
        }
        self.transcript_cells.push(cell.clone());
        let mut display = cell.display_lines(tui.terminal.last_known_screen_size.width);
        if !display.is_empty() {
            if !cell.is_stream_continuation() {
                if self.has_emitted_history_lines {
                    display.insert(0, Line::from(""));
                } else {
                    self.has_emitted_history_lines = true;
                }
            }
            if self.overlay.is_some() {
                self.deferred_history_lines.extend(display);
            } else {
                tui.insert_history_lines(display);
            }
        }
    }

    pub(crate) fn apply_active_history_from_handle(&mut self) {
        if let Some(handle) = self.sessions.get(&self.active_session_id) {
            self.transcript_cells = handle.history().to_vec();
            self.has_emitted_history_lines = !self.transcript_cells.is_empty();
        } else {
            self.transcript_cells.clear();
            self.has_emitted_history_lines = false;
        }
        self.deferred_history_lines.clear();
    }

    pub(crate) fn sync_active_handle_history(&mut self) {
        if let Some(handle) = self.sessions.get_mut(&self.active_session_id) {
            handle.set_history(self.transcript_cells.clone());
        }
    }

    fn replay_active_session_from_last_user(&mut self, tui: &mut tui::Tui) {
        let session_id = self.active_session_id.clone();
        let width = tui.terminal.last_known_screen_size.width;

        {
            let Some(handle) = self.sessions.get_mut(&session_id) else {
                return;
            };
            let header_label = handle
                .summary
                .as_ref()
                .map(|summary| format!("#{}", summary.agent_id.as_str()))
                .unwrap_or_else(|| "#main".to_string());

            let header = format!("Attached to {header_label} (shadow snapshot)");
            handle.widget_mut().add_info_message(header, None);
            self.transcript_cells = handle.history().to_vec();
            self.has_emitted_history_lines = !self.transcript_cells.is_empty();
        }

        let Some(handle) = self.sessions.get(&session_id) else {
            return;
        };
        let history = handle.history();
        if history.is_empty() {
            return;
        }

        let replay_end = history.len().saturating_sub(1);
        if replay_end == 0 {
            return;
        }

        let mut start_idx = 0;
        for idx in (0..replay_end).rev() {
            if history[idx]
                .as_any()
                .downcast_ref::<UserHistoryCell>()
                .is_some()
            {
                start_idx = idx;
                break;
            }
        }

        for cell in &history[start_idx..replay_end] {
            let mut display = cell.display_lines(width);
            if display.is_empty() {
                continue;
            }
            if !cell.is_stream_continuation() {
                if self.has_emitted_history_lines {
                    display.insert(0, Line::from(""));
                } else {
                    self.has_emitted_history_lines = true;
                }
            }
            tui.insert_history_lines(display);
        }

        self.flush_pending_child_summaries(&session_id);
    }

    fn enqueue_child_summary(&mut self, parent_id: &str, summary: ChildSummary) {
        if let Some(parent) = self.sessions.get_mut(parent_id) {
            if parent_id == self.active_session_id {
                Self::render_child_summary_on_widget(parent.widget_mut(), summary);
            } else {
                parent.push_child_summary(summary);
            }
        } else {
            tracing::warn!(
                parent = %parent_id,
                "unable to route delegate summary to parent conversation"
            );
        }
    }

    fn flush_pending_child_summaries(&mut self, session_id: &str) {
        if let Some(handle) = self.sessions.get_mut(session_id) {
            let summaries = handle.drain_child_summaries();
            for summary in summaries {
                Self::render_child_summary_on_widget(handle.widget_mut(), summary);
            }
        }
    }

    fn render_child_summary_on_widget(widget: &mut ChatWidget, summary: ChildSummary) {
        match summary {
            ChildSummary::Completion(data) => {
                let ChildCompletionSummary {
                    child_conversation_id: _,
                    label,
                    hint,
                    output,
                    mode: _,
                } = data;
                widget.add_delegate_completion(output.as_deref(), hint, &label);
            }
            ChildSummary::Failure {
                child_conversation_id: _,
                label,
                error,
                mode: _,
            } => {
                widget.add_error_message(format!("{} failed: {error}", label.base_label));
            }
        }
    }

    async fn handle_event(&mut self, tui: &mut tui::Tui, event: AppEvent) -> Result<bool> {
        match event {
            AppEvent::NewSession => {
                let init = crate::chatwidget::ChatWidgetInit {
                    config: self.config.clone(),
                    frame_requester: tui.frame_requester(),
                    app_event_tx: self.app_event_tx.scoped(),
                    initial_prompt: None,
                    initial_images: Vec::new(),
                    enhanced_keys_supported: self.enhanced_keys_supported,
                    auth_manager: self.auth_manager.clone(),
                    feedback: self.feedback.clone(),
                };
                let mut session = ChatWidget::new_session(init, self.server.clone()).await?;
                let session_conversation_id = session.conversation_id;
                session
                    .widget
                    .ensure_conversation_id(&session_conversation_id.to_string());
                spawn_event_forwarder(
                    self.app_event_tx.clone(),
                    session_conversation_id,
                    session.event_rx,
                );
                self.sessions.insert(
                    self.primary_session_id.clone(),
                    SessionHandle::new(session.widget, None, DelegateSessionMode::Standard, None),
                );
                self.active_session_id = self.primary_session_id.clone();
                self.apply_active_history_from_handle();
                self.replay_active_session_from_last_user(tui);
                self.sync_active_handle_history();
                tui.frame_requester().schedule_frame();
            }
            AppEvent::DelegateUpdate(update) => {
                self.handle_delegate_update(update).await;
            }
            AppEvent::DelegateShadowEvent {
                conversation_id,
                event,
            } => {
                let agent_id = self.agent_id_for_conversation(&conversation_id);
                self.delegate_orchestrator
                    .push_shadow_event(agent_id, &conversation_id, &event)
                    .await;
            }
            AppEvent::DelegateShadowUserInput {
                conversation_id,
                inputs,
            } => {
                let agent_id = self.agent_id_for_conversation(&conversation_id);
                self.delegate_orchestrator
                    .push_shadow_user_inputs(agent_id, &conversation_id, &inputs)
                    .await;
            }
            AppEvent::DelegateShadowAgentOutput {
                conversation_id,
                outputs,
            } => {
                let agent_id = self.agent_id_for_conversation(&conversation_id);
                self.delegate_orchestrator
                    .push_shadow_outputs(agent_id, &conversation_id, &outputs)
                    .await;
            }
            AppEvent::ShowStatus => {
                let metrics = self.delegate_orchestrator.shadow_metrics().await;
                let ma = &self.config.multi_agent;
                let shadow_data = if ma.enable_shadow_cache {
                    Some(StatusShadowData {
                        enabled: true,
                        cached_sessions: metrics.session_count,
                        max_sessions: ma.max_shadow_sessions,
                        total_events: metrics.events,
                        total_user_inputs: metrics.user_inputs,
                        total_agent_outputs: metrics.agent_outputs,
                        total_raw_bytes: metrics.total_bytes,
                        total_compressed_bytes: metrics.total_compressed_bytes,
                        memory_limit_bytes: ma.max_shadow_memory_bytes,
                        compression_enabled: ma.compress_shadows,
                    })
                } else {
                    Some(StatusShadowData {
                        enabled: false,
                        cached_sessions: metrics.session_count,
                        max_sessions: ma.max_shadow_sessions,
                        total_events: metrics.events,
                        total_user_inputs: metrics.user_inputs,
                        total_agent_outputs: metrics.agent_outputs,
                        total_raw_bytes: metrics.total_bytes,
                        total_compressed_bytes: metrics.total_compressed_bytes,
                        memory_limit_bytes: ma.max_shadow_memory_bytes,
                        compression_enabled: ma.compress_shadows,
                    })
                };
                self.active_widget_mut().add_status_output(shadow_data);
            }
            AppEvent::InsertHistoryCell {
                conversation_id,
                cell,
            } => {
                let Some(target_id) = conversation_id else {
                    tracing::error!("received history cell without conversation id; dropping");
                    return Ok(true);
                };

                let cell: Arc<dyn HistoryCell> = cell.into();
                if let Some(handle) = self.sessions.get_mut(&target_id) {
                    handle.push_history(cell.clone());
                    if target_id == self.active_session_id {
                        self.render_history_cell(cell, tui);
                    }
                } else {
                    tracing::warn!(
                        conversation = %target_id,
                        "received history cell for unknown conversation"
                    );
                }
            }
            AppEvent::StartCommitAnimation => {
                if self
                    .commit_anim_running
                    .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    let tx = self.app_event_tx.clone();
                    let running = self.commit_anim_running.clone();
                    thread::spawn(move || {
                        while running.load(Ordering::Relaxed) {
                            thread::sleep(Duration::from_millis(50));
                            tx.send(AppEvent::CommitTick);
                        }
                    });
                }
            }
            AppEvent::StopCommitAnimation => {
                self.commit_anim_running.store(false, Ordering::Release);
            }
            AppEvent::CommitTick => {
                self.active_widget_mut().on_commit_tick();
            }
            AppEvent::CodexEvent {
                conversation_id,
                event,
            } => {
                self.handle_codex_event(&conversation_id, event);
            }
            AppEvent::ConversationHistory(ev) => {
                self.on_conversation_history_for_backtrack(tui, ev).await?;
            }
            AppEvent::ExitRequest => {
                return Ok(false);
            }
            AppEvent::CodexOp(op) => self.active_widget_mut().submit_op(op),
            AppEvent::DiffResult(text) => {
                // Clear the in-progress state in the bottom pane
                self.active_widget_mut().on_diff_complete();
                // Enter alternate screen using TUI helper and build pager lines
                let _ = tui.enter_alt_screen();
                let pager_lines: Vec<ratatui::text::Line<'static>> = if text.trim().is_empty() {
                    vec!["No changes detected.".italic().into()]
                } else {
                    text.lines().map(ansi_escape_line).collect()
                };
                self.overlay = Some(Overlay::new_static_with_lines(
                    pager_lines,
                    "D I F F".to_string(),
                ));
                tui.frame_requester().schedule_frame();
            }
            AppEvent::StartFileSearch(query) => {
                if !query.is_empty() {
                    self.file_search.on_user_query(query);
                }
            }
            AppEvent::FileSearchResult { query, matches } => {
                self.active_widget_mut()
                    .apply_file_search_result(query, matches);
            }
            AppEvent::UpdateReasoningEffort(effort) => {
                self.on_update_reasoning_effort(effort);
            }
            AppEvent::UpdateModel(model) => {
                self.active_widget_mut().set_model(&model);
                self.config.model = model.clone();
                if let Some(family) = find_family_for_model(&model) {
                    self.config.model_family = family;
                }
            }
            AppEvent::OpenReasoningPopup { model, presets } => {
                self.active_widget_mut()
                    .open_reasoning_popup(model, presets);
            }
            AppEvent::OpenFullAccessConfirmation { preset } => {
                self.active_widget_mut()
                    .open_full_access_confirmation(preset);
            }
            AppEvent::PersistModelSelection { model, effort } => {
                let profile = self.active_profile.clone();
                let result = persist_model_selection(
                    &self.config.codex_home,
                    profile.as_deref(),
                    &model,
                    effort,
                )
                .await;

                match result {
                    Ok(()) => {
                        let effort_label = effort
                            .map(|eff| format!(" with {eff} reasoning"))
                            .unwrap_or_else(|| " with default reasoning".to_string());
                        let message = match profile.as_deref() {
                            Some(profile) => format!(
                                "Model changed to {model}{effort_label} for {profile} profile"
                            ),
                            None => format!("Model changed to {model}{effort_label}"),
                        };
                        self.active_widget_mut().add_info_message(message, None);
                    }
                    Err(err) => {
                        tracing::error!(
                            error = %err,
                            "failed to persist model selection"
                        );
                        let message = match profile.as_deref() {
                            Some(profile) => {
                                format!("Failed to save model for profile `{profile}`: {err}")
                            }
                            None => format!("Failed to save default model: {err}"),
                        };
                        self.active_widget_mut().add_error_message(message);
                    }
                }
            }
            AppEvent::UpdateAskForApprovalPolicy(policy) => {
                self.active_widget_mut().set_approval_policy(policy);
            }
            AppEvent::UpdateSandboxPolicy(policy) => {
                self.active_widget_mut().set_sandbox_policy(policy);
            }
            AppEvent::UpdateFullAccessWarningAcknowledged(ack) => {
                self.active_widget_mut()
                    .set_full_access_warning_acknowledged(ack);
            }
            AppEvent::PersistFullAccessWarningAcknowledged => {
                if let Err(err) = set_hide_full_access_warning(&self.config.codex_home, true) {
                    tracing::error!(
                        error = %err,
                        "failed to persist full access warning acknowledgement"
                    );
                    self.active_widget_mut().add_error_message(format!(
                        "Failed to save full access confirmation preference: {err}"
                    ));
                }
            }
            AppEvent::OpenApprovalsPopup => {
                self.active_widget_mut().open_approvals_popup();
            }
            AppEvent::OpenDelegatePicker => {
                let sessions = self.delegate_orchestrator.active_sessions().await;
                let detached_runs: Vec<DetachedRunSummary> =
                    self.delegate_orchestrator.detached_runs().await;
                let mut picker_sessions = Vec::with_capacity(sessions.len());
                for summary in sessions {
                    let run_id = if summary.mode == DelegateSessionMode::Detached {
                        self.delegate_orchestrator
                            .parent_run_for_conversation(summary.conversation_id.as_str())
                            .await
                    } else {
                        None
                    };
                    let shadow = self
                        .delegate_orchestrator
                        .shadow_session_summary(summary.conversation_id.as_str())
                        .await;
                    picker_sessions.push(crate::chatwidget::DelegatePickerSession {
                        summary,
                        run_id,
                        shadow,
                    });
                }
                let active_delegate_id = if self.active_session_id != self.primary_session_id {
                    Some(self.active_session_id.clone())
                } else {
                    None
                };
                let active_delegate = active_delegate_id.as_deref();
                self.active_widget_mut().open_delegate_picker(
                    picker_sessions,
                    detached_runs,
                    active_delegate,
                );
            }
            AppEvent::EnterDelegateSession(conversation_id) => {
                if let Err(err) = self.activate_delegate_session(tui, conversation_id).await {
                    tracing::error!("failed to enter delegate session: {err}");
                    self.active_widget_mut()
                        .add_error_message(format!("Failed to open delegate: {err}"));
                }
            }
            AppEvent::ExitDelegateSession => {
                if let Err(err) = self.return_to_primary(tui).await {
                    tracing::error!("failed to return to primary agent: {err}");
                    self.active_widget_mut()
                        .add_error_message(format!("Failed to return to main agent: {err}"));
                }
            }
            AppEvent::DismissDetachedRun(run_id) => {
                match self
                    .delegate_orchestrator
                    .dismiss_detached_run(&run_id)
                    .await
                {
                    Ok(()) => self
                        .active_widget_mut()
                        .add_info_message(format!("Dismissed detached run {run_id}"), None),
                    Err(err) => self.active_widget_mut().add_error_message(err),
                }
            }
            AppEvent::PreviewDelegateSession(conversation_id) => {
                match self
                    .delegate_orchestrator
                    .session_summary(&conversation_id)
                    .await
                {
                    Some(summary) => {
                        match self
                            .delegate_orchestrator
                            .recent_messages(&conversation_id, None, 3)
                            .await
                        {
                            Ok(messages) => {
                                self.active_widget_mut()
                                    .show_delegate_preview(&summary, &messages);
                            }
                            Err(err) => {
                                self.active_widget_mut().add_error_message(format!(
                                    "Failed to load session preview: {err}"
                                ));
                            }
                        }
                    }
                    None => {
                        self.active_widget_mut().add_error_message(format!(
                            "Unknown delegate session {conversation_id}"
                        ));
                    }
                }
            }
            AppEvent::DismissDelegateSession(conversation_id) => {
                let label = self
                    .delegate_orchestrator
                    .session_summary(&conversation_id)
                    .await
                    .map(|summary| format!("#{}", summary.agent_id.as_str()))
                    .unwrap_or_else(|| conversation_id.clone());

                match self
                    .delegate_orchestrator
                    .dismiss_session(&conversation_id)
                    .await
                {
                    Ok(()) => {
                        self.active_widget_mut()
                            .add_info_message(format!("Dismissed delegate session {label}"), None);
                    }
                    Err(err) => {
                        self.active_widget_mut()
                            .add_error_message(format!("Failed to dismiss {label}: {err}"));
                    }
                }
            }
            AppEvent::InsertUserTextMessage(text) => {
                self.active_widget_mut().submit_text_message(text);
            }
            AppEvent::OpenReviewBranchPicker(cwd) => {
                self.active_widget_mut()
                    .show_review_branch_picker(&cwd)
                    .await;
            }
            AppEvent::OpenReviewCommitPicker(cwd) => {
                self.active_widget_mut()
                    .show_review_commit_picker(&cwd)
                    .await;
            }
            AppEvent::OpenReviewCustomPrompt => {
                self.active_widget_mut().show_review_custom_prompt();
            }
            AppEvent::FullScreenApprovalRequest(request) => match request {
                ApprovalRequest::ApplyPatch { cwd, changes, .. } => {
                    let _ = tui.enter_alt_screen();
                    let diff_summary = DiffSummary::new(changes, cwd);
                    self.overlay = Some(Overlay::new_static_with_renderables(
                        vec![diff_summary.into()],
                        "P A T C H".to_string(),
                    ));
                }
                ApprovalRequest::Exec { command, .. } => {
                    let _ = tui.enter_alt_screen();
                    let full_cmd = strip_bash_lc_and_escape(&command);
                    let full_cmd_lines = highlight_bash_to_lines(&full_cmd);
                    self.overlay = Some(Overlay::new_static_with_lines(
                        full_cmd_lines,
                        "E X E C".to_string(),
                    ));
                }
            },
        }
        Ok(true)
    }

    async fn handle_delegate_update(&mut self, event: DelegateEvent) {
        match event {
            DelegateEvent::Info {
                agent_id,
                conversation_id,
                message,
            } => {
                if let Some(handle) = self.sessions.get_mut(&conversation_id) {
                    let label = format!("#{}", agent_id.as_str());
                    handle
                        .widget_mut()
                        .add_info_message(format!("{label}: {message}"), None);
                } else {
                    tracing::warn!(
                        agent = %agent_id.as_str(),
                        conversation = %conversation_id,
                        "received delegate info for unknown conversation"
                    );
                }
            }
            other => {
                let (run_id, owner_conversation_id) = match &other {
                    DelegateEvent::Started {
                        run_id,
                        owner_conversation_id,
                        ..
                    }
                    | DelegateEvent::Delta {
                        run_id,
                        owner_conversation_id,
                        ..
                    }
                    | DelegateEvent::Completed {
                        run_id,
                        owner_conversation_id,
                        ..
                    }
                    | DelegateEvent::Failed {
                        run_id,
                        owner_conversation_id,
                        ..
                    } => (run_id.clone(), owner_conversation_id.clone()),
                    DelegateEvent::Info { .. } => unreachable!(),
                };

                let mut parent_forward: Option<(String, ChildSummary)> = None;

                if let Some(handle) = self.sessions.get_mut(&owner_conversation_id) {
                    match other {
                        DelegateEvent::Started {
                            run_id,
                            agent_id,
                            owner_conversation_id: _,
                            prompt,
                            parent_run_id,
                            mode,
                            ..
                        } => {
                            if let Some(parent_run_id) = parent_run_id.as_ref() {
                                if let Some(parent_conversation) = self
                                    .delegate_orchestrator
                                    .owner_conversation_for_run(parent_run_id)
                                    .await
                                {
                                    self.run_parent_map
                                        .insert(run_id.clone(), parent_conversation.clone());
                                    handle.set_parent_id(Some(parent_conversation));
                                }
                            } else {
                                self.run_parent_map.remove(&run_id);
                                handle.set_parent_id(None);
                            }

                            let display = handle.delegate_tree.insert(
                                run_id.clone(),
                                agent_id.clone(),
                                parent_run_id.clone(),
                            );
                            let claim_status =
                                parent_run_id.is_none() && handle.delegate_status_owner.is_none();
                            if claim_status {
                                handle.delegate_status_owner = Some(run_id.clone());
                                handle
                                    .widget_mut()
                                    .set_delegate_status_owner(&run_id, &agent_id);
                            }
                            handle.widget_mut().on_delegate_started(
                                &run_id,
                                &agent_id,
                                &prompt,
                                display.label,
                                claim_status,
                                mode,
                            );
                        }
                        DelegateEvent::Delta {
                            run_id,
                            owner_conversation_id: _,
                            chunk,
                            ..
                        } => {
                            handle.widget_mut().on_delegate_delta(&run_id, &chunk);
                        }
                        DelegateEvent::Completed {
                            run_id,
                            agent_id,
                            owner_conversation_id: _,
                            output,
                            duration,
                            mode,
                            ..
                        } => {
                            let display = handle.delegate_tree.display_for(&run_id, &agent_id);
                            handle.delegate_tree.remove(&run_id);
                            if handle.delegate_status_owner.as_deref() == Some(run_id.as_str()) {
                                handle.delegate_status_owner = None;
                                if let Some((next_run_id, next_agent)) =
                                    handle.delegate_tree.first_active_root()
                                {
                                    handle.delegate_status_owner = Some(next_run_id.clone());
                                    handle
                                        .widget_mut()
                                        .set_delegate_status_owner(&next_run_id, &next_agent);
                                } else {
                                    handle.widget_mut().clear_delegate_status_owner();
                                }
                            }
                            let streamed = handle
                                .widget_mut()
                                .on_delegate_completed(&run_id, &display.label);
                            let hint = Some(format!(
                                "finished in {}",
                                Self::format_delegate_duration(duration)
                            ));
                            let forwarded_output = if display.depth == 0 && !streamed {
                                output.clone()
                            } else {
                                None
                            };
                            let hint_for_widget = hint.clone();
                            handle.widget_mut().add_delegate_completion(
                                forwarded_output.as_deref(),
                                hint_for_widget,
                                &display.label,
                            );
                            if mode == DelegateSessionMode::Detached {
                                handle
                                    .widget_mut()
                                    .notify_detached_completion(&display.label);
                                handle.widget_mut().show_detached_completion_actions(
                                    &agent_id,
                                    &run_id,
                                    output.as_deref(),
                                );
                            }
                            let parent_id = self
                                .run_parent_map
                                .get(&run_id)
                                .cloned()
                                .or_else(|| handle.parent_id().cloned());
                            if let Some(parent_id) = parent_id {
                                parent_forward = Some((
                                    parent_id,
                                    ChildSummary::Completion(ChildCompletionSummary {
                                        child_conversation_id: owner_conversation_id.clone(),
                                        label: display.label.clone(),
                                        hint,
                                        output: forwarded_output,
                                        mode,
                                    }),
                                ));
                            }
                            self.run_parent_map.remove(&run_id);
                        }
                        DelegateEvent::Failed {
                            run_id,
                            agent_id,
                            owner_conversation_id: _,
                            error,
                            mode,
                            ..
                        } => {
                            let display = handle.delegate_tree.display_for(&run_id, &agent_id);
                            handle.delegate_tree.remove(&run_id);
                            if handle.delegate_status_owner.as_deref() == Some(run_id.as_str()) {
                                handle.delegate_status_owner = None;
                                if let Some((next_run_id, next_agent)) =
                                    handle.delegate_tree.first_active_root()
                                {
                                    handle.delegate_status_owner = Some(next_run_id.clone());
                                    handle
                                        .widget_mut()
                                        .set_delegate_status_owner(&next_run_id, &next_agent);
                                } else {
                                    handle.widget_mut().clear_delegate_status_owner();
                                }
                            }
                            handle
                                .widget_mut()
                                .on_delegate_failed(&run_id, &display.label, &error);
                            if mode == DelegateSessionMode::Detached {
                                handle
                                    .widget_mut()
                                    .notify_detached_failure(&display.label, &error);
                            }
                            let parent_id = self
                                .run_parent_map
                                .get(&run_id)
                                .cloned()
                                .or_else(|| handle.parent_id().cloned());
                            if let Some(parent_id) = parent_id {
                                parent_forward = Some((
                                    parent_id,
                                    ChildSummary::Failure {
                                        child_conversation_id: owner_conversation_id.clone(),
                                        label: display.label.clone(),
                                        error: error.clone(),
                                        mode,
                                    },
                                ));
                            }
                            self.run_parent_map.remove(&run_id);
                        }
                        DelegateEvent::Info { .. } => unreachable!(),
                    }
                } else {
                    tracing::warn!(
                        run_id = %run_id,
                        conversation = %owner_conversation_id,
                        "received delegate event for unknown conversation"
                    );
                    return;
                }

                if let Some((parent_id, summary)) = parent_forward {
                    self.enqueue_child_summary(&parent_id, summary);
                }
            }
        }
    }

    async fn activate_delegate_session(
        &mut self,
        tui: &mut tui::Tui,
        conversation_id: String,
    ) -> Result<(), String> {
        if self.active_session_id == conversation_id {
            return Ok(());
        }

        self.sync_active_handle_history();
        self.active_widget_mut().set_delegate_context(None);

        let active = self
            .delegate_orchestrator
            .enter_session(&conversation_id)
            .await
            .map_err(|err| format!("{err}"))?;

        self.activate_delegate_session_from_active(Some(tui), conversation_id, active)
            .await
    }

    async fn activate_delegate_session_from_active(
        &mut self,
        mut tui: Option<&mut tui::Tui>,
        conversation_id: String,
        active: ActiveDelegateSession,
    ) -> Result<(), String> {
        let ActiveDelegateSession {
            summary,
            conversation,
            session_configured,
            config,
            event_rx,
            shadow_snapshot,
            shadow_summary,
        } = active;

        let mut event_rx = Some(event_rx);

        use Entry::*;
        match self.sessions.entry(conversation_id.clone()) {
            Occupied(mut occ) => {
                let handle = occ.get_mut();
                handle.widget.ensure_conversation_id(&conversation_id);
                handle.set_summary(Some(summary.clone()));
                handle.set_mode(summary.mode);
                handle.set_shadow_summary(shadow_summary.clone());
                handle.widget.set_delegate_context(Some(summary.clone()));
                if let Some(snapshot) = shadow_snapshot.as_ref() {
                    handle.set_history(Vec::new());
                    handle.widget.hydrate_from_shadow(snapshot);
                } else {
                    handle.widget.clear_shadow_capture();
                    handle.widget.add_info_message(
                        "Shadow cache unavailable; replaying from rollout.".to_string(),
                        None,
                    );
                }
                drop(occ);
                if let Some(rx) = event_rx.take() {
                    drop(rx);
                }
            }
            Vacant(vacant) => {
                #[allow(unused_mut)]
                let frame_requester = match tui.as_mut() {
                    Some(tui_ref) => tui_ref.frame_requester(),
                    None => {
                        #[cfg(test)]
                        {
                            crate::tui::FrameRequester::test_dummy()
                        }
                        #[cfg(not(test))]
                        {
                            unreachable!("delegate session activation requires tui");
                        }
                    }
                };
                let init = ChatWidgetInit {
                    config: config.clone(),
                    frame_requester,
                    app_event_tx: self.app_event_tx.scoped(),
                    initial_prompt: None,
                    initial_images: Vec::new(),
                    enhanced_keys_supported: self.enhanced_keys_supported,
                    auth_manager: self.auth_manager.clone(),
                    feedback: self.feedback.clone(),
                };
                let mut session = ChatWidget::new_session_from_existing_with_events(
                    init,
                    conversation.clone(),
                    session_configured.clone(),
                    event_rx
                        .take()
                        .expect("delegate session event receiver consumed"),
                );
                session.widget.ensure_conversation_id(&conversation_id);
                session.widget.set_delegate_context(Some(summary.clone()));
                if let Some(snapshot) = shadow_snapshot.as_ref() {
                    session.widget.hydrate_from_shadow(snapshot);
                } else {
                    session.widget.clear_shadow_capture();
                    session.widget.add_info_message(
                        "Shadow cache unavailable; replaying from rollout.".to_string(),
                        None,
                    );
                }
                spawn_event_forwarder(
                    self.app_event_tx.clone(),
                    session.conversation_id,
                    session.event_rx,
                );
                vacant.insert(SessionHandle::new(
                    session.widget,
                    Some(summary.clone()),
                    summary.mode,
                    shadow_summary.clone(),
                ));
            }
        }

        let parent_conversation_id = if let Some(parent_run) = self
            .delegate_orchestrator
            .parent_run_for_conversation(&conversation_id)
            .await
        {
            self.delegate_orchestrator
                .owner_conversation_for_run(&parent_run)
                .await
        } else {
            None
        };

        if let Some(handle) = self.sessions.get_mut(&conversation_id) {
            handle.set_parent_id(parent_conversation_id.clone());
        }
        if let Some(parent_id) = parent_conversation_id.as_ref() {
            if let Some(parent) = self.sessions.get_mut(parent_id) {
                parent.add_child(conversation_id.clone());
            }
        }

        self.active_session_id = conversation_id.clone();
        if let Some(handle) = self.sessions.get_mut(&self.active_session_id)
            && let Some(summary) = handle.summary.clone()
        {
            handle.widget.set_delegate_context(Some(summary));
        }
        self.apply_active_history_from_handle();
        if let Some(tui_ref) = tui.as_mut() {
            self.replay_active_session_from_last_user(tui_ref);
        }
        self.sync_active_handle_history();
        self.delegate_orchestrator
            .touch_session(&conversation_id)
            .await;
        if let Some(tui_ref) = tui {
            tui_ref.frame_requester().schedule_frame();
        }
        Ok(())
    }

    #[cfg(test)]
    async fn activate_delegate_session_with_active(
        &mut self,
        conversation_id: String,
        active: ActiveDelegateSession,
    ) -> Result<(), String> {
        if self.active_session_id == conversation_id {
            return Ok(());
        }

        self.sync_active_handle_history();
        self.active_widget_mut().set_delegate_context(None);
        self.activate_delegate_session_from_active(None, conversation_id, active)
            .await
    }

    fn agent_id_for_conversation(&self, conversation_id: &str) -> Option<&AgentId> {
        self.sessions
            .get(conversation_id)
            .and_then(|handle| handle.summary.as_ref().map(|summary| &summary.agent_id))
    }

    fn handle_codex_event(&mut self, conversation_id: &str, event: Event) {
        if let Some(handle) = self.sessions.get_mut(conversation_id) {
            handle.widget.ensure_conversation_id(conversation_id);
            handle.widget.handle_codex_event(event);
        }
    }

    async fn return_to_primary(&mut self, tui: &mut tui::Tui) -> Result<(), String> {
        if self.active_session_id == self.primary_session_id {
            return Ok(());
        }

        self.sync_active_handle_history();

        let active_id = self.active_session_id.clone();
        let capture = if let Some(handle) = self.sessions.get_mut(&active_id) {
            if let Some(summary) = handle.summary_mut() {
                summary.last_interacted_at = SystemTime::now();
            }
            handle.widget.take_delegate_capture()
        } else {
            None
        };

        self.active_session_id = self.primary_session_id.clone();
        self.apply_active_history_from_handle();
        self.replay_active_session_from_last_user(tui);
        if let Some(primary) = self.sessions.get_mut(&self.primary_session_id) {
            primary.widget.set_delegate_context(None);
        }

        if let Some(handle) = self.sessions.get_mut(&active_id)
            && let Some(summary) = handle.summary.clone()
        {
            handle.widget.set_delegate_context(Some(summary.clone()));
            if let Some(capture) = capture
                && let Some(primary) = self.sessions.get_mut(&self.primary_session_id)
            {
                primary.widget.apply_delegate_summary(&summary, capture);
            }
        }

        self.delegate_orchestrator.touch_session(&active_id).await;
        self.sync_active_handle_history();
        tui.frame_requester().schedule_frame();
        Ok(())
    }

    fn format_delegate_duration(duration: Duration) -> String {
        if duration.as_secs() >= 60 {
            let mins = duration.as_secs() / 60;
            let secs = duration.as_secs() % 60;
            format!("{mins}m{secs:02}s")
        } else if duration.as_millis() >= 1000 {
            format!("{:.1}s", duration.as_secs_f32())
        } else {
            format!("{:.0}ms", duration.as_millis())
        }
    }

    pub(crate) fn token_usage(&self) -> codex_core::protocol::TokenUsage {
        self.active_widget()
            .map(ChatWidget::token_usage)
            .unwrap_or_default()
    }

    fn on_update_reasoning_effort(&mut self, effort: Option<ReasoningEffortConfig>) {
        self.active_widget_mut().set_reasoning_effort(effort);
        self.config.model_reasoning_effort = effort;
    }

    async fn handle_key_event(&mut self, tui: &mut tui::Tui, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Char('t'),
                modifiers: crossterm::event::KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            } => {
                // Enter alternate screen and set viewport to full size.
                let _ = tui.enter_alt_screen();
                self.overlay = Some(Overlay::new_transcript(self.transcript_cells.clone()));
                tui.frame_requester().schedule_frame();
            }
            // Esc primes/advances backtracking only in normal (not working) mode
            // with the composer focused and empty. In any other state, forward
            // Esc so the active UI (e.g. status indicator, modals, popups)
            // handles it.
            KeyEvent {
                code: KeyCode::Esc,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                if self
                    .active_widget()
                    .is_some_and(|w| w.is_normal_backtrack_mode() && w.composer_is_empty())
                {
                    self.handle_backtrack_esc_key(tui);
                } else {
                    self.active_widget_mut().handle_key_event(key_event);
                }
            }
            // Enter confirms backtrack when primed + count > 0. Otherwise pass to widget.
            KeyEvent {
                code: KeyCode::Enter,
                kind: KeyEventKind::Press,
                ..
            } if self.backtrack.primed
                && self.backtrack.nth_user_message != usize::MAX
                && self
                    .active_widget()
                    .is_some_and(super::chatwidget::ChatWidget::composer_is_empty) =>
            {
                // Delegate to helper for clarity; preserves behavior.
                self.confirm_backtrack_from_main();
            }
            KeyEvent {
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                // Any non-Esc key press should cancel a primed backtrack.
                // This avoids stale "Esc-primed" state after the user starts typing
                // (even if they later backspace to empty).
                if key_event.code != KeyCode::Esc && self.backtrack.primed {
                    self.reset_backtrack_state();
                }
                self.active_widget_mut().handle_key_event(key_event);
            }
            _ => {
                // Ignore Release key events.
            }
        };
    }
}

pub(crate) struct SessionHandle {
    widget: ChatWidget,
    summary: Option<DelegateSessionSummary>,
    mode: DelegateSessionMode,
    history: Vec<Arc<dyn HistoryCell>>,
    shadow: Option<ShadowSessionSummary>,
    delegate_tree: DelegateTree,
    delegate_status_owner: Option<String>,
    parent_conversation_id: Option<String>,
    #[allow(dead_code)]
    child_conversations: HashSet<String>,
    pending_child_summaries: VecDeque<ChildSummary>,
}

impl SessionHandle {
    fn new(
        widget: ChatWidget,
        summary: Option<DelegateSessionSummary>,
        mode: DelegateSessionMode,
        shadow: Option<ShadowSessionSummary>,
    ) -> Self {
        Self {
            widget,
            summary,
            mode,
            history: Vec::new(),
            shadow,
            delegate_tree: DelegateTree::default(),
            delegate_status_owner: None,
            parent_conversation_id: None,
            child_conversations: HashSet::new(),
            pending_child_summaries: VecDeque::new(),
        }
    }

    fn summary_mut(&mut self) -> Option<&mut DelegateSessionSummary> {
        self.summary.as_mut()
    }

    pub(crate) fn replace(
        &mut self,
        widget: ChatWidget,
        summary: Option<DelegateSessionSummary>,
        mode: DelegateSessionMode,
        history: Option<Vec<Arc<dyn HistoryCell>>>,
        shadow: Option<ShadowSessionSummary>,
    ) {
        self.widget = widget;
        self.summary = summary;
        self.mode = mode;
        if let Some(history) = history {
            self.history = history;
        }
        self.shadow = shadow;
    }

    pub(crate) fn widget(&self) -> &ChatWidget {
        &self.widget
    }

    pub(crate) fn widget_mut(&mut self) -> &mut ChatWidget {
        &mut self.widget
    }

    pub(crate) fn push_history(&mut self, cell: Arc<dyn HistoryCell>) {
        self.history.push(cell);
    }

    pub(crate) fn set_history(&mut self, history: Vec<Arc<dyn HistoryCell>>) {
        self.history = history;
    }

    pub(crate) fn history(&self) -> &[Arc<dyn HistoryCell>] {
        &self.history
    }

    pub(crate) fn set_summary(&mut self, summary: Option<DelegateSessionSummary>) {
        self.summary = summary;
    }

    pub(crate) fn set_mode(&mut self, mode: DelegateSessionMode) {
        self.mode = mode;
    }

    pub(crate) fn set_shadow_summary(&mut self, shadow: Option<ShadowSessionSummary>) {
        self.shadow = shadow;
    }

    #[allow(dead_code)]
    pub(crate) fn parent_id(&self) -> Option<&String> {
        self.parent_conversation_id.as_ref()
    }

    pub(crate) fn set_parent_id(&mut self, parent: Option<String>) {
        self.parent_conversation_id = parent;
    }

    pub(crate) fn add_child(&mut self, conversation_id: String) {
        self.child_conversations.insert(conversation_id);
    }

    #[allow(dead_code)]
    pub(crate) fn remove_child(&mut self, conversation_id: &str) {
        self.child_conversations.remove(conversation_id);
    }

    #[allow(dead_code)]
    pub(crate) fn child_conversations(&self) -> impl Iterator<Item = &String> {
        self.child_conversations.iter()
    }

    fn push_child_summary(&mut self, summary: ChildSummary) {
        self.pending_child_summaries.push_back(summary);
    }

    fn drain_child_summaries(&mut self) -> Vec<ChildSummary> {
        self.pending_child_summaries.drain(..).collect()
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn expect_unique_session_configured(
    session_configured: Arc<SessionConfiguredEvent>,
) -> SessionConfiguredEvent {
    Arc::unwrap_or_clone(session_configured)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_backtrack::BacktrackState;
    use crate::app_backtrack::user_count;
    use crate::app_event::AppEvent;
    use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
    use crate::file_search::FileSearchManager;
    use crate::history_cell;
    use crate::history_cell::AgentMessageCell;
    use crate::history_cell::HistoryCell;
    use crate::history_cell::UserHistoryCell;
    use crate::history_cell::new_session_info;

    use codex_common::CliConfigOverrides;
    use codex_core::AuthManager;
    use codex_core::CodexAuth;
    use codex_core::ConversationManager;
    use codex_core::config::ConfigOverrides;

    use codex_core::protocol::SessionConfiguredEvent;
    use codex_core::protocol::SessionSource;

    use codex_core::protocol::Event;
    use codex_core::protocol::EventMsg;
    use codex_core::protocol::InputItem;
    use codex_core::protocol::InputMessageKind;
    use codex_core::protocol::UserMessageEvent;
    use codex_protocol::ConversationId;
    use ratatui::prelude::Line;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::time::SystemTime;

    fn make_test_app() -> App {
        let (app, _rx) = make_test_app_with_receiver();
        app
    }

    fn make_test_app_with_receiver() -> (App, tokio::sync::mpsc::UnboundedReceiver<AppEvent>) {
        let (mut chat_widget, app_event_tx, rx, _op_rx) = make_chatwidget_manual_with_sender();
        let config = chat_widget.config_ref().clone();
        let session_id = ConversationId::new().to_string();
        chat_widget.ensure_conversation_id(&session_id);

        let server = Arc::new(ConversationManager::with_auth(CodexAuth::from_api_key(
            "Test API Key",
        )));
        let auth_manager =
            AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
        let file_search = FileSearchManager::new(config.cwd.clone(), app_event_tx.clone());
        let delegate_orchestrator = Arc::new(AgentOrchestrator::new(
            config.codex_home.clone(),
            auth_manager.clone(),
            SessionSource::Cli,
            CliConfigOverrides::default(),
            ConfigOverrides {
                model: None,
                review_model: None,
                cwd: None,
                approval_policy: None,
                sandbox_mode: None,
                model_provider: None,
                config_profile: None,
                codex_linux_sandbox_exe: None,
                base_instructions: None,
                include_plan_tool: None,
                include_delegate_tool: None,
                include_apply_patch_tool: None,
                include_view_image_tool: None,
                show_raw_agent_reasoning: None,
                tools_web_search_request: None,
            },
            Vec::new(),
            config.multi_agent.max_concurrent_delegates,
            ShadowConfig::disabled(),
        ));

        let mut sessions = HashMap::new();
        sessions.insert(
            session_id.clone(),
            SessionHandle::new(chat_widget, None, DelegateSessionMode::Standard, None),
        );

        (
            App {
                server,
                app_event_tx,
                sessions,
                active_session_id: session_id.clone(),
                primary_session_id: session_id,
                auth_manager,
                delegate_orchestrator,
                config,
                active_profile: None,
                file_search,
                transcript_cells: Vec::new(),
                overlay: None,
                deferred_history_lines: Vec::new(),
                has_emitted_history_lines: false,
                enhanced_keys_supported: false,
                commit_anim_running: Arc::new(AtomicBool::new(false)),
                backtrack: BacktrackState::default(),
                feedback: codex_feedback::CodexFeedback::new(),
                pending_update_action: None,
                run_parent_map: HashMap::new(),
            },
            rx,
        )
    }

    #[test]
    fn update_reasoning_effort_updates_config() {
        let mut app = make_test_app();
        app.config.model_reasoning_effort = Some(ReasoningEffortConfig::Medium);
        app.sessions
            .get_mut(&app.active_session_id)
            .unwrap()
            .widget
            .set_reasoning_effort(Some(ReasoningEffortConfig::Medium));

        app.on_update_reasoning_effort(Some(ReasoningEffortConfig::High));

        assert_eq!(
            app.config.model_reasoning_effort,
            Some(ReasoningEffortConfig::High)
        );
        assert_eq!(
            app.sessions
                .get(&app.active_session_id)
                .unwrap()
                .widget
                .config_ref()
                .model_reasoning_effort,
            Some(ReasoningEffortConfig::High)
        );
    }

    #[tokio::test]
    async fn delegate_events_route_to_owner_only() {
        let mut app = make_test_app();

        let child_conversation_id = "child".to_string();
        let parent_conversation_id = app.active_session_id.clone();

        let (mut child_widget, _, _, _) = make_chatwidget_manual_with_sender();
        child_widget.ensure_conversation_id(&child_conversation_id);
        let child_handle =
            SessionHandle::new(child_widget, None, DelegateSessionMode::Standard, None);
        app.sessions
            .insert(child_conversation_id.clone(), child_handle);

        let started = DelegateEvent::Started {
            run_id: "run-1".to_string(),
            agent_id: AgentId::parse("critic").unwrap(),
            owner_conversation_id: child_conversation_id.clone(),
            prompt: "prompt".to_string(),
            started_at: SystemTime::now(),
            parent_run_id: None,
            mode: DelegateSessionMode::Standard,
        };

        app.handle_delegate_update(started).await;

        assert!(
            app.sessions
                .get(&parent_conversation_id)
                .unwrap()
                .pending_child_summaries
                .is_empty()
        );
        assert!(
            app.sessions
                .get(&child_conversation_id)
                .unwrap()
                .pending_child_summaries
                .is_empty()
        );
        assert!(app.run_parent_map.is_empty());
    }

    #[tokio::test]
    async fn child_completion_bubbles_to_parent() {
        let mut app = make_test_app();

        let child_conversation_id = "child".to_string();
        let parent_conversation_id = app.active_session_id.clone();

        let (mut child_widget, _, _, _) = make_chatwidget_manual_with_sender();
        child_widget.ensure_conversation_id(&child_conversation_id);
        app.sessions.insert(
            child_conversation_id.clone(),
            SessionHandle::new(child_widget, None, DelegateSessionMode::Standard, None),
        );

        app.active_session_id = child_conversation_id.clone();

        app.run_parent_map
            .insert("run-1".to_string(), parent_conversation_id.clone());

        let completed = DelegateEvent::Completed {
            run_id: "run-1".to_string(),
            agent_id: AgentId::parse("critic").unwrap(),
            owner_conversation_id: child_conversation_id.clone(),
            output: Some("Child output".to_string()),
            duration: Duration::from_secs(2),
            mode: DelegateSessionMode::Standard,
        };

        app.handle_delegate_update(completed).await;

        let parent_handle = app
            .sessions
            .get(&parent_conversation_id)
            .unwrap()
            .pending_child_summaries
            .clone();
        assert_eq!(parent_handle.len(), 1);
        matches!(parent_handle[0], ChildSummary::Completion(_));

        assert!(
            app.sessions
                .get(&child_conversation_id)
                .unwrap()
                .pending_child_summaries
                .is_empty()
        );
        assert!(app.run_parent_map.is_empty());
    }

    #[tokio::test]
    async fn follow_up_session_should_apply_shadow_even_with_existing_history() {
        use crate::tui::FrameRequester;
        use codex_multi_agent::shadow::ShadowHistoryEntry;
        use codex_multi_agent::shadow::ShadowHistoryKind;
        use codex_multi_agent::shadow::ShadowMetrics;
        use codex_multi_agent::shadow::ShadowSnapshot;
        use codex_multi_agent::shadow::ShadowTranscriptCapture;

        let (mut app, mut app_events) = make_test_app_with_receiver();

        let new_conversation = app
            .server
            .new_conversation(app.config.clone())
            .await
            .expect("new conversation");
        let conversation_id = new_conversation.conversation_id.to_string();
        let agent_id = AgentId::parse("critic").unwrap();

        let summary = DelegateSessionSummary {
            conversation_id: conversation_id.clone(),
            agent_id: agent_id.clone(),
            last_interacted_at: SystemTime::now(),
            cwd: app.config.cwd.clone(),
            mode: DelegateSessionMode::Standard,
        };

        let initial_prompt = "How should I carry a box of apples safely?".to_string();
        let follow_up_prompt = "Follow-up: The box weighs 50 kg.".to_string();

        let make_user_event = |id: &str, message: &str| Event {
            id: id.to_string(),
            msg: EventMsg::UserMessage(UserMessageEvent {
                message: message.to_string(),
                kind: Some(InputMessageKind::Plain),
                images: None,
            }),
        };
        let events = vec![
            make_user_event("event-initial", &initial_prompt),
            make_user_event("event-follow-up", &follow_up_prompt),
        ];
        let events_len = events.len();

        let shadow_snapshot = ShadowSnapshot {
            conversation_id: conversation_id.clone(),
            agent_id: agent_id.clone(),
            history: vec![
                ShadowHistoryEntry {
                    kind: ShadowHistoryKind::User,
                    lines: vec![initial_prompt.clone()],
                    is_stream_continuation: false,
                },
                ShadowHistoryEntry {
                    kind: ShadowHistoryKind::User,
                    lines: vec![follow_up_prompt.clone()],
                    is_stream_continuation: false,
                },
            ],
            capture: ShadowTranscriptCapture {
                user_inputs: vec![
                    InputItem::Text {
                        text: initial_prompt.clone(),
                    },
                    InputItem::Text {
                        text: follow_up_prompt.clone(),
                    },
                ],
                agent_outputs: Vec::new(),
            },
            metrics: ShadowMetrics {
                session_count: 1,
                events: events_len,
                user_inputs: 2,
                ..ShadowMetrics::default()
            },
            events,
            recorded_at: SystemTime::now(),
        };

        let init = ChatWidgetInit {
            config: app.config.clone(),
            frame_requester: FrameRequester::test_dummy(),
            app_event_tx: app.app_event_tx.scoped(),
            initial_prompt: None,
            initial_images: Vec::new(),
            enhanced_keys_supported: app.enhanced_keys_supported,
            auth_manager: app.auth_manager.clone(),
            feedback: app.feedback.clone(),
        };
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        drop(event_tx);
        let mut widget_session = ChatWidget::new_session_from_existing_with_events(
            init,
            new_conversation.conversation.clone(),
            Arc::new(new_conversation.session_configured.clone()),
            event_rx,
        );
        widget_session
            .widget
            .ensure_conversation_id(&conversation_id);

        let mut handle = SessionHandle::new(
            widget_session.widget,
            Some(summary.clone()),
            summary.mode,
            None,
        );
        handle.push_history(
            Arc::new(history_cell::new_user_prompt("earlier history".to_string()))
                as Arc<dyn HistoryCell>,
        );
        handle.widget.set_delegate_context(Some(summary.clone()));

        app.sessions.insert(conversation_id.clone(), handle);

        let (shadow_event_tx, shadow_event_rx) = tokio::sync::mpsc::unbounded_channel();
        drop(shadow_event_tx);

        let active = ActiveDelegateSession {
            summary: summary.clone(),
            conversation: new_conversation.conversation.clone(),
            session_configured: Arc::new(new_conversation.session_configured.clone()),
            config: app.config.clone(),
            event_rx: shadow_event_rx,
            shadow_snapshot: Some(shadow_snapshot),
            shadow_summary: None,
        };

        app.activate_delegate_session_with_active(conversation_id.clone(), active)
            .await
            .expect("activate delegate session");

        while let Ok(event) = app_events.try_recv() {
            if let AppEvent::InsertHistoryCell {
                conversation_id: Some(target_id),
                cell,
            } = event
            {
                let cell: Arc<dyn HistoryCell> = cell.into();
                if let Some(handle) = app.sessions.get_mut(&target_id) {
                    handle.push_history(cell);
                }
            }
        }

        let transcript: String = app
            .sessions
            .get(&conversation_id)
            .unwrap()
            .history()
            .iter()
            .flat_map(|cell| cell.display_lines(120))
            .flat_map(|line| line.spans.into_iter())
            .map(|span| span.content.to_string())
            .collect();

        assert!(
            transcript.contains(&follow_up_prompt),
            "transcript missing follow-up prompt: {transcript}"
        );
    }

    #[test]
    fn backtrack_selection_with_duplicate_history_targets_unique_turn() {
        let mut app = make_test_app();

        let user_cell = |text: &str| -> Arc<dyn HistoryCell> {
            Arc::new(UserHistoryCell {
                message: text.to_string(),
            }) as Arc<dyn HistoryCell>
        };
        let agent_cell = |text: &str| -> Arc<dyn HistoryCell> {
            Arc::new(AgentMessageCell::new(
                vec![Line::from(text.to_string())],
                true,
            )) as Arc<dyn HistoryCell>
        };

        let make_header = |is_first| {
            let event = SessionConfiguredEvent {
                session_id: ConversationId::new(),
                model: "gpt-test".to_string(),
                reasoning_effort: None,
                history_log_id: 0,
                history_entry_count: 0,
                initial_messages: None,
                rollout_path: PathBuf::new(),
            };
            Arc::new(new_session_info(
                app.sessions
                    .get(&app.active_session_id)
                    .unwrap()
                    .widget
                    .config_ref(),
                event,
                is_first,
            )) as Arc<dyn HistoryCell>
        };

        // Simulate the transcript after trimming for a fork, replaying history, and
        // appending the edited turn. The session header separates the retained history
        // from the forked conversation's replayed turns.
        app.transcript_cells = vec![
            make_header(true),
            user_cell("first question"),
            agent_cell("answer first"),
            user_cell("follow-up"),
            agent_cell("answer follow-up"),
            make_header(false),
            user_cell("first question"),
            agent_cell("answer first"),
            user_cell("follow-up (edited)"),
            agent_cell("answer edited"),
        ];

        assert_eq!(user_count(&app.transcript_cells), 2);

        app.backtrack.base_id = Some(ConversationId::new());
        app.backtrack.primed = true;
        app.backtrack.nth_user_message = user_count(&app.transcript_cells).saturating_sub(1);

        app.confirm_backtrack_from_main();

        let (_, nth, prefill) = app.backtrack.pending.clone().expect("pending backtrack");
        assert_eq!(nth, 1);
        assert_eq!(prefill, "follow-up (edited)");
    }

    #[test]
    fn expect_unique_session_configured_clones_when_shared() {
        let event = SessionConfiguredEvent {
            session_id: ConversationId::new(),
            model: "gpt-test".to_string(),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            rollout_path: PathBuf::new(),
        };

        let shared = Arc::new(event.clone());
        let _other_owner = Arc::clone(&shared);

        let resolved = expect_unique_session_configured(shared);

        assert_eq!(resolved.model, event.model);
        assert_eq!(resolved.history_log_id, event.history_log_id);
        assert_eq!(resolved.history_entry_count, event.history_entry_count);
    }
}

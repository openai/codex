use crate::UpdateAction;
use crate::app_backtrack::BacktrackState;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::ApprovalRequest;
use crate::chatwidget::ChatWidget;
use crate::chatwidget::ChatWidgetInit;
use crate::chatwidget::DelegateDisplayLabel;
use crate::diff_render::DiffSummary;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::file_search::FileSearchManager;
use crate::history_cell::HistoryCell;
use crate::pager_overlay::Overlay;
use crate::render::highlight::highlight_bash_to_lines;
use crate::resume_picker::ResumeSelection;
use crate::tui;
use crate::tui::TuiEvent;
use codex_ansi_escape::ansi_escape_line;
use codex_core::AuthManager;
use codex_core::ConversationManager;
use codex_core::config::Config;
use codex_core::config::persist_model_selection;
use codex_core::config::set_hide_full_access_warning;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::SessionConfiguredEvent;
use codex_core::protocol::SessionSource;
use codex_core::protocol::TokenUsage;
use codex_core::protocol_config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_multi_agent::AgentId;
use codex_multi_agent::AgentOrchestrator;
use codex_multi_agent::DelegateEvent;
use codex_multi_agent::DelegateSessionMode;
use codex_multi_agent::DelegateSessionSummary;
use codex_multi_agent::DetachedRunSummary;
use codex_multi_agent::delegate_tool_adapter;
use codex_protocol::ConversationId;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use std::time::SystemTime;
use tokio::select;
use tokio::sync::mpsc::unbounded_channel;
// use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AppExitInfo {
    pub token_usage: TokenUsage,
    pub conversation_id: Option<ConversationId>,
    pub update_action: Option<UpdateAction>,
}

pub(crate) struct App {
    pub(crate) server: Arc<ConversationManager>,
    pub(crate) app_event_tx: AppEventSender,
    pub(crate) chat_widget: ChatWidget,
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
    delegate_sessions: HashMap<String, DelegateSessionState>,
    active_delegate: Option<String>,
    active_delegate_summary: Option<DelegateSessionSummary>,
    primary_chat_backup: Option<ChatWidget>,
    /// Set when the user confirms an update; propagated on exit.
    pub(crate) pending_update_action: Option<UpdateAction>,
    delegate_tree: DelegateTree,
    delegate_status_owner: Option<String>,
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

        let chat_widget = match resume_selection {
            ResumeSelection::StartFresh | ResumeSelection::Exit => {
                let init = crate::chatwidget::ChatWidgetInit {
                    config: config.clone(),
                    frame_requester: tui.frame_requester(),
                    app_event_tx: app_event_tx.clone(),
                    initial_prompt: initial_prompt.clone(),
                    initial_images: initial_images.clone(),
                    enhanced_keys_supported,
                    auth_manager: auth_manager.clone(),
                    feedback: feedback.clone(),
                };
                ChatWidget::new(init, conversation_manager.clone())
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
                    app_event_tx: app_event_tx.clone(),
                    initial_prompt: initial_prompt.clone(),
                    initial_images: initial_images.clone(),
                    enhanced_keys_supported,
                    auth_manager: auth_manager.clone(),
                    feedback: feedback.clone(),
                };
                ChatWidget::new_from_existing(
                    init,
                    resumed.conversation,
                    resumed.session_configured,
                )
            }
        };

        let file_search = FileSearchManager::new(config.cwd.clone(), app_event_tx.clone());

        let mut app = Self {
            server: conversation_manager,
            app_event_tx,
            chat_widget,
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
            delegate_sessions: HashMap::new(),
            active_delegate: None,
            active_delegate_summary: None,
            primary_chat_backup: None,
            pending_update_action: None,
            delegate_tree: DelegateTree::default(),
            delegate_status_owner: None,
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
            conversation_id: app.chat_widget.conversation_id(),
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
                    self.chat_widget.handle_paste(pasted);
                }
                TuiEvent::Draw => {
                    self.chat_widget.maybe_post_pending_notification(tui);
                    if self
                        .chat_widget
                        .handle_paste_burst_tick(tui.frame_requester())
                    {
                        return Ok(true);
                    }
                    tui.draw(
                        self.chat_widget.desired_height(tui.terminal.size()?.width),
                        |frame| {
                            frame.render_widget_ref(&self.chat_widget, frame.area());
                            if let Some((x, y)) = self.chat_widget.cursor_pos(frame.area()) {
                                frame.set_cursor_position((x, y));
                            }
                        },
                    )?;
                }
            }
        }
        Ok(true)
    }

    async fn handle_event(&mut self, tui: &mut tui::Tui, event: AppEvent) -> Result<bool> {
        match event {
            AppEvent::NewSession => {
                let init = crate::chatwidget::ChatWidgetInit {
                    config: self.config.clone(),
                    frame_requester: tui.frame_requester(),
                    app_event_tx: self.app_event_tx.clone(),
                    initial_prompt: None,
                    initial_images: Vec::new(),
                    enhanced_keys_supported: self.enhanced_keys_supported,
                    auth_manager: self.auth_manager.clone(),
                    feedback: self.feedback.clone(),
                };
                self.chat_widget = ChatWidget::new(init, self.server.clone());
                tui.frame_requester().schedule_frame();
            }
            AppEvent::DelegateUpdate(update) => {
                self.handle_delegate_update(update);
            }
            AppEvent::InsertHistoryCell(cell) => {
                let cell: Arc<dyn HistoryCell> = cell.into();
                if let Some(Overlay::Transcript(t)) = &mut self.overlay {
                    t.insert_cell(cell.clone());
                    tui.frame_requester().schedule_frame();
                }
                self.transcript_cells.push(cell.clone());
                let mut display = cell.display_lines(tui.terminal.last_known_screen_size.width);
                if !display.is_empty() {
                    // Only insert a separating blank line for new cells that are not
                    // part of an ongoing stream. Streaming continuations should not
                    // accrue extra blank lines between chunks.
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
                self.chat_widget.on_commit_tick();
            }
            AppEvent::CodexEvent(event) => {
                self.chat_widget.handle_codex_event(event);
            }
            AppEvent::ConversationHistory(ev) => {
                self.on_conversation_history_for_backtrack(tui, ev).await?;
            }
            AppEvent::ExitRequest => {
                return Ok(false);
            }
            AppEvent::CodexOp(op) => self.chat_widget.submit_op(op),
            AppEvent::DiffResult(text) => {
                // Clear the in-progress state in the bottom pane
                self.chat_widget.on_diff_complete();
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
                self.chat_widget.apply_file_search_result(query, matches);
            }
            AppEvent::UpdateReasoningEffort(effort) => {
                self.on_update_reasoning_effort(effort);
            }
            AppEvent::UpdateModel(model) => {
                self.chat_widget.set_model(&model);
                self.config.model = model.clone();
                if let Some(family) = find_family_for_model(&model) {
                    self.config.model_family = family;
                }
            }
            AppEvent::OpenReasoningPopup { model, presets } => {
                self.chat_widget.open_reasoning_popup(model, presets);
            }
            AppEvent::OpenFullAccessConfirmation { preset } => {
                self.chat_widget.open_full_access_confirmation(preset);
            }
            AppEvent::PersistModelSelection { model, effort } => {
                let profile = self.active_profile.as_deref();
                match persist_model_selection(&self.config.codex_home, profile, &model, effort)
                    .await
                {
                    Ok(()) => {
                        let effort_label = effort
                            .map(|eff| format!(" with {eff} reasoning"))
                            .unwrap_or_else(|| " with default reasoning".to_string());
                        if let Some(profile) = profile {
                            self.chat_widget.add_info_message(
                                format!(
                                    "Model changed to {model}{effort_label} for {profile} profile"
                                ),
                                None,
                            );
                        } else {
                            self.chat_widget.add_info_message(
                                format!("Model changed to {model}{effort_label}"),
                                None,
                            );
                        }
                    }
                    Err(err) => {
                        tracing::error!(
                            error = %err,
                            "failed to persist model selection"
                        );
                        if let Some(profile) = profile {
                            self.chat_widget.add_error_message(format!(
                                "Failed to save model for profile `{profile}`: {err}"
                            ));
                        } else {
                            self.chat_widget
                                .add_error_message(format!("Failed to save default model: {err}"));
                        }
                    }
                }
            }
            AppEvent::UpdateAskForApprovalPolicy(policy) => {
                self.chat_widget.set_approval_policy(policy);
            }
            AppEvent::UpdateSandboxPolicy(policy) => {
                self.chat_widget.set_sandbox_policy(policy);
            }
            AppEvent::UpdateFullAccessWarningAcknowledged(ack) => {
                self.chat_widget.set_full_access_warning_acknowledged(ack);
            }
            AppEvent::PersistFullAccessWarningAcknowledged => {
                if let Err(err) = set_hide_full_access_warning(&self.config.codex_home, true) {
                    tracing::error!(
                        error = %err,
                        "failed to persist full access warning acknowledgement"
                    );
                    self.chat_widget.add_error_message(format!(
                        "Failed to save full access confirmation preference: {err}"
                    ));
                }
            }
            AppEvent::OpenApprovalsPopup => {
                self.chat_widget.open_approvals_popup();
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
                    picker_sessions
                        .push(crate::chatwidget::DelegatePickerSession { summary, run_id });
                }
                self.chat_widget.open_delegate_picker(
                    picker_sessions,
                    detached_runs,
                    self.active_delegate.as_deref(),
                );
            }
            AppEvent::EnterDelegateSession(conversation_id) => {
                if let Err(err) = self.activate_delegate_session(tui, conversation_id).await {
                    tracing::error!("failed to enter delegate session: {err}");
                    self.chat_widget
                        .add_error_message(format!("Failed to open delegate: {err}"));
                }
            }
            AppEvent::ExitDelegateSession => {
                if let Err(err) = self.return_to_primary(tui).await {
                    tracing::error!("failed to return to primary agent: {err}");
                    self.chat_widget
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
                        .chat_widget
                        .add_info_message(format!("Dismissed detached run {run_id}"), None),
                    Err(err) => self.chat_widget.add_error_message(err),
                }
            }
            AppEvent::InsertUserTextMessage(text) => {
                self.chat_widget.submit_text_message(text);
            }
            AppEvent::OpenReviewBranchPicker(cwd) => {
                self.chat_widget.show_review_branch_picker(&cwd).await;
            }
            AppEvent::OpenReviewCommitPicker(cwd) => {
                self.chat_widget.show_review_commit_picker(&cwd).await;
            }
            AppEvent::OpenReviewCustomPrompt => {
                self.chat_widget.show_review_custom_prompt();
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

    fn handle_delegate_update(&mut self, event: DelegateEvent) {
        match event {
            DelegateEvent::Started {
                run_id,
                agent_id,
                prompt,
                parent_run_id,
                mode,
                ..
            } => {
                let display = self.delegate_tree.insert(
                    run_id.clone(),
                    agent_id.clone(),
                    parent_run_id.clone(),
                );
                let claim_status = parent_run_id.is_none() && self.delegate_status_owner.is_none();
                if claim_status {
                    self.delegate_status_owner = Some(run_id.clone());
                    self.chat_widget
                        .set_delegate_status_owner(&run_id, &agent_id);
                }
                self.chat_widget.on_delegate_started(
                    &run_id,
                    &agent_id,
                    &prompt,
                    display.label,
                    claim_status,
                    mode,
                );
            }
            DelegateEvent::Delta { run_id, chunk, .. } => {
                self.chat_widget.on_delegate_delta(&run_id, &chunk);
            }
            DelegateEvent::Completed {
                run_id,
                agent_id,
                output,
                duration,
                mode,
            } => {
                let display = self.delegate_tree.display_for(&run_id, &agent_id);
                self.delegate_tree.remove(&run_id);
                if self.delegate_status_owner.as_deref() == Some(run_id.as_str()) {
                    self.delegate_status_owner = None;
                    if let Some((next_run_id, next_agent)) = self.delegate_tree.first_active_root()
                    {
                        self.delegate_status_owner = Some(next_run_id.clone());
                        self.chat_widget
                            .set_delegate_status_owner(&next_run_id, &next_agent);
                    } else {
                        self.chat_widget.clear_delegate_status_owner();
                    }
                }
                let streamed = self
                    .chat_widget
                    .on_delegate_completed(&run_id, &display.label);
                let hint = Some(format!(
                    "finished in {}",
                    Self::format_delegate_duration(duration)
                ));
                let response = if display.depth == 0 {
                    output.as_deref().filter(|_| !streamed)
                } else {
                    None
                };
                self.chat_widget
                    .add_delegate_completion(response, hint, &display.label);
                if mode == DelegateSessionMode::Detached {
                    self.chat_widget.notify_detached_completion(&display.label);
                    self.chat_widget.show_detached_completion_actions(
                        &agent_id,
                        &run_id,
                        output.as_deref(),
                    );
                }
            }
            DelegateEvent::Failed {
                run_id,
                agent_id,
                error,
                mode,
            } => {
                let display = self.delegate_tree.display_for(&run_id, &agent_id);
                self.delegate_tree.remove(&run_id);
                if self.delegate_status_owner.as_deref() == Some(run_id.as_str()) {
                    self.delegate_status_owner = None;
                    if let Some((next_run_id, next_agent)) = self.delegate_tree.first_active_root()
                    {
                        self.delegate_status_owner = Some(next_run_id.clone());
                        self.chat_widget
                            .set_delegate_status_owner(&next_run_id, &next_agent);
                    } else {
                        self.chat_widget.clear_delegate_status_owner();
                    }
                }
                self.chat_widget
                    .on_delegate_failed(&run_id, &display.label, &error);
                if mode == DelegateSessionMode::Detached {
                    self.chat_widget
                        .notify_detached_failure(&display.label, &error);
                }
            }
        }
    }

    async fn activate_delegate_session(
        &mut self,
        tui: &mut tui::Tui,
        conversation_id: String,
    ) -> Result<(), String> {
        if self.active_delegate.as_deref() == Some(conversation_id.as_str()) {
            return Ok(());
        }

        if self.active_delegate.is_some() {
            self.stash_active_delegate();
        }

        let state = if let Some(state) = self.delegate_sessions.remove(&conversation_id) {
            state
        } else {
            let session = self
                .delegate_orchestrator
                .enter_session(&conversation_id)
                .await
                .map_err(|err| format!("{err}"))?;
            let init = ChatWidgetInit {
                config: session.config.clone(),
                frame_requester: tui.frame_requester(),
                app_event_tx: self.app_event_tx.clone(),
                initial_prompt: None,
                initial_images: Vec::new(),
                enhanced_keys_supported: self.enhanced_keys_supported,
                auth_manager: self.auth_manager.clone(),
                feedback: self.feedback.clone(),
            };
            let session_configured = expect_unique_session_configured(session.session_configured);
            let mut chat_widget =
                ChatWidget::new_from_existing(init, session.conversation, session_configured);
            chat_widget.set_delegate_context(Some(session.summary.clone()));
            DelegateSessionState {
                summary: session.summary,
                chat_widget,
            }
        };

        let DelegateSessionState {
            summary,
            mut chat_widget,
        } = state;
        chat_widget.set_delegate_context(Some(summary.clone()));
        let mut previous = std::mem::replace(&mut self.chat_widget, chat_widget);
        previous.set_delegate_context(None);
        self.primary_chat_backup = Some(previous);
        self.active_delegate = Some(conversation_id.clone());
        self.active_delegate_summary = Some(summary.clone());
        self.chat_widget.set_delegate_context(Some(summary.clone()));
        self.delegate_orchestrator
            .touch_session(&conversation_id)
            .await;
        tui.frame_requester().schedule_frame();
        Ok(())
    }

    fn stash_active_delegate(&mut self) {
        if let Some(active_id) = self.active_delegate.take() {
            let mut summary = match self.active_delegate_summary.take() {
                Some(summary) => summary,
                None => return,
            };
            let Some(main_chat) = self.primary_chat_backup.take() else {
                self.active_delegate_summary = Some(summary);
                return;
            };
            summary.last_interacted_at = SystemTime::now();
            let mut delegate_chat = std::mem::replace(&mut self.chat_widget, main_chat);
            delegate_chat.set_delegate_context(Some(summary.clone()));
            self.chat_widget.set_delegate_context(None);
            self.delegate_sessions.insert(
                active_id,
                DelegateSessionState {
                    summary,
                    chat_widget: delegate_chat,
                },
            );
        }
    }

    async fn return_to_primary(&mut self, tui: &mut tui::Tui) -> Result<(), String> {
        if let Some(active_id) = self.active_delegate.take() {
            let Some(mut summary) = self.active_delegate_summary.take() else {
                return Err("delegate summary missing".to_string());
            };
            let capture = self.chat_widget.take_delegate_capture();
            let main_chat = self
                .primary_chat_backup
                .take()
                .ok_or_else(|| "primary conversation unavailable".to_string())?;
            summary.last_interacted_at = SystemTime::now();
            let mut delegate_chat = std::mem::replace(&mut self.chat_widget, main_chat);
            delegate_chat.set_delegate_context(Some(summary.clone()));
            self.chat_widget.set_delegate_context(None);
            self.delegate_sessions.insert(
                active_id.clone(),
                DelegateSessionState {
                    summary: summary.clone(),
                    chat_widget: delegate_chat,
                },
            );
            self.delegate_orchestrator.touch_session(&active_id).await;
            self.primary_chat_backup = None;
            self.active_delegate_summary = None;
            if let Some(capture) = capture {
                self.chat_widget.apply_delegate_summary(&summary, capture);
            }
            tui.frame_requester().schedule_frame();
        }
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
        self.chat_widget.token_usage()
    }

    fn on_update_reasoning_effort(&mut self, effort: Option<ReasoningEffortConfig>) {
        self.chat_widget.set_reasoning_effort(effort);
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
                if self.chat_widget.is_normal_backtrack_mode()
                    && self.chat_widget.composer_is_empty()
                {
                    self.handle_backtrack_esc_key(tui);
                } else {
                    self.chat_widget.handle_key_event(key_event);
                }
            }
            // Enter confirms backtrack when primed + count > 0. Otherwise pass to widget.
            KeyEvent {
                code: KeyCode::Enter,
                kind: KeyEventKind::Press,
                ..
            } if self.backtrack.primed
                && self.backtrack.nth_user_message != usize::MAX
                && self.chat_widget.composer_is_empty() =>
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
                self.chat_widget.handle_key_event(key_event);
            }
            _ => {
                // Ignore Release key events.
            }
        };
    }
}

struct DelegateSessionState {
    summary: DelegateSessionSummary,
    chat_widget: ChatWidget,
}

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
    use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
    use crate::file_search::FileSearchManager;
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

    use codex_protocol::ConversationId;
    use ratatui::prelude::Line;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    fn make_test_app() -> App {
        let (chat_widget, app_event_tx, _rx, _op_rx) = make_chatwidget_manual_with_sender();
        let config = chat_widget.config_ref().clone();

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
        ));

        App {
            server,
            app_event_tx,
            chat_widget,
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
            delegate_sessions: HashMap::new(),
            active_delegate: None,
            active_delegate_summary: None,
            primary_chat_backup: None,
            pending_update_action: None,
            delegate_tree: DelegateTree::default(),
            delegate_status_owner: None,
        }
    }

    #[test]
    fn update_reasoning_effort_updates_config() {
        let mut app = make_test_app();
        app.config.model_reasoning_effort = Some(ReasoningEffortConfig::Medium);
        app.chat_widget
            .set_reasoning_effort(Some(ReasoningEffortConfig::Medium));

        app.on_update_reasoning_effort(Some(ReasoningEffortConfig::High));

        assert_eq!(
            app.config.model_reasoning_effort,
            Some(ReasoningEffortConfig::High)
        );
        assert_eq!(
            app.chat_widget.config_ref().model_reasoning_effort,
            Some(ReasoningEffortConfig::High)
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
                app.chat_widget.config_ref(),
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

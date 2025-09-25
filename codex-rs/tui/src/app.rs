use crate::app_backtrack::BacktrackState;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::chat_host::ChatHost;
use crate::chatwidget::ChatWidget;
use crate::file_search::FileSearchManager;
use crate::history_cell::HistoryCell;
use crate::pager_overlay::Overlay;
use crate::resume_picker::ResumeSelection;
use crate::shims::HostApi;
use crate::tui;
use crate::tui::TuiEvent;
use codex_ansi_escape::ansi_escape_line;
use codex_core::AuthManager;
use codex_core::ConversationManager;
use codex_core::config::Config;
use codex_core::config::persist_model_selection;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::TokenUsage;
use codex_core::protocol_config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::mcp_protocol::ConversationId;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use tokio::select;
use tokio::sync::mpsc::unbounded_channel;
// use uuid::Uuid;
use crate::shims::EventOutcome;
use crate::shims::ShimStack;

#[derive(Debug, Clone)]
pub struct AppExitInfo {
    pub token_usage: TokenUsage,
    pub conversation_id: Option<ConversationId>,
}

pub(crate) struct App {
    pub(crate) server: Arc<ConversationManager>,
    pub(crate) app_event_tx: AppEventSender,
    pub(crate) chat: ChatHost,
    pub(crate) auth_manager: Arc<AuthManager>,

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
    // Optional shim stack for feature modules
    pub(crate) shims: ShimStack,
    // Whether to enable thread shims and multi-session host
    threads_enabled: bool,
    // Pending request to fork the current conversation into a child thread.
    thread_fork_pending: Option<ThreadForkPending>,
}

#[derive(Debug, Clone)]
struct ThreadForkPending {
    parent_idx: usize,
    base_id: Option<ConversationId>,
}

impl App {
    // Build resolved keymap (defaults overridden by config.tui.keymap)
    fn resolve_keymap(&self) -> ResolvedKeymap {
        let cfg = self.config.tui_keymap.as_ref();
        ResolvedKeymap {
            open_threads: cfg
                .and_then(|m| m.open_threads.clone())
                .unwrap_or_else(|| "ctrl-]".to_string()),
            new_session: cfg
                .and_then(|m| m.new_session.clone())
                .unwrap_or_else(|| "ctrl-shift-n".to_string()),
            fork_thread: cfg
                .and_then(|m| m.fork_thread.clone())
                .unwrap_or_else(|| "ctrl-shift-t".to_string()),
            prev_thread: cfg
                .and_then(|m| m.prev_thread.clone())
                .unwrap_or_else(|| "ctrl-left".to_string()),
            next_thread: cfg
                .and_then(|m| m.next_thread.clone())
                .unwrap_or_else(|| "ctrl-right".to_string()),
        }
    }

    // Matches a KeyEvent against a small string spec like "ctrl-]", "ctrl-\\",
    // "ctrl-left", "ctrl-right", "ctrl-shift-n".
    fn matches_key(&self, ev: &KeyEvent, spec: &str) -> bool {
        use crossterm::event::KeyCode;
        use crossterm::event::KeyModifiers;
        let s = spec.to_ascii_lowercase();
        let ctrl = s.contains("ctrl");
        let shift = s.contains("shift");
        // Extract last token as the key (rightmost after '-')
        let key_token = s.split('-').next_back().unwrap_or("");
        let has_ctrl = ev.modifiers.contains(KeyModifiers::CONTROL);
        let has_shift = ev.modifiers.contains(KeyModifiers::SHIFT);
        if ctrl != has_ctrl || (shift && !has_shift) || (!shift && has_shift && ctrl) {
            // require ctrl match; require shift only when specified
            return false;
        }
        match key_token {
            "]" => matches!(ev.code, KeyCode::Char(']')),
            "\\" => matches!(ev.code, KeyCode::Char('\\')),
            "left" => matches!(ev.code, KeyCode::Left),
            "right" => matches!(ev.code, KeyCode::Right),
            // single char like 'n' or 't'
            k if k.len() == 1 => {
                if let Some(ch) = k.chars().next() {
                    match ev.code {
                        KeyCode::Char(c) => c.to_ascii_lowercase() == ch,
                        _ => false,
                    }
                } else {
                    false
                }
            }
            _ => false,
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        tui: &mut tui::Tui,
        auth_manager: Arc<AuthManager>,
        config: Config,
        active_profile: Option<String>,
        initial_prompt: Option<String>,
        initial_images: Vec<PathBuf>,
        resume_selection: ResumeSelection,
        enable_thread_shims: bool,
    ) -> Result<AppExitInfo> {
        use tokio_stream::StreamExt;
        let (app_event_tx, mut app_event_rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(app_event_tx);

        let conversation_manager = Arc::new(ConversationManager::new(auth_manager.clone()));

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
            chat: ChatHost::from_initial(chat_widget, enable_thread_shims),
            auth_manager: auth_manager.clone(),
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
            shims: ShimStack::new(),
            threads_enabled: enable_thread_shims,
            thread_fork_pending: None,
        };

        if enable_thread_shims {
            use crate::shims::thread::commands::ThreadCommandShim;
            use crate::shims::thread::header::ThreadHeaderShim;
            use crate::shims::thread::status::ThreadStatusShim;
            use crate::shims::title_tool::TitleToolShim;
            // Enable status + commands + title tool shims; omit footer hints to keep footer clean.
            app.shims.push(ThreadHeaderShim::new());
            app.shims.push(ThreadStatusShim::new());
            app.shims.push(ThreadCommandShim::new());
            app.shims.push(TitleToolShim::new());
        }
        // Title summary shim removed; TitleToolShim handles title updates now.

        let tui_events = tui.event_stream();
        tokio::pin!(tui_events);

        tui.frame_requester().schedule_frame();

        // Seed initial shim decorations so early header insert can include shim lines.
        {
            let mut header_lines: Vec<ratatui::text::Line<'static>> = Vec::new();
            let mut status_line: Option<ratatui::text::Line<'static>> = None;
            app.shims
                .augment_header_with_host(&mut header_lines, &app.chat);
            app.shims
                .augment_status_line_with_host(&mut status_line, &app.chat);
            app.chat.set_shim_decorations(header_lines, status_line);
        }

        while select! {
            Some(mut event) = app_event_rx.recv() => {
                if app
                    .shims
                    .on_app_event_with_host(&mut event, &mut app.chat)
                    == EventOutcome::Consumed
                {
                    true
                } else {
                    app.handle_event(tui, event).await?
                }
            }
            Some(event) = tui_events.next() => {
                app.handle_tui_event(tui, event).await?
            }
        } {}
        tui.terminal.clear()?;
        Ok(AppExitInfo {
            token_usage: app.token_usage(),
            conversation_id: app.chat.conversation_id(),
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
                    if self
                        .shims
                        .on_key_event_with_host(&key_event, &mut self.chat)
                        == EventOutcome::Consumed
                    {
                        return Ok(true);
                    }
                    self.handle_key_event(tui, key_event).await;
                }
                TuiEvent::Paste(pasted) => {
                    // Many terminals convert newlines to \r when pasting (e.g., iTerm2),
                    // but tui-textarea expects \n. Normalize CR to LF.
                    // [tui-textarea]: https://github.com/rhysd/tui-textarea/blob/4d18622eeac13b309e0ff6a55a46ac6706da68cf/src/textarea.rs#L782-L783
                    // [iTerm2]: https://github.com/gnachman/iTerm2/blob/5d0c0d9f68523cbd0494dad5422998964a2ecd8d/sources/iTermPasteHelper.m#L206-L216
                    let pasted = pasted.replace("\r", "\n");
                    self.chat.handle_paste(pasted);
                }
                TuiEvent::Draw => {
                    self.chat.maybe_post_pending_notification(tui);
                    // Allow shims to compute optional header/status decorations.
                    let mut header_lines: Vec<ratatui::text::Line<'static>> = Vec::new();
                    let mut status_line: Option<ratatui::text::Line<'static>> = None;
                    self.shims
                        .augment_header_with_host(&mut header_lines, &self.chat);
                    self.shims
                        .augment_status_line_with_host(&mut status_line, &self.chat);
                    self.chat.set_shim_decorations(header_lines, status_line);
                    if self.chat.handle_paste_burst_tick(tui.frame_requester()) {
                        return Ok(true);
                    }
                    tui.draw(
                        self.chat.desired_height(tui.terminal.size()?.width),
                        |frame| {
                            frame.render_widget_ref(&self.chat, frame.area());
                            if let Some((x, y)) = self.chat.cursor_pos(frame.area()) {
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
                };
                let widget = ChatWidget::new(init, self.server.clone());
                // In threads mode, append as top-level (root) when already using Multi.
                match &mut self.chat {
                    ChatHost::Multi(m) if self.threads_enabled => {
                        let id = m.next_session_id();
                        let idx = m.add_top_level_session(widget, id);
                        m.switch_active(idx);
                    }
                    _ => {
                        self.chat = ChatHost::from_initial(widget, self.threads_enabled);
                    }
                }
                // Refresh shim decorations after creating a thread (update header overlay).
                let mut header_lines: Vec<ratatui::text::Line<'static>> = Vec::new();
                let mut status_line: Option<ratatui::text::Line<'static>> = None;
                self.shims
                    .augment_header_with_host(&mut header_lines, &self.chat);
                self.shims
                    .augment_status_line_with_host(&mut status_line, &self.chat);
                self.chat.set_shim_decorations(header_lines, status_line);
                tui.frame_requester().schedule_frame();
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
                self.chat.on_commit_tick();
            }
            AppEvent::CodexEvent(event) => {
                // Forward event to chat widget(s)
                self.chat.handle_codex_event(event);
                // If the thread manager is open, refresh its items live.
                // Rebuild the items using the same logic as when opening.
                if self.threads_enabled {
                    let items = self.build_thread_selection_items();
                    let _ = self.chat.refresh_thread_popup(items);
                }
            }
            AppEvent::ConversationHistory(ev) => {
                if self.thread_fork_pending.is_some() {
                    self.on_conversation_history_for_thread_fork(tui, ev)
                        .await?;
                } else {
                    self.on_conversation_history_for_backtrack(tui, ev).await?;
                }
            }
            AppEvent::ExitRequest => {
                return Ok(false);
            }
            AppEvent::CodexOp(op) => self.chat.submit_op(op),
            AppEvent::DiffResult(text) => {
                // Clear the in-progress state in the bottom pane
                self.chat.on_diff_complete();
                // Enter alternate screen using TUI helper and build pager lines
                let _ = tui.enter_alt_screen();
                let pager_lines: Vec<ratatui::text::Line<'static>> = if text.trim().is_empty() {
                    vec!["No changes detected.".italic().into()]
                } else {
                    text.lines().map(ansi_escape_line).collect()
                };
                self.overlay = Some(Overlay::new_static_with_title(
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
                self.chat.apply_file_search_result(query, matches);
            }
            AppEvent::UpdateReasoningEffort(effort) => {
                self.on_update_reasoning_effort(effort);
            }
            AppEvent::UpdateModel(model) => {
                self.chat.set_model(&model);
                self.config.model = model.clone();
                if let Some(family) = find_family_for_model(&model) {
                    self.config.model_family = family;
                }
            }
            AppEvent::PersistModelSelection { model, effort } => {
                let profile = self.active_profile.as_deref();
                match persist_model_selection(&self.config.codex_home, profile, &model, effort)
                    .await
                {
                    Ok(()) => {
                        if let Some(profile) = profile {
                            self.chat.add_info_message(
                                format!("Model changed to {model} for {profile} profile"),
                                None,
                            );
                        } else {
                            self.chat
                                .add_info_message(format!("Model changed to {model}"), None);
                        }
                    }
                    Err(err) => {
                        tracing::error!(
                            error = %err,
                            "failed to persist model selection"
                        );
                        if let Some(profile) = profile {
                            self.chat.add_error_message(format!(
                                "Failed to save model for profile `{profile}`: {err}"
                            ));
                        } else {
                            self.chat
                                .add_error_message(format!("Failed to save default model: {err}"));
                        }
                    }
                }
            }
            AppEvent::UpdateAskForApprovalPolicy(policy) => {
                self.chat.set_approval_policy(policy);
            }
            AppEvent::UpdateSandboxPolicy(policy) => {
                self.chat.set_sandbox_policy(policy);
            }
            AppEvent::OpenReviewBranchPicker(cwd) => {
                self.chat.show_review_branch_picker(&cwd).await;
            }
            AppEvent::OpenReviewCommitPicker(cwd) => {
                self.chat.show_review_commit_picker(&cwd).await;
            }
            AppEvent::OpenReviewCustomPrompt => {
                self.chat.show_review_custom_prompt();
            }
            AppEvent::OpenThreadManager => {
                // Build a list of actions similar to /model
                let mut items: Vec<crate::bottom_pane::SelectionItem> = Vec::new();
                // Actions
                let current_parent = self.chat.active_index();
                // New session (top-level)
                items.push(crate::bottom_pane::SelectionItem {
                    name: "New session".to_string(),
                    description: Some("Create new session".to_string()),
                    is_current: false,
                    actions: vec![Box::new(
                        move |tx: &crate::app_event_sender::AppEventSender| {
                            tx.send(AppEvent::NewSession);
                        },
                    )],
                    dismiss_on_select: true,
                    search_value: None,
                });
                // New thread (fork) — child of current
                items.push(crate::bottom_pane::SelectionItem {
                    name: "New thread (fork)".to_string(),
                    description: Some("Create a child thread, keeping all context".to_string()),
                    is_current: false,
                    actions: vec![Box::new(
                        move |tx: &crate::app_event_sender::AppEventSender| {
                            tx.send(AppEvent::NewChildThread(current_parent));
                        },
                    )],
                    dismiss_on_select: true,
                    search_value: None,
                });

                let items_extra = self.build_thread_selection_items();
                items.extend(items_extra);

                // Delegate to the widget to show the popup
                // Use a compact FN-key toolbar in the popup footer to avoid cluttering main chat.
                let km = self.resolve_keymap();
                let toolbar = format!(
                    "{} Prev   {} Next   {} New   {} Fork   Enter Select   Esc Back",
                    km.prev_thread.to_uppercase(),
                    km.next_thread.to_uppercase(),
                    km.new_session.to_uppercase(),
                    km.fork_thread.to_uppercase()
                );
                self.chat.open_thread_popup_with_hint(items, Some(toolbar));
            }

            AppEvent::ForkChildOfActive => {
                let parent_idx = self.chat.active_index();
                if let Some(base_id) = self.chat.conversation_id() {
                    self.thread_fork_pending = Some(ThreadForkPending {
                        parent_idx,
                        base_id: Some(base_id),
                    });
                    self.chat.submit_op(codex_core::protocol::Op::GetPath);
                } else {
                    // No current conversation yet; create empty child.
                    self.app_event_tx.send(AppEvent::NewChildThread(parent_idx));
                }
            }
            AppEvent::SwitchThread(index) => {
                self.chat.switch_to(index);
                // Refresh header/status decorations after switching
                let mut header_lines: Vec<ratatui::text::Line<'static>> = Vec::new();
                let mut status_line: Option<ratatui::text::Line<'static>> = None;
                self.shims
                    .augment_header_with_host(&mut header_lines, &self.chat);
                self.shims
                    .augment_status_line_with_host(&mut status_line, &self.chat);
                self.chat.set_shim_decorations(header_lines, status_line);
                tui.frame_requester().schedule_frame();
            }
            AppEvent::NewChildThread(parent_idx) => {
                // Fork current conversation into a child of parent_idx, keeping context.
                if let Some(base_id) = self.chat.conversation_id() {
                    self.thread_fork_pending = Some(ThreadForkPending {
                        parent_idx,
                        base_id: Some(base_id),
                    });
                    self.chat.submit_op(codex_core::protocol::Op::GetPath);
                } else {
                    // If no conversation yet, fall back to an empty child session.
                    let init = crate::chatwidget::ChatWidgetInit {
                        config: self.config.clone(),
                        frame_requester: tui.frame_requester(),
                        app_event_tx: self.app_event_tx.clone(),
                        initial_prompt: None,
                        initial_images: Vec::new(),
                        enhanced_keys_supported: self.enhanced_keys_supported,
                        auth_manager: self.auth_manager.clone(),
                    };
                    let widget = ChatWidget::new(init, self.server.clone());
                    match &mut self.chat {
                        ChatHost::Multi(m) if self.threads_enabled => {
                            let id = m.next_session_id();
                            let idx = m.add_child_session(parent_idx, widget, id);
                            m.switch_active(idx);
                        }
                        _ => {
                            self.chat = ChatHost::from_initial(widget, self.threads_enabled);
                        }
                    }
                    let mut header_lines: Vec<ratatui::text::Line<'static>> = Vec::new();
                    let mut status_line: Option<ratatui::text::Line<'static>> = None;
                    self.shims
                        .augment_header_with_host(&mut header_lines, &self.chat);
                    self.shims
                        .augment_status_line_with_host(&mut status_line, &self.chat);
                    self.chat.set_shim_decorations(header_lines, status_line);
                    tui.frame_requester().schedule_frame();
                }
            }
        }
        Ok(true)
    }

    /// Build selection items for the thread manager popup based on the current
    /// chat host state.
    fn build_thread_selection_items(&self) -> Vec<crate::bottom_pane::SelectionItem> {
        let mut items: Vec<crate::bottom_pane::SelectionItem> = Vec::new();
        // Tree view of sessions using parent indices and titles
        let count = self.chat.session_count();
        let active = self.chat.active_index();
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); count];
        let mut roots: Vec<usize> = Vec::new();
        for i in 0..count {
            if let Some(p) = self.chat.session_parent_index(i) {
                if p < count {
                    children[p].push(i);
                } else {
                    roots.push(i);
                }
            } else {
                roots.push(i);
            }
        }
        fn push_items(
            items: &mut Vec<crate::bottom_pane::SelectionItem>,
            chat: &crate::chat_host::ChatHost,
            children: &Vec<Vec<usize>>,
            idx: usize,
            depth: usize,
            active: usize,
        ) {
            let indent = "  ".repeat(depth);
            // Compute display label independent of topic title.
            let display_label = if chat.session_parent_index(idx).is_none() {
                if idx == 0 {
                    "main".to_string()
                } else {
                    // Root sessions (besides main) are named main-1, main-2, ...
                    let ordinal = {
                        let mut n = 0usize;
                        for r in 0..chat.session_count() {
                            if chat.session_parent_index(r).is_none() {
                                if r == 0 {
                                    continue;
                                }
                                n += 1;
                                if r == idx {
                                    break;
                                }
                            }
                        }
                        n
                    };
                    format!("main-{ordinal}")
                }
            } else {
                // Child sessions are numbered relative to siblings: #1, #2, ...
                if let Some(parent) = chat.session_parent_index(idx)
                    && let Some(sibs) = children.get(parent)
                {
                    match sibs.iter().position(|&c| c == idx) {
                        Some(pos) => format!("#{}", pos + 1),
                        None => "#1".to_string(),
                    }
                } else {
                    "#1".to_string()
                }
            };
            let label = format!("{indent}{display_label}");
            let current = idx == active;
            // Build description: topic title and optional status
            let topic = chat.session_title_at(idx).unwrap_or_default();
            let status = chat.status_snapshot_at(idx).unwrap_or_default();
            let mut desc = String::new();
            if !topic.trim().is_empty() {
                desc.push_str(&topic);
            }
            if !status.trim().is_empty() {
                if !desc.is_empty() {
                    desc.push_str("   ");
                }
                desc.push_str("‣ ");
                desc.push_str(&status);
            }
            let description = if desc.is_empty() { None } else { Some(desc) };
            items.push(crate::bottom_pane::SelectionItem {
                name: label,
                description,
                is_current: current,
                actions: vec![Box::new(
                    move |tx: &crate::app_event_sender::AppEventSender| {
                        tx.send(AppEvent::SwitchThread(idx));
                    },
                )],
                dismiss_on_select: true,
                search_value: None,
            });
            for &c in &children[idx] {
                push_items(items, chat, children, c, depth + 1, active);
            }
        }
        if count > 0 {
            // Show main first if present
            if roots.contains(&0) {
                push_items(&mut items, &self.chat, &children, 0, 0, active);
            }
            for &r in roots.iter().filter(|&&r| r != 0) {
                push_items(&mut items, &self.chat, &children, r, 0, active);
            }
        }
        items
    }

    pub(crate) fn token_usage(&self) -> codex_core::protocol::TokenUsage {
        self.chat.token_usage()
    }

    fn on_update_reasoning_effort(&mut self, effort: Option<ReasoningEffortConfig>) {
        self.chat.set_reasoning_effort(effort);
        self.config.model_reasoning_effort = effort;
    }

    async fn on_conversation_history_for_thread_fork(
        &mut self,
        tui: &mut tui::Tui,
        ev: codex_core::protocol::ConversationPathResponseEvent,
    ) -> color_eyre::eyre::Result<()> {
        let Some(pending) = self.thread_fork_pending.take() else {
            return Ok(());
        };
        if let Some(base) = pending.base_id
            && ev.conversation_id != base
        {
            // Not for our pending fork; keep waiting.
            self.thread_fork_pending = Some(pending);
            return Ok(());
        }
        let new_conv = self
            .server
            .resume_conversation_from_rollout(
                self.config.clone(),
                ev.path.clone(),
                self.auth_manager.clone(),
            )
            .await
            .wrap_err("failed to fork session from rollout path")?;

        let conv = new_conv.conversation;
        let session_configured = new_conv.session_configured;
        let init = crate::chatwidget::ChatWidgetInit {
            config: self.config.clone(),
            frame_requester: tui.frame_requester(),
            app_event_tx: self.app_event_tx.clone(),
            initial_prompt: None,
            initial_images: Vec::new(),
            enhanced_keys_supported: self.enhanced_keys_supported,
            auth_manager: self.auth_manager.clone(),
        };
        let widget =
            crate::chatwidget::ChatWidget::new_from_existing(init, conv, session_configured);

        match &mut self.chat {
            ChatHost::Multi(m) if self.threads_enabled => {
                let id = m.next_session_id();
                let idx = m.add_child_session(pending.parent_idx, widget, id);
                m.switch_active(idx);
            }
            _ => {
                self.chat = ChatHost::from_initial(widget, self.threads_enabled);
            }
        }
        let mut header_lines: Vec<ratatui::text::Line<'static>> = Vec::new();
        let mut status_line: Option<ratatui::text::Line<'static>> = None;
        self.shims
            .augment_header_with_host(&mut header_lines, &self.chat);
        self.shims
            .augment_status_line_with_host(&mut status_line, &self.chat);
        self.chat.set_shim_decorations(header_lines, status_line);
        tui.frame_requester().schedule_frame();
        Ok(())
    }

    async fn handle_key_event(&mut self, tui: &mut tui::Tui, key_event: KeyEvent) {
        // Helper: configurable key bindings
        let keymap = self.resolve_keymap();
        match key_event {
            // New session
            KeyEvent { .. } if self.matches_key(&key_event, &keymap.new_session) => {
                self.app_event_tx.send(AppEvent::NewSession);
            }
            // Fork child thread
            KeyEvent { .. } if self.matches_key(&key_event, &keymap.fork_thread) => {
                self.app_event_tx.send(AppEvent::ForkChildOfActive);
            }
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
            // with an empty composer. In any other state, forward Esc so the
            // active UI (e.g. status indicator, modals, popups) handles it.
            KeyEvent {
                code: KeyCode::Esc,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                if self.chat.is_normal_backtrack_mode() && self.chat.composer_is_empty() {
                    self.handle_backtrack_esc_key(tui);
                } else {
                    self.chat.handle_key_event(key_event);
                }
            }
            // Enter confirms backtrack when primed + count > 0. Otherwise pass to widget.
            KeyEvent {
                code: KeyCode::Enter,
                kind: KeyEventKind::Press,
                ..
            } if self.backtrack.primed
                && self.backtrack.nth_user_message != usize::MAX
                && self.chat.composer_is_empty() =>
            {
                // Delegate to helper for clarity; preserves behavior.
                self.confirm_backtrack_from_main();
            }
            // Open thread manager
            KeyEvent { .. } if self.matches_key(&key_event, &keymap.open_threads) => {
                self.app_event_tx.send(AppEvent::OpenThreadManager);
            }
            // Ctrl-Shift-N : New session
            KeyEvent {
                code: KeyCode::Char('n'),
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                modifiers,
                ..
            } if modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                && modifiers.contains(crossterm::event::KeyModifiers::SHIFT) =>
            {
                self.app_event_tx.send(AppEvent::NewSession);
            }
            // Ctrl-Shift-T : New thread (fork)
            KeyEvent {
                code: KeyCode::Char('t'),
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                modifiers,
                ..
            } if modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                && modifiers.contains(crossterm::event::KeyModifiers::SHIFT) =>
            {
                self.app_event_tx.send(AppEvent::ForkChildOfActive);
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
                self.chat.handle_key_event(key_event);
            }
            _ => {
                // Ignore Release key events.
            }
        }
    }
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
    use codex_core::AuthManager;
    use codex_core::CodexAuth;
    use codex_core::ConversationManager;
    use codex_core::protocol::SessionConfiguredEvent;
    use codex_protocol::mcp_protocol::ConversationId;
    use ratatui::prelude::Line;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    fn make_test_app() -> App {
        let (chat_widget, app_event_tx, _rx, _op_rx) = make_chatwidget_manual_with_sender();
        let chat = ChatHost::single(chat_widget);
        let config = chat.config_ref().clone();

        let server = Arc::new(ConversationManager::with_auth(CodexAuth::from_api_key(
            "Test API Key",
        )));
        let auth_manager =
            AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
        let file_search = FileSearchManager::new(config.cwd.clone(), app_event_tx.clone());

        App {
            server,
            app_event_tx,
            chat,
            auth_manager,
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
            shims: ShimStack::new(),
            threads_enabled: false,
            thread_fork_pending: None,
        }
    }

    #[test]
    fn update_reasoning_effort_updates_config() {
        let mut app = make_test_app();
        app.config.model_reasoning_effort = Some(ReasoningEffortConfig::Medium);
        app.chat
            .set_reasoning_effort(Some(ReasoningEffortConfig::Medium));

        app.on_update_reasoning_effort(Some(ReasoningEffortConfig::High));

        assert_eq!(
            app.config.model_reasoning_effort,
            Some(ReasoningEffortConfig::High)
        );
        assert_eq!(
            app.chat.config_ref().model_reasoning_effort,
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
                app.chat.config_ref(),
                event,
                is_first,
                Vec::new(),
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
}
#[derive(Clone)]
struct ResolvedKeymap {
    open_threads: String,
    new_session: String,
    fork_thread: String,
    prev_thread: String,
    next_thread: String,
}

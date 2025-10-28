use crate::app_event::{AppEvent, TerminalRunController, TerminalRunEvent};
use crate::app_event_sender::AppEventSender;
use crate::chatwidget::{ChatWidget, EscIntent, GhostState};
use crate::bottom_pane::SettingsSection;
use crate::cloud_tasks_service;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::file_search::FileSearchManager;
use crate::get_git_diff::get_git_diff;
use crate::get_login_status;
use crate::history::state::HistorySnapshot;
use crate::history_cell;
use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::onboarding::onboarding_screen::OnboardingScreen;
use crate::onboarding::onboarding_screen::OnboardingScreenArgs;
use crate::slash_command::SlashCommand;
use crate::thread_spawner;
use crate::tui;
use crate::tui::TerminalInfo;
use code_core::config::add_project_allowed_command;
use code_core::config::Config;
use code_core::config_types::Notifications;
use code_core::protocol::Event;
use code_core::protocol::Op;
use code_core::protocol::SandboxPolicy;
use code_core::ConversationManager;
use code_login::{AuthManager, AuthMode, ServerOptions};
use code_cloud_tasks_client::TaskId;
use code_cloud_tasks_client::CloudTaskError;
use code_protocol::protocol::SessionSource;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::execute;
use crossterm::terminal::supports_keyboard_enhancement;
use crossterm::SynchronizedUpdate; // trait for stdout().sync_update
use futures::FutureExt;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtyPair, PtySize};
use ratatui::buffer::Buffer;
use ratatui::CompletedFrame;
use shlex::try_join;
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, HashMap, HashSet};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver, Sender as StdSender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;
use std::time::Instant;
#[cfg(unix)]
use signal_hook::consts::signal::SIGTERM;
#[cfg(unix)]
use signal_hook::{flag, SigId};
use tokio::sync::oneshot;

/// Time window for debouncing redraw requests.
///
/// Temporarily widened to ~30 FPS (33 ms) to coalesce bursts of updates while
/// we smooth out per-frame hotspots; keeps redraws responsive without pegging
/// the main thread.
const REDRAW_DEBOUNCE: Duration = Duration::from_millis(33);
const DEFAULT_PTY_ROWS: u16 = 24;
const DEFAULT_PTY_COLS: u16 = 80;

/// Top-level application state: which full-screen view is currently active.
#[allow(clippy::large_enum_variant)]
enum AppState<'a> {
    Onboarding {
        screen: OnboardingScreen,
    },
    /// The main chat UI is visible.
    Chat {
        /// Boxed to avoid a large enum variant and reduce the overall size of
        /// `AppState`.
        widget: Box<ChatWidget<'a>>,
    },
}

struct TerminalRunState {
    command: Vec<String>,
    display: String,
    cancel_tx: Option<oneshot::Sender<()>>,
    running: bool,
    controller: Option<TerminalRunController>,
    writer_tx: Option<Arc<Mutex<Option<StdSender<Vec<u8>>>>>>,
    pty: Option<Arc<Mutex<Box<dyn MasterPty + Send>>>>,
}

struct FrameTimer {
    state: Mutex<FrameTimerState>,
    cv: Condvar,
}

struct FrameTimerState {
    deadlines: BinaryHeap<Reverse<Instant>>,
    worker_running: bool,
}

impl FrameTimer {
    fn new() -> Self {
        Self {
            state: Mutex::new(FrameTimerState {
                deadlines: BinaryHeap::new(),
                worker_running: false,
            }),
            cv: Condvar::new(),
        }
    }

    fn schedule(self: &Arc<Self>, duration: Duration, tx: AppEventSender) {
        let deadline = Instant::now() + duration;
        let mut state = self.state.lock().unwrap();
        state.deadlines.push(Reverse(deadline));
        let should_spawn = if !state.worker_running {
            state.worker_running = true;
            true
        } else {
            false
        };
        self.cv.notify_one();
        drop(state);

        if should_spawn {
            let timer = Arc::clone(self);
            let tx_for_thread = tx.clone();
            if thread_spawner::spawn_lightweight("frame-timer", move || timer.run(tx_for_thread)).is_none() {
                let mut state = self.state.lock().unwrap();
                state.worker_running = false;
                let drained = state.deadlines.len();
                state.deadlines.clear();
                drop(state);
                for _ in 0..drained.max(1) {
                    tx.send(AppEvent::RequestRedraw);
                }
                tracing::warn!(
                    drained_deadlines = drained,
                    "frame timer spawn rejected: background thread limit reached; flushed deadlines"
                );
            }
        }
    }

    fn run(self: Arc<Self>, tx: AppEventSender) {
        let mut state = self.state.lock().unwrap();
        loop {
            let deadline = match state.deadlines.peek().copied() {
                Some(Reverse(deadline)) => deadline,
                None => {
                    state.worker_running = false;
                    break;
                }
            };

            let now = Instant::now();
            if deadline <= now {
                state.deadlines.pop();
                drop(state);
                tx.send(AppEvent::RequestRedraw);
                state = self.state.lock().unwrap();
                continue;
            }

            let wait_dur = deadline.saturating_duration_since(now);
            let (new_state, result) = self.cv.wait_timeout(state, wait_dur).unwrap();
            state = new_state;

            if result.timed_out() {
                continue;
            }
        }
    }
}

struct LoginFlowState {
    shutdown: code_login::ShutdownHandle,
    join_handle: tokio::task::JoinHandle<()>,
}

pub(crate) struct App<'a> {
    _server: Arc<ConversationManager>,
    app_event_tx: AppEventSender,
    // Split event receivers: high‑priority (input) and bulk (streaming)
    app_event_rx_high: Receiver<AppEvent>,
    app_event_rx_bulk: Receiver<AppEvent>,
    app_state: AppState<'a>,

    /// Config is stored here so we can recreate ChatWidgets as needed.
    config: Config,

    /// Latest available release version (if detected) so new widgets can surface it.
    latest_upgrade_version: Option<String>,

    file_search: FileSearchManager,

    /// True when a redraw has been scheduled but not yet executed (debounce window).
    pending_redraw: Arc<AtomicBool>,
    /// Tracks whether a frame is currently queued or being drawn. Used to coalesce
    /// rapid-fire redraw requests without dropping the final state.
    redraw_inflight: Arc<AtomicBool>,
    /// Set if a redraw request arrived while another frame was in flight. Ensures we
    /// queue one more frame immediately after the current draw completes.
    post_frame_redraw: Arc<AtomicBool>,
    /// Shared scheduler for future animation frames. Ensures the shortest
    /// requested interval wins while preserving later deadlines.
    frame_timer: Arc<FrameTimer>,
    /// Controls the input reader thread spawned at startup.
    input_running: Arc<AtomicBool>,

    enhanced_keys_supported: bool,
    /// Tracks keys seen as pressed when keyboard enhancements are unavailable
    /// so duplicate release events can be filtered and release-only terminals
    /// still synthesize a press.
    non_enhanced_pressed_keys: HashSet<KeyCode>,

    /// Debug flag for logging LLM requests/responses
    _debug: bool,
    /// Show per-cell ordering overlay when true
    show_order_overlay: bool,

    /// Controls the animation thread that sends CommitTick events.
    commit_anim_running: Arc<AtomicBool>,

    /// Terminal information queried at startup
    terminal_info: TerminalInfo,

    #[cfg(unix)]
    sigterm_guard: Option<SigId>,
    #[cfg(unix)]
    sigterm_flag: Arc<AtomicBool>,

    /// Perform a hard clear on the first frame to ensure the entire buffer
    /// starts with our theme background. This avoids terminals that may show
    /// profile defaults until all cells are explicitly painted.
    clear_on_first_frame: bool,

    /// Pending ghost snapshot state to apply after a conversation fork completes.
    pending_jump_back_ghost_state: Option<GhostState>,
    /// Pending history snapshot to seed the next widget after a jump-back fork.
    pending_jump_back_history_snapshot: Option<HistorySnapshot>,

    /// Track last known terminal size. If it changes (true resize or a
    /// tab switch that altered the viewport), perform a full clear on the next
    /// draw to avoid ghost cells from the previous size. This is cheap and
    /// happens rarely, but fixes Windows/macOS terminals that don't fully
    /// repaint after focus/size changes until a manual resize occurs.
    last_frame_size: Option<ratatui::prelude::Size>,

    // Double‑Esc timing for undo timeline
    last_esc_time: Option<Instant>,

    /// If true, enable lightweight timing collection and report on exit.
    timing_enabled: bool,
    timing: TimingStats,

    buffer_diff_profiler: BufferDiffProfiler,

    /// True when TUI is currently rendering in the terminal's alternate screen.
    alt_screen_active: bool,

    terminal_runs: HashMap<u64, TerminalRunState>,

    terminal_title_override: Option<String>,
    login_flow: Option<LoginFlowState>,
}

/// Aggregate parameters needed to create a `ChatWidget`, as creation may be
/// deferred until after the Git warning screen is dismissed.
#[derive(Clone, Debug)]
pub(crate) struct ChatWidgetArgs {
    pub(crate) config: Config,
    initial_prompt: Option<String>,
    initial_images: Vec<PathBuf>,
    enhanced_keys_supported: bool,
    terminal_info: TerminalInfo,
    show_order_overlay: bool,
    enable_perf: bool,
    resume_picker: bool,
    latest_upgrade_version: Option<String>,
}

impl App<'_> {
    const DEFAULT_TERMINAL_TITLE: &'static str = "Code";

    pub(crate) fn new(
        config: Config,
        initial_prompt: Option<String>,
        initial_images: Vec<std::path::PathBuf>,
        show_trust_screen: bool,
        debug: bool,
        show_order_overlay: bool,
        terminal_info: TerminalInfo,
        enable_perf: bool,
        resume_picker: bool,
        startup_footer_notice: Option<String>,
        latest_upgrade_version: Option<String>,
    ) -> Self {
        let auth_manager = AuthManager::shared_with_mode_and_originator(
            config.code_home.clone(),
            AuthMode::ApiKey,
            config.responses_originator_header.clone(),
        );
        let conversation_manager = Arc::new(ConversationManager::new(
            auth_manager.clone(),
            SessionSource::Cli,
        ));

        // Split queues so interactive input never waits behind bulk updates.
        let (high_tx, app_event_rx_high) = channel();
        let (bulk_tx, app_event_rx_bulk) = channel();
        let app_event_tx = AppEventSender::new_dual(high_tx.clone(), bulk_tx.clone());
        let pending_redraw = Arc::new(AtomicBool::new(false));
        let redraw_inflight = Arc::new(AtomicBool::new(false));
        let post_frame_redraw = Arc::new(AtomicBool::new(false));
        let frame_timer = Arc::new(FrameTimer::new());

        let enhanced_keys_supported = supports_keyboard_enhancement().unwrap_or(false);

        // Spawn a dedicated thread for reading the crossterm event loop and
        // re-publishing the events as AppEvents, as appropriate.
        // Create the input thread stop flag up front so we can store it on `Self`.
        let input_running = Arc::new(AtomicBool::new(true));
        #[cfg(unix)]
        let mut sigterm_guard = None;
        #[cfg(unix)]
        let sigterm_flag = Arc::new(AtomicBool::new(false));
        {
            let app_event_tx = app_event_tx.clone();
            let input_running_thread = input_running.clone();
            let drop_release_events = enhanced_keys_supported;
            std::thread::spawn(move || {
                // Track recent typing to temporarily increase poll frequency for low latency.
                let mut last_key_time = Instant::now();
                loop {
                    if !input_running_thread.load(Ordering::Relaxed) { break; }
                    // This timeout is necessary to avoid holding the event lock
                    // that crossterm::event::read() acquires. In particular,
                    // reading the cursor position (crossterm::cursor::position())
                    // needs to acquire the event lock, and so will fail if it
                    // can't acquire it within 2 sec. Resizing the terminal
                    // crashes the app if the cursor position can't be read.
                    // Keep the timeout small to minimize input-to-echo latency.
                    // Dynamically adapt poll timeout: when the user is actively typing,
                    // use a very small timeout to minimize key->echo latency; otherwise
                    // back off to reduce CPU when idle.
                    let hot_typing = Instant::now().duration_since(last_key_time) <= Duration::from_millis(250);
                    let poll_timeout = if hot_typing { Duration::from_millis(2) } else { Duration::from_millis(10) };
                    match crossterm::event::poll(poll_timeout) {
                        Ok(true) => match crossterm::event::read() {
                            Ok(event) => {
                                match event {
                                    crossterm::event::Event::Key(key_event) => {
                                        // Some Windows terminals (e.g., legacy conhost) only report
                                        // `Release` events when keyboard enhancement flags are not
                                        // supported. Preserve those events so onboarding works there.
                                        if !drop_release_events
                                            || matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                                        {
                                            last_key_time = Instant::now();
                                            app_event_tx.send(AppEvent::KeyEvent(key_event));
                                        }
                                    }
                                    crossterm::event::Event::Resize(_, _) => {
                                        app_event_tx.send(AppEvent::RequestRedraw);
                                    }
                                    // When the terminal/tab regains focus, issue a redraw.
                                    // Some terminals clear the alt‑screen buffer on focus switches,
                                    // which can leave the status bar and inline images blank until
                                    // the next resize. A focus‑gain repaint fixes this immediately.
                                    crossterm::event::Event::FocusGained => {
                                        app_event_tx.send(AppEvent::RequestRedraw);
                                    }
                                    crossterm::event::Event::FocusLost => {
                                        // No action needed; keep state as‑is.
                                    }
                                    crossterm::event::Event::Paste(pasted) => {
                                        // Many terminals convert newlines to \r when pasting (e.g., iTerm2),
                                        // but tui-textarea expects \n. Normalize CR to LF.
                                        // [tui-textarea]: https://github.com/rhysd/tui-textarea/blob/4d18622eeac13b309e0ff6a55a46ac6706da68cf/src/textarea.rs#L782-L783
                                        // [iTerm2]: https://github.com/gnachman/iTerm2/blob/5d0c0d9f68523cbd0494dad5422998964a2ecd8d/sources/iTermPasteHelper.m#L206-L216
                                        let pasted = pasted.replace("\r", "\n");
                                        app_event_tx.send(AppEvent::Paste(pasted));
                                    }
                                    crossterm::event::Event::Mouse(mouse_event) => {
                                        app_event_tx.send(AppEvent::MouseEvent(mouse_event));
                                    }
                                    // All other event variants are explicitly handled above.
                                }
                            }
                            Err(err) => {
                                if err.kind() == std::io::ErrorKind::Interrupted {
                                    continue;
                                }
                                tracing::error!("input thread failed to read event: {err}");
                                input_running_thread.store(false, Ordering::Release);
                                app_event_tx.send(AppEvent::ExitRequest);
                                break;
                            }
                        },
                        Ok(false) => {
                            // Timeout expired, no `Event` is available. If the user is typing
                            // keep the loop hot; otherwise sleep briefly to cut idle CPU.
                            if !hot_typing {
                                std::thread::sleep(Duration::from_millis(5));
                            }
                        }
                        Err(err) => {
                            if err.kind() == std::io::ErrorKind::Interrupted {
                                continue;
                            }
                            tracing::error!("input thread failed to poll events: {err}");
                            input_running_thread.store(false, Ordering::Release);
                            app_event_tx.send(AppEvent::ExitRequest);
                            break;
                        }
                    }
                }
            });
        }

        #[cfg(unix)]
        {
            let term_trigger = Arc::new(AtomicBool::new(false));
            let tx = app_event_tx.clone();
            let running_for_thread = input_running.clone();
            let trigger_for_thread = Arc::clone(&term_trigger);
            let flag_for_thread = sigterm_flag.clone();
            let listener = move || {
                while running_for_thread.load(Ordering::Relaxed) {
                    if trigger_for_thread.swap(false, Ordering::SeqCst) {
                        running_for_thread.store(false, Ordering::Release);
                        flag_for_thread.store(true, Ordering::Release);
                        tx.send(AppEvent::ExitRequest);
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            };

            if thread_spawner::spawn_lightweight("sigterm-listener", listener).is_some() {
                match flag::register(SIGTERM, Arc::clone(&term_trigger)) {
                    Ok(sig_id) => {
                        sigterm_guard = Some(sig_id);
                    }
                    Err(err) => {
                        tracing::warn!("failed to register SIGTERM handler: {err}");
                        input_running.store(false, Ordering::Release);
                    }
                }
            } else {
                tracing::warn!("SIGTERM listener spawn skipped: background thread limit reached");
            }
        }

        let login_status = get_login_status(&config);
        let should_show_onboarding =
            should_show_onboarding(login_status, &config, show_trust_screen);
        let app_state = if should_show_onboarding {
            let show_login_screen = should_show_login_screen(login_status, &config);
            let chat_widget_args = ChatWidgetArgs {
                config: config.clone(),
                initial_prompt,
                initial_images,
                enhanced_keys_supported,
                terminal_info: terminal_info.clone(),
                show_order_overlay,
                enable_perf,
                resume_picker,
                latest_upgrade_version: latest_upgrade_version.clone(),
            };
            AppState::Onboarding {
                screen: OnboardingScreen::new(OnboardingScreenArgs {
                    event_tx: app_event_tx.clone(),
                    code_home: config.code_home.clone(),
                    cwd: config.cwd.clone(),
                    show_trust_screen,
                    show_login_screen,
                    chat_widget_args,
                    login_status,
                }),
            }
        } else {
            let mut chat_widget = ChatWidget::new(
                config.clone(),
                app_event_tx.clone(),
                initial_prompt,
                initial_images,
                enhanced_keys_supported,
                terminal_info.clone(),
                show_order_overlay,
                latest_upgrade_version.clone(),
            );
            chat_widget.enable_perf(enable_perf);
            if resume_picker {
                chat_widget.show_resume_picker();
            }
            // Check for initial animations after widget is created
            chat_widget.check_for_initial_animations();
            if let Some(notice) = startup_footer_notice {
                chat_widget.debug_notice(notice);
            }
            AppState::Chat {
                widget: Box::new(chat_widget),
            }
        };

        let file_search = FileSearchManager::new(config.cwd.clone(), app_event_tx.clone());
        let start_in_alt = config.tui.alternate_screen;
        Self {
            _server: conversation_manager,
            app_event_tx,
            app_event_rx_high,
            app_event_rx_bulk,
            app_state,
            config,
            latest_upgrade_version,
            file_search,
            pending_redraw,
            redraw_inflight,
            post_frame_redraw,
            frame_timer,
            input_running,
            enhanced_keys_supported,
            non_enhanced_pressed_keys: HashSet::new(),
            _debug: debug,
            show_order_overlay,
            commit_anim_running: Arc::new(AtomicBool::new(false)),
            terminal_info,
            clear_on_first_frame: true,
            pending_jump_back_ghost_state: None,
            pending_jump_back_history_snapshot: None,
            last_frame_size: None,
            last_esc_time: None,
            timing_enabled: enable_perf,
            timing: TimingStats::default(),
            buffer_diff_profiler: BufferDiffProfiler::new_from_env(),
            alt_screen_active: start_in_alt,
            terminal_runs: HashMap::new(),
            terminal_title_override: None,
            login_flow: None,
            #[cfg(unix)]
            sigterm_guard,
            #[cfg(unix)]
            sigterm_flag,
        }
    }

    #[cfg(unix)]
    pub(crate) fn sigterm_triggered(&self) -> bool {
        self.sigterm_flag.load(Ordering::Relaxed)
    }

    #[cfg(unix)]
    pub(crate) fn clear_sigterm_guard(&mut self) {
        self.sigterm_guard.take();
    }

    fn apply_terminal_title(&self) {
        let title = self
            .terminal_title_override
            .as_deref()
            .unwrap_or(Self::DEFAULT_TERMINAL_TITLE);
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::SetTitle(title.to_string())
        );
    }

    fn sanitize_notification_text(input: &str) -> String {
        let mut sanitized = String::with_capacity(input.len());
        for ch in input.chars() {
            match ch {
                '\u{00}'..='\u{08}' | '\u{0B}' | '\u{0C}' | '\u{0E}'..='\u{1F}' | '\u{7F}' => {}
                '\n' | '\r' | '\t' => {
                    if !sanitized.ends_with(' ') {
                        sanitized.push(' ');
                    }
                }
                _ => sanitized.push(ch),
            }
        }
        sanitized
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn format_notification_message(title: &str, body: Option<&str>) -> Option<String> {
        let title = Self::sanitize_notification_text(title);
        let body = body.map(Self::sanitize_notification_text);
        let mut message = match body {
            Some(ref b) if !b.is_empty() => {
                if title.is_empty() {
                    b.clone()
                } else {
                    format!("{}: {}", title, b)
                }
            }
            _ => title.clone(),
        };

        if message.is_empty() {
            return None;
        }

        const MAX_LEN: usize = 160;
        if message.chars().count() > MAX_LEN {
            let mut truncated = String::new();
            for ch in message.chars() {
                if truncated.chars().count() >= MAX_LEN.saturating_sub(3) {
                    break;
                }
                truncated.push(ch);
            }
            truncated.push_str("...");
            message = truncated;
        }

        Some(message)
    }

    fn emit_osc9_notification(message: &str) {
        let payload = format!("\u{1b}]9;{}\u{7}", message);
        let mut stdout = std::io::stdout();
        let _ = stdout.write_all(payload.as_bytes());
        let _ = stdout.flush();
    }


    /// Schedule a redraw immediately and open a short debounce window to coalesce
    /// subsequent requests. Crucially, even if a timer is already armed (e.g., an
    /// animation scheduled a future frame), we still trigger an immediate redraw
    /// to keep keypress echo latency low.
    #[allow(clippy::unwrap_used)]
    fn schedule_redraw(&self) {
        // Only queue a new frame when one is not already in flight; otherwise record
        // that we owe a follow-up immediately after the active frame completes.
        let should_send = self
            .redraw_inflight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok();
        if should_send {
            self.app_event_tx.send(AppEvent::Redraw);
        } else {
            self.post_frame_redraw.store(true, Ordering::Release);
        }

        // Arm debounce window if not already armed.
        if self
            .pending_redraw
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            let pending_redraw = self.pending_redraw.clone();
            let pending_redraw_for_thread = pending_redraw.clone();
            if thread_spawner::spawn_lightweight("redraw-debounce", move || {
                thread::sleep(REDRAW_DEBOUNCE);
                pending_redraw_for_thread.store(false, Ordering::Release);
            })
            .is_none()
            {
                pending_redraw.store(false, Ordering::Release);
            }
        }
    }
    
    /// Schedule a redraw after the specified duration.
    fn schedule_redraw_in(&self, duration: Duration) {
        self.frame_timer
            .schedule(duration, self.app_event_tx.clone());
    }

    fn handle_login_mode_change(&mut self, using_chatgpt_auth: bool) {
        self.config.using_chatgpt_auth = using_chatgpt_auth;
        if let AppState::Chat { widget } = &mut self.app_state {
            widget.set_using_chatgpt_auth(using_chatgpt_auth);
            let _ = widget.reload_auth();
        }
    }

    fn start_terminal_run(
        &mut self,
        id: u64,
        command: Vec<String>,
        display: Option<String>,
        controller: Option<TerminalRunController>,
    ) {
        if command.is_empty() {
            self.app_event_tx.send(AppEvent::TerminalChunk {
                id,
                chunk: b"Install command not resolved".to_vec(),
                _is_stderr: true,
            });
            self.app_event_tx.send(AppEvent::TerminalExit {
                id,
                exit_code: Some(1),
                _duration: Duration::from_millis(0),
            });
            return;
        }

        let joined_display = try_join(command.iter().map(|s| s.as_str()))
            .ok()
            .unwrap_or_else(|| command.join(" "));

        let display_line = display.clone().unwrap_or_else(|| joined_display.clone());

        if !display_line.trim().is_empty() {
            let line = format!("$ {display_line}\n");
            self.app_event_tx.send(AppEvent::TerminalChunk {
                id,
                chunk: line.into_bytes(),
                _is_stderr: false,
            });
        }

        let stored_command = command.clone();
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let (writer_tx_raw, writer_rx) = channel::<Vec<u8>>();
        let writer_tx_shared = Arc::new(Mutex::new(Some(writer_tx_raw)));
        let controller_clone = controller.clone();
        let cwd = self.config.cwd.clone();
        let controller_tx = controller.map(|c| c.tx);

        let (pty_rows, pty_cols) = match &self.app_state {
            AppState::Chat { widget } => widget
                .terminal_dimensions_hint()
                .unwrap_or((DEFAULT_PTY_ROWS, DEFAULT_PTY_COLS)),
            _ => (DEFAULT_PTY_ROWS, DEFAULT_PTY_COLS),
        };

        let pty_system = native_pty_system();
        let pair = match pty_system.openpty(PtySize {
            rows: pty_rows,
            cols: pty_cols,
            pixel_width: 0,
            pixel_height: 0,
        }) {
            Ok(pair) => pair,
            Err(err) => {
                let msg = format!("Failed to open PTY: {err}\n");
                self.app_event_tx.send(AppEvent::TerminalChunk {
                    id,
                    chunk: msg.clone().into_bytes(),
                    _is_stderr: true,
                });
                if let Some(ref ctrl) = controller_tx {
                    let _ = ctrl.send(TerminalRunEvent::Chunk {
                        data: msg.clone().into_bytes(),
                        _is_stderr: true,
                    });
                    let _ = ctrl.send(TerminalRunEvent::Exit {
                        exit_code: Some(1),
                        _duration: Duration::from_millis(0),
                    });
                }
                self.app_event_tx.send(AppEvent::TerminalExit {
                    id,
                    exit_code: Some(1),
                    _duration: Duration::from_millis(0),
                });
                return;
            }
        };

        let PtyPair { master, slave } = pair;
        let master = Arc::new(Mutex::new(master));

        let writer = {
            let guard = match master.lock() {
                Ok(guard) => guard,
                Err(_) => {
                    let msg = "Failed to acquire terminal writer: poisoned lock\n".to_string();
                    self.app_event_tx.send(AppEvent::TerminalChunk {
                        id,
                        chunk: msg.clone().into_bytes(),
                        _is_stderr: true,
                    });
                    if let Some(ref ctrl) = controller_tx {
                        let _ = ctrl.send(TerminalRunEvent::Chunk {
                            data: msg.clone().into_bytes(),
                            _is_stderr: true,
                        });
                        let _ = ctrl.send(TerminalRunEvent::Exit {
                            exit_code: Some(1),
                            _duration: Duration::from_millis(0),
                        });
                    }
                    self.app_event_tx.send(AppEvent::TerminalExit {
                        id,
                        exit_code: Some(1),
                        _duration: Duration::from_millis(0),
                    });
                    return;
                }
            };
            let result = guard.take_writer();
            drop(guard);
            match result {
                Ok(writer) => writer,
                Err(err) => {
                    let msg = format!("Failed to acquire terminal writer: {err}\n");
                    self.app_event_tx.send(AppEvent::TerminalChunk {
                        id,
                        chunk: msg.clone().into_bytes(),
                        _is_stderr: true,
                    });
                    if let Some(ref ctrl) = controller_tx {
                        let _ = ctrl.send(TerminalRunEvent::Chunk {
                            data: msg.clone().into_bytes(),
                            _is_stderr: true,
                        });
                        let _ = ctrl.send(TerminalRunEvent::Exit {
                            exit_code: Some(1),
                            _duration: Duration::from_millis(0),
                        });
                    }
                    self.app_event_tx.send(AppEvent::TerminalExit {
                        id,
                        exit_code: Some(1),
                        _duration: Duration::from_millis(0),
                    });
                    return;
                }
            }
        };

        let reader = {
            let guard = match master.lock() {
                Ok(guard) => guard,
                Err(_) => {
                    let msg = "Failed to read terminal output: poisoned lock\n".to_string();
                    self.app_event_tx.send(AppEvent::TerminalChunk {
                        id,
                        chunk: msg.clone().into_bytes(),
                        _is_stderr: true,
                    });
                    if let Some(ref ctrl) = controller_tx {
                        let _ = ctrl.send(TerminalRunEvent::Chunk {
                            data: msg.clone().into_bytes(),
                            _is_stderr: true,
                        });
                        let _ = ctrl.send(TerminalRunEvent::Exit {
                            exit_code: Some(1),
                            _duration: Duration::from_millis(0),
                        });
                    }
                    self.app_event_tx.send(AppEvent::TerminalExit {
                        id,
                        exit_code: Some(1),
                        _duration: Duration::from_millis(0),
                    });
                    return;
                }
            };
            let result = guard.try_clone_reader();
            drop(guard);
            match result {
                Ok(reader) => reader,
                Err(err) => {
                    let msg = format!("Failed to read terminal output: {err}\n");
                    self.app_event_tx.send(AppEvent::TerminalChunk {
                        id,
                        chunk: msg.clone().into_bytes(),
                        _is_stderr: true,
                    });
                    if let Some(ref ctrl) = controller_tx {
                        let _ = ctrl.send(TerminalRunEvent::Chunk {
                            data: msg.clone().into_bytes(),
                            _is_stderr: true,
                        });
                        let _ = ctrl.send(TerminalRunEvent::Exit {
                            exit_code: Some(1),
                            _duration: Duration::from_millis(0),
                        });
                    }
                    self.app_event_tx.send(AppEvent::TerminalExit {
                        id,
                        exit_code: Some(1),
                        _duration: Duration::from_millis(0),
                    });
                    return;
                }
            }
        };

        let mut command_builder = CommandBuilder::new(command[0].clone());
        for arg in &command[1..] {
            command_builder.arg(arg);
        }
        command_builder.cwd(&cwd);

        let mut child = match slave.spawn_command(command_builder) {
            Ok(child) => child,
            Err(err) => {
                let msg = format!("Failed to spawn command: {err}\n");
                self.app_event_tx.send(AppEvent::TerminalChunk {
                    id,
                    chunk: msg.clone().into_bytes(),
                    _is_stderr: true,
                });
                if let Some(ref ctrl) = controller_tx {
                    let _ = ctrl.send(TerminalRunEvent::Chunk {
                        data: msg.clone().into_bytes(),
                        _is_stderr: true,
                    });
                    let _ = ctrl.send(TerminalRunEvent::Exit {
                        exit_code: Some(1),
                        _duration: Duration::from_millis(0),
                    });
                }
                self.app_event_tx.send(AppEvent::TerminalExit {
                    id,
                    exit_code: Some(1),
                    _duration: Duration::from_millis(0),
                });
                return;
            }
        };

        let mut killer = child.clone_killer();

        let master_for_state = Arc::clone(&master);
        self.terminal_runs.insert(
            id,
            TerminalRunState {
                command: stored_command,
                display: display_line.clone(),
                cancel_tx: Some(cancel_tx),
                running: true,
                controller: controller_clone,
                writer_tx: Some(writer_tx_shared.clone()),
                pty: Some(master_for_state),
            },
        );

        let tx = self.app_event_tx.clone();
        let controller_tx_task = controller_tx.clone();
        let master_for_task = Arc::clone(&master);
        let writer_tx_for_task = writer_tx_shared.clone();
        tokio::spawn(async move {
            let start_time = Instant::now();
            let controller_tx = controller_tx_task;
            let _master = master_for_task;

            let writer_handle = tokio::task::spawn_blocking(move || {
                let mut writer = writer;
                while let Ok(bytes) = writer_rx.recv() {
                    if writer.write_all(&bytes).is_err() {
                        break;
                    }
                    if writer.flush().is_err() {
                        break;
                    }
                }
            });

            let tx_reader = tx.clone();
            let controller_tx_reader = controller_tx.clone();
            let reader_handle = tokio::task::spawn_blocking(move || {
                let mut buf = [0u8; 8192];
                let mut reader = reader;
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let chunk = buf[..n].to_vec();
                            tx_reader.send(AppEvent::TerminalChunk {
                                id,
                                chunk: chunk.clone(),
                                _is_stderr: false,
                            });
                            if let Some(ref ctrl) = controller_tx_reader {
                                let _ = ctrl.send(TerminalRunEvent::Chunk {
                                    data: chunk,
                                    _is_stderr: false,
                                });
                            }
                        }
                        Err(err) => {
                            let msg = format!("Error reading terminal output: {err}\n");
                            tx_reader.send(AppEvent::TerminalChunk {
                                id,
                                chunk: msg.clone().into_bytes(),
                                _is_stderr: true,
                            });
                            if let Some(ref ctrl) = controller_tx_reader {
                                let _ = ctrl.send(TerminalRunEvent::Chunk {
                                    data: msg.into_bytes(),
                                    _is_stderr: true,
                                });
                            }
                            break;
                        }
                    }
                }
            });

            let mut cancel_rx = cancel_rx.fuse();
            let mut cancel_triggered = false;
            let wait_handle = tokio::task::spawn_blocking(move || child.wait());
            futures::pin_mut!(wait_handle);
            let wait_status = loop {
                tokio::select! {
                    res = &mut wait_handle => break res,
                    res = &mut cancel_rx, if !cancel_triggered => {
                        if res.is_ok() {
                            cancel_triggered = true;
                            let _ = killer.kill();
                        }
                    }
                }
            };

            {
                let mut guard = writer_tx_for_task.lock().unwrap();
                guard.take();
            }

            let _ = reader_handle.await;
            let _ = writer_handle.await;

            let (exit_code, duration) = match wait_status {
                Ok(Ok(status)) => (Some(status.exit_code() as i32), start_time.elapsed()),
                Ok(Err(err)) => {
                    let msg = format!("Process wait failed: {err}\n");
                    tx.send(AppEvent::TerminalChunk {
                        id,
                        chunk: msg.clone().into_bytes(),
                        _is_stderr: true,
                    });
                    if let Some(ref ctrl) = controller_tx {
                        let _ = ctrl.send(TerminalRunEvent::Chunk {
                            data: msg.clone().into_bytes(),
                            _is_stderr: true,
                        });
                    }
                    (None, start_time.elapsed())
                }
                Err(err) => {
                    let msg = format!("Process join failed: {err}\n");
                    tx.send(AppEvent::TerminalChunk {
                        id,
                        chunk: msg.clone().into_bytes(),
                        _is_stderr: true,
                    });
                    if let Some(ref ctrl) = controller_tx {
                        let _ = ctrl.send(TerminalRunEvent::Chunk {
                            data: msg.clone().into_bytes(),
                            _is_stderr: true,
                        });
                    }
                    (None, start_time.elapsed())
                }
            };

            if let Some(ref ctrl) = controller_tx {
                let _ = ctrl.send(TerminalRunEvent::Exit {
                    exit_code,
                    _duration: duration,
                });
            }
            tx.send(AppEvent::TerminalExit {
                id,
                exit_code,
                _duration: duration,
            });
        });
    }

    pub(crate) fn run(&mut self, terminal: &mut tui::Tui) -> Result<()> {
        // Insert an event to trigger the first render.
        let app_event_tx = self.app_event_tx.clone();
        app_event_tx.send(AppEvent::RequestRedraw);
        // Some Windows/macOS terminals report an initial size that stabilizes
        // shortly after entering the alt screen. Schedule one follow‑up frame
        // to catch any late size change without polling.
        app_event_tx.send(AppEvent::ScheduleFrameIn(Duration::from_millis(120)));

        'main: loop {
            let event = match self.next_event_priority() { Some(e) => e, None => break 'main };
            match event {
                AppEvent::InsertHistory(mut lines) => match &mut self.app_state {
                    AppState::Chat { widget } => {
                        // Coalesce consecutive InsertHistory events to reduce redraw churn.
                        while let Ok(AppEvent::InsertHistory(mut more)) = self.app_event_rx_bulk.try_recv() {
                            lines.append(&mut more);
                        }
                        tracing::debug!("app: InsertHistory lines={}", lines.len());
                        if self.alt_screen_active {
                            widget.insert_history_lines(lines)
                        } else {
                            use std::io::stdout;
                            // Compute desired bottom height now, so growing/shrinking input
                            // adjusts the reserved region immediately even before the next frame.
                            let width = terminal.size().map(|s| s.width).unwrap_or(80);
                            let reserve = widget.desired_bottom_height(width).max(1);
                            let _ = execute!(stdout(), crossterm::terminal::BeginSynchronizedUpdate);
                            crate::insert_history::insert_history_lines_above(terminal, reserve, lines);
                            let _ = execute!(stdout(), crossterm::terminal::EndSynchronizedUpdate);
                            self.schedule_redraw();
                        }
                    },
                    AppState::Onboarding { .. } => {}
                },
                AppEvent::InsertHistoryWithKind { id, kind, lines } => match &mut self.app_state {
                    AppState::Chat { widget } => {
                        tracing::debug!("app: InsertHistoryWithKind kind={:?} id={:?} lines={}", kind, id, lines.len());
                        // Always update widget history, even in terminal mode.
                        // In terminal mode, the widget will emit an InsertHistory event
                        // which we will mirror to scrollback in the handler above.
                        let to_mirror = lines.clone();
                        widget.insert_history_lines_with_kind(kind, id, lines);
                        if !self.alt_screen_active {
                            use std::io::stdout;
                            let width = terminal.size().map(|s| s.width).unwrap_or(80);
                            let reserve = widget.desired_bottom_height(width).max(1);
                            let _ = execute!(stdout(), crossterm::terminal::BeginSynchronizedUpdate);
                            crate::insert_history::insert_history_lines_above(terminal, reserve, to_mirror);
                            let _ = execute!(stdout(), crossterm::terminal::EndSynchronizedUpdate);
                            self.schedule_redraw();
                        }
                    },
                    AppState::Onboarding { .. } => {}
                },
                AppEvent::InsertFinalAnswer { id, lines, source } => match &mut self.app_state {
                    AppState::Chat { widget } => {
                        tracing::debug!("app: InsertFinalAnswer id={:?} lines={} source_len={}", id, lines.len(), source.len());
                        let to_mirror = lines.clone();
                        widget.insert_final_answer_with_id(id, lines, source);
                        if !self.alt_screen_active {
                            use std::io::stdout;
                            let width = terminal.size().map(|s| s.width).unwrap_or(80);
                            let reserve = widget.desired_bottom_height(width).max(1);
                            let _ = execute!(stdout(), crossterm::terminal::BeginSynchronizedUpdate);
                            crate::insert_history::insert_history_lines_above(terminal, reserve, to_mirror);
                            let _ = execute!(stdout(), crossterm::terminal::EndSynchronizedUpdate);
                            self.schedule_redraw();
                        }
                    },
                    AppState::Onboarding { .. } => {}
                },
                AppEvent::InsertBackgroundEvent { message, placement, order } => match &mut self.app_state {
                    AppState::Chat { widget } => {
                        tracing::debug!(
                            "app: InsertBackgroundEvent placement={:?} len={}",
                            placement,
                            message.len()
                        );
                        widget.insert_background_event_with_placement(message, placement, order);
                    }
                    AppState::Onboarding { .. } => {}
                },
                AppEvent::AutoUpgradeCompleted { version } => match &mut self.app_state {
                    AppState::Chat { widget } => widget.on_auto_upgrade_completed(version),
                    AppState::Onboarding { .. } => {}
                },
                AppEvent::RateLimitFetchFailed { message } => match &mut self.app_state {
                    AppState::Chat { widget } => widget.on_rate_limit_refresh_failed(message),
                    AppState::Onboarding { .. } => {}
                },
                AppEvent::RequestRedraw => {
                    self.schedule_redraw();
                }
                AppEvent::FlushPendingExecEnds => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.flush_pending_exec_ends();
                    }
                    self.schedule_redraw();
                }
                AppEvent::Redraw => {
                    if self.timing_enabled { self.timing.on_redraw_begin(); }
                    let t0 = Instant::now();
                    let draw_result = std::io::stdout().sync_update(|_| self.draw_next_frame(terminal));
                    self.redraw_inflight.store(false, Ordering::Release);
                    let needs_follow_up = self.post_frame_redraw.swap(false, Ordering::AcqRel);
                    if needs_follow_up {
                        self.schedule_redraw();
                    }
                    draw_result??;
                    if self.timing_enabled { self.timing.on_redraw_end(t0); }
                }
                AppEvent::StartCommitAnimation => {
                    if self
                        .commit_anim_running
                        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                        .is_ok()
                    {
                        let tx = self.app_event_tx.clone();
                        let running = self.commit_anim_running.clone();
                        let running_for_thread = running.clone();
                        let tick_ms: u64 = self
                            .config
                            .tui
                            .stream
                            .commit_tick_ms
                            .or(if self.config.tui.stream.responsive { Some(30) } else { None })
                            .unwrap_or(50);
                        if thread_spawner::spawn_lightweight("commit-anim", move || {
                            while running_for_thread.load(Ordering::Relaxed) {
                                thread::sleep(Duration::from_millis(tick_ms));
                                tx.send(AppEvent::CommitTick);
                            }
                        })
                        .is_none()
                        {
                            running.store(false, Ordering::Release);
                        }
                    }
                }
                AppEvent::StopCommitAnimation => {
                    self.commit_anim_running.store(false, Ordering::Release);
                }
                AppEvent::CommitTick => {
                    if self.pending_redraw.load(Ordering::Relaxed) { continue; }
                    // Advance streaming animation: commit at most one queued line
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.on_commit_tick();
                    }
                }
                AppEvent::KeyEvent(mut key_event) => {
                    if self.timing_enabled { self.timing.on_key(); }
                    #[cfg(windows)]
                    {
                        use crossterm::event::KeyCode;
                        use crossterm::event::KeyEventKind;
                        if matches!(key_event.kind, KeyEventKind::Repeat) {
                            match key_event.code {
                                KeyCode::Left
                                | KeyCode::Right
                                | KeyCode::Up
                                | KeyCode::Down
                                | KeyCode::Home
                                | KeyCode::End
                                | KeyCode::Backspace
                                | KeyCode::Delete => {}
                                _ => continue,
                            }
                        }
                    }
                    // On terminals without keyboard enhancement flags (notably some Windows
                    // Git Bash/mintty setups), crossterm may emit duplicate key-up events or
                    // only report releases. Track which keys were seen as pressed so matching
                    // releases can be dropped, and synthesize a press when a release arrives
                    // without a prior press.
                    if !self.enhanced_keys_supported {
                        let key_code = key_event.code.clone();
                        match key_event.kind {
                            KeyEventKind::Press | KeyEventKind::Repeat => {
                                self.non_enhanced_pressed_keys.insert(key_code);
                            }
                            KeyEventKind::Release => {
                                if self.non_enhanced_pressed_keys.remove(&key_code) {
                                    continue;
                                }

                                let mut release_handled = false;
                                if let KeyCode::Char(ch) = key_code {
                                    let alts: Vec<char> = ch
                                        .to_lowercase()
                                        .chain(ch.to_uppercase())
                                        .filter(|&c| c != ch)
                                        .collect();

                                    for alt in alts {
                                        if self
                                            .non_enhanced_pressed_keys
                                            .remove(&KeyCode::Char(alt))
                                        {
                                            release_handled = true;
                                            break;
                                        }
                                    }
                                }

                                if release_handled {
                                    continue;
                                }

                                key_event = KeyEvent::new(
                                    Self::normalize_non_enhanced_release_code(key_event.code),
                                    key_event.modifiers,
                                );
                            }
                        }
                    }
                    // Reset double‑Esc timer on any non‑Esc key
                    if !matches!(key_event.code, KeyCode::Esc) {
                        self.last_esc_time = None;
                    }

                    match key_event {
                        KeyEvent { code: KeyCode::Esc, kind: KeyEventKind::Press | KeyEventKind::Repeat, .. } => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                let now = Instant::now();
                                const THRESHOLD: Duration = Duration::from_millis(600);
                                let double_ready = self
                                    .last_esc_time
                                    .is_some_and(|prev| now.duration_since(prev) <= THRESHOLD);

                                let mut handled = false;
                                let mut attempts = 0;
                                let esc_event = key_event;

                                while attempts < 8 {
                                    attempts += 1;
                                    let route = widget.describe_esc_context();
                                    let mut intent = route.intent;

                                    if intent == EscIntent::None {
                                        break;
                                    }

                                    if intent == EscIntent::ShowUndoHint
                                        && route.allows_double_esc
                                        && double_ready
                                    {
                                        intent = EscIntent::OpenUndoTimeline;
                                    }

                                    let performed = widget.execute_esc_intent(intent, esc_event);

                                    match intent {
                                        EscIntent::CloseFilePopup if !route.consume => {
                                            if !performed {
                                                break;
                                            }
                                            continue;
                                        }
                                        EscIntent::CloseFilePopup => {
                                            handled = true;
                                            break;
                                        }
                                        EscIntent::ShowUndoHint => {
                                            if route.allows_double_esc && !double_ready {
                                                self.last_esc_time = Some(now);
                                            } else {
                                                self.last_esc_time = None;
                                            }
                                            handled = true;
                                            break;
                                        }
                                        EscIntent::OpenUndoTimeline => {
                                            self.last_esc_time = None;
                                            handled = true;
                                            break;
                                        }
                                        EscIntent::CancelTask | EscIntent::ClearComposer => {
                                            self.last_esc_time = Some(now);
                                            handled = true;
                                            break;
                                        }
                                        _ => {
                                            handled = true;
                                            break;
                                        }
                                    }
                                }

                                if handled {
                                    continue;
                                }
                            }
                            // Otherwise fall through
                        }
                        // Fallback: attempt clipboard image paste on common shortcuts.
                        // Many terminals (e.g., iTerm2) do not emit Event::Paste for raw-image
                        // clipboards. When the user presses paste shortcuts, try an image read
                        // by dispatching a paste with an empty string. The composer will then
                        // attempt `paste_image_to_temp_png()` and no-op if no image exists.
                        KeyEvent {
                            code: KeyCode::Char('v'),
                            modifiers: crossterm::event::KeyModifiers::CONTROL,
                            kind: KeyEventKind::Press | KeyEventKind::Repeat,
                            ..
                        } => {
                            self.dispatch_paste_event(String::new());
                        }
                        KeyEvent {
                            code: KeyCode::Char('v'),
                            modifiers: m,
                            kind: KeyEventKind::Press | KeyEventKind::Repeat,
                            ..
                        } if m.contains(crossterm::event::KeyModifiers::CONTROL)
                            && m.contains(crossterm::event::KeyModifiers::SHIFT) =>
                        {
                            self.dispatch_paste_event(String::new());
                        }
                        KeyEvent {
                            code: KeyCode::Insert,
                            modifiers: crossterm::event::KeyModifiers::SHIFT,
                            kind: KeyEventKind::Press | KeyEventKind::Repeat,
                            ..
                        } => {
                            self.dispatch_paste_event(String::new());
                        }
                        KeyEvent {
                            code: KeyCode::Char('m'),
                            modifiers: crossterm::event::KeyModifiers::CONTROL,
                            kind: KeyEventKind::Press,
                            ..
                        } => {
                            // Toggle mouse capture to allow text selection
                            use crossterm::event::DisableMouseCapture;
                            use crossterm::event::EnableMouseCapture;
                            use crossterm::execute;
                            use std::io::stdout;

                            // Static variable to track mouse capture state
                            static mut MOUSE_CAPTURE_ENABLED: bool = true;

                            unsafe {
                                MOUSE_CAPTURE_ENABLED = !MOUSE_CAPTURE_ENABLED;
                                if MOUSE_CAPTURE_ENABLED {
                                    let _ = execute!(stdout(), EnableMouseCapture);
                                } else {
                                    let _ = execute!(stdout(), DisableMouseCapture);
                                }
                            }
                            self.app_event_tx.send(AppEvent::RequestRedraw);
                        }
                        KeyEvent {
                            code: KeyCode::Char('c'),
                            modifiers: crossterm::event::KeyModifiers::CONTROL,
                            kind: KeyEventKind::Press,
                            ..
                        } => match &mut self.app_state {
                            AppState::Chat { widget } => {
                                match widget.on_ctrl_c() {
                                    crate::bottom_pane::CancellationEvent::Handled => {
                                        if widget.ctrl_c_requests_exit() {
                                            self.app_event_tx.send(AppEvent::ExitRequest);
                                        }
                                    }
                                    crate::bottom_pane::CancellationEvent::Ignored => {}
                                }
                            }
                            AppState::Onboarding { .. } => { self.app_event_tx.send(AppEvent::ExitRequest); }
                        },
                        KeyEvent {
                            code: KeyCode::Char('z'),
                            modifiers: crossterm::event::KeyModifiers::CONTROL,
                            kind: KeyEventKind::Press,
                            ..
                        } => {
                            // Prefer in-app undo in Chat (composer) over shell suspend.
                            match &mut self.app_state {
                                AppState::Chat { widget } => {
                                    widget.handle_key_event(key_event);
                                    self.app_event_tx.send(AppEvent::RequestRedraw);
                                }
                                AppState::Onboarding { .. } => {
                                    #[cfg(unix)]
                                    {
                                        self.suspend(terminal)?;
                                    }
                                    // No-op on non-Unix platforms.
                                }
                            }
                        }
                        KeyEvent {
                            code: KeyCode::Char('r'),
                            modifiers: crossterm::event::KeyModifiers::CONTROL,
                            kind: KeyEventKind::Press,
                            ..
                        }
                        | KeyEvent {
                            code: KeyCode::Char('r'),
                            modifiers: crossterm::event::KeyModifiers::CONTROL,
                            kind: KeyEventKind::Repeat,
                            ..
                        } => {
                            // Toggle reasoning/thinking visibility (Ctrl+R)
                            match &mut self.app_state {
                                AppState::Chat { widget } => {
                                    widget.toggle_reasoning_visibility();
                                }
                                AppState::Onboarding { .. } => {}
                            }
                        }
                        KeyEvent {
                            code: KeyCode::Char('t'),
                            modifiers: crossterm::event::KeyModifiers::CONTROL,
                            kind: KeyEventKind::Press | KeyEventKind::Repeat,
                            ..
                        } => {
                            let _ = self.toggle_screen_mode(terminal);
                            // Propagate mode to widget so it can adapt layout
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.set_standard_terminal_mode(!self.alt_screen_active);
                            }
                        }
                        KeyEvent {
                            code: KeyCode::Char('d'),
                            modifiers: crossterm::event::KeyModifiers::CONTROL,
                            kind: KeyEventKind::Press,
                            ..
                        } => {
                            // Toggle diffs overlay
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.toggle_diffs_popup();
                            }
                        }
                        // (Ctrl+Y disabled): Previously cycled syntax themes; now intentionally no-op
                        KeyEvent {
                            kind: KeyEventKind::Press | KeyEventKind::Repeat,
                            ..
                        } => {
                            self.dispatch_key_event(key_event);
                        }
                        _ => {
                            // Ignore Release key events.
                        }
                    };
                }
                AppEvent::MouseEvent(mouse_event) => {
                    self.dispatch_mouse_event(mouse_event);
                }
                AppEvent::Paste(text) => {
                    self.dispatch_paste_event(text);
                }
                AppEvent::RegisterPastedImage { placeholder, path } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.register_pasted_image(placeholder, path);
                    }
                }
                AppEvent::CodexEvent(event) => {
                    self.dispatch_code_event(event);
                }
                AppEvent::ExitRequest => {
                    // Stop background threads and break the UI loop.
                    self.commit_anim_running.store(false, Ordering::Release);
                    self.input_running.store(false, Ordering::Release);
                    break 'main;
                }
                AppEvent::CancelRunningTask => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.cancel_running_task_from_approval();
                    }
                }
                AppEvent::RegisterApprovedCommand { command, match_kind, persist, semantic_prefix } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.register_approved_command(
                            command.clone(),
                            match_kind.clone(),
                            semantic_prefix.clone(),
                        );
                        if persist {
                            if let Err(err) = add_project_allowed_command(
                                &self.config.code_home,
                                &self.config.cwd,
                                &command,
                                match_kind.clone(),
                            ) {
                                widget.history_push_plain_state(history_cell::new_error_event(format!(
                                    "Failed to persist always-allow command: {err:#}",
                                )));
                            } else {
                                let display = strip_bash_lc_and_escape(&command);
                                widget.push_background_tail(format!(
                                    "Always allowing `{display}` for this project.",
                                ));
                            }
                        }
                    }
                }
                AppEvent::MarkTaskIdle => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.mark_task_idle_after_denied();
                    }
                }
                AppEvent::OpenTerminal(launch) => {
                    let mut spawn = None;
                    let requires_immediate_command = !launch.command.is_empty();
                    let restricted = !matches!(self.config.sandbox_policy, SandboxPolicy::DangerFullAccess);
                    if let AppState::Chat { widget } = &mut self.app_state {
                        if restricted && requires_immediate_command {
                            widget.history_push_plain_state(history_cell::new_error_event(
                                "Terminal requires Full Access to auto-run install commands.".to_string(),
                            ));
                            widget.show_agents_overview_ui();
                        } else {
                            widget.terminal_open(&launch);
                            if requires_immediate_command {
                                spawn = Some((
                                    launch.id,
                                    launch.command.clone(),
                                    Some(launch.command_display.clone()),
                                    launch.controller.clone(),
                                ));
                            }
                        }
                    }
                    if let Some((id, command, display, controller)) = spawn {
                        self.start_terminal_run(id, command, display, controller);
                    }
                }
                AppEvent::TerminalChunk {
                    id,
                    chunk,
                    _is_stderr: is_stderr,
                } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.terminal_append_chunk(id, &chunk, is_stderr);
                    }
                }
                AppEvent::TerminalExit {
                    id,
                    exit_code,
                    _duration: duration,
                } => {
                    let after = if let AppState::Chat { widget } = &mut self.app_state {
                        widget.terminal_finalize(id, exit_code, duration)
                    } else {
                        None
                    };
                    let controller_present = if let Some(run) = self.terminal_runs.get_mut(&id) {
                        run.running = false;
                        run.cancel_tx = None;
                        if let Some(writer_shared) = run.writer_tx.take() {
                            let mut guard = writer_shared.lock().unwrap();
                            guard.take();
                        }
                        run.pty = None;
                        run.controller.is_some()
                    } else {
                        false
                    };
                    if exit_code == Some(0) && !controller_present {
                        self.terminal_runs.remove(&id);
                    }
                    if let Some(after) = after {
                        self.app_event_tx.send(AppEvent::TerminalAfter(after));
                    }
                }
                AppEvent::TerminalCancel { id } => {
                    let mut remove_entry = false;
                    if let Some(run) = self.terminal_runs.get_mut(&id) {
                        let had_controller = run.controller.is_some();
                        if let Some(tx) = run.cancel_tx.take() {
                            if !tx.is_closed() {
                                let _ = tx.send(());
                            }
                        }
                        run.running = false;
                        run.controller = None;
                        if let Some(writer_shared) = run.writer_tx.take() {
                            let mut guard = writer_shared.lock().unwrap();
                            guard.take();
                        }
                        run.pty = None;
                        remove_entry = had_controller;
                    }
                    if remove_entry {
                        self.terminal_runs.remove(&id);
                    }
                }
                AppEvent::TerminalRerun { id } => {
                    let command_and_controller = self
                        .terminal_runs
                        .get(&id)
                        .and_then(|run| {
                            (!run.running).then(|| {
                                (
                                    run.command.clone(),
                                    run.display.clone(),
                                    run.controller.clone(),
                                )
                            })
                        });
                    if let Some((command, display, controller)) = command_and_controller {
                        if let AppState::Chat { widget } = &mut self.app_state {
                            widget.terminal_mark_running(id);
                        }
                        self.start_terminal_run(id, command, Some(display), controller);
                    }
                }
                AppEvent::TerminalRunCommand {
                    id,
                    command,
                    command_display,
                    controller,
                } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.terminal_set_command_display(id, command_display.clone());
                        widget.terminal_mark_running(id);
                    }
                    self.start_terminal_run(id, command, Some(command_display), controller);
                }
                AppEvent::TerminalSendInput { id, data } => {
                    if let Some(run) = self.terminal_runs.get_mut(&id) {
                        if let Some(writer_shared) = run.writer_tx.as_ref() {
                            let mut guard = writer_shared.lock().unwrap();
                            if let Some(tx) = guard.as_ref() {
                                if tx.send(data).is_err() {
                                    guard.take();
                                }
                            }
                        }
                    }
                }
                AppEvent::TerminalResize { id, rows, cols } => {
                    if rows == 0 || cols == 0 {
                        continue;
                    }
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.terminal_apply_resize(id, rows, cols);
                    }
                    if let Some(run) = self.terminal_runs.get(&id) {
                        if let Some(pty) = run.pty.as_ref() {
                            if let Ok(guard) = pty.lock() {
                                let _ = guard.resize(PtySize {
                                    rows,
                                    cols,
                                    pixel_width: 0,
                                    pixel_height: 0,
                                });
                            }
                        }
                    }
                }
                AppEvent::TerminalUpdateMessage { id, message } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.terminal_update_message(id, message);
                    }
                }
                AppEvent::TerminalSetAssistantMessage { id, message } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.terminal_set_assistant_message(id, message);
                    }
                }
                AppEvent::TerminalAwaitCommand { id, suggestion, ack } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.terminal_prepare_command(id, suggestion, ack.0);
                    }
                }
                AppEvent::TerminalForceClose { id } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.close_terminal_overlay();
                    }
                    self.terminal_runs.remove(&id);
                }
                AppEvent::TerminalApprovalDecision { id, approved } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.handle_terminal_approval_decision(id, approved);
                    }
                }
                AppEvent::StartAutoDriveCelebration { message } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.start_auto_drive_card_celebration(message);
                    }
                }
                AppEvent::StopAutoDriveCelebration => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.stop_auto_drive_card_celebration();
                    }
                }
                AppEvent::TerminalAfter(after) => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.handle_terminal_after(after);
                    }
                }
                AppEvent::RequestValidationToolInstall { name, command } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        if let Some(launch) = widget.launch_validation_tool_install(&name, &command) {
                            self.app_event_tx.send(AppEvent::OpenTerminal(launch));
                        }
                    }
                }
                AppEvent::RunUpdateCommand { command, display, latest_version } => {
                    if crate::updates::upgrade_ui_enabled() {
                        if let AppState::Chat { widget } = &mut self.app_state {
                            if let Some(launch) = widget.launch_update_command(command, display, latest_version) {
                                self.app_event_tx.send(AppEvent::OpenTerminal(launch));
                            }
                        }
                    }
                }
                AppEvent::SetAutoUpgradeEnabled(enabled) => {
                    if crate::updates::upgrade_ui_enabled() {
                        if let AppState::Chat { widget } = &mut self.app_state {
                            widget.set_auto_upgrade_enabled(enabled);
                        }
                        self.config.auto_upgrade_enabled = enabled;
                    }
                }
                AppEvent::ShowAutoDriveSettings => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.show_auto_drive_settings();
                    }
                }
                AppEvent::CloseAutoDriveSettings => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.close_auto_drive_settings();
                    }
                }
                AppEvent::AutoDriveSettingsChanged {
                    review_enabled,
                    agents_enabled,
                    cross_check_enabled,
                    qa_automation_enabled,
                    continue_mode,
                } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.apply_auto_drive_settings(
                            review_enabled,
                            agents_enabled,
                            cross_check_enabled,
                            qa_automation_enabled,
                            continue_mode,
                        );
                    }
                }
                AppEvent::RequestAgentInstall { name, selected_index } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        if let Some(launch) = widget.launch_agent_install(name, selected_index) {
                            self.app_event_tx.send(AppEvent::OpenTerminal(launch));
                        }
                    }
                }
                AppEvent::AgentsOverviewSelectionChanged { index } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.set_agents_overview_selection(index);
                    }
                }
                // fallthrough handled by break
                AppEvent::CodexOp(op) => match &mut self.app_state {
                    AppState::Chat { widget } => widget.submit_op(op),
                    AppState::Onboarding { .. } => {}
                },
                AppEvent::AutoCoordinatorDecision {
                    status,
                    progress_past,
                    progress_current,
                    goal,
                    cli,
                    agents_timing,
                    agents,
                    transcript,
                } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.auto_handle_decision(
                            status,
                            progress_past,
                            progress_current,
                            goal,
                            cli,
                            agents_timing,
                            agents,
                            transcript,
                        );
                    }
                }
                AppEvent::AutoCoordinatorUserReply {
                    user_response,
                    cli_command,
                } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.auto_handle_user_reply(user_response, cli_command);
                    }
                }
                AppEvent::AutoCoordinatorThinking { delta, summary_index } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.auto_handle_thinking(delta, summary_index);
                    }
                }
                AppEvent::AutoCoordinatorTokenMetrics {
                    total_usage,
                    last_turn_usage,
                    turn_count,
                } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.auto_handle_token_metrics(
                            total_usage,
                            last_turn_usage,
                            turn_count,
                        );
                    }
                }
                AppEvent::AutoCoordinatorCompactedHistory { conversation } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.auto_handle_compacted_history(conversation);
                    }
                }
                AppEvent::AutoCoordinatorCountdown { countdown_id, seconds_left } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.auto_handle_countdown(countdown_id, seconds_left);
                    }
                }
                AppEvent::AutoCoordinatorRestart { token, attempt } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.auto_handle_restart(token, attempt);
                    }
                }
                AppEvent::PerformUndoRestore {
                    commit,
                    restore_files,
                    restore_conversation,
                } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.perform_undo_restore(commit.as_deref(), restore_files, restore_conversation);
                    }
                }
                AppEvent::DispatchCommand(command, command_text) => {
                    // Persist UI-only slash commands to cross-session history.
                    // For prompt-expanding commands (/plan, /solve, /code) we let the
                    // expanded prompt be recorded by the normal submission path.
                    if !command.is_prompt_expanding() {
                        let _ = self
                            .app_event_tx
                            .send(AppEvent::CodexOp(Op::AddToHistory { text: command_text.clone() }));
                    }
                    // Extract command arguments by removing the slash command from the beginning
                    // e.g., "/browser status" -> "status", "/chrome 9222" -> "9222"
                    let command_args = {
                        let cmd_with_slash = format!("/{}", command.command());
                        if command_text.starts_with(&cmd_with_slash) {
                            command_text[cmd_with_slash.len()..].trim().to_string()
                        } else {
                            // Fallback: if format doesn't match, use the full text
                            command_text.clone()
                        }
                    };

                    match command {
                        SlashCommand::Undo => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.handle_undo_command();
                            }
                        }
                        SlashCommand::Review => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                if command_args.is_empty() {
                                    widget.open_review_dialog();
                                } else {
                                    widget.handle_review_command(command_args);
                                }
                            }
                        }
                        SlashCommand::Cloud => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.handle_cloud_command(command_args);
                            }
                        }
                        SlashCommand::Branch => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.handle_branch_command(command_args);
                            }
                        }
                        SlashCommand::Merge => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.handle_merge_command();
                            }
                        }
                        SlashCommand::Resume => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.show_resume_picker();
                            }
                        }
                        SlashCommand::New => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.abort_active_turn_for_new_chat();
                            }
                            // Start a brand new conversation (core session) with no carried history.
                            // Replace the chat widget entirely, mirroring SwitchCwd flow but without import.
                            let mut new_widget = ChatWidget::new(
                                self.config.clone(),
                                self.app_event_tx.clone(),
                                None,
                                Vec::new(),
                                self.enhanced_keys_supported,
                                self.terminal_info.clone(),
                                self.show_order_overlay,
                                self.latest_upgrade_version.clone(),
                            );
                            new_widget.enable_perf(self.timing_enabled);
                            self.app_state = AppState::Chat { widget: Box::new(new_widget) };
                            self.terminal_runs.clear();
                            self.app_event_tx.send(AppEvent::RequestRedraw);
                        }
                        SlashCommand::Init => {
                            // Guard: do not run if a task is active.
                            if let AppState::Chat { widget } = &mut self.app_state {
                                const INIT_PROMPT: &str =
                                    include_str!("../prompt_for_init_command.md");
                                widget.submit_text_message(INIT_PROMPT.to_string());
                            }
                        }
                        SlashCommand::Compact => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.clear_token_usage();
                                self.app_event_tx.send(AppEvent::CodexOp(Op::Compact));
                            }
                        }
                        SlashCommand::Quit => { break 'main; }
                        SlashCommand::Login => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.handle_login_command();
                            }
                        }
                        SlashCommand::Logout => {
                            if let Err(e) = code_login::logout(&self.config.code_home) { tracing::error!("failed to logout: {e}"); }
                            break 'main;
                        }
                        SlashCommand::Diff => {
                            let tx = self.app_event_tx.clone();
                            tokio::spawn(async move {
                                match get_git_diff().await {
                                    Ok((is_git_repo, diff_text)) => {
                                        let text = if is_git_repo {
                                            diff_text
                                        } else {
                                            "`/diff` — _not inside a git repository_".to_string()
                                        };
                                        tx.send(AppEvent::DiffResult(text));
                                    }
                                    Err(e) => {
                                        tx.send(AppEvent::DiffResult(format!("Failed to compute diff: {e}")));
                                    }
                                }
                            });
                        }
                        SlashCommand::Mention => {
                            // The mention feature is handled differently in our fork
                            // For now, just add @ to the composer
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.insert_str("@");
                            }
                        }
                        SlashCommand::Cmd => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.handle_project_command(command_args);
                            }
                        }
                        SlashCommand::Auto => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                let goal = if command_args.is_empty() {
                                    None
                                } else {
                                    Some(command_args.clone())
                                };
                                widget.handle_auto_command(goal);
                            }
                        }
                        SlashCommand::Status => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.add_status_output();
                            }
                        }
                        SlashCommand::Limits => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.handle_limits_command(command_args);
                            }
                        }
                        SlashCommand::Update => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.handle_update_command(command_args.trim());
                            }
                        }
                        SlashCommand::Settings => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                let section = command
                                    .settings_section_from_args(&command_args)
                                    .and_then(ChatWidget::settings_section_from_hint);
                                widget.show_settings_overlay(section);
                            }
                        }
                        SlashCommand::Notifications => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.handle_notifications_command(command_args);
                            }
                        }
                        SlashCommand::Agents => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.handle_agents_command(command_args);
                            }
                        }
                        SlashCommand::Github => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.handle_github_command(command_args);
                            }
                        }
                        SlashCommand::Validation => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.handle_validation_command(command_args);
                            }
                        }
                        SlashCommand::Mcp => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.handle_mcp_command(command_args);
                            }
                        }
                        SlashCommand::Model => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                if command_args.trim().is_empty() {
                                    widget.show_settings_overlay(Some(SettingsSection::Model));
                                } else {
                                    widget.handle_model_command(command_args);
                                }
                            }
                        }
                        SlashCommand::Reasoning => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.handle_reasoning_command(command_args);
                            }
                        }
                        SlashCommand::Verbosity => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.handle_verbosity_command(command_args);
                            }
                        }
                        SlashCommand::Theme => {
                            // Theme selection is handled in submit_user_message
                            // This case is here for completeness
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.show_settings_overlay(Some(SettingsSection::Theme));
                            }
                        }
                        SlashCommand::Prompts => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.add_prompts_output();
                            }
                        }
                        SlashCommand::Perf => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.handle_perf_command(command_args);
                            }
                        }
                        SlashCommand::Demo => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.handle_demo_command();
                            }
                        }
                        // Prompt-expanding commands should have been handled in submit_user_message
                        // but add a fallback just in case. Use a helper that shows the original
                        // slash command in history while sending the expanded prompt to the model.
                        SlashCommand::Plan | SlashCommand::Solve | SlashCommand::Code => {
                            // These should have been expanded already, but handle them anyway
                            if let AppState::Chat { widget } = &mut self.app_state {
                                let expanded = command.expand_prompt(command_args.trim());
                                if let Some(prompt) = expanded {
                                    widget.submit_prompt_with_display(command_text.clone(), prompt);
                                }
                            }
                        }
                        SlashCommand::Browser => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                widget.handle_browser_command(command_args);
                            }
                        }
                        SlashCommand::Chrome => {
                            if let AppState::Chat { widget } = &mut self.app_state {
                                tracing::info!("[cdp] /chrome invoked, args='{}'", command_args);
                                if command_args.trim().is_empty() {
                                    widget.show_settings_overlay(Some(SettingsSection::Chrome));
                                } else {
                                    widget.handle_chrome_command(command_args);
                                }
                            }
                        }
                        #[cfg(debug_assertions)]
                        SlashCommand::TestApproval => {
                            use code_core::protocol::EventMsg;
                            use std::collections::HashMap;

                            use code_core::protocol::ApplyPatchApprovalRequestEvent;
                            use code_core::protocol::FileChange;

                            self.app_event_tx.send(AppEvent::CodexEvent(Event {
                                id: "1".to_string(),
                                event_seq: 0,
                                // msg: EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
                                //     call_id: "1".to_string(),
                                //     command: vec!["git".into(), "apply".into()],
                                //     cwd: self.config.cwd.clone(),
                                //     reason: Some("test".to_string()),
                                // }),
                                msg: EventMsg::ApplyPatchApprovalRequest(
                                    ApplyPatchApprovalRequestEvent {
                                        call_id: "1".to_string(),
                                        changes: HashMap::from([
                                            (
                                                PathBuf::from("/tmp/test.txt"),
                                                FileChange::Add {
                                                    content: "test".to_string(),
                                                },
                                            ),
                                            (
                                                PathBuf::from("/tmp/test2.txt"),
                                                FileChange::Update {
                                                    unified_diff: "+test\n-test2".to_string(),
                                                    move_path: None,
                                                    original_content: "test2".to_string(),
                                                    new_content: "test".to_string(),
                                                },
                                            ),
                                        ]),
                                        reason: None,
                                        grant_root: Some(PathBuf::from("/tmp")),
                                    },
                                ),
                                order: None,
                            }));
                        }
                    }
                }
                AppEvent::SwitchCwd(new_cwd, initial_prompt) => {
                    let target = new_cwd.clone();
                    self.config.cwd = target.clone();
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.switch_cwd(target, initial_prompt);
                    }
                }
                AppEvent::ResumeFrom(path) => {
                    // Replace the current chat widget with a new one configured to resume
                    let mut cfg = self.config.clone();
                    cfg.experimental_resume = Some(path);
                    if let AppState::Chat { .. } = &self.app_state {
                        let mut new_widget = ChatWidget::new(
                            cfg,
                            self.app_event_tx.clone(),
                            None,
                            Vec::new(),
                            self.enhanced_keys_supported,
                            self.terminal_info.clone(),
                            self.show_order_overlay,
                            self.latest_upgrade_version.clone(),
                        );
                        new_widget.enable_perf(self.timing_enabled);
                        self.app_state = AppState::Chat { widget: Box::new(new_widget) };
                        self.terminal_runs.clear();
                        self.app_event_tx.send(AppEvent::RequestRedraw);
                    }
                }
                AppEvent::PrepareAgents => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.prepare_agents();
                    }
                }
                AppEvent::ShowAgentEditor { name } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.ensure_settings_overlay_section(SettingsSection::Agents);
                        widget.show_agent_editor_ui(name);
                    }
                }
                AppEvent::UpdateModelSelection { model, effort } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.apply_model_selection(model, effort);
                    }
                }
                AppEvent::UpdateTextVerbosity(new_verbosity) => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.set_text_verbosity(new_verbosity);
                    }
                }
                AppEvent::UpdateGithubWatcher(enabled) => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.set_github_watcher(enabled);
                    }
                }
                AppEvent::UpdateTuiNotifications(enabled) => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.set_tui_notifications(enabled);
                    }
                    self.config.tui.notifications = Notifications::Enabled(enabled);
                    self.config.tui_notifications = Notifications::Enabled(enabled);
                }
                AppEvent::UpdateValidationTool { name, enable } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.toggle_validation_tool(&name, enable);
                    }
                }
                AppEvent::UpdateValidationGroup { group, enable } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.toggle_validation_group(group, enable);
                    }
                }
                AppEvent::SetTerminalTitle { title } => {
                    self.terminal_title_override = title;
                    self.apply_terminal_title();
                }
                AppEvent::EmitTuiNotification { title, body } => {
                    if let Some(message) = Self::format_notification_message(&title, body.as_deref()) {
                        Self::emit_osc9_notification(&message);
                    }
                }
                AppEvent::UpdateMcpServer { name, enable } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.toggle_mcp_server(&name, enable);
                    }
                }
                AppEvent::UpdateSubagentCommand(cmd) => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.apply_subagent_update(cmd);
                    }
                }
                AppEvent::DeleteSubagentCommand(name) => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.delete_subagent_by_name(&name);
                    }
                }
                // ShowAgentsSettings removed
                AppEvent::ShowAgentsOverview => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.ensure_settings_overlay_section(SettingsSection::Agents);
                        widget.show_agents_overview_ui();
                    }
                }
                // ShowSubagentEditor removed; use ShowSubagentEditorForName/ShowSubagentEditorNew
                AppEvent::ShowSubagentEditorForName { name } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.ensure_settings_overlay_section(SettingsSection::Agents);
                        widget.show_subagent_editor_for_name(name);
                    }
                }
                AppEvent::ShowSubagentEditorNew => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.ensure_settings_overlay_section(SettingsSection::Agents);
                        widget.show_new_subagent_editor();
                    }
                }
                AppEvent::UpdateAgentConfig { name, enabled, args_read_only, args_write, instructions, command } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.apply_agent_update(&name, enabled, args_read_only, args_write, instructions, command);
                    }
                }
                AppEvent::PrefillComposer(text) => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.insert_str(&text);
                    }
                }
                AppEvent::SubmitTextWithPreface { visible, preface } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.submit_text_message_with_preface(visible, preface);
                    }
                }
                AppEvent::RunReviewCommand(args) => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.handle_review_command(args);
                    }
                }
                AppEvent::ToggleReviewAutoResolve => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.toggle_review_auto_resolve();
                    }
                }
                AppEvent::RunReviewWithScope {
                    prompt,
                    hint,
                    preparation_label,
                    metadata,
                    auto_resolve,
                } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.start_review_with_scope(
                            prompt,
                            hint,
                            preparation_label,
                            metadata,
                            auto_resolve,
                        );
                    }
                }
                AppEvent::OpenReviewCustomPrompt => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.show_review_custom_prompt();
                    }
                }
                AppEvent::FetchCloudTasks { environment } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.show_cloud_tasks_loading();
                    }
                    let tx = self.app_event_tx.clone();
                    let env_clone = environment.clone();
                    tokio::spawn(async move {
                        match cloud_tasks_service::fetch_tasks(environment).await {
                            Ok(tasks) => tx.send(AppEvent::PresentCloudTasks {
                                environment: env_clone,
                                tasks,
                            }),
                            Err(err) => tx.send(AppEvent::CloudTasksError {
                                message: err.to_string(),
                            }),
                        }
                    });
                }
                AppEvent::PresentCloudTasks { environment, tasks } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.present_cloud_tasks(environment, tasks);
                    }
                }
                AppEvent::CloudTasksError { message } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.show_cloud_tasks_error(message);
                    }
                }
                AppEvent::FetchCloudEnvironments => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.show_cloud_environment_loading();
                    }
                    let tx = self.app_event_tx.clone();
                    tokio::spawn(async move {
                        match cloud_tasks_service::fetch_environments().await {
                            Ok(envs) => tx.send(AppEvent::PresentCloudEnvironments { environments: envs }),
                            Err(err) => tx.send(AppEvent::CloudTasksError { message: err.to_string() }),
                        }
                    });
                }
                AppEvent::PresentCloudEnvironments { environments } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.present_cloud_environment_picker(environments);
                    }
                }
                AppEvent::SetCloudEnvironment { environment } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.set_cloud_environment(environment);
                    }
                }
                AppEvent::ShowCloudTaskActions { task_id } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.show_cloud_task_actions(task_id);
                    }
                }
                AppEvent::FetchCloudTaskDiff { task_id } => {
                    let tx = self.app_event_tx.clone();
                    tokio::spawn(async move {
                        let task = TaskId(task_id.clone());
                        match cloud_tasks_service::fetch_task_diff(task.clone()).await {
                            Ok(Some(diff)) => {
                                tx.send(AppEvent::DiffResult(diff));
                            }
                            Ok(None) => tx.send(AppEvent::CloudTasksError {
                                message: format!("Task {} has no diff available", task.0),
                            }),
                            Err(err) => tx.send(AppEvent::CloudTasksError { message: err.to_string() }),
                        }
                    });
                }
                AppEvent::FetchCloudTaskMessages { task_id } => {
                    let tx = self.app_event_tx.clone();
                    tokio::spawn(async move {
                        let task = TaskId(task_id.clone());
                        match cloud_tasks_service::fetch_task_messages(task).await {
                            Ok(messages) if !messages.is_empty() => {
                                let joined = messages.join("\n\n");
                                tx.send(AppEvent::InsertBackgroundEvent {
                                    message: format!("Cloud task output for {task_id}:\n{joined}"),
                                    placement: crate::app_event::BackgroundPlacement::Tail,
                                    order: None,
                                });
                            }
                            Ok(_) => tx.send(AppEvent::CloudTasksError {
                                message: format!("Task {task_id} has no assistant messages"),
                            }),
                            Err(err) => tx.send(AppEvent::CloudTasksError { message: err.to_string() }),
                        }
                    });
                }
                AppEvent::ApplyCloudTask { task_id, preflight } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.show_cloud_task_apply_status(&task_id, preflight);
                    }
                    let tx = self.app_event_tx.clone();
                    tokio::spawn(async move {
                        let task = TaskId(task_id.clone());
                        let result = cloud_tasks_service::apply_task(task, preflight).await;
                        tx.send(AppEvent::CloudTaskApplyFinished {
                            task_id,
                            outcome: result.map_err(|err| CloudTaskError::Msg(err.to_string())),
                            preflight,
                        });
                    });
                }
                AppEvent::CloudTaskApplyFinished { task_id, outcome, preflight } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.handle_cloud_task_apply_finished(task_id, outcome, preflight);
                    }
                }
                AppEvent::OpenCloudTaskCreate => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.show_cloud_task_create_prompt();
                    }
                }
                AppEvent::SubmitCloudTaskCreate { env_id, prompt, best_of_n } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.show_cloud_task_create_progress();
                    }
                    let tx = self.app_event_tx.clone();
                    tokio::spawn(async move {
                        let result = cloud_tasks_service::create_task(env_id.clone(), prompt.clone(), best_of_n).await;
                        tx.send(AppEvent::CloudTaskCreated {
                            env_id,
                            result: result.map_err(|err| CloudTaskError::Msg(err.to_string())),
                        });
                    });
                }
                AppEvent::CloudTaskCreated { env_id, result } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.handle_cloud_task_created(env_id.clone(), result);
                    }
                }
                AppEvent::StartReviewCommitPicker => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.show_review_commit_loading();
                    }
                    let cwd = self.config.cwd.clone();
                    let tx = self.app_event_tx.clone();
                    tokio::spawn(async move {
                        let commits = code_core::git_info::recent_commits(&cwd, 60).await;
                        tx.send(AppEvent::PresentReviewCommitPicker { commits });
                    });
                }
                AppEvent::PresentReviewCommitPicker { commits } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.present_review_commit_picker(commits);
                    }
                }
                AppEvent::StartReviewBranchPicker => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.show_review_branch_loading();
                    }
                    let cwd = self.config.cwd.clone();
                    let tx = self.app_event_tx.clone();
                    tokio::spawn(async move {
                        let (branches, current_branch) = tokio::join!(
                            code_core::git_info::local_git_branches(&cwd),
                            code_core::git_info::current_branch_name(&cwd),
                        );
                        tx.send(AppEvent::PresentReviewBranchPicker {
                            current_branch,
                            branches,
                        });
                    });
                }
                AppEvent::PresentReviewBranchPicker {
                    current_branch,
                    branches,
                } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.present_review_branch_picker(current_branch, branches);
                    }
                }
                AppEvent::DiffResult(text) => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.add_diff_output(text);
                    }
                }
                AppEvent::UpdateTheme(new_theme) => {
                    // Switch the theme immediately
                    if matches!(new_theme, code_core::config_types::ThemeName::Custom) {
                        // Prefer runtime custom colors; fall back to config on disk
                        if let Some(colors) = crate::theme::custom_theme_colors() {
                            crate::theme::init_theme(&code_core::config_types::ThemeConfig { name: new_theme, colors, label: crate::theme::custom_theme_label(), is_dark: crate::theme::custom_theme_is_dark() });
                        } else if let Ok(cfg) = code_core::config::Config::load_with_cli_overrides(vec![], code_core::config::ConfigOverrides::default()) {
                            crate::theme::init_theme(&cfg.tui.theme);
                        } else {
                            crate::theme::switch_theme(new_theme);
                        }
                    } else {
                        crate::theme::switch_theme(new_theme);
                    }

                    // Clear terminal with new theme colors
                    let theme_bg = crate::colors::background();
                    let theme_fg = crate::colors::text();
                    let _ = crossterm::execute!(
                        std::io::stdout(),
                        crossterm::style::SetColors(crossterm::style::Colors::new(
                            theme_fg.into(),
                            theme_bg.into()
                        )),
                        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
                        crossterm::cursor::MoveTo(0, 0),
                        crossterm::terminal::EnableLineWrap
                    );
                    self.apply_terminal_title();

                    // Update config and save to file
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.set_theme(new_theme);
                    }

                    // Force a full redraw on the next frame so the entire
                    // ratatui back buffer is cleared and repainted with the
                    // new theme. This avoids any stale cells lingering on
                    // terminals that preserve previous cell attributes.
                    self.clear_on_first_frame = true;
                    self.schedule_redraw();
                }
                AppEvent::PreviewTheme(new_theme) => {
                    // Switch the theme immediately for preview (no history event)
                    if matches!(new_theme, code_core::config_types::ThemeName::Custom) {
                        if let Some(colors) = crate::theme::custom_theme_colors() {
                            crate::theme::init_theme(&code_core::config_types::ThemeConfig { name: new_theme, colors, label: crate::theme::custom_theme_label(), is_dark: crate::theme::custom_theme_is_dark() });
                        } else if let Ok(cfg) = code_core::config::Config::load_with_cli_overrides(vec![], code_core::config::ConfigOverrides::default()) {
                            crate::theme::init_theme(&cfg.tui.theme);
                        } else {
                            crate::theme::switch_theme(new_theme);
                        }
                    } else {
                        crate::theme::switch_theme(new_theme);
                    }

                    // Clear terminal with new theme colors
                    let theme_bg = crate::colors::background();
                    let theme_fg = crate::colors::text();
                    let _ = crossterm::execute!(
                        std::io::stdout(),
                        crossterm::style::SetColors(crossterm::style::Colors::new(
                            theme_fg.into(),
                            theme_bg.into()
                        )),
                        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
                        crossterm::cursor::MoveTo(0, 0),
                        crossterm::terminal::EnableLineWrap
                    );
                    self.apply_terminal_title();

                    // Retint pre-rendered history cells so the preview reflects immediately
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.retint_history_for_preview();
                    }

                    // Don't update config or add to history for previews
                    // Force a full redraw so previews repaint cleanly as you cycle
                    self.clear_on_first_frame = true;
                    self.schedule_redraw();
                }
                AppEvent::UpdateSpinner(name) => {
                    // Switch spinner immediately
                    crate::spinner::switch_spinner(&name);
                    // Update config and save to file
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.set_spinner(name.clone());
                    }
                    self.schedule_redraw();
                }
                AppEvent::PreviewSpinner(name) => {
                    // Switch spinner immediately for preview (no history event)
                    crate::spinner::switch_spinner(&name);
                    // No config change on preview
                    self.schedule_redraw();
                }
                AppEvent::ComposerExpanded => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.on_composer_expanded();
                    }
                    self.schedule_redraw();
                }
                AppEvent::ShowLoginAccounts => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.show_login_accounts_view();
                    }
                }
                AppEvent::ShowLoginAddAccount => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.show_login_add_account_view();
                    }
                }
                AppEvent::CycleAccessMode => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.cycle_access_mode();
                    }
                    self.schedule_redraw();
                }
                AppEvent::CycleAutoDriveVariant => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.cycle_auto_drive_variant();
                    }
                    self.schedule_redraw();
                }
                AppEvent::LoginStartChatGpt => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        if !widget.login_add_view_active() {
                            continue 'main;
                        }

                        if let Some(flow) = self.login_flow.take() {
                            flow.shutdown.shutdown();
                            flow.join_handle.abort();
                        }

                        let opts = ServerOptions::new(
                            self.config.code_home.clone(),
                            code_login::CLIENT_ID.to_string(),
                            self.config.responses_originator_header.clone(),
                        );

                        match code_login::run_login_server(opts) {
                            Ok(server) => {
                                widget.notify_login_chatgpt_started(server.auth_url.clone());
                                let shutdown = server.cancel_handle();
                                let tx = self.app_event_tx.clone();
                                let join_handle = tokio::spawn(async move {
                                    let result = server
                                        .block_until_done()
                                        .await
                                        .map_err(|e| e.to_string());
                                    tx.send(AppEvent::LoginChatGptComplete { result });
                                });
                                self.login_flow = Some(LoginFlowState { shutdown, join_handle });
                            }
                            Err(err) => {
                                widget.notify_login_chatgpt_failed(format!(
                                    "Failed to start ChatGPT login: {err}"
                                ));
                            }
                        }
                    }
                }
                AppEvent::LoginCancelChatGpt => {
                    if let Some(flow) = self.login_flow.take() {
                        flow.shutdown.shutdown();
                        flow.join_handle.abort();
                    }
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.notify_login_chatgpt_cancelled();
                    }
                }
                AppEvent::LoginChatGptComplete { result } => {
                    if let Some(flow) = self.login_flow.take() {
                        flow.shutdown.shutdown();
                        // Allow the task to finish naturally; if still running, abort.
                        if !flow.join_handle.is_finished() {
                            flow.join_handle.abort();
                        }
                    }

                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.notify_login_chatgpt_complete(result);
                    }
                }
                AppEvent::LoginUsingChatGptChanged { using_chatgpt_auth } => {
                    self.handle_login_mode_change(using_chatgpt_auth);
                }
                AppEvent::OnboardingAuthComplete(result) => {
                    if let AppState::Onboarding { screen } = &mut self.app_state {
                        screen.on_auth_complete(result);
                    }
                }
                AppEvent::OnboardingComplete(ChatWidgetArgs {
                    config,
                    enhanced_keys_supported,
                    initial_images,
                    initial_prompt,
                    terminal_info,
                    show_order_overlay,
                    enable_perf,
                    resume_picker,
                    latest_upgrade_version,
                }) => {
                    let mut w = ChatWidget::new(
                        config,
                        app_event_tx.clone(),
                        initial_prompt,
                        initial_images,
                        enhanced_keys_supported,
                        terminal_info,
                        show_order_overlay,
                        latest_upgrade_version,
                    );
                    w.enable_perf(enable_perf);
                    if resume_picker {
                        w.show_resume_picker();
                    }
                    self.app_state = AppState::Chat { widget: Box::new(w) };
                    self.terminal_runs.clear();
                }
                AppEvent::StartFileSearch(query) => {
                    if !query.is_empty() {
                        self.file_search.on_user_query(query);
                    }
                }
                AppEvent::FileSearchResult { query, matches } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.apply_file_search_result(query, matches);
                    }
                }
                AppEvent::ShowChromeOptions(port) => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.show_chrome_options(port);
                    }
                }
                AppEvent::ChromeLaunchOptionSelected(option, port) => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.handle_chrome_launch_option(option, port);
                    }
                }
                AppEvent::JumpBack {
                    nth,
                    prefill,
                    history_snapshot,
                } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        let ghost_state = widget.snapshot_ghost_state();
                        // Build response items from current UI history
                        let items = widget.export_response_items();
                        let cfg = widget.config_ref().clone();

                        // Compute prefix up to selected user message now
                        let prefix_items = {
                            let mut user_seen = 0usize;
                            let mut cut = items.len();
                            for (idx, it) in items.iter().enumerate().rev() {
                                if let code_protocol::models::ResponseItem::Message { role, .. } = it {
                                    if role == "user" {
                                        user_seen += 1;
                                        if user_seen == nth { cut = idx; break; }
                                    }
                                }
                            }
                            items.iter().take(cut).cloned().collect::<Vec<_>>()
                        };

                        self.pending_jump_back_ghost_state = Some(ghost_state);
                        self.pending_jump_back_history_snapshot = history_snapshot;

                        // Perform the fork off the UI thread to avoid nested runtimes
                        let server = self._server.clone();
                        let tx = self.app_event_tx.clone();
                        let prefill_clone = prefill.clone();
                        std::thread::spawn(move || {
                            let rt = tokio::runtime::Builder::new_multi_thread()
                                .enable_all()
                                .build()
                                .expect("build tokio runtime");
                            // Clone cfg for the async block to keep original for the event
                            let cfg_for_rt = cfg.clone();
                            let result = rt.block_on(async move {
                                // Fallback: start a new conversation instead of forking
                                server.new_conversation(cfg_for_rt).await
                            });
                            if let Ok(new_conv) = result {
                                tx.send(AppEvent::JumpBackForked { cfg, new_conv: crate::app_event::Redacted(new_conv), prefix_items, prefill: prefill_clone });
                            } else if let Err(e) = result {
                                tracing::error!("error forking conversation: {e:#}");
                            }
                        });
                    }
                }
                AppEvent::JumpBackForked { cfg, new_conv, prefix_items, prefill } => {
                    // Replace widget with a new one bound to the forked conversation
                    let session_conf = new_conv.0.session_configured.clone();
                    let conv = new_conv.0.conversation.clone();

                    let mut ghost_state = self.pending_jump_back_ghost_state.take();
                    let history_snapshot = self.pending_jump_back_history_snapshot.take();
                    let emit_prefix = history_snapshot.is_none();

                    if let AppState::Chat { widget } = &mut self.app_state {
                        let auth_manager = widget.auth_manager();
                        let mut new_widget = ChatWidget::new_from_existing(
                            cfg,
                            conv,
                            session_conf,
                            self.app_event_tx.clone(),
                            self.enhanced_keys_supported,
                            self.terminal_info.clone(),
                            self.show_order_overlay,
                            self.latest_upgrade_version.clone(),
                            auth_manager,
                            false,
                        );
                        if let Some(state) = ghost_state.take() {
                            new_widget.adopt_ghost_state(state);
                        } else {
                            tracing::warn!("jump-back fork missing ghost snapshot state; redo may be unavailable");
                        }
                        if let Some(snapshot) = history_snapshot.as_ref() {
                            new_widget.restore_history_snapshot(snapshot);
                        }
                        new_widget.enable_perf(self.timing_enabled);
                        new_widget.check_for_initial_animations();
                        *widget = Box::new(new_widget);
                    } else {
                        let auth_manager = AuthManager::shared_with_mode_and_originator(
                            cfg.code_home.clone(),
                            AuthMode::ApiKey,
                            cfg.responses_originator_header.clone(),
                        );
                        let mut new_widget = ChatWidget::new_from_existing(
                            cfg,
                            conv,
                            session_conf,
                            self.app_event_tx.clone(),
                            self.enhanced_keys_supported,
                            self.terminal_info.clone(),
                            self.show_order_overlay,
                            self.latest_upgrade_version.clone(),
                            auth_manager,
                            false,
                        );
                        if let Some(state) = ghost_state.take() {
                            new_widget.adopt_ghost_state(state);
                        }
                        if let Some(snapshot) = history_snapshot.as_ref() {
                            new_widget.restore_history_snapshot(snapshot);
                        }
                        new_widget.enable_perf(self.timing_enabled);
                        new_widget.check_for_initial_animations();
                        self.app_state = AppState::Chat { widget: Box::new(new_widget) };
                    }
                    self.terminal_runs.clear();
                    // Reset any transient state from the previous widget/session
                    self.commit_anim_running.store(false, Ordering::Release);
                    self.last_esc_time = None;
                    // Force a clean repaint of the new UI state
                    self.clear_on_first_frame = true;

                    // Replay prefix to the UI
                    if emit_prefix {
                        let ev = code_core::protocol::Event {
                            id: "fork".to_string(),
                            event_seq: 0,
                            msg: code_core::protocol::EventMsg::ReplayHistory(
                                code_core::protocol::ReplayHistoryEvent {
                                    items: prefix_items,
                                    history_snapshot: None,
                                }
                            ),
                            order: None,
                        };
                        self.app_event_tx.send(AppEvent::CodexEvent(ev));
                    }

                    // Prefill composer with the edited text
                    if let AppState::Chat { widget } = &mut self.app_state {
                        if !prefill.is_empty() { widget.insert_str(&prefill); }
                    }
                    self.app_event_tx.send(AppEvent::RequestRedraw);
                }
                AppEvent::ScheduleFrameIn(duration) => {
                    // Schedule the next redraw with the requested duration
                    self.schedule_redraw_in(duration);
                }
                AppEvent::GhostSnapshotFinished { job_id, result, elapsed } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.handle_ghost_snapshot_finished(job_id, result, elapsed);
                    }
                }
            }
        }
        if self.alt_screen_active {
            terminal.clear()?;
        }

        Ok(())
    }

    /// Pull the next event with priority for interactive input.
    /// Never returns None due to idleness; only returns None if both channels disconnect.
    fn next_event_priority(&self) -> Option<AppEvent> {
        use std::sync::mpsc::RecvTimeoutError::{Timeout, Disconnected};
        loop {
            if let Ok(ev) = self.app_event_rx_high.try_recv() { return Some(ev); }
            if let Ok(ev) = self.app_event_rx_bulk.try_recv() { return Some(ev); }
            match self.app_event_rx_high.recv_timeout(Duration::from_millis(10)) {
                Ok(ev) => return Some(ev),
                Err(Timeout) => continue,
                Err(Disconnected) => break,
            }
        }
        // High channel disconnected; try blocking on bulk as a last resort
        self.app_event_rx_bulk.recv().ok()
    }

    #[cfg(unix)]
    fn suspend(&mut self, terminal: &mut tui::Tui) -> Result<()> {
        tui::restore()?;
        // SAFETY: Unix-only code path. We intentionally send SIGTSTP to the
        // current process group (pid 0) to trigger standard job-control
        // suspension semantics. This FFI does not involve any raw pointers,
        // is not called from a signal handler, and uses a constant signal.
        // Errors from kill are acceptable (e.g., if already stopped) — the
        // subsequent re-init path will still leave the terminal in a good state.
        // We considered `nix`, but didn't think it was worth pulling in for this one call.
        unsafe { libc::kill(0, libc::SIGTSTP) };
        let (new_terminal, new_terminal_info) = tui::init(&self.config)?;
        *terminal = new_terminal;
        self.terminal_info = new_terminal_info;
        terminal.clear()?;
        self.app_event_tx.send(AppEvent::RequestRedraw);
        Ok(())
    }

    /// Toggle between alternate-screen TUI and standard terminal buffer (Ctrl+T).
    fn toggle_screen_mode(&mut self, _terminal: &mut tui::Tui) -> Result<()> {
        if self.alt_screen_active {
            // Leave alt screen only; keep raw mode enabled for key handling.
            let _ = crate::tui::leave_alt_screen_only();
            // Clear the normal buffer so our buffered transcript starts at a clean screen
            let _ = crossterm::execute!(
                std::io::stdout(),
                crossterm::style::ResetColor,
                crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
                crossterm::cursor::MoveTo(0, 0),
                crossterm::terminal::EnableLineWrap
            );
            self.alt_screen_active = false;
            // Persist preference
            let _ = code_core::config::set_tui_alternate_screen(&self.config.code_home, false);
            // Immediately mirror the entire transcript into the terminal scrollback so
            // the user sees full history when entering standard mode.
            if let AppState::Chat { widget } = &self.app_state {
                let transcript = widget.export_transcript_lines_for_buffer();
                if !transcript.is_empty() {
                    // Best-effort: compute current width and bottom reservation.
                    // We don't have `terminal` here; schedule a one-shot redraw event
                    // that carries the transcript via InsertHistory to reuse the normal path.
                    self.app_event_tx.send(AppEvent::InsertHistory(transcript));
                }
            }
            // Ensure the input is painted in its reserved region immediately.
            self.schedule_redraw();
        } else {
            // Re-enter alt screen and force a clean repaint.
            let fg = crate::colors::text();
            let bg = crate::colors::background();
            let _ = crate::tui::enter_alt_screen_only(fg, bg);
            self.clear_on_first_frame = true;
            self.alt_screen_active = true;
            // Persist preference
            let _ = code_core::config::set_tui_alternate_screen(&self.config.code_home, true);
            // Request immediate redraw
            self.schedule_redraw();
        }
        Ok(())
    }

    pub(crate) fn token_usage(&self) -> code_core::protocol::TokenUsage {
        let usage = match &self.app_state {
            AppState::Chat { widget } => widget.token_usage().clone(),
            AppState::Onboarding { .. } => code_core::protocol::TokenUsage::default(),
        };
        // ensure background helpers stop before returning
        self.commit_anim_running.store(false, Ordering::Release);
        self.input_running.store(false, Ordering::Release);
        usage
    }

    pub(crate) fn session_id(&self) -> Option<uuid::Uuid> {
        match &self.app_state {
            AppState::Chat { widget } => widget.session_id(),
            AppState::Onboarding { .. } => None,
        }
    }

    /// Return a human-readable performance summary if timing was enabled.
    pub(crate) fn perf_summary(&self) -> Option<String> {
        if !self.timing_enabled {
            return None;
        }
        let mut out = String::new();
        if let AppState::Chat { widget } = &self.app_state {
            out.push_str(&widget.perf_summary());
            out.push_str("\n\n");
        }
        out.push_str(&self.timing.summarize());
        Some(out)
    }

    fn draw_next_frame(&mut self, terminal: &mut tui::Tui) -> Result<()> {
        // Always render a frame. In standard-terminal mode we still draw the
        // chat UI (without status/HUD) directly into the normal buffer.
        // Hard clear on the very first frame (and while onboarding) to ensure a
        // clean background across terminals that don't respect our color attrs
        // during EnterAlternateScreen.
        if self.alt_screen_active && (self.clear_on_first_frame || matches!(self.app_state, AppState::Onboarding { .. })) {
            terminal.clear()?;
            self.clear_on_first_frame = false;
        }

        // If the terminal area changed (actual resize or tab switch that altered
        // viewport), force a full clear once to prevent ghost artifacts. Some
        // terminals on Windows/macOS do not reliably deliver Resize events on
        // focus switches; querying the size each frame is cheap and lets us
        // detect the change without extra event wiring.
        let screen_size = terminal.size()?;
        if self
            .last_frame_size
            .map(|prev| prev != screen_size)
            .unwrap_or(false)
        {
            terminal.clear()?;
        }
        self.last_frame_size = Some(screen_size);

        let completed_frame = terminal.draw(|frame| {
            match &mut self.app_state {
                AppState::Chat { widget } => {
                    if let Some((x, y)) = widget.cursor_pos(frame.area()) {
                        frame.set_cursor_position((x, y));
                    }
                    frame.render_widget_ref(&**widget, frame.area())
                }
                AppState::Onboarding { screen } => frame.render_widget_ref(&*screen, frame.area()),
            }
        })?;
        self.buffer_diff_profiler.record(&completed_frame);
        Ok(())
    }

    /// Dispatch a KeyEvent to the current view and let it decide what to do
    /// with it.
    fn dispatch_key_event(&mut self, key_event: KeyEvent) {
        match &mut self.app_state {
            AppState::Chat { widget } => {
                widget.handle_key_event(key_event);
            }
            AppState::Onboarding { screen } => match key_event.code {
                KeyCode::Char('q') => {
                    self.app_event_tx.send(AppEvent::ExitRequest);
                }
                _ => screen.handle_key_event(key_event),
            },
        }
    }

    fn dispatch_paste_event(&mut self, pasted: String) {
        match &mut self.app_state {
            AppState::Chat { widget } => widget.handle_paste(pasted),
            AppState::Onboarding { .. } => {}
        }
    }

    fn dispatch_mouse_event(&mut self, mouse_event: crossterm::event::MouseEvent) {
        match &mut self.app_state {
            AppState::Chat { widget } => {
                widget.handle_mouse_event(mouse_event);
            }
            AppState::Onboarding { .. } => {}
        }
    }

    fn dispatch_code_event(&mut self, event: Event) {
        match &mut self.app_state {
            AppState::Chat { widget } => widget.handle_code_event(event),
            AppState::Onboarding { .. } => {}
        }
    }

    fn normalize_non_enhanced_release_code(code: KeyCode) -> KeyCode {
        match code {
            KeyCode::Char('\r') | KeyCode::Char('\n') => KeyCode::Enter,
            KeyCode::Char('\t') => KeyCode::Tab,
            KeyCode::Char('\u{1b}') => KeyCode::Esc,
            other => other,
        }
    }

}

struct BufferDiffProfiler {
    enabled: bool,
    prev: Option<Buffer>,
    frame_seq: u64,
    log_every: usize,
    min_changed: usize,
    min_percent: f64,
}

impl BufferDiffProfiler {
    fn new_from_env() -> Self {
        match std::env::var("CODE_BUFFER_DIFF_METRICS") {
            Ok(raw) => {
                let trimmed = raw.trim();
                if trimmed.is_empty() || trimmed == "0" {
                    Self::disabled()
                } else {
                    let log_every = trimmed.parse::<usize>().unwrap_or(1).max(1);
                    let min_changed = std::env::var("CODE_BUFFER_DIFF_MIN_CHANGED")
                        .ok()
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(100);
                    let min_percent = std::env::var("CODE_BUFFER_DIFF_MIN_PERCENT")
                        .ok()
                        .and_then(|v| v.trim().parse::<f64>().ok())
                        .unwrap_or(1.0_f64);
                    Self {
                        enabled: true,
                        prev: None,
                        frame_seq: 0,
                        log_every,
                        min_changed,
                        min_percent,
                    }
                }
            }
            Err(_) => Self::disabled(),
        }
    }

    fn disabled() -> Self {
        Self {
            enabled: false,
            prev: None,
            frame_seq: 0,
            log_every: 1,
            min_changed: usize::MAX,
            min_percent: f64::MAX,
        }
    }

    fn record(&mut self, frame: &CompletedFrame<'_>) {
        if !self.enabled {
            return;
        }

        let current_buffer = frame.buffer.clone();
        self.frame_seq = self.frame_seq.saturating_add(1);

        if let Some(prev_buffer) = &self.prev {
            if self.should_log_frame() {
                if prev_buffer.area != current_buffer.area {
                    tracing::info!(
                        target: "code_tui::buffer_diff",
                        frame = self.frame_seq,
                        prev_width = prev_buffer.area.width,
                        prev_height = prev_buffer.area.height,
                        width = current_buffer.area.width,
                        height = current_buffer.area.height,
                        "Buffer area changed; skipping diff metrics for this frame"
                    );
                } else {
                    let inspected = prev_buffer.content.len().min(current_buffer.content.len());
                    let updates = prev_buffer.diff(&current_buffer);
                    let changed = updates.len();
                    if changed == 0 {
                        self.prev = Some(current_buffer);
                        return;
                    }
                    let percent = if inspected > 0 {
                        (changed as f64 / inspected as f64) * 100.0
                    } else {
                        0.0
                    };
                    if changed < self.min_changed && percent < self.min_percent {
                        self.prev = Some(current_buffer);
                        return;
                    }
                    let mut min_col = u16::MAX;
                    let mut max_col = 0u16;
                    let mut rows = BTreeSet::new();
                    let mut longest_run = 0usize;
                    let mut current_run = 0usize;
                    let mut last_cell = None;
                    for (x, y, _) in &updates {
                        min_col = min_col.min(*x);
                        max_col = max_col.max(*x);
                        rows.insert(*y);
                        match last_cell {
                            Some((last_x, last_y)) if *y == last_y && *x == last_x + 1 => {
                                current_run += 1;
                            }
                            _ => {
                                current_run = 1;
                            }
                        }
                        if current_run > longest_run {
                            longest_run = current_run;
                        }
                        last_cell = Some((*x, *y));
                    }
                    let row_min = rows.iter().copied().min().unwrap_or(0);
                    let row_max = rows.iter().copied().max().unwrap_or(0);
                    let mut spans: Vec<(u16, u16)> = Vec::new();
                    if !rows.is_empty() {
                        let mut iter = rows.iter();
                        let mut start = *iter.next().unwrap();
                        let mut prev = start;
                        for &row in iter {
                            if row == prev + 1 {
                                prev = row;
                                continue;
                            }
                            spans.push((start, prev));
                            start = row;
                            prev = row;
                        }
                        spans.push((start, prev));
                    }
                    spans.sort_by(|(a_start, a_end), (b_start, b_end)| {
                        let a_len = usize::from(*a_end) - usize::from(*a_start) + 1;
                        let b_len = usize::from(*b_end) - usize::from(*b_start) + 1;
                        b_len.cmp(&a_len)
                    });
                    let top_spans: Vec<(u16, u16)> = spans.into_iter().take(3).collect();
                    let (col_min, col_max) = if min_col == u16::MAX {
                        (0u16, 0u16)
                    } else {
                        (min_col, max_col)
                    };
                    let skipped_cells = current_buffer.content.iter().filter(|cell| cell.skip).count();
                    tracing::info!(
                        target: "code_tui::buffer_diff",
                        frame = self.frame_seq,
                        inspected,
                        changed,
                        percent = format!("{percent:.2}"),
                        width = current_buffer.area.width,
                        height = current_buffer.area.height,
                        dirty_rows = rows.len(),
                        longest_run,
                        row_min,
                        row_max,
                        col_min,
                        col_max,
                        row_spans = ?top_spans,
                        skipped_cells,
                        "Buffer diff metrics"
                    );
                }
            }
        }

        self.prev = Some(current_buffer);
    }

    fn should_log_frame(&self) -> bool {
        let interval = self.log_every.max(1) as u64;
        interval == 1 || self.frame_seq % interval == 0
    }
}

fn should_show_onboarding(
    login_status: crate::LoginStatus,
    _config: &Config,
    show_trust_screen: bool,
) -> bool {
    if show_trust_screen {
        return true;
    }
    matches!(login_status, crate::LoginStatus::NotAuthenticated)
}

fn should_show_login_screen(login_status: crate::LoginStatus, _config: &Config) -> bool {
    matches!(login_status, crate::LoginStatus::NotAuthenticated)
}

// (legacy tests removed)
#[derive(Default, Clone, Debug)]
struct TimingStats {
    frames_drawn: u64,
    redraw_events: u64,
    key_events: u64,
    draw_ns: Vec<u64>,
    key_to_frame_ns: Vec<u64>,
    last_key_event: Option<Instant>,
    key_waiting_for_frame: bool,
}

impl TimingStats {
    fn on_key(&mut self) {
        self.key_events = self.key_events.saturating_add(1);
        self.last_key_event = Some(Instant::now());
        self.key_waiting_for_frame = true;
    }
    fn on_redraw_begin(&mut self) { self.redraw_events = self.redraw_events.saturating_add(1); }
    fn on_redraw_end(&mut self, started: Instant) {
        self.frames_drawn = self.frames_drawn.saturating_add(1);
        let dt = started.elapsed().as_nanos() as u64;
        self.draw_ns.push(dt);
        if self.key_waiting_for_frame {
            if let Some(t0) = self.last_key_event.take() {
                let d = t0.elapsed().as_nanos() as u64;
                self.key_to_frame_ns.push(d);
            }
            self.key_waiting_for_frame = false;
        }
    }
    fn pct(ns: &[u64], p: f64) -> f64 {
        if ns.is_empty() { return 0.0; }
        let mut v = ns.to_vec();
        v.sort_unstable();
        let idx = ((v.len() as f64 - 1.0) * p).round() as usize;
        (v[idx] as f64) / 1_000_000.0
    }
    fn summarize(&self) -> String {
        let draw_p50 = Self::pct(&self.draw_ns, 0.50);
        let draw_p95 = Self::pct(&self.draw_ns, 0.95);
        let kf_p50 = Self::pct(&self.key_to_frame_ns, 0.50);
        let kf_p95 = Self::pct(&self.key_to_frame_ns, 0.95);
        format!(
            "app-timing: frames={}\n  redraw_events={} key_events={}\n  draw_ms: p50={:.2} p95={:.2}\n  key->frame_ms: p50={:.2} p95={:.2}",
            self.frames_drawn,
            self.redraw_events,
            self.key_events,
            draw_p50, draw_p95,
            kf_p50, kf_p95,
        )
    }
}

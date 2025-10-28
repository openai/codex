#![allow(dead_code)]

use super::{ChatWidget, ExecCallId, RunningCommand};
use crate::app_event::{AppEvent, AutoContinueMode};
use crate::app_event_sender::AppEventSender;
use crate::auto_drive_strings;
use crate::history_cell::{self, HistoryCellType};
use crate::markdown_render::render_markdown_text;
use crate::tui::TerminalInfo;
use crate::bottom_pane::SettingsSection;
use crossterm::event::KeyEvent;
use code_core::config::{Config, ConfigOverrides, ConfigToml};
use code_core::history::state::HistoryRecord;
use code_core::protocol::{BackgroundEventEvent, Event, EventMsg, OrderMeta};
use once_cell::sync::Lazy;
use chrono::Utc;
use ratatui::text::Line;
use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

static TEST_RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("test runtime")
});

pub struct ChatWidgetHarness {
    chat: ChatWidget<'static>,
    events: Receiver<AppEvent>,
    helper_seq: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct LayoutMetrics {
    pub scroll_offset: u16,
    pub last_viewport_height: u16,
    pub last_max_scroll: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoContinueModeFixture {
    Immediate,
    TenSeconds,
    SixtySeconds,
    Manual,
}

impl AutoContinueModeFixture {
    fn into_internal(self) -> AutoContinueMode {
        match self {
            Self::Immediate => AutoContinueMode::Immediate,
            Self::TenSeconds => AutoContinueMode::TenSeconds,
            Self::SixtySeconds => AutoContinueMode::SixtySeconds,
            Self::Manual => AutoContinueMode::Manual,
        }
    }
}

impl ChatWidgetHarness {
    pub fn new() -> Self {
        // Stabilize time-of-day dependent greeting so VT100 snapshots remain deterministic.
        // Safe: tests run single-threaded by design.
        unsafe { std::env::set_var("CODEX_TUI_FAKE_HOUR", "12"); }

        unsafe {
            std::env::set_var("CODEX_TUI_FORCE_MINIMAL_HEADER", "1");
        }

        unsafe {
            std::env::set_var("CODE_TUI_TEST_MODE", "1");
        }

        let cfg = Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            std::env::temp_dir(),
        )
        .expect("config");

        let (tx_raw, rx) = mpsc::channel::<AppEvent>();
        let app_event_tx = AppEventSender::new(tx_raw);
        let terminal_info = TerminalInfo {
            picker: None,
            font_size: (8, 16),
        };

        let runtime = &*TEST_RUNTIME;
        let _guard = runtime.enter();

        let chat = ChatWidget::new(
            cfg,
            app_event_tx,
            None,
            Vec::new(),
            false,
            terminal_info,
            false,
            None,
        );

        let mut harness = Self {
            chat,
            events: rx,
            helper_seq: 0,
        };
        harness.chat.auto_state.elapsed_override = Some(Duration::from_secs(1));
        harness
    }

    pub fn handle_event(&mut self, event: Event) {
        self.chat.handle_code_event(event);
    }

    pub(crate) fn flush_into_widget(&mut self) {
        let mut queue: VecDeque<AppEvent> = self
            .drain_events()
            .into_iter()
            .filter(|event| !matches!(event, AppEvent::RequestRedraw))
            .collect();

        while let Some(event) = queue.pop_front() {
            match event {
                AppEvent::InsertHistory(lines) => {
                    self.chat.insert_history_lines(lines);
                }
                AppEvent::InsertHistoryWithKind { id, kind, lines } => {
                    self.chat.insert_history_lines_with_kind(kind, id, lines);
                }
                AppEvent::InsertFinalAnswer { id, lines, source } => {
                    self.chat.insert_final_answer_with_id(id, lines, source);
                }
                AppEvent::InsertBackgroundEvent { message, placement, order } => {
                    self.chat
                        .insert_background_event_with_placement(message, placement, order);
                }
                AppEvent::UpdateAgentConfig {
                    name,
                    enabled,
                    args_read_only,
                    args_write,
                    instructions,
                    command,
                } => {
                    let runtime = &*TEST_RUNTIME;
                    let _guard = runtime.enter();
                    self.chat.apply_agent_update(
                        &name,
                        enabled,
                        args_read_only,
                        args_write,
                        instructions,
                        command,
                    );
                }
                AppEvent::ShowAgentsOverview => {
                    self.chat.ensure_settings_overlay_section(SettingsSection::Agents);
                    self.chat.show_agents_overview_ui();
                }
                AppEvent::CommitTick => {
                    self.chat.on_commit_tick();
                    let newly_emitted = self.drain_events();
                    if !newly_emitted.is_empty() {
                        queue.extend(
                            newly_emitted
                                .into_iter()
                                .filter(|event| !matches!(event, AppEvent::RequestRedraw)),
                        );
                    }
                }
                AppEvent::StartCommitAnimation
                | AppEvent::StopCommitAnimation
                | AppEvent::ScheduleFrameIn(_)
                | AppEvent::SetTerminalTitle { .. }
                | AppEvent::EmitTuiNotification { .. }
                | AppEvent::RequestRedraw
                | AppEvent::Redraw
                | AppEvent::PreviewTheme(_)
                | AppEvent::PreviewSpinner(_) => {}
                // Other events are either no-ops for VT100 rendering or handled elsewhere.
                _ => {}
            }
            if !queue.is_empty() {
                continue;
            }
            let newly_emitted = self.drain_events();
            if !newly_emitted.is_empty() {
                queue.extend(
                    newly_emitted
                        .into_iter()
                        .filter(|event| !matches!(event, AppEvent::RequestRedraw)),
                );
            }
        }
    }

    pub fn send_key(&mut self, key_event: KeyEvent) {
        self.chat.handle_key_event(key_event);
        self.flush_into_widget();
    }

    pub(crate) fn drain_events(&self) -> Vec<AppEvent> {
        let mut out = Vec::new();
        while let Ok(ev) = self.events.try_recv() {
            out.push(ev);
        }
        out
    }

    pub(crate) fn chat(&mut self) -> &mut ChatWidget<'static> {
        &mut self.chat
    }

    pub(crate) fn with_chat<R>(&mut self, f: impl FnOnce(&mut ChatWidget<'static>) -> R) -> R {
        let runtime = &*TEST_RUNTIME;
        let _guard = runtime.enter();
        f(&mut self.chat)
    }

    pub fn open_agents_settings_overlay(&mut self) {
        self.chat.ensure_settings_overlay_section(SettingsSection::Agents);
        self.chat.show_agents_overview_ui();
        self.flush_into_widget();
    }

    pub fn open_settings_overlay_overview(&mut self) {
        self.chat.show_settings_overlay(None);
        self.flush_into_widget();
    }

    pub fn suppress_rate_limit_refresh(&mut self) {
        self.chat.rate_limit_last_fetch_at = Some(Utc::now());
        self.chat.rate_limit_fetch_inflight = false;
    }

    pub fn show_agent_editor(&mut self, name: impl Into<String>) {
        self.chat.show_agent_editor_ui(name.into());
        self.flush_into_widget();
    }

    pub fn is_settings_overlay_visible(&mut self) -> bool {
        self.flush_into_widget();
        self.chat.settings.overlay.is_some()
    }

    pub fn settings_overlay_is_agents_active(&mut self) -> bool {
        self.flush_into_widget();
        self.chat
            .settings
            .overlay
            .as_ref()
            .map(|overlay| overlay.active_section() == SettingsSection::Agents)
            .unwrap_or(false)
    }

    pub fn agents_settings_is_agent_editor_active(&mut self) -> bool {
        self.flush_into_widget();
        self.chat
            .settings
            .overlay
            .as_ref()
            .and_then(|overlay| overlay.agents_content())
            .map(|content| content.is_agent_editor_active())
            .unwrap_or(false)
    }

    pub fn is_bottom_pane_active(&mut self) -> bool {
        self.flush_into_widget();
        self.chat.bottom_pane.has_active_view()
    }

    pub(crate) fn layout_metrics(&self) -> LayoutMetrics {
        LayoutMetrics {
            scroll_offset: self.chat.layout.scroll_offset,
            last_viewport_height: self.chat.layout.last_history_viewport_height.get(),
            last_max_scroll: self.chat.layout.last_max_scroll.get(),
        }
    }

    #[cfg(any(test, feature = "test-helpers"))]
    pub fn override_running_tool_elapsed(
        &mut self,
        call_id: &str,
        duration: Duration,
    ) {
        self.flush_into_widget();
        for cell in &mut self.chat.history_cells {
            if let Some(running) = cell
                .as_any_mut()
                .downcast_mut::<history_cell::RunningToolCallCell>()
            {
                if running.state().call_id.as_deref() == Some(call_id) {
                    running.override_elapsed_for_testing(duration);
                    break;
                }
            }
        }
    }

    pub fn push_user_prompt(&mut self, message: impl Into<String>) {
        let state = history_cell::new_user_prompt(message.into());
        self.chat.history_push_plain_state(state);
    }

    pub fn push_background_event(&mut self, message: impl Into<String>) {
        let seq = self.next_helper_seq();
        self.handle_event(Event {
            id: format!("bg-helper-{seq}"),
            event_seq: 0,
            msg: EventMsg::BackgroundEvent(BackgroundEventEvent {
                message: message.into(),
            }),
            order: Some(OrderMeta {
                request_ordinal: 0,
                output_index: Some(u32::MAX),
                sequence_number: Some(seq),
            }),
        });
    }

    pub fn push_assistant_markdown(&mut self, markdown: impl Into<String>) {
        let markdown = markdown.into();
        let mut rendered = render_markdown_text(&markdown);
        let mut lines: Vec<Line<'static>> = Vec::with_capacity(rendered.lines.len() + 1);
        lines.push(Line::from("assistant"));
        lines.extend(rendered.lines.drain(..));
        let state = history_cell::plain_message_state_from_lines(lines, HistoryCellType::Assistant);
        self.chat.history_push_plain_state(state);
    }

    pub fn auto_drive_activate(
        &mut self,
        goal: impl Into<String>,
        review_enabled: bool,
        agents_enabled: bool,
        continue_mode: AutoContinueModeFixture,
    ) {
        let goal = goal.into();
        {
            let chat = self.chat();
            let placeholder = auto_drive_strings::next_auto_drive_phrase().to_string();
            let mode = continue_mode.into_internal();
            chat.auto_state.reset();
            chat.auto_state.elapsed_override = Some(Duration::from_secs(1));
            chat.auto_state.goal = Some(goal);
            chat.auto_state.review_enabled = review_enabled;
            chat.auto_state.subagents_enabled = agents_enabled;
            chat.auto_state.continue_mode = mode;
            chat.auto_state.reset_countdown();
            let started_at = Instant::now() - Duration::from_secs(1);
            chat.auto_state.started_at = Some(started_at);
            chat.auto_state.on_prompt_submitted();
            chat.auto_state.set_coordinator_waiting(true);
            chat.auto_state.placeholder_phrase = Some(placeholder);
            chat.auto_state.current_display_line = None;
            chat.auto_state.current_progress_current = None;
            chat.auto_state.current_progress_past = None;
            chat.auto_state.current_cli_prompt = None;
            chat.auto_state.on_complete_review();
            chat.auto_state.last_run_summary = None;
            chat.auto_state.last_decision_summary = None;
            chat.auto_state.last_decision_progress_past = None;
            chat.auto_state.last_decision_progress_current = None;
            chat.auto_state.current_summary = None;
            chat.auto_state.current_summary_index = None;
            chat.auto_state.current_reasoning_title = None;
            chat.auto_state.thinking_prefix_stripped = false;
            chat.refresh_auto_drive_visuals();
            chat.request_redraw();
        }
        self.flush_into_widget();
    }

    pub fn auto_drive_set_waiting_for_response(
        &mut self,
        display: impl Into<String>,
        progress_current: Option<String>,
        progress_past: Option<String>,
    ) {
        {
            let chat = self.chat();
            chat.auto_state.on_complete_review();
            chat.auto_state.on_prompt_submitted();
            chat.auto_state.set_coordinator_waiting(false);
            chat.auto_state.current_display_line = Some(display.into());
            chat.auto_state.current_display_is_summary = false;
            chat.auto_state.placeholder_phrase = None;
            chat.auto_state.current_progress_current = progress_current.clone();
            chat.auto_state.current_progress_past = progress_past.clone();
            chat.auto_state.last_decision_progress_current = progress_current;
            chat.auto_state.last_decision_progress_past = progress_past;
            chat.auto_state.last_decision_summary = None;
            chat.auto_rebuild_live_ring();
            chat.request_redraw();
        }
        self.flush_into_widget();
    }

    pub fn auto_drive_mark_cli_running(&mut self) {
        let call_id = ExecCallId(format!("helper-cli-{}", self.helper_seq));
        self.helper_seq = self.helper_seq.wrapping_add(1);
        {
            let chat = self.chat();
            chat.exec.running_commands.insert(
                call_id,
                RunningCommand {
                    command: Vec::new(),
                    parsed: Vec::new(),
                    history_index: None,
                    history_id: None,
                    explore_entry: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    wait_total: None,
                    wait_active: false,
                    wait_notes: Vec::new(),
                },
            );
            chat.refresh_auto_drive_visuals();
        }
        self.flush_into_widget();
    }

    pub fn auto_drive_simulate_cli_submission(&mut self) {
        {
            let chat = self.chat();
            chat.auto_state.on_prompt_submitted();
            chat.auto_state.set_coordinator_waiting(false);
            chat.auto_state.current_progress_current = None;
            chat.auto_state.current_progress_past = None;
            chat.refresh_auto_drive_visuals();
            chat.request_redraw();
        }
        self.flush_into_widget();
    }

    pub fn auto_drive_set_awaiting_submission(
        &mut self,
        cli_prompt: impl Into<String>,
        headline: impl Into<String>,
        summary: Option<String>,
    ) {
        {
            let chat = self.chat();
            chat.auto_state.on_prompt_ready(true);
            chat.auto_state.set_waiting_for_response(false);
            chat.auto_state.set_coordinator_waiting(false);
            chat.auto_state.on_complete_review();
            chat.auto_state.current_cli_prompt = Some(cli_prompt.into());
            chat.auto_state.current_display_line = Some(headline.into());
            chat.auto_state.current_display_is_summary = summary.is_some();
            chat.auto_state.placeholder_phrase = None;
            if let Some(text) = summary {
                chat.auto_state.current_summary = Some(text.clone());
                chat.auto_state.last_decision_summary = Some(text);
            } else {
                chat.auto_state.current_summary = None;
                chat.auto_state.last_decision_summary = None;
            }
            chat.auto_state.reset_countdown();
            chat.auto_state.countdown_id = chat.auto_state.countdown_id.wrapping_add(1);
            chat.auto_state.seconds_remaining =
                chat.auto_state.countdown_seconds().unwrap_or(0);
            chat.auto_rebuild_live_ring();
            chat.request_redraw();
        }
        self.flush_into_widget();
    }

    pub fn auto_drive_override_countdown(&mut self, seconds_remaining: u8) {
        {
            let chat = self.chat();
            chat.auto_state.seconds_remaining = seconds_remaining;
            chat.auto_rebuild_live_ring();
            chat.request_redraw();
        }
        self.flush_into_widget();
    }

    pub fn auto_drive_set_continue_mode(&mut self, mode: AutoContinueModeFixture) {
        {
            let chat = self.chat();
            let mode = mode.into_internal();
            chat.auto_state.continue_mode = mode;
            chat.auto_state.reset_countdown();
            let countdown = chat.auto_state.countdown_seconds();
            chat.auto_state.countdown_id = chat.auto_state.countdown_id.wrapping_add(1);
            chat.auto_state.seconds_remaining = countdown.unwrap_or(0);
            if chat.auto_state.awaiting_coordinator_submit() && !chat.auto_state.is_paused_manual() {
                if let Some(seconds) = countdown {
                    chat.auto_spawn_countdown(chat.auto_state.countdown_id, seconds);
                } else {
                    let _ = chat.auto_handle_countdown(chat.auto_state.countdown_id, 0);
                }
                if countdown == Some(0) {
                    chat.auto_state.on_prompt_submitted();
                    chat.auto_state.set_coordinator_waiting(false);
                    chat.auto_state.seconds_remaining = 0;
                }
            }
            chat.refresh_auto_drive_visuals();
            chat.request_redraw();
        }
        self.flush_into_widget();
    }

    pub fn auto_drive_advance_countdown(&mut self, seconds_left: u8) {
        {
            let chat = self.chat();
            let countdown_id = chat.auto_state.countdown_id;
            let runtime = &*TEST_RUNTIME;
            let _guard = runtime.enter();
            chat.auto_handle_countdown(countdown_id, seconds_left);
        }
        self.flush_into_widget();
    }

    pub fn auto_drive_set_waiting_for_review(&mut self, summary: Option<String>) {
        {
            let chat = self.chat();
            chat.auto_state.on_prompt_submitted();
            chat.auto_state.on_begin_review(summary.is_some());
            chat.auto_state.set_coordinator_waiting(false);
            if let Some(text) = summary {
                chat.auto_state.current_summary = Some(text.clone());
                chat.auto_state.current_display_line = Some(text);
                chat.auto_state.current_display_is_summary = true;
            } else {
                chat.auto_state.current_summary = None;
                chat.auto_state.current_display_line = None;
                chat.auto_state.current_display_is_summary = false;
            }
            chat.auto_state.placeholder_phrase = None;
            chat.auto_rebuild_live_ring();
            chat.request_redraw();
        }
        self.flush_into_widget();
    }

    pub(crate) fn set_standard_terminal_mode(&mut self, enabled: bool) {
        self.chat.set_standard_terminal_mode(enabled);
    }

    pub(crate) fn force_scroll_offset(&mut self, offset: u16) {
        self.chat.layout.scroll_offset = offset;
    }

    pub(crate) fn scroll_offset(&self) -> u16 {
        self.chat.layout.scroll_offset
    }

    pub(crate) fn poll_until<F>(&mut self, mut predicate: F, timeout: Duration) -> Vec<AppEvent>
    where
        F: FnMut(&[AppEvent]) -> bool,
    {
        let deadline = Instant::now() + timeout;
        let mut collected = Vec::new();

        loop {
            while let Ok(event) = self.events.try_recv() {
                if !matches!(event, AppEvent::RequestRedraw) {
                    collected.push(event);
                }
            }

            if predicate(&collected) {
                break;
            }

            if Instant::now() >= deadline {
                break;
            }

            std::thread::sleep(Duration::from_millis(5));
        }

        collected
    }

    pub(crate) fn history_records(&mut self) -> Vec<HistoryRecord> {
        self.flush_into_widget();
        self.chat.history_state.records.clone()
    }

    pub fn count_agent_run_cells(&mut self) -> usize {
        self.flush_into_widget();
        self.chat
            .history_cells
            .iter()
            .filter(|cell| {
                cell.as_any()
                    .downcast_ref::<history_cell::AgentRunCell>()
                    .is_some()
            })
            .count()
    }

    fn next_helper_seq(&mut self) -> u64 {
        let next = self.helper_seq;
        self.helper_seq = self.helper_seq.saturating_add(1);
        next
    }
}

pub fn assert_has_insert_history(events: &[AppEvent]) {
    let found = events.iter().any(|event| {
        matches!(
            event,
            AppEvent::InsertHistory(_) | AppEvent::InsertHistoryWithKind { .. } | AppEvent::InsertFinalAnswer { .. }
        ) || matches!(
            event,
            AppEvent::CodexEvent(ev)
                if crate::chatwidget::is_test_mode()
                    && matches!(ev.msg, EventMsg::SessionConfigured(_))
        )
    });
    assert!(found, "expected InsertHistory-like event, got: {events:#?}");
}

pub fn assert_has_background_event_containing(events: &[AppEvent], needle: &str) {
    let found = events.iter().any(|event| {
        matches!(event, AppEvent::InsertBackgroundEvent { message, .. } if message.contains(needle))
    });
    assert!(
        found,
        "expected InsertBackgroundEvent containing '{needle}', got: {events:#?}"
    );
}

pub fn assert_has_terminal_chunk_containing(events: &[AppEvent], needle: &str) {
    let found = events.iter().any(|event| {
        if let AppEvent::TerminalChunk { chunk, .. } = event {
            String::from_utf8_lossy(chunk).contains(needle)
        } else {
            false
        }
    });
    assert!(
        found,
        "expected TerminalChunk containing '{needle}', got: {events:#?}"
    );
}

pub fn assert_has_codex_event(events: &[AppEvent]) {
    assert!(
        events.iter().any(|event| matches!(event, AppEvent::CodexEvent(_))),
        "expected CodexEvent, got: {events:#?}"
    );
}

pub fn assert_no_events(events: &[AppEvent]) {
    assert!(events.is_empty(), "expected no events, got: {events:#?}");
}

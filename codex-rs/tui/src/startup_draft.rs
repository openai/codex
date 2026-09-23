//! Keep the first composer editable and bottom-anchored while startup work continues.
//! Submit keys confirm one draft locally; session dispatch waits for the protected handoff.

use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;
use std::time::Instant;

use crossterm::SynchronizedUpdate;
use ratatui::layout::Size;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::unbounded_channel;
use tokio_stream::Stream;
use tokio_stream::StreamExt;

use crate::TerminalRestoreGuard;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPane;
use crate::bottom_pane::BottomPaneParams;
use crate::bottom_pane::ChatComposer;
use crate::bottom_pane::ChatComposerConfig;
use crate::bottom_pane::ComposerDraftSnapshot;
use crate::history_cell;
use crate::history_cell::HistoryCell;
use crate::keymap::RuntimeKeymap;
use crate::legacy_core::config::Config;
use crate::render::Insets;
use crate::render::renderable::FlexRenderable;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableExt;
use crate::render::renderable::RenderableItem;
use crate::resume_picker::SessionSelection;
use crate::tui;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use crate::version::CODEX_CLI_VERSION;

const STARTUP_EVENT_BATCH_SIZE: usize = 64;
const STARTUP_PASTE_NEWLINE_TIMEOUT: Duration = Duration::from_millis(120);

#[path = "startup_draft_layout.rs"]
mod layout;

#[path = "startup_draft_input.rs"]
mod input;
#[cfg(test)]
use input::handle_startup_draft_key;

/// Locally resolved presentation used before the first editable frame.
pub(crate) struct StartupScreen {
    pub(crate) use_alt_screen: bool,
    pub(crate) transcript_mode: crate::transcript_mode::TranscriptMode,
    pub(crate) status_line_enabled: bool,
    pub(crate) keymap: RuntimeKeymap,
    pub(crate) disable_paste_burst: bool,
}

/// Identifies the first interactive surface expected for the current invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StartupDraftInitialScreen {
    Composer,
    Onboarding,
    SessionPicker,
}

/// Describes the session being prepared behind the provisional composer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StartupDraftSessionAction {
    New,
    NewFromCommandCenter,
    Resume,
    Fork,
}

/// Marks intentional startup cancellation so unrelated I/O interrupts remain errors.
#[derive(Debug, thiserror::Error)]
#[error("startup cancelled")]
pub(crate) struct StartupCancelled;

impl StartupCancelled {
    /// Distinguish intentional user cancellation from unrelated interrupted I/O.
    pub(crate) fn matches(error: &io::Error) -> bool {
        error.kind() == io::ErrorKind::Interrupted
            && error.get_ref().is_some_and(<dyn std::error::Error + std::marker::Send + std::marker::Sync + 'static>::is::<Self>)
    }
}

/// Owns a real composer without allowing prompts, commands, or image access before startup.
pub(crate) struct StartupDraft {
    tui: Tui,
    terminal_restore_guard: TerminalRestoreGuard,
    pump: StartupDraftPump,
}

/// Keeps terminal input responsive and carries one submission intent into the live session.
pub(crate) struct StartupDraftPump {
    header: Box<dyn HistoryCell>,
    bottom_pane: BottomPane,
    events: Pin<Box<dyn Stream<Item = TuiEvent> + Send>>,
    app_event_rx: UnboundedReceiver<AppEvent>,
    initial_screen: StartupDraftInitialScreen,
    session_action: StartupDraftSessionAction,
    resolved_selection: Option<SessionSelection>,
    configured_cwd: Option<PathBuf>,
    pending_paste_newline: Option<(Instant, String)>,
    submission_pending: bool,
    key_chord_matcher: crate::keymap::KeyChordMatcher,
    key_chords: std::sync::Arc<crate::keymap::RuntimeChordKeymap>,
}

impl StartupDraft {
    /// Adopt the initialized terminal and apply editing policy before the first composer paint.
    pub(crate) fn new(
        mut initialized_terminal: tui::InitializedTerminal,
        terminal_restore_guard: TerminalRestoreGuard,
        initial_screen: StartupDraftInitialScreen,
        session_action: StartupDraftSessionAction,
        screen: StartupScreen,
    ) -> io::Result<Self> {
        initialized_terminal.terminal.clear()?;

        let mut tui = Tui::new(
            initialized_terminal.terminal,
            initialized_terminal.enhanced_keys_supported,
            initialized_terminal.stderr_guard,
        );
        tui.terminal_app_over_ssh = initialized_terminal.terminal_app_over_ssh;
        tui.set_alt_screen_enabled(screen.use_alt_screen);
        let mut pump = StartupDraftPump::new(&tui, initial_screen, session_action);
        pump.bottom_pane
            .set_status_line_enabled(screen.status_line_enabled);
        pump.bottom_pane.set_keymap_bindings(&screen.keymap);
        pump.bottom_pane
            .set_disable_paste_burst(screen.disable_paste_burst);
        pump.key_chords = screen.keymap.chords;
        let mut draft = Self {
            tui,
            terminal_restore_guard,
            pump,
        };
        // Present the screen switch and its first composer frame together.
        std::io::stdout().sync_update(|_| {
            draft
                .tui
                .set_owned_screen(screen.transcript_mode.is_owned())?;
            draft.pump.redraw_if_visible(&mut draft.tui)
        })??;
        Ok(draft)
    }

    /// Accept draft edits on the current task until an existing startup operation completes.
    pub(crate) async fn run_until<F>(&mut self, future: F) -> io::Result<F::Output>
    where
        F: Future,
    {
        self.pump.run_until(&mut self.tui, future).await
    }

    /// Capture queued draft edits before lending the terminal to another input owner.
    pub(crate) async fn flush_pending_events(&mut self) -> io::Result<()> {
        self.pump.flush_pending_events(&mut self.tui).await
    }

    /// Apply the loaded editing preferences without enabling startup actions or submission.
    pub(crate) fn apply_config(&mut self, config: &Config) {
        self.pump.apply_config(config);
    }

    /// Lend the original terminal to an existing interactive startup screen.
    pub(crate) fn tui_mut(&mut self) -> &mut Tui {
        self.pump.key_chord_matcher.cancel();
        &mut self.tui
    }

    /// Transfer terminal ownership while keeping the same draft pump available to later startup.
    pub(crate) fn into_parts(self) -> (Tui, TerminalRestoreGuard, StartupDraftPump) {
        (self.tui, self.terminal_restore_guard, self.pump)
    }
}

impl StartupDraftPump {
    pub(crate) fn new(
        tui: &Tui,
        initial_screen: StartupDraftInitialScreen,
        session_action: StartupDraftSessionAction,
    ) -> Self {
        let (app_event_tx, app_event_rx) = unbounded_channel();
        Self {
            header: startup_session_header(/*config*/ None),
            bottom_pane: startup_draft_bottom_pane(
                AppEventSender::new(app_event_tx),
                tui.frame_requester(),
                tui.enhanced_keys_supported(),
            ),
            events: tui.event_stream(),
            app_event_rx,
            initial_screen,
            session_action,
            pending_paste_newline: None,
            resolved_selection: None,
            configured_cwd: None,
            submission_pending: false,
            key_chord_matcher: Default::default(),
            key_chords: RuntimeKeymap::defaults().chords,
        }
    }

    pub(crate) fn take_draft(&mut self) -> ComposerDraftSnapshot {
        self.bottom_pane.flush_composer_paste_burst();
        self.bottom_pane.composer_draft_snapshot()
    }

    /// Keep a provisional composer responsive when the caller has one to display.
    pub(crate) async fn run_with_optional_draft<F, T, E>(
        draft: Option<&mut Self>,
        tui: &mut Tui,
        future: F,
    ) -> Result<T, E>
    where
        F: Future<Output = Result<T, E>>,
        E: From<io::Error>,
    {
        match draft {
            Some(draft) => draft.run_until(tui, future).await?,
            None => future.await,
        }
    }

    /// Refresh the session header and safe editor shortcuts without enabling modal editing.
    pub(crate) fn apply_config(&mut self, config: &Config) {
        if self
            .configured_cwd
            .as_deref()
            .is_some_and(|cwd| cwd != config.cwd.as_path())
        {
            self.cancel_submission();
        }
        self.configured_cwd = Some(config.cwd.to_path_buf());
        let local_settings = crate::local_settings::LocalSettings::from(config);
        self.header = startup_session_header(Some(config));
        self.bottom_pane.set_status_line_enabled(
            local_settings
                .tui
                .status_line
                .as_ref()
                .is_none_or(|items| !items.is_empty()),
        );
        self.bottom_pane
            .set_disable_paste_burst(local_settings.tui.disable_paste_burst.unwrap_or(false));
        self.bottom_pane.request_redraw();
        if let Ok(keymap) = RuntimeKeymap::from_config(&local_settings.tui.keymap) {
            if crate::keymap::keymap_action_id("composer", "submit").is_none_or(|submit| {
                self.key_chords.configured_specs(submit) != keymap.chords.configured_specs(submit)
            }) {
                self.cancel_submission();
            }
            self.bottom_pane.set_keymap_bindings(&keymap);
            self.key_chord_matcher.cancel();
            self.key_chords = keymap.chords;
        }
    }

    /// Align the loading message and revoke confirmation when startup changes its destination.
    pub(crate) fn update_session_selection(
        &mut self,
        tui: &mut Tui,
        session_selection: &SessionSelection,
    ) -> io::Result<()> {
        let session_action = match session_selection {
            SessionSelection::StartFresh
            | SessionSelection::Exit
            | SessionSelection::AgentsOverview => StartupDraftSessionAction::New,
            SessionSelection::Resume(_) => StartupDraftSessionAction::Resume,
            SessionSelection::Fork(_) => StartupDraftSessionAction::Fork,
        };
        let destination_changed = match self.resolved_selection.as_ref() {
            Some(SessionSelection::StartFresh) => {
                !matches!(session_selection, SessionSelection::StartFresh)
            }
            Some(SessionSelection::AgentsOverview) => {
                !matches!(session_selection, SessionSelection::AgentsOverview)
            }
            Some(SessionSelection::Exit) => !matches!(session_selection, SessionSelection::Exit),
            Some(SessionSelection::Resume(previous)) => {
                !matches!(session_selection, SessionSelection::Resume(next) if previous.thread_id == next.thread_id && previous.cwd == next.cwd)
            }
            Some(SessionSelection::Fork(previous)) => {
                !matches!(session_selection, SessionSelection::Fork(next) if previous.thread_id == next.thread_id && previous.cwd == next.cwd)
            }
            None => self.session_action != session_action,
        };
        if destination_changed
            || matches!(
                session_selection,
                SessionSelection::AgentsOverview | SessionSelection::Exit
            )
        {
            self.cancel_submission();
        }
        self.resolved_selection = Some(session_selection.clone());
        self.session_action = session_action;
        if self.initial_screen == StartupDraftInitialScreen::Composer {
            self.draw(tui, tui.terminal.last_known_screen_size)?;
        }
        Ok(())
    }

    /// Poll one existing startup future alongside the original terminal input stream.
    pub(crate) async fn run_until<F>(&mut self, tui: &mut Tui, future: F) -> io::Result<F::Output>
    where
        F: Future,
    {
        if self.initial_screen == StartupDraftInitialScreen::Composer {
            self.draw(tui, tui.terminal.last_known_screen_size)?;
        }
        tokio::pin!(future);
        loop {
            tokio::select! {
                output = &mut future => return Ok(output),
                event = self.events.next() => {
                    let Some(event) = event else {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "terminal input stream closed during startup",
                        ));
                    };
                    self.handle_event(tui, event)?;
                }
            }
        }
    }

    /// Preserve pending draft edits while continuing to reject startup actions and submission.
    pub(crate) async fn flush_pending_events(&mut self, tui: &mut Tui) -> io::Result<()> {
        loop {
            for _ in 0..STARTUP_EVENT_BATCH_SIZE {
                let Poll::Ready(Some(event)) = std::future::poll_fn(|context| {
                    Poll::Ready(self.events.as_mut().poll_next(context))
                })
                .await
                else {
                    self.bottom_pane.flush_composer_paste_burst();
                    return Ok(());
                };
                self.handle_event(tui, event)?;
            }

            tokio::task::yield_now().await;
        }
    }

    /// Resolve an ambiguous paste newline before its provisional input owner disappears.
    pub(crate) async fn flush_pending_paste_newline(&mut self, tui: &mut Tui) -> io::Result<()> {
        while let Some((started_at, _)) = &self.pending_paste_newline {
            let Some(remaining) = STARTUP_PASTE_NEWLINE_TIMEOUT.checked_sub(started_at.elapsed())
            else {
                self.pending_paste_newline = None;
                break;
            };
            let Ok(Some(event)) = tokio::time::timeout(remaining, self.events.next()).await else {
                self.pending_paste_newline = None;
                break;
            };
            self.handle_event(tui, event)?;
        }
        self.bottom_pane.flush_composer_paste_burst();
        Ok(())
    }

    /// Preserve the editable draft, its cursor, and any pending large-paste placeholders.
    pub(crate) fn into_draft(mut self) -> ComposerDraftSnapshot {
        let draft = self.take_draft();
        crate::startup_recovery::remember(draft.clone());
        draft
    }

    /// Transfer the single confirmed submission without removing its visible draft.
    pub(crate) fn take_submission_intent(&mut self) -> bool {
        std::mem::take(&mut self.submission_pending)
    }

    fn cancel_submission(&mut self) {
        if std::mem::take(&mut self.submission_pending) {
            self.bottom_pane.set_footer_hint_override(/*items*/ None);
        }
    }

    /// Redraw the composer without revealing it while a protected startup screen owns input.
    pub(crate) fn redraw_if_visible(&mut self, tui: &mut Tui) -> io::Result<()> {
        if self.initial_screen == StartupDraftInitialScreen::Composer {
            self.show(tui)?;
        }
        Ok(())
    }

    /// Reveal the editable composer once an expected protected screen has finished.
    pub(crate) fn show(&mut self, tui: &mut Tui) -> io::Result<()> {
        self.initial_screen = StartupDraftInitialScreen::Composer;
        self.draw(tui, tui.terminal.last_known_screen_size)
    }

    fn draw(&mut self, tui: &mut Tui, screen_size: Size) -> io::Result<()> {
        if self.bottom_pane.flush_paste_burst_if_due() {
            crate::startup_recovery::remember(self.bottom_pane.composer_recovery_snapshot());
        }
        if self.bottom_pane.is_in_paste_burst() {
            tui.frame_requester()
                .schedule_frame_in(ChatComposer::recommended_paste_flush_delay());
        }
        self.bottom_pane.pre_draw_tick();
        let owned = tui.is_owned_screen();
        let owned_layout =
            layout::OwnedStartupLayout::new(&self.header, &self.bottom_pane, self.session_action);
        let renderable = if owned {
            RenderableItem::Borrowed(&owned_layout)
        } else {
            startup_draft_renderable(&self.header, &self.bottom_pane, self.session_action)
        };
        let desired_height = if owned {
            screen_size.height
        } else {
            renderable.desired_height(screen_size.width)
        };
        tui.draw_with_resize_reflow(desired_height, screen_size, |frame| {
            let area = frame.area();
            renderable.render(area, frame.buffer);
            if let Some((x, y)) = renderable.cursor_pos(area) {
                frame.set_cursor_style(renderable.cursor_style(area));
                frame.set_cursor_position((x, y));
            }
        })?;
        Ok(())
    }
}

fn startup_session_header(config: Option<&Config>) -> Box<dyn HistoryCell> {
    let placeholder_style = Style::default().add_modifier(Modifier::DIM | Modifier::ITALIC);
    let directory = config.map_or_else(
        || PathBuf::from("loading"),
        |config| config.cwd.to_path_buf(),
    );
    Box::new(
        history_cell::SessionHeaderHistoryCell::new_with_style(
            "loading".to_string(),
            placeholder_style,
            /*reasoning_effort*/ None,
            /*show_fast_status*/ false,
            directory,
            CODEX_CLI_VERSION,
        )
        .with_yolo_mode(config.is_some_and(history_cell::is_yolo_mode)),
    )
}

fn startup_draft_renderable<'a>(
    header: &'a dyn Renderable,
    bottom_pane: &'a BottomPane,
    session_action: StartupDraftSessionAction,
) -> RenderableItem<'a> {
    let mut renderable = FlexRenderable::new();
    renderable.push(/*flex*/ 1, RenderableItem::Borrowed(header));
    let loading_message = match session_action {
        StartupDraftSessionAction::New | StartupDraftSessionAction::NewFromCommandCenter => None,
        StartupDraftSessionAction::Resume => Some("  Resuming session…"),
        StartupDraftSessionAction::Fork => Some("  Forking session…"),
    };
    if let Some(loading_message) = loading_message {
        renderable.push(
            /*flex*/ 0,
            RenderableItem::Owned(Box::new(loading_message.dim())),
        );
    }
    renderable.push(
        /*flex*/ 0,
        bottom_pane
            .as_renderable_with_options(crate::bottom_pane::ComposerRenderOptions::default())
            .inset(Insets::tlbr(
                /*top*/ u16::from(loading_message.is_none()),
                /*left*/ 0,
                /*bottom*/ 0,
                /*right*/ 0,
            )),
    );
    RenderableItem::Owned(Box::new(renderable))
}

fn startup_draft_bottom_pane(
    app_event_tx: AppEventSender,
    frame_requester: FrameRequester,
    enhanced_keys_supported: bool,
) -> BottomPane {
    let mut bottom_pane = BottomPane::new_with_composer_config(
        BottomPaneParams {
            app_event_tx,
            frame_requester,
            has_input_focus: true,
            enhanced_keys_supported,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
            animations_enabled: crate::system_motion::mode() == crate::motion::MotionMode::Animated,
            effects: Default::default(),
            skills: None,
        },
        ChatComposerConfig::plain_text(),
    );
    bottom_pane.set_context_window_pending(/*pending*/ true);
    bottom_pane
}

#[cfg(test)]
#[path = "startup_draft_tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
#[path = "startup_draft_submission_tests.rs"]
mod submission_tests;

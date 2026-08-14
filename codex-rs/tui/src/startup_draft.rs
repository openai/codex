//! Display an editable, non-submitting composer while startup work continues.

use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;
use std::time::Instant;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::layout::Size;
use ratatui::style::Modifier;
use ratatui::style::Style;
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
use crate::key_hint;
use crate::keymap::RuntimeKeymap;
use crate::legacy_core::config::Config;
use crate::render::Insets;
use crate::render::renderable::FlexRenderable;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableExt;
use crate::render::renderable::RenderableItem;
use crate::tui;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use crate::version::CODEX_CLI_VERSION;

const STARTUP_EVENT_BATCH_SIZE: usize = 64;
const STARTUP_PASTE_NEWLINE_TIMEOUT: Duration = Duration::from_millis(120);

/// Identifies the first interactive surface expected for the current invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StartupDraftInitialScreen {
    Composer,
    Onboarding,
    SessionPicker,
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

/// Keeps the existing terminal responsive without owning it or permitting startup submission.
pub(crate) struct StartupDraftPump {
    header: Box<dyn HistoryCell>,
    bottom_pane: BottomPane,
    events: Pin<Box<dyn Stream<Item = TuiEvent> + Send>>,
    app_event_rx: UnboundedReceiver<AppEvent>,
    initial_screen: StartupDraftInitialScreen,
    pending_paste_newline: Option<(Instant, String)>,
}

impl StartupDraft {
    /// Initialize the terminal without showing a composer before an expected session picker.
    pub(crate) fn new(initial_screen: StartupDraftInitialScreen) -> io::Result<Self> {
        let mut initialized_terminal = tui::init()?;
        let terminal_restore_guard = TerminalRestoreGuard::new();
        initialized_terminal.terminal.clear()?;

        let tui = Tui::new(
            initialized_terminal.terminal,
            initialized_terminal.enhanced_keys_supported,
            initialized_terminal.stderr_guard,
        );
        let (app_event_tx, app_event_rx) = unbounded_channel();
        let bottom_pane = startup_draft_bottom_pane(
            AppEventSender::new(app_event_tx),
            tui.frame_requester(),
            tui.enhanced_keys_supported(),
        );
        let events = tui.event_stream();
        let mut draft = Self {
            tui,
            terminal_restore_guard,
            pump: StartupDraftPump {
                header: startup_session_header(/*config*/ None),
                bottom_pane,
                events,
                app_event_rx,
                initial_screen,
                pending_paste_newline: None,
            },
        };
        draft.pump.show_initial_screen(&mut draft.tui)?;
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
        &mut self.tui
    }

    /// Transfer terminal ownership while keeping the same draft pump available to later startup.
    pub(crate) fn into_parts(self) -> (Tui, TerminalRestoreGuard, StartupDraftPump) {
        (self.tui, self.terminal_restore_guard, self.pump)
    }
}

impl StartupDraftPump {
    /// Refresh the session header and safe editor shortcuts without enabling modal editing.
    pub(crate) fn apply_config(&mut self, config: &Config) {
        self.header = startup_session_header(Some(config));
        self.bottom_pane
            .set_disable_paste_burst(config.disable_paste_burst);
        self.bottom_pane.request_redraw();
        if let Ok(keymap) = RuntimeKeymap::from_config(&config.tui_keymap) {
            self.bottom_pane.set_keymap_bindings(&keymap);
        }
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
        self.bottom_pane.flush_composer_paste_burst();
        self.bottom_pane.composer_draft_snapshot()
    }

    /// Draw the initial composer only when no protected startup screen must appear first.
    fn show_initial_screen(&mut self, tui: &mut Tui) -> io::Result<()> {
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

    fn handle_event(&mut self, tui: &mut Tui, event: TuiEvent) -> io::Result<()> {
        let screen_size = tui.screen_size_for_event(&event)?;
        if let Some((started_at, mut newlines)) = self.pending_paste_newline.take() {
            let continues_paste = match &event {
                TuiEvent::Key(KeyEvent {
                    code: KeyCode::Char(_),
                    modifiers,
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) => !key_hint::has_ctrl_or_alt(*modifiers),
                TuiEvent::Key(KeyEvent {
                    code: KeyCode::Enter,
                    modifiers,
                    kind: KeyEventKind::Press | KeyEventKind::Repeat,
                    ..
                }) if modifiers.is_empty() => {
                    if started_at.elapsed() <= STARTUP_PASTE_NEWLINE_TIMEOUT {
                        newlines.push('\n');
                        self.pending_paste_newline = Some((Instant::now(), newlines));
                    }
                    return Ok(());
                }
                TuiEvent::Paste(text) => !text.is_empty(),
                TuiEvent::Draw | TuiEvent::Resize(_) | TuiEvent::Resume => {
                    self.pending_paste_newline = Some((started_at, newlines));
                    if self.initial_screen == StartupDraftInitialScreen::Composer {
                        self.draw(tui, screen_size)?;
                    }
                    return Ok(());
                }
                TuiEvent::Key(_) => false,
            };
            if continues_paste && started_at.elapsed() <= STARTUP_PASTE_NEWLINE_TIMEOUT {
                self.bottom_pane.handle_paste(newlines);
            }
        }
        match event {
            TuiEvent::Key(key) => {
                if self.initial_screen != StartupDraftInitialScreen::Composer
                    && !key_hint::ctrl(KeyCode::Char('c')).is_press(key)
                    && !key_hint::ctrl(KeyCode::Char('d')).is_press(key)
                {
                    return Ok(());
                }
                if key.code == KeyCode::Enter
                    && key.modifiers.is_empty()
                    && self.bottom_pane.is_in_paste_burst()
                {
                    let text_len = self.bottom_pane.composer_text().len();
                    self.bottom_pane.flush_composer_paste_burst();
                    if self
                        .bottom_pane
                        .composer_text()
                        .len()
                        .saturating_sub(text_len)
                        > 1
                    {
                        self.pending_paste_newline = Some((Instant::now(), "\n".to_string()));
                    }
                }
                handle_startup_draft_key(&mut self.bottom_pane, key).inspect_err(|error| {
                    if StartupCancelled::matches(error)
                        && let Err(clear_error) = tui.terminal.clear()
                    {
                        tracing::warn!(
                            error = %clear_error,
                            "failed to clear the cancelled startup composer"
                        );
                    }
                })?;
            }
            TuiEvent::Paste(text) => {
                if self.initial_screen == StartupDraftInitialScreen::Composer {
                    self.bottom_pane.flush_composer_paste_burst();
                    self.bottom_pane.handle_paste(text);
                }
            }
            TuiEvent::Draw | TuiEvent::Resize(_) | TuiEvent::Resume => {}
        }
        if self.initial_screen == StartupDraftInitialScreen::Composer {
            self.draw(tui, screen_size)?;
        }
        while self.app_event_rx.try_recv().is_ok() {}
        Ok(())
    }

    fn draw(&mut self, tui: &mut Tui, screen_size: Size) -> io::Result<()> {
        let _ = self.bottom_pane.flush_paste_burst_if_due();
        if self.bottom_pane.is_in_paste_burst() {
            tui.frame_requester()
                .schedule_frame_in(ChatComposer::recommended_paste_flush_delay());
        }
        self.bottom_pane.pre_draw_tick();
        let renderable = startup_draft_renderable(&self.header, &self.bottom_pane);
        let desired_height = renderable.desired_height(screen_size.width);
        tui.draw_with_resize_reflow(desired_height, screen_size, |frame| {
            let area = frame.area();
            renderable.render(area, frame.buffer);
            if let Some((x, y)) = renderable.cursor_pos(area) {
                frame.set_cursor_style(renderable.cursor_style(area));
                frame.set_cursor_position((x, y));
            }
        })
    }
}

fn handle_startup_draft_key(bottom_pane: &mut BottomPane, key: KeyEvent) -> io::Result<()> {
    let _ = bottom_pane.flush_paste_burst_if_due();
    if key.kind == KeyEventKind::Release
        || key.code == KeyCode::Enter && key.modifiers.is_empty()
        || matches!(key.code, KeyCode::Tab | KeyCode::BackTab)
        || bottom_pane.is_startup_composer_action(key)
    {
        return Ok(());
    }

    if let KeyCode::Char(_) = key.code {
        let (code, modifiers) = key_hint::normalize_key_parts(key.code, key.modifiers);
        if key_hint::has_ctrl_or_alt(modifiers) && matches!(code, KeyCode::Char('r' | 'v')) {
            return Ok(());
        }
        let is_ctrl_c = key_hint::ctrl(KeyCode::Char('c')).is_press(key);
        let is_ctrl_d = key_hint::ctrl(KeyCode::Char('d')).is_press(key);
        if key.kind == KeyEventKind::Press && (is_ctrl_c || is_ctrl_d) {
            bottom_pane.flush_composer_paste_burst();
            if bottom_pane.composer_is_empty() {
                return Err(io::Error::new(io::ErrorKind::Interrupted, StartupCancelled));
            }
            if is_ctrl_c {
                bottom_pane.on_ctrl_c();
                return Ok(());
            }
        }
    }

    if key_hint::has_ctrl_or_alt(key.modifiers) && !bottom_pane.is_safe_startup_editor_key(key)
        || key.code == KeyCode::Enter && !bottom_pane.is_safe_startup_editor_key(key)
        || key
            .modifiers
            .intersects(KeyModifiers::SUPER | KeyModifiers::HYPER | KeyModifiers::META)
        || !matches!(
            key.code,
            KeyCode::Char(_)
                | KeyCode::Enter
                | KeyCode::Esc
                | KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Home
                | KeyCode::End
                | KeyCode::Backspace
                | KeyCode::Delete
        )
    {
        return Ok(());
    }

    let _ = bottom_pane.handle_key_event(key);
    Ok(())
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
) -> RenderableItem<'a> {
    let mut renderable = FlexRenderable::new();
    renderable.push(/*flex*/ 1, RenderableItem::Borrowed(header));
    renderable.push(
        /*flex*/ 0,
        bottom_pane
            .as_renderable_with_composer_right_reserve(/*composer_right_reserve*/ 0)
            .inset(Insets::tlbr(
                /*top*/ 1, /*left*/ 0, /*bottom*/ 0, /*right*/ 0,
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
            animations_enabled: true,
            skills: None,
        },
        ChatComposerConfig::plain_text(),
    );
    bottom_pane.set_context_window_pending(/*pending*/ true);
    bottom_pane
}

#[cfg(test)]
#[path = "startup_draft_tests.rs"]
mod tests;

use std::io::Result;
use std::io::stdout;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering;

use crossterm::cursor::MoveTo;
use crossterm::cursor::Show;
use crossterm::event::KeyCode;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use ratatui::crossterm::execute;
use ratatui::layout::Position;
use ratatui::layout::Rect;

use crate::key_hint;

use super::DisableAlternateScroll;
use super::EnableAlternateScroll;
use super::Terminal;

pub const SUSPEND_KEY: key_hint::KeyBinding = key_hint::ctrl(KeyCode::Char('z'));

/// Coordinates suspend/resume handling so the TUI can restore terminal context after SIGTSTP.
///
/// On suspend, it records which resume path to take (realign inline viewport vs. restore alt
/// screen) and caches the inline cursor row so the cursor can be placed meaningfully before
/// yielding.
///
/// After resume, `prepare_resume_action` consumes the pending intent and returns a
/// `PreparedResumeAction` describing any viewport adjustments to apply inside the synchronized
/// draw.
///
/// Callers keep `suspend_cursor_y` up to date during normal drawing so the suspend step always
/// has the latest cursor position.
///
/// The type is `Clone`, using Arc/atomic internals so bookkeeping can be shared across tasks
/// and moved into the boxed `'static` event stream without borrowing `self`.
#[derive(Clone)]
pub struct SuspendContext {
    /// Resume intent captured at suspend time; cleared once applied after resume.
    resume_pending: Arc<Mutex<Option<ResumeAction>>>,
    /// Inline viewport cursor row used to place the cursor before yielding during suspend.
    suspend_cursor_y: Arc<AtomicU16>,
}

impl SuspendContext {
    /// Creates a new suspend context.
    ///
    /// # Returns
    /// - `SuspendContext`: Initialized suspend context.
    pub(crate) fn new() -> Self {
        Self {
            resume_pending: Arc::new(Mutex::new(None)),
            suspend_cursor_y: Arc::new(AtomicU16::new(0)),
        }
    }

    /// Captures how to resume, stashes cursor position, and yields during SIGTSTP.
    ///
    /// # Arguments
    /// - `alt_screen_active` (&Arc<AtomicBool>): Tracks alt-screen state.
    ///
    /// # Returns
    /// - `Result<()>`: Result of the suspend operation.
    pub(crate) fn suspend(&self, alt_screen_active: &Arc<AtomicBool>) -> Result<()> {
        if alt_screen_active.load(Ordering::Relaxed) {
            let _ = execute!(stdout(), DisableAlternateScroll);
            let _ = execute!(stdout(), LeaveAlternateScreen);
            self.set_resume_action(ResumeAction::RestoreAlt);
        } else {
            self.set_resume_action(ResumeAction::RealignInline);
        }
        let y = self.suspend_cursor_y.load(Ordering::Relaxed);
        let _ = execute!(stdout(), MoveTo(0, y), Show);
        suspend_process()
    }

    /// Consumes the pending resume intent and computes any viewport changes.
    ///
    /// # Arguments
    /// - `terminal` (&mut Terminal): Terminal instance.
    /// - `alt_saved_viewport` (&mut Option<Rect>): Saved alt-screen viewport.
    ///
    /// # Returns
    /// - `Option<PreparedResumeAction>`: Resume action if pending.
    pub(crate) fn prepare_resume_action(
        &self,
        terminal: &mut Terminal,
        alt_saved_viewport: &mut Option<Rect>,
    ) -> Option<PreparedResumeAction> {
        let action = self.take_resume_action()?;
        match action {
            ResumeAction::RealignInline => {
                let cursor_pos = terminal
                    .get_cursor_position()
                    .unwrap_or(terminal.last_known_cursor_pos);
                let viewport = Rect::new(0, cursor_pos.y, 0, 0);
                Some(PreparedResumeAction::RealignViewport(viewport))
            }
            ResumeAction::RestoreAlt => {
                if let Ok(Position { y, .. }) = terminal.get_cursor_position()
                    && let Some(saved) = alt_saved_viewport.as_mut()
                {
                    saved.y = y;
                }
                Some(PreparedResumeAction::RestoreAltScreen)
            }
        }
    }

    /// Sets the cached inline cursor row for suspend handling.
    ///
    /// # Arguments
    /// - `value` (u16): Cursor row to store.
    ///
    /// # Returns
    /// - `()`: No return value.
    pub(crate) fn set_cursor_y(&self, value: u16) {
        self.suspend_cursor_y.store(value, Ordering::Relaxed);
    }

    /// Records a pending resume action to apply after SIGTSTP returns control.
    ///
    /// # Arguments
    /// - `value` (ResumeAction): Pending resume action.
    ///
    /// # Returns
    /// - `()`: No return value.
    fn set_resume_action(&self, value: ResumeAction) {
        *self
            .resume_pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(value);
    }

    /// Takes and clears any pending resume action captured at suspend time.
    ///
    /// # Returns
    /// - `Option<ResumeAction>`: Pending action, if present.
    fn take_resume_action(&self) -> Option<ResumeAction> {
        self.resume_pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }
}

/// Captures what should happen when returning from suspend.
///
/// Either realign the inline viewport to keep the cursor position, or re-enter the alt screen
/// to restore the overlay UI.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResumeAction {
    /// Shift the inline viewport to keep the cursor anchored after resume.
    RealignInline,
    /// Re-enter the alt screen and restore the overlay UI.
    RestoreAlt,
}

/// Describes the viewport change to apply when resuming from suspend during the synchronized draw.
///
/// Either restore the alt screen (with viewport reset) or realign the inline viewport.
#[derive(Clone, Debug)]
pub(crate) enum PreparedResumeAction {
    /// Re-enter the alt screen and reset the viewport to the terminal dimensions.
    RestoreAltScreen,
    /// Apply a viewport shift to keep the inline cursor position stable.
    RealignViewport(Rect),
}

impl PreparedResumeAction {
    /// Applies the prepared resume action to the terminal.
    ///
    /// # Arguments
    /// - `terminal` (&mut Terminal): Terminal instance.
    ///
    /// # Returns
    /// - `Result<()>`: Result of the apply operation.
    pub(crate) fn apply(self, terminal: &mut Terminal) -> Result<()> {
        match self {
            PreparedResumeAction::RealignViewport(area) => {
                terminal.set_viewport_area(area);
            }
            PreparedResumeAction::RestoreAltScreen => {
                if terminal.backend().is_crossterm() {
                    execute!(stdout(), EnterAlternateScreen)?;
                    execute!(stdout(), EnableAlternateScroll)?;
                }
                if let Ok(size) = terminal.size() {
                    terminal.set_viewport_area(Rect::new(0, 0, size.width, size.height));
                    terminal.clear()?;
                }
            }
        }
        Ok(())
    }
}

/// Delivers SIGTSTP after restoring terminal state, then re-applies terminal modes.
///
/// # Returns
/// - `Result<()>`: Result of the suspend operation.
fn suspend_process() -> Result<()> {
    super::restore()?;
    unsafe { libc::kill(0, libc::SIGTSTP) };
    super::set_modes()?;
    Ok(())
}

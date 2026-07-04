use std::io::Result;
use std::io::stdout;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering;

use crossterm::cursor::MoveTo;
use crossterm::cursor::Show;
use crossterm::event::KeyCode;
use ratatui::crossterm::execute;
use ratatui::layout::Rect;

use crate::key_hint;

use super::Terminal;
use super::screen_session::ScreenSession;

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
    pub(crate) fn new() -> Self {
        Self {
            resume_pending: Arc::new(Mutex::new(None)),
            suspend_cursor_y: Arc::new(AtomicU16::new(0)),
        }
    }

    /// Capture how to resume, stash cursor position, and temporarily yield during SIGTSTP.
    ///
    /// - If the alt screen is active, exit alt-scroll/alt-screen and record `RestoreAlt`;
    ///   otherwise record `RealignInline`.
    /// - Update the cached inline cursor row so suspend can place the cursor meaningfully.
    /// - Trigger SIGTSTP so the process can be resumed and continue drawing with the saved state.
    pub(crate) fn suspend(&self, screen_session: &ScreenSession) -> Result<()> {
        let mut first_error = None;
        if screen_session.is_active() {
            let mut terminal_stdout = stdout();
            first_error = screen_session.suspend_to_writer(&mut terminal_stdout).err();
            self.set_resume_action(ResumeAction::RestoreAlt);
        } else {
            self.set_resume_action(ResumeAction::RealignInline);
        }
        let y = self.suspend_cursor_y.load(Ordering::Relaxed);
        let _ = execute!(stdout(), MoveTo(0, y), Show);
        suspend_process()?;
        super::reapply_raw_mode_after_resume()?;

        // The shell writes its job-control status and the resumed command after `fg`, so the
        // cursor may no longer be on the row cached before suspending. The event stream remains
        // paused until this method returns, which makes it safe for the probe to consume both an
        // interleaved focus report and the cursor-position response without racing the background
        // input reader.
        match crate::terminal_probe::cursor_position(crate::terminal_probe::DEFAULT_TIMEOUT) {
            Ok(Some(position)) => self.set_cursor_y(position.y),
            Ok(None) => tracing::debug!("terminal cursor position unavailable after resume"),
            Err(err) => tracing::debug!(
                error = %err,
                "failed to read terminal cursor position after resume"
            ),
        }
        super::flush_terminal_input_buffer();
        tracing::trace!(
            event = "tui_suspend_resumed",
            cursor_y = self.cursor_y(),
            "restored terminal state after resume"
        );
        match first_error {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    /// Consume the pending resume intent and precompute any viewport changes needed post-resume.
    ///
    /// Returns a `PreparedResumeAction` describing how to realign the viewport once drawing
    /// resumes; returns `None` when there was no pending suspend intent.
    pub(crate) fn prepare_resume_action(
        &self,
        screen_session: &ScreenSession,
    ) -> Option<PreparedResumeAction> {
        match self.take_resume_action() {
            None if screen_session.is_suspended() => Some(PreparedResumeAction::RestoreAltScreen),
            None => None,
            Some(ResumeAction::RealignInline) => {
                let viewport = Rect::new(
                    /*x*/ 0,
                    self.cursor_y(),
                    /*width*/ 0,
                    /*height*/ 0,
                );
                Some(PreparedResumeAction::RealignViewport(viewport))
            }
            Some(ResumeAction::RestoreAlt) => {
                screen_session.update_saved_viewport_y(self.cursor_y());
                Some(PreparedResumeAction::RestoreAltScreen)
            }
        }
    }

    /// Set the cached inline cursor row so suspend can place the cursor meaningfully.
    ///
    /// Call during normal drawing when the inline viewport moves so suspend has a fresh cursor
    /// position to restore before yielding.
    pub(crate) fn set_cursor_y(&self, value: u16) {
        self.suspend_cursor_y.store(value, Ordering::Relaxed);
    }

    fn cursor_y(&self) -> u16 {
        self.suspend_cursor_y.load(Ordering::Relaxed)
    }

    /// Record a pending resume action to apply after SIGTSTP returns control.
    fn set_resume_action(&self, value: ResumeAction) {
        *self
            .resume_pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(value);
    }

    /// Take and clear any pending resume action captured at suspend time.
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
    pub(crate) fn apply(
        self,
        terminal: &mut Terminal,
        screen_session: &ScreenSession,
    ) -> Result<()> {
        match self {
            PreparedResumeAction::RealignViewport(area) => {
                terminal.set_viewport_area(area);
            }
            PreparedResumeAction::RestoreAltScreen => {
                screen_session.resume(terminal)?;
            }
        }
        Ok(())
    }
}

/// Deliver SIGTSTP after restoring terminal state, then re-applies terminal modes once resumed.
fn suspend_process() -> Result<()> {
    super::restore()?;
    super::terminal_stderr::pause()?;
    unsafe {
        libc::kill(/*pid*/ 0, libc::SIGTSTP)
    };
    // After the process resumes, reapply terminal modes so drawing can continue.
    super::terminal_stderr::resume()?;
    super::set_modes()?;
    Ok(())
}

//! Restore terminal modes and screen placement across suspend/resume.

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
use ratatui::crossterm::execute;
use ratatui::layout::Rect;
use ratatui::layout::Size;

use crate::key_hint;

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
    pub(crate) fn suspend(&self, alt_screen_active: &Arc<AtomicBool>) -> Result<()> {
        if alt_screen_active.load(Ordering::Relaxed) {
            // Leave alt-screen so the terminal returns to the normal buffer while suspended; also turn off alt-scroll.
            let _ = super::ALTERNATE_SCREEN.leave(&mut stdout());
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

        Ok(())
    }

    /// Consume the pending resume intent and precompute any viewport changes needed post-resume.
    ///
    /// Returns a `PreparedResumeAction` describing how to realign the viewport once drawing
    /// resumes; returns `None` when there was no pending suspend intent.
    pub(crate) fn prepare_resume_action(
        &self,
        alt_saved_viewport: &mut Option<Rect>,
    ) -> Option<PreparedResumeAction> {
        let action = self.take_resume_action()?;
        match action {
            ResumeAction::RealignInline => {
                let viewport = Rect::new(
                    /*x*/ 0,
                    self.cursor_y(),
                    /*width*/ 0,
                    /*height*/ 0,
                );
                Some(PreparedResumeAction::RealignViewport(viewport))
            }
            ResumeAction::RestoreAlt => {
                if let Some(saved) = alt_saved_viewport.as_mut() {
                    saved.y = self.cursor_y();
                }
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
        screen_size: Size,
        owned: bool,
        capture_mouse: bool,
    ) -> Result<()> {
        match self {
            PreparedResumeAction::RealignViewport(area) => {
                terminal.set_viewport_area(area);
            }
            PreparedResumeAction::RestoreAltScreen => {
                super::ALTERNATE_SCREEN.enter(terminal.backend_mut(), capture_mouse)?;
                if owned {
                    terminal.hide_cursor()?;
                }
                terminal.set_viewport_area(Rect::from(screen_size));
                terminal.clear()?;
            }
        }
        Ok(())
    }
}

/// Deliver SIGTSTP after restoring terminal state, then re-applies terminal modes once resumed.
fn suspend_process() -> Result<()> {
    static JOB_RESUMED: AtomicBool = AtomicBool::new(false);
    extern "C" fn record_job_resumed(_signal: libc::c_int) {
        JOB_RESUMED.store(true, Ordering::Release);
    }
    super::restore()?;
    super::terminal_stderr::pause()?;
    // SAFETY: both actions are initialized before they are passed to libc.
    let (mut action, mut previous_action): (libc::sigaction, libc::sigaction) =
        unsafe { std::mem::zeroed() };
    let (mut continue_set, mut previous_mask): (libc::sigset_t, libc::sigset_t) =
        unsafe { std::mem::zeroed() };
    action.sa_sigaction = record_job_resumed as *const () as libc::sighandler_t;
    action.sa_flags = libc::SA_RESTART;
    let action_installed = unsafe {
        libc::sigemptyset(&mut action.sa_mask);
        libc::sigaction(libc::SIGCONT, &action, &mut previous_action) == 0
    };
    let unblock_result = if action_installed {
        unsafe {
            libc::sigemptyset(&mut continue_set);
            libc::sigaddset(&mut continue_set, libc::SIGCONT);
            libc::pthread_sigmask(libc::SIG_UNBLOCK, &continue_set, &mut previous_mask)
        }
    } else {
        0
    };
    let suspend_result = if !action_installed {
        Err(std::io::Error::last_os_error())
    } else if unblock_result != 0 {
        Err(std::io::Error::from_raw_os_error(unblock_result))
    } else if unsafe {
        JOB_RESUMED.store(false, Ordering::Relaxed);
        libc::kill(/*pid*/ 0, libc::SIGTSTP)
    } == 0
    {
        for _ in 0..1000 {
            if JOB_RESUMED.load(Ordering::Acquire) {
                break;
            }
            std::thread::park_timeout(std::time::Duration::from_millis(1));
        }
        if JOB_RESUMED.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(std::io::ErrorKind::TimedOut.into())
        }
    } else {
        Err(std::io::Error::last_os_error())
    };
    let restore_mask_result = if action_installed && unblock_result == 0 {
        let result = unsafe {
            libc::pthread_sigmask(libc::SIG_SETMASK, &previous_mask, std::ptr::null_mut())
        };
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::from_raw_os_error(result))
        }
    } else {
        Ok(())
    };
    // SAFETY: previous_action was initialized by the successful sigaction call above.
    let restore_result = if !action_installed
        || unsafe { libc::sigaction(libc::SIGCONT, &previous_action, std::ptr::null_mut()) } == 0
    {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    };
    suspend_result
        .and(restore_mask_result)
        .and(restore_result)
        .and(super::terminal_stderr::resume())
        .and(super::set_modes())
}

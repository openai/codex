//! Alternate-screen ownership and physical terminal transitions.

use std::io;
use std::io::Write;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use crossterm::event::DisableMouseCapture;
use crossterm::event::EnableMouseCapture;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use ratatui::crossterm::execute;
use ratatui::layout::Rect;

use super::DisableAlternateScroll;
use super::EnableAlternateScroll;
use super::Terminal;

const INITIAL_OWNER_COUNT: usize = 1;
static PHYSICAL_ALT_SCREEN: PhysicalAltScreenTracker = PhysicalAltScreenTracker::new();

struct PhysicalAltScreenTracker {
    active: AtomicBool,
    alternate_scroll_active: AtomicBool,
    mouse_capture_active: AtomicBool,
}

impl PhysicalAltScreenTracker {
    const fn new() -> Self {
        Self {
            active: AtomicBool::new(/*v*/ false),
            alternate_scroll_active: AtomicBool::new(/*v*/ false),
            mouse_capture_active: AtomicBool::new(/*v*/ false),
        }
    }

    fn mark_entered(&self) {
        self.active.store(/*val*/ true, Ordering::Release);
    }

    fn mark_left(&self) {
        self.active.store(/*val*/ false, Ordering::Release);
    }

    fn mark_alternate_scroll_enabled(&self) {
        self.alternate_scroll_active
            .store(/*val*/ true, Ordering::Release);
    }

    fn mark_alternate_scroll_disabled(&self) {
        self.alternate_scroll_active
            .store(/*val*/ false, Ordering::Release);
    }

    fn mark_mouse_capture_enabled(&self) {
        self.mouse_capture_active
            .store(/*val*/ true, Ordering::Release);
    }

    fn mark_mouse_capture_disabled(&self) {
        self.mouse_capture_active
            .store(/*val*/ false, Ordering::Release);
    }

    fn restore(&self, writer: &mut impl Write) -> io::Result<()> {
        let mouse_result = if self.mouse_capture_active.load(Ordering::Acquire) {
            let result = execute!(writer, DisableMouseCapture);
            if result.is_ok() {
                self.mark_mouse_capture_disabled();
            }
            result
        } else {
            Ok(())
        };
        let scroll_result = if self.alternate_scroll_active.load(Ordering::Acquire) {
            let result = execute!(writer, DisableAlternateScroll);
            if result.is_ok() {
                self.mark_alternate_scroll_disabled();
            }
            result
        } else {
            Ok(())
        };
        let leave_result = if self.active.load(Ordering::Acquire) {
            execute!(writer, LeaveAlternateScreen)
        } else {
            Ok(())
        };
        if leave_result.is_ok() {
            self.mark_left();
        }
        merge_results(mouse_result, merge_results(scroll_result, leave_result))
    }
}

#[derive(Clone)]
pub(super) struct ScreenSession {
    inner: Arc<Mutex<ScreenSessionInner>>,
}

struct ScreenSessionInner {
    availability: AltScreenAvailability,
    ownership: ScreenOwnership,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AltScreenAvailability {
    Enabled,
    Disabled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScreenOwnership {
    Inline,
    Alternate {
        owners: NonZeroUsize,
        saved_viewport: Rect,
        physical: PhysicalAltScreen,
        input_mode: AltScreenInputMode,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PhysicalAltScreen {
    Active,
    Suspended,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AltScreenInputMode {
    AlternateScroll,
    MouseCapture,
}

impl AltScreenInputMode {
    fn enable(self, commands: &mut impl ScreenCommands) -> io::Result<()> {
        match self {
            Self::AlternateScroll => commands.enable_alternate_scroll(),
            Self::MouseCapture => commands.enable_mouse_capture(),
        }
    }

    fn disable(self, commands: &mut impl ScreenCommands) -> io::Result<()> {
        match self {
            Self::AlternateScroll => commands.disable_alternate_scroll(),
            Self::MouseCapture => commands.disable_mouse_capture(),
        }
    }
}

impl ScreenSession {
    pub(super) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ScreenSessionInner {
                availability: AltScreenAvailability::Enabled,
                ownership: ScreenOwnership::Inline,
            })),
        }
    }

    pub(super) fn set_enabled(&self, enabled: bool) {
        self.lock().availability = match enabled {
            true => AltScreenAvailability::Enabled,
            false => AltScreenAvailability::Disabled,
        };
    }

    pub(super) fn is_active(&self) -> bool {
        matches!(self.lock().ownership, ScreenOwnership::Alternate { .. })
    }

    pub(super) fn is_suspended(&self) -> bool {
        matches!(
            self.lock().ownership,
            ScreenOwnership::Alternate {
                physical: PhysicalAltScreen::Suspended,
                ..
            }
        )
    }

    pub(super) fn saved_viewport(&self) -> Option<Rect> {
        match self.lock().ownership {
            ScreenOwnership::Inline => None,
            ScreenOwnership::Alternate { saved_viewport, .. } => Some(saved_viewport),
        }
    }

    pub(super) fn update_saved_viewport_y(&self, y: u16) {
        if let ScreenOwnership::Alternate { saved_viewport, .. } = &mut self.lock().ownership {
            saved_viewport.y = y;
        }
    }

    pub(super) fn enter(&self, terminal: &mut Terminal) -> io::Result<()> {
        self.enter_with_input_mode(terminal, AltScreenInputMode::AlternateScroll)
    }

    pub(super) fn enter_owned(&self, terminal: &mut Terminal) -> io::Result<()> {
        self.enter_with_input_mode(terminal, AltScreenInputMode::MouseCapture)
    }

    fn enter_with_input_mode(
        &self,
        terminal: &mut Terminal,
        input_mode: AltScreenInputMode,
    ) -> io::Result<()> {
        let was_active = self.is_active();
        let saved_viewport = terminal.viewport_area;
        let command_result = self.acquire(terminal, saved_viewport, input_mode);
        let layout_result = if !was_active && self.is_active() {
            expand_to_full_screen(terminal)
        } else {
            Ok(())
        };
        merge_results(command_result, layout_result)
    }

    pub(super) fn leave(&self, terminal: &mut Terminal) -> io::Result<()> {
        let saved_viewport = self.saved_viewport();
        let was_active = saved_viewport.is_some();
        let result = self.release(terminal);
        if was_active
            && !self.is_active()
            && let Some(saved_viewport) = saved_viewport
        {
            terminal.set_viewport_area(saved_viewport);
        }
        result
    }

    pub(super) fn finish(&self, terminal: &mut Terminal) -> io::Result<()> {
        let saved_viewport = self.saved_viewport();
        let result = self.finish_commands(terminal);
        if !self.is_active()
            && let Some(saved_viewport) = saved_viewport
        {
            terminal.set_viewport_area(saved_viewport);
        }
        result
    }

    pub(super) fn suspend(&self, terminal: &mut Terminal) -> io::Result<()> {
        let was_suspended = self.is_suspended();
        let result = self.suspend_commands(terminal);
        if !was_suspended
            && self.is_suspended()
            && let Some(saved_viewport) = self.saved_viewport()
        {
            terminal.set_viewport_area(saved_viewport);
        }
        result
    }

    pub(super) fn suspend_to_writer(&self, writer: &mut impl Write) -> io::Result<()> {
        self.suspend_commands(&mut WriterCommands(writer))
    }

    pub(super) fn resume(&self, terminal: &mut Terminal) -> io::Result<()> {
        let was_suspended = self.is_suspended();
        let command_result = self.resume_commands(terminal);
        let layout_result = if was_suspended && !self.is_suspended() {
            expand_to_full_screen(terminal)
        } else {
            Ok(())
        };
        merge_results(command_result, layout_result)
    }

    fn acquire(
        &self,
        commands: &mut impl ScreenCommands,
        saved_viewport: Rect,
        input_mode: AltScreenInputMode,
    ) -> io::Result<()> {
        let mut inner = self.lock();
        if inner.availability == AltScreenAvailability::Disabled {
            return Ok(());
        }
        match &mut inner.ownership {
            ScreenOwnership::Inline => {
                commands.enter_alternate_screen()?;
                inner.ownership = ScreenOwnership::Alternate {
                    owners: NonZeroUsize::MIN,
                    saved_viewport,
                    physical: PhysicalAltScreen::Active,
                    input_mode,
                };
                if let Err(enable_error) = input_mode.enable(commands) {
                    let disable_result = input_mode.disable(commands);
                    let leave_result = commands.leave_alternate_screen();
                    if leave_result.is_ok() {
                        inner.ownership = ScreenOwnership::Inline;
                    }
                    return merge_results(
                        Err(enable_error),
                        merge_results(disable_result, leave_result),
                    );
                }
                Ok(())
            }
            ScreenOwnership::Alternate { owners, .. } => {
                let next = owners
                    .get()
                    .checked_add(INITIAL_OWNER_COUNT)
                    .ok_or_else(|| io::Error::other("alternate-screen owner count overflow"))?;
                *owners = NonZeroUsize::new(next)
                    .ok_or_else(|| io::Error::other("alternate-screen owner count became zero"))?;
                Ok(())
            }
        }
    }

    fn release(&self, commands: &mut impl ScreenCommands) -> io::Result<()> {
        let mut inner = self.lock();
        let ScreenOwnership::Alternate {
            owners,
            physical,
            input_mode,
            ..
        } = &mut inner.ownership
        else {
            return Ok(());
        };
        if owners.get() > INITIAL_OWNER_COUNT {
            *owners = NonZeroUsize::new(owners.get() - INITIAL_OWNER_COUNT).ok_or_else(|| {
                io::Error::other("nested alternate-screen owner count became zero")
            })?;
            return Ok(());
        }
        if *physical == PhysicalAltScreen::Suspended {
            inner.ownership = ScreenOwnership::Inline;
            return Ok(());
        }

        let disable_result = input_mode.disable(commands);
        let leave_result = commands.leave_alternate_screen();
        if leave_result.is_ok() {
            inner.ownership = ScreenOwnership::Inline;
        }
        merge_results(disable_result, leave_result)
    }

    fn finish_commands(&self, commands: &mut impl ScreenCommands) -> io::Result<()> {
        let mut inner = self.lock();
        let ScreenOwnership::Alternate {
            physical,
            input_mode,
            ..
        } = inner.ownership
        else {
            return Ok(());
        };
        if physical == PhysicalAltScreen::Suspended {
            inner.ownership = ScreenOwnership::Inline;
            return Ok(());
        }

        let disable_result = input_mode.disable(commands);
        let leave_result = commands.leave_alternate_screen();
        if leave_result.is_ok() {
            inner.ownership = ScreenOwnership::Inline;
        }
        merge_results(disable_result, leave_result)
    }

    fn suspend_commands(&self, commands: &mut impl ScreenCommands) -> io::Result<()> {
        let mut inner = self.lock();
        let ScreenOwnership::Alternate {
            physical,
            input_mode,
            ..
        } = &mut inner.ownership
        else {
            return Ok(());
        };
        if *physical == PhysicalAltScreen::Suspended {
            return Ok(());
        }
        let disable_result = input_mode.disable(commands);
        let leave_result = commands.leave_alternate_screen();
        if leave_result.is_ok() {
            *physical = PhysicalAltScreen::Suspended;
        }
        merge_results(disable_result, leave_result)
    }

    fn resume_commands(&self, commands: &mut impl ScreenCommands) -> io::Result<()> {
        let mut inner = self.lock();
        let ScreenOwnership::Alternate {
            physical,
            input_mode,
            ..
        } = &mut inner.ownership
        else {
            return Ok(());
        };
        if *physical == PhysicalAltScreen::Active {
            return Ok(());
        }
        commands.enter_alternate_screen()?;
        *physical = PhysicalAltScreen::Active;
        if let Err(enable_error) = input_mode.enable(commands) {
            let disable_result = input_mode.disable(commands);
            let leave_result = commands.leave_alternate_screen();
            if leave_result.is_ok() {
                *physical = PhysicalAltScreen::Suspended;
            }
            return merge_results(
                Err(enable_error),
                merge_results(disable_result, leave_result),
            );
        }
        Ok(())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ScreenSessionInner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Command sink shared by terminal-backed transitions, job-control writes, and state tests.
trait ScreenCommands {
    fn enter_alternate_screen(&mut self) -> io::Result<()>;
    fn leave_alternate_screen(&mut self) -> io::Result<()>;
    fn enable_alternate_scroll(&mut self) -> io::Result<()>;
    fn disable_alternate_scroll(&mut self) -> io::Result<()>;
    fn enable_mouse_capture(&mut self) -> io::Result<()>;
    fn disable_mouse_capture(&mut self) -> io::Result<()>;
}

pub(super) fn restore_physical_alt_screen(writer: &mut impl Write) -> io::Result<()> {
    PHYSICAL_ALT_SCREEN.restore(writer)
}

impl ScreenCommands for Terminal {
    fn enter_alternate_screen(&mut self) -> io::Result<()> {
        let result = execute!(self.backend_mut(), EnterAlternateScreen);
        if result.is_ok() {
            PHYSICAL_ALT_SCREEN.mark_entered();
        }
        result
    }

    fn leave_alternate_screen(&mut self) -> io::Result<()> {
        let result = execute!(self.backend_mut(), LeaveAlternateScreen);
        if result.is_ok() {
            PHYSICAL_ALT_SCREEN.mark_left();
        }
        result
    }

    fn enable_alternate_scroll(&mut self) -> io::Result<()> {
        let result = execute!(self.backend_mut(), EnableAlternateScroll);
        if result.is_ok() {
            PHYSICAL_ALT_SCREEN.mark_alternate_scroll_enabled();
        }
        result
    }

    fn disable_alternate_scroll(&mut self) -> io::Result<()> {
        let result = execute!(self.backend_mut(), DisableAlternateScroll);
        if result.is_ok() {
            PHYSICAL_ALT_SCREEN.mark_alternate_scroll_disabled();
        }
        result
    }

    fn enable_mouse_capture(&mut self) -> io::Result<()> {
        let result = execute!(self.backend_mut(), EnableMouseCapture);
        if result.is_ok() {
            PHYSICAL_ALT_SCREEN.mark_mouse_capture_enabled();
        }
        result
    }

    fn disable_mouse_capture(&mut self) -> io::Result<()> {
        let result = execute!(self.backend_mut(), DisableMouseCapture);
        if result.is_ok() {
            PHYSICAL_ALT_SCREEN.mark_mouse_capture_disabled();
        }
        result
    }
}

struct WriterCommands<'a, W>(&'a mut W);

impl<W: Write> ScreenCommands for WriterCommands<'_, W> {
    fn enter_alternate_screen(&mut self) -> io::Result<()> {
        let result = execute!(self.0, EnterAlternateScreen);
        if result.is_ok() {
            PHYSICAL_ALT_SCREEN.mark_entered();
        }
        result
    }

    fn leave_alternate_screen(&mut self) -> io::Result<()> {
        let result = execute!(self.0, LeaveAlternateScreen);
        if result.is_ok() {
            PHYSICAL_ALT_SCREEN.mark_left();
        }
        result
    }

    fn enable_alternate_scroll(&mut self) -> io::Result<()> {
        let result = execute!(self.0, EnableAlternateScroll);
        if result.is_ok() {
            PHYSICAL_ALT_SCREEN.mark_alternate_scroll_enabled();
        }
        result
    }

    fn disable_alternate_scroll(&mut self) -> io::Result<()> {
        let result = execute!(self.0, DisableAlternateScroll);
        if result.is_ok() {
            PHYSICAL_ALT_SCREEN.mark_alternate_scroll_disabled();
        }
        result
    }

    fn enable_mouse_capture(&mut self) -> io::Result<()> {
        let result = execute!(self.0, EnableMouseCapture);
        if result.is_ok() {
            PHYSICAL_ALT_SCREEN.mark_mouse_capture_enabled();
        }
        result
    }

    fn disable_mouse_capture(&mut self) -> io::Result<()> {
        let result = execute!(self.0, DisableMouseCapture);
        if result.is_ok() {
            PHYSICAL_ALT_SCREEN.mark_mouse_capture_disabled();
        }
        result
    }
}

fn expand_to_full_screen(terminal: &mut Terminal) -> io::Result<()> {
    let size = terminal.size()?;
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        size.width,
        size.height,
    ));
    terminal.clear()
}

fn merge_results(first: io::Result<()>, second: io::Result<()>) -> io::Result<()> {
    match (first, second) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(err), Ok(())) | (Ok(()), Err(err)) => Err(err),
        (Err(first), Err(second)) => Err(io::Error::new(
            first.kind(),
            format!("{first}; additionally: {second}"),
        )),
    }
}

#[cfg(test)]
#[path = "screen_session_tests.rs"]
mod tests;

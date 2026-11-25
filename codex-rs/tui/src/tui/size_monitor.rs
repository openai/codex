//! Recovers missed tmux resizes and mouse setting changes off the UI thread.
//!
//! One worker belongs to the event broker, including across nested screens. Queries never
//! hold the shared lock. Pauses and newer geometry invalidate both in-flight and queued
//! samples. Dropping the monitor wakes the worker without joining a possibly blocked OS call;
//! that single in-flight call may finish later, but cannot publish or start another query.

use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use super::alternate_screen::ALTERNATE_SCREEN;
use crate::terminal_capabilities;

use ratatui::layout::Size;
use tokio::sync::broadcast;

const CHECK_INTERVAL: Duration = Duration::from_millis(500);
// tmux does not notify the pane when "set mouse" changes. Unlike the geometry
// ioctl, this check starts a subprocess, so sample less often and only in fullscreen.
const MOUSE_CHECK_INTERVAL: Duration = Duration::from_secs(2);

struct State {
    size: Size,
    generation: u64,
    pending: Option<Size>,
    mouse: Option<terminal_capabilities::MouseCapture>,
    active: bool,
    stopped: bool,
}

pub(super) struct SizeMonitor {
    state: Arc<Mutex<State>>,
    worker: thread::JoinHandle<()>,
}

impl SizeMonitor {
    pub(super) fn start(
        poll_session: bool,
        size: Size,
        draw_tx: broadcast::Sender<()>,
    ) -> Option<Self> {
        if !poll_session {
            return None;
        }
        let mut last_mouse_check = Instant::now();
        Self::spawn(size, draw_tx, move |previous_mouse| {
            let mouse = if ALTERNATE_SCREEN.is_active()
                && last_mouse_check.elapsed() >= MOUSE_CHECK_INTERVAL
            {
                last_mouse_check = Instant::now();
                terminal_capabilities::sample_mouse(
                    previous_mouse.unwrap_or_else(|| ALTERNATE_SCREEN.mouse_policy()),
                )
            } else {
                None
            };
            // Unlike terminal::size(), window_size() has no tput subprocess fallback.
            let size =
                crossterm::terminal::window_size().map(|size| Size::new(size.columns, size.rows));
            (size, mouse)
        })
        .inspect_err(|error| tracing::warn!(%error, "Failed to start terminal size monitor"))
        .ok()
    }

    fn spawn(
        size: Size,
        draw_tx: broadcast::Sender<()>,
        mut query: impl FnMut(
            Option<terminal_capabilities::MouseCapture>,
        ) -> (
            io::Result<Size>,
            Option<terminal_capabilities::MouseCapture>,
        ) + Send
        + 'static,
    ) -> io::Result<Self> {
        let state = Arc::new(Mutex::new(State {
            size,
            generation: 0,
            pending: None,
            mouse: None,
            active: true,
            stopped: false,
        }));
        let shared = state.clone();
        let worker = thread::Builder::new()
            .name("codex-terminal-size".into())
            .spawn(move || {
                loop {
                    thread::park_timeout(CHECK_INTERVAL);
                    let (generation, previous_mouse) = {
                        let state = shared
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        if state.stopped {
                            break;
                        }
                        if !state.active {
                            continue;
                        }
                        (state.generation, state.mouse)
                    };
                    let (sampled, mouse) = query(previous_mouse);
                    let mut state = shared
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if state.stopped {
                        break;
                    }
                    if state.generation != generation {
                        continue;
                    }
                    state.pending = sampled
                        .ok()
                        .filter(|size| size.width > 0 && size.height > 0 && *size != state.size);
                    let mouse_changed = mouse.is_some() && mouse != state.mouse;
                    if mouse_changed {
                        state.mouse = mouse;
                    }
                    if state.pending.is_some() || mouse_changed {
                        drop(state);
                        let _ = draw_tx.send(());
                    }
                }
            })?;
        Ok(Self { state, worker })
    }

    pub(super) fn observe(&self, size: Size) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.size = size;
        state.generation = state.generation.wrapping_add(1);
        state.pending = None;
    }

    pub(super) fn set_active(&self, active: bool) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.active = active;
        state.generation = state.generation.wrapping_add(1);
        state.pending = None;
        state.mouse = None;
    }

    // Screen entry probes tmux synchronously. Discard older samples, including
    // any query still in flight when that probe finished.
    pub(super) fn reset_mouse(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.generation = state.generation.wrapping_add(1);
        state.mouse = None;
    }

    pub(super) fn mouse(&self) -> Option<terminal_capabilities::MouseCapture> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .mouse
    }

    pub(super) fn take_resize(&self) -> Option<Size> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let size = state.pending.take()?;
        state.size = size;
        state.generation = state.generation.wrapping_add(1);
        tracing::debug!(?size, "Recovered terminal size from background check");
        Some(size)
    }
}

impl Drop for SizeMonitor {
    fn drop(&mut self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stopped = true;
        self.worker.thread().unpark();
    }
}

#[cfg(test)]
#[path = "size_monitor_tests.rs"]
mod tests;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;

use tokio::runtime::Handle;
use tokio::time::Duration;
use tokio::time::sleep;

use crate::app_event::AppEvent;

pub(crate) struct ScrollEventHelper {
    app_event_tx: Sender<AppEvent>,
    scroll_delta: Arc<AtomicI32>,
    timer_scheduled: Arc<AtomicBool>,
    runtime: Handle,
}

/// How long to wait after the first scroll event before sending the
/// accumulated scroll delta to the main thread.
const DEBOUNCE_WINDOW: Duration = Duration::from_millis(100);

/// Utility to debounce scroll events so we can determine the **magnitude** of
/// each scroll burst by accumulating individual wheel events over a short
/// window.  The debounce timer now runs on Tokio so we avoid spinning up a new
/// operating-system thread for every burst.
impl ScrollEventHelper {
    pub(crate) fn new(app_event_tx: Sender<AppEvent>) -> Self {
        Self {
            app_event_tx,
            scroll_delta: Arc::new(AtomicI32::new(0)),
            timer_scheduled: Arc::new(AtomicBool::new(false)),
            runtime: Handle::current(),
        }
    }

    pub(crate) fn scroll_up(&self) {
        self.scroll_delta.fetch_sub(1, Ordering::Relaxed);
        self.schedule_notification();
    }

    pub(crate) fn scroll_down(&self) {
        self.scroll_delta.fetch_add(1, Ordering::Relaxed);
        self.schedule_notification();
    }

    /// Starts a one-shot timer **only once** per burst of wheel events.
    fn schedule_notification(&self) {
        // If the timer is already scheduled, do nothing.
        if self
            .timer_scheduled
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        // Otherwise, schedule a new timer.
        let tx = self.app_event_tx.clone();
        let delta = Arc::clone(&self.scroll_delta);
        let timer_flag = Arc::clone(&self.timer_scheduled);

        // Use self.runtime instead of tokio::spawn() because the calling thread
        // in app.rs is not part of the Tokio runtime: it is a plain OS thread.
        self.runtime.spawn(async move {
            sleep(DEBOUNCE_WINDOW).await;

            let accumulated = delta.swap(0, Ordering::SeqCst);
            if accumulated != 0 {
                let _ = tx.send(AppEvent::Scroll(accumulated));
            }

            timer_flag.store(false, Ordering::SeqCst);
        });
    }
}

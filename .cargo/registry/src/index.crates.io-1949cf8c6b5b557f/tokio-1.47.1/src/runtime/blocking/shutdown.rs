//! A shutdown channel.
//!
//! Each worker holds the `Sender` half. When all the `Sender` halves are
//! dropped, the `Receiver` receives a notification.

use crate::loom::sync::Arc;
use crate::sync::oneshot;

use std::time::Duration;

#[derive(Debug, Clone)]
pub(super) struct Sender {
    _tx: Arc<oneshot::Sender<()>>,
}

#[derive(Debug)]
pub(super) struct Receiver {
    rx: oneshot::Receiver<()>,
}

pub(super) fn channel() -> (Sender, Receiver) {
    let (tx, rx) = oneshot::channel();
    let tx = Sender { _tx: Arc::new(tx) };
    let rx = Receiver { rx };

    (tx, rx)
}

impl Receiver {
    /// Blocks the current thread until all `Sender` handles drop.
    ///
    /// If `timeout` is `Some`, the thread is blocked for **at most** `timeout`
    /// duration. If `timeout` is `None`, then the thread is blocked until the
    /// shutdown signal is received.
    ///
    /// If the timeout has elapsed, it returns `false`, otherwise it returns `true`.
    pub(crate) fn wait(&mut self, timeout: Option<Duration>) -> bool {
        use crate::runtime::context::try_enter_blocking_region;

        if timeout == Some(Duration::from_nanos(0)) {
            return false;
        }

        let mut e = match try_enter_blocking_region() {
            Some(enter) => enter,
            _ => {
                if std::thread::panicking() {
                    // Don't panic in a panic
                    return false;
                } else {
                    panic!(
                        "Cannot drop a runtime in a context where blocking is not allowed. \
                        This happens when a runtime is dropped from within an asynchronous context."
                    );
                }
            }
        };

        // The oneshot completes with an Err
        //
        // If blocking fails to wait, this indicates a problem parking the
        // current thread (usually, shutting down a runtime stored in a
        // thread-local).
        if let Some(timeout) = timeout {
            e.block_on_timeout(&mut self.rx, timeout).is_ok()
        } else {
            let _ = e.block_on(&mut self.rx);
            true
        }
    }
}

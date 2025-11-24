use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::ratelimit::{RateLimiter, RateLimitingCategory};
use crate::{sentry_debug, Envelope};

enum Task {
    SendEnvelope(Envelope),
    Flush(SyncSender<()>),
    Shutdown,
}

pub struct TransportThread {
    sender: SyncSender<Task>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl TransportThread {
    pub fn new<SendFn>(mut send: SendFn) -> Self
    where
        SendFn: FnMut(Envelope, &mut RateLimiter) + Send + 'static,
    {
        let (sender, receiver) = sync_channel(30);
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_worker = shutdown.clone();
        let handle = thread::Builder::new()
            .name("sentry-transport".into())
            .spawn(move || {
                let mut rl = RateLimiter::new();

                for task in receiver.into_iter() {
                    if shutdown_worker.load(Ordering::SeqCst) {
                        return;
                    }
                    let envelope = match task {
                        Task::SendEnvelope(envelope) => envelope,
                        Task::Flush(sender) => {
                            sender.send(()).ok();
                            continue;
                        }
                        Task::Shutdown => {
                            return;
                        }
                    };

                    if let Some(time_left) = rl.is_disabled(RateLimitingCategory::Any) {
                        sentry_debug!(
                            "Skipping event send because we're disabled due to rate limits for {}s",
                            time_left.as_secs()
                        );
                        continue;
                    }
                    match rl.filter_envelope(envelope) {
                        Some(envelope) => {
                            send(envelope, &mut rl);
                        }
                        None => {
                            sentry_debug!("Envelope was discarded due to per-item rate limits");
                        }
                    };
                }
            })
            .ok();

        Self {
            sender,
            shutdown,
            handle,
        }
    }

    pub fn send(&self, envelope: Envelope) {
        let _ = self.sender.send(Task::SendEnvelope(envelope));
    }

    pub fn flush(&self, timeout: Duration) -> bool {
        let (sender, receiver) = sync_channel(1);
        let _ = self.sender.send(Task::Flush(sender));
        receiver.recv_timeout(timeout).is_err()
    }
}

impl Drop for TransportThread {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = self.sender.send(Task::Shutdown);
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}

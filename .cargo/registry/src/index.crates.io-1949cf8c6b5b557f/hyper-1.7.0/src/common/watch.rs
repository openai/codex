//! An SPSC broadcast channel.
//!
//! - The value can only be a `usize`.
//! - The consumer is only notified if the value is different.
//! - The value `0` is reserved for closed.

use atomic_waker::AtomicWaker;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::task;

type Value = usize;

pub(crate) const CLOSED: usize = 0;

pub(crate) fn channel(initial: Value) -> (Sender, Receiver) {
    debug_assert!(
        initial != CLOSED,
        "watch::channel initial state of 0 is reserved"
    );

    let shared = Arc::new(Shared {
        value: AtomicUsize::new(initial),
        waker: AtomicWaker::new(),
    });

    (
        Sender {
            shared: shared.clone(),
        },
        Receiver { shared },
    )
}

pub(crate) struct Sender {
    shared: Arc<Shared>,
}

pub(crate) struct Receiver {
    shared: Arc<Shared>,
}

struct Shared {
    value: AtomicUsize,
    waker: AtomicWaker,
}

impl Sender {
    pub(crate) fn send(&mut self, value: Value) {
        if self.shared.value.swap(value, Ordering::SeqCst) != value {
            self.shared.waker.wake();
        }
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        self.send(CLOSED);
    }
}

impl Receiver {
    pub(crate) fn load(&mut self, cx: &mut task::Context<'_>) -> Value {
        self.shared.waker.register(cx.waker());
        self.shared.value.load(Ordering::SeqCst)
    }

    pub(crate) fn peek(&self) -> Value {
        self.shared.value.load(Ordering::Relaxed)
    }
}

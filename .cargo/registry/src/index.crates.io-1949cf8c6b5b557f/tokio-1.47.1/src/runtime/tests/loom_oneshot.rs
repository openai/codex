use crate::loom::sync::{Arc, Mutex};
use loom::sync::Notify;

pub(crate) fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let inner = Arc::new(Inner {
        notify: Notify::new(),
        value: Mutex::new(None),
    });

    let tx = Sender {
        inner: inner.clone(),
    };
    let rx = Receiver { inner };

    (tx, rx)
}

pub(crate) struct Sender<T> {
    inner: Arc<Inner<T>>,
}

pub(crate) struct Receiver<T> {
    inner: Arc<Inner<T>>,
}

struct Inner<T> {
    notify: Notify,
    value: Mutex<Option<T>>,
}

impl<T> Sender<T> {
    pub(crate) fn send(self, value: T) {
        *self.inner.value.lock() = Some(value);
        self.inner.notify.notify();
    }
}

impl<T> Receiver<T> {
    pub(crate) fn recv(self) -> T {
        loop {
            if let Some(v) = self.inner.value.lock().take() {
                return v;
            }

            self.inner.notify.wait();
        }
    }
}

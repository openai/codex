use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::PoisonError;

use codex_app_server_protocol::RequestId;

trait Cancellable: Send {
    fn cancel(&self);
}

impl<F> Cancellable for F
where
    F: Fn() + Send,
{
    fn cancel(&self) {
        (self)();
    }
}

#[derive(Clone, Default)]
pub(crate) struct CancellationRegistry {
    inner: Arc<StdMutex<HashMap<RequestId, Box<dyn Cancellable + Send>>>>,
}

impl CancellationRegistry {
    pub(crate) fn insert<F>(&self, id: RequestId, f: F)
    where
        F: Fn() + Send + 'static,
    {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id, Box::new(f));
    }

    pub(crate) fn cancel(&self, id: &RequestId) -> bool {
        // Remove the callback while holding the lock, but invoke it only after
        // releasing the lock to avoid deadlocks or long critical sections.
        let callback = {
            let mut guard = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
            guard.remove(id)
        };
        if let Some(c) = callback {
            c.cancel();
            true
        } else {
            false
        }
    }

    pub(crate) fn remove(&self, id: &RequestId) {
        let mut guard = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        guard.remove(id);
    }
}

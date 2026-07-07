use super::*;
use std::pin::Pin;
use std::sync::Mutex as StdMutex;

type ThreadTeardownFuture = Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>>;

#[derive(Clone, Default)]
pub(crate) struct PendingThreadUnloads {
    inner: Arc<PendingThreadUnloadsInner>,
}

#[derive(Default)]
struct PendingThreadUnloadsInner {
    registry: StdMutex<PendingThreadUnloadRegistry>,
    tasks: TaskTracker,
}

#[derive(Default)]
struct PendingThreadUnloadRegistry {
    closing: bool,
    entries: HashMap<ThreadId, watch::Sender<bool>>,
}

pub(super) enum PendingThreadUnloadClaimResult {
    Claimed(PendingThreadUnloadClaim),
    Pending(watch::Receiver<bool>),
    Closing,
}

pub(super) enum PendingThreadUnloadStartResult {
    Started(watch::Receiver<bool>),
    Pending(watch::Receiver<bool>),
    Closing,
}

pub(super) struct PendingThreadUnloadClaim {
    start_tx: Option<oneshot::Sender<ThreadTeardownFuture>>,
    completed: watch::Receiver<bool>,
}

impl PendingThreadUnloadClaim {
    pub(super) fn start<F>(mut self, teardown: F) -> watch::Receiver<bool>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        if let Some(start_tx) = self.start_tx.take() {
            let _ = start_tx.send(Box::pin(teardown));
        }
        self.completed.clone()
    }
}

struct PendingThreadUnloadOwner {
    pending: PendingThreadUnloads,
    thread_id: ThreadId,
    completed: watch::Sender<bool>,
}

impl Drop for PendingThreadUnloadOwner {
    fn drop(&mut self) {
        self.pending.release(self.thread_id, &self.completed);
    }
}

impl PendingThreadUnloads {
    fn lock_registry(&self) -> std::sync::MutexGuard<'_, PendingThreadUnloadRegistry> {
        self.inner
            .registry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(super) fn contains(&self, thread_id: ThreadId) -> bool {
        self.lock_registry().entries.contains_key(&thread_id)
    }

    pub(super) fn subscribe(&self, thread_id: ThreadId) -> Option<watch::Receiver<bool>> {
        self.lock_registry()
            .entries
            .get(&thread_id)
            .map(watch::Sender::subscribe)
    }

    pub(super) fn try_claim(&self, thread_id: ThreadId) -> PendingThreadUnloadClaimResult {
        let mut registry = self.lock_registry();
        if registry.closing {
            return PendingThreadUnloadClaimResult::Closing;
        }
        if let Some(completed) = registry.entries.get(&thread_id) {
            return PendingThreadUnloadClaimResult::Pending(completed.subscribe());
        }

        let (completed, completed_rx) = watch::channel(false);
        registry.entries.insert(thread_id, completed.clone());
        let owner = PendingThreadUnloadOwner {
            pending: self.clone(),
            thread_id,
            completed,
        };
        let (start_tx, start_rx) = oneshot::channel::<ThreadTeardownFuture>();
        let task = self.inner.tasks.spawn(async move {
            if let Ok(teardown) = start_rx.await {
                teardown.await;
            }
            drop(owner);
        });
        drop(task);
        PendingThreadUnloadClaimResult::Claimed(PendingThreadUnloadClaim {
            start_tx: Some(start_tx),
            completed: completed_rx,
        })
    }

    pub(super) fn try_start<F>(
        &self,
        thread_id: ThreadId,
        teardown: F,
    ) -> PendingThreadUnloadStartResult
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        match self.try_claim(thread_id) {
            PendingThreadUnloadClaimResult::Claimed(claim) => {
                PendingThreadUnloadStartResult::Started(claim.start(teardown))
            }
            PendingThreadUnloadClaimResult::Pending(completed) => {
                PendingThreadUnloadStartResult::Pending(completed)
            }
            PendingThreadUnloadClaimResult::Closing => PendingThreadUnloadStartResult::Closing,
        }
    }

    fn release(&self, thread_id: ThreadId, completed: &watch::Sender<bool>) {
        let mut registry = self.lock_registry();
        if registry
            .entries
            .get(&thread_id)
            .is_some_and(|current| current.same_channel(completed))
        {
            registry.entries.remove(&thread_id);
        }
        drop(registry);
        let _ = completed.send(true);
    }

    pub(super) async fn close_and_wait(&self) {
        {
            let mut registry = self.lock_registry();
            registry.closing = true;
            self.inner.tasks.close();
        }
        self.inner.tasks.wait().await;
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.lock_registry().entries.is_empty()
    }
}

pub(super) async fn wait_for_thread_unload(mut completed: watch::Receiver<bool>) {
    while !*completed.borrow_and_update() {
        if completed.changed().await.is_err() {
            break;
        }
    }
}

pub(super) fn start_thread_teardown<F>(
    pending: PendingThreadUnloads,
    thread_id: ThreadId,
    teardown: F,
) -> Option<watch::Receiver<bool>>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    match pending.try_start(thread_id, teardown) {
        PendingThreadUnloadStartResult::Started(completed)
        | PendingThreadUnloadStartResult::Pending(completed) => Some(completed),
        PendingThreadUnloadStartResult::Closing => None,
    }
}

#[cfg(test)]
#[path = "thread_teardown_tests.rs"]
mod tests;

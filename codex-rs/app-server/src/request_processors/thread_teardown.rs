use super::*;
use std::pin::Pin;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

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
    entries: HashMap<ThreadId, Arc<PendingThreadUnloadOperation>>,
}

struct PendingThreadUnloadOperation {
    completed: watch::Sender<bool>,
    thread_ids: StdMutex<HashSet<ThreadId>>,
    finished: AtomicBool,
}

pub(super) enum PendingThreadUnloadClaimResult {
    Claimed(PendingThreadUnloadClaim),
    Pending(PendingThreadUnloadConflicts),
    Closing,
}

#[allow(
    dead_code,
    reason = "consumed by the stacked complete-tree teardown layer"
)]
pub(super) enum PendingThreadUnloadExtendResult {
    Extended,
    /// No requested ID was added. The tracked owner must end and release its existing group
    /// before any caller awaits these conflicts, then retry one claim for the full known set.
    Pending(PendingThreadUnloadConflicts),
    Finished,
}

pub(super) struct PendingThreadUnloadConflicts {
    completions: Vec<watch::Receiver<bool>>,
}

pub(super) enum PendingThreadUnloadStartResult {
    Started(watch::Receiver<bool>),
    Pending(watch::Receiver<bool>),
    Closing,
}

pub(super) struct PendingThreadUnloadClaim {
    start_tx: Option<oneshot::Sender<ThreadTeardownFuture>>,
    completed: watch::Receiver<bool>,
    pending: PendingThreadUnloads,
    operation: Arc<PendingThreadUnloadOperation>,
}

impl PendingThreadUnloadClaim {
    pub(super) fn start<F>(self, teardown: F) -> watch::Receiver<bool>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.start_with(|_| teardown)
    }

    pub(super) fn start_with<F, Fut>(mut self, teardown: F) -> watch::Receiver<bool>
    where
        F: FnOnce(PendingThreadUnloadClaimHandle) -> Fut,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        if let Some(start_tx) = self.start_tx.take() {
            let handle = PendingThreadUnloadClaimHandle {
                pending: self.pending.clone(),
                operation: Arc::clone(&self.operation),
            };
            let _ = start_tx.send(Box::pin(teardown(handle)));
        }
        self.completed.clone()
    }
}

pub(super) struct PendingThreadUnloadClaimHandle {
    pending: PendingThreadUnloads,
    operation: Arc<PendingThreadUnloadOperation>,
}

impl PendingThreadUnloadClaimHandle {
    #[allow(
        dead_code,
        reason = "consumed by the stacked complete-tree teardown layer"
    )]
    pub(super) fn try_extend<I>(&self, thread_ids: I) -> PendingThreadUnloadExtendResult
    where
        I: IntoIterator<Item = ThreadId>,
    {
        self.pending.try_extend(self, thread_ids)
    }
}

struct PendingThreadUnloadOwner {
    pending: PendingThreadUnloads,
    operation: Arc<PendingThreadUnloadOperation>,
}

impl Drop for PendingThreadUnloadOwner {
    fn drop(&mut self) {
        self.pending.release(&self.operation);
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
            .map(|operation| operation.completed.subscribe())
    }

    pub(super) fn try_claim(&self, thread_id: ThreadId) -> PendingThreadUnloadClaimResult {
        self.try_claim_many([thread_id])
    }

    pub(super) fn try_claim_many<I>(&self, thread_ids: I) -> PendingThreadUnloadClaimResult
    where
        I: IntoIterator<Item = ThreadId>,
    {
        let thread_ids = dedupe_thread_ids(thread_ids);
        let mut registry = self.lock_registry();
        if registry.closing {
            return PendingThreadUnloadClaimResult::Closing;
        }
        let conflicts = conflicting_completions(&registry, &thread_ids, /*owner*/ None);
        if !conflicts.is_empty() {
            return PendingThreadUnloadClaimResult::Pending(PendingThreadUnloadConflicts {
                completions: conflicts,
            });
        }

        let (completed, completed_rx) = watch::channel(false);
        let operation = Arc::new(PendingThreadUnloadOperation {
            completed,
            thread_ids: StdMutex::new(thread_ids.iter().copied().collect()),
            finished: AtomicBool::new(false),
        });
        for thread_id in &thread_ids {
            registry.entries.insert(*thread_id, Arc::clone(&operation));
        }
        let owner = PendingThreadUnloadOwner {
            pending: self.clone(),
            operation: Arc::clone(&operation),
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
            pending: self.clone(),
            operation,
        })
    }

    #[allow(
        dead_code,
        reason = "consumed by the stacked complete-tree teardown layer"
    )]
    fn try_extend<I>(
        &self,
        owner: &PendingThreadUnloadClaimHandle,
        thread_ids: I,
    ) -> PendingThreadUnloadExtendResult
    where
        I: IntoIterator<Item = ThreadId>,
    {
        let thread_ids = dedupe_thread_ids(thread_ids);
        let mut registry = self.lock_registry();
        if owner.operation.finished.load(Ordering::Acquire) {
            return PendingThreadUnloadExtendResult::Finished;
        }
        let conflicts = conflicting_completions(&registry, &thread_ids, Some(&owner.operation));
        if !conflicts.is_empty() {
            return PendingThreadUnloadExtendResult::Pending(PendingThreadUnloadConflicts {
                completions: conflicts,
            });
        }

        let mut owned_thread_ids = owner
            .operation
            .thread_ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for thread_id in thread_ids {
            registry
                .entries
                .entry(thread_id)
                .or_insert_with(|| Arc::clone(&owner.operation));
            owned_thread_ids.insert(thread_id);
        }
        PendingThreadUnloadExtendResult::Extended
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
            PendingThreadUnloadClaimResult::Pending(conflicts) => match conflicts.into_single() {
                Some(completed) => PendingThreadUnloadStartResult::Pending(completed),
                None => PendingThreadUnloadStartResult::Closing,
            },
            PendingThreadUnloadClaimResult::Closing => PendingThreadUnloadStartResult::Closing,
        }
    }

    fn release(&self, operation: &Arc<PendingThreadUnloadOperation>) {
        let mut registry = self.lock_registry();
        let thread_ids = operation
            .thread_ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for thread_id in thread_ids.iter() {
            if registry
                .entries
                .get(thread_id)
                .is_some_and(|current| Arc::ptr_eq(current, operation))
            {
                registry.entries.remove(thread_id);
            }
        }
        operation.finished.store(true, Ordering::Release);
        drop(thread_ids);
        drop(registry);
        let _ = operation.completed.send(true);
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

impl PendingThreadUnloadConflicts {
    fn into_single(mut self) -> Option<watch::Receiver<bool>> {
        debug_assert_eq!(self.completions.len(), 1);
        self.completions.pop()
    }
}

fn dedupe_thread_ids<I>(thread_ids: I) -> Vec<ThreadId>
where
    I: IntoIterator<Item = ThreadId>,
{
    let mut seen = HashSet::new();
    thread_ids
        .into_iter()
        .filter(|thread_id| seen.insert(*thread_id))
        .collect()
}

fn conflicting_completions(
    registry: &PendingThreadUnloadRegistry,
    thread_ids: &[ThreadId],
    owner: Option<&Arc<PendingThreadUnloadOperation>>,
) -> Vec<watch::Receiver<bool>> {
    let mut conflicts = Vec::<Arc<PendingThreadUnloadOperation>>::new();
    for thread_id in thread_ids {
        let Some(operation) = registry.entries.get(thread_id) else {
            continue;
        };
        if owner.is_some_and(|owner| Arc::ptr_eq(operation, owner))
            || conflicts
                .iter()
                .any(|conflict| Arc::ptr_eq(conflict, operation))
        {
            continue;
        }
        conflicts.push(Arc::clone(operation));
    }
    conflicts
        .into_iter()
        .map(|operation| operation.completed.subscribe())
        .collect()
}

pub(super) async fn wait_for_thread_unload(mut completed: watch::Receiver<bool>) {
    while !*completed.borrow_and_update() {
        if completed.changed().await.is_err() {
            break;
        }
    }
}

#[allow(
    dead_code,
    reason = "consumed by the stacked complete-tree teardown layer"
)]
pub(super) async fn wait_for_thread_unloads(conflicts: PendingThreadUnloadConflicts) {
    for completed in conflicts.completions {
        wait_for_thread_unload(completed).await;
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

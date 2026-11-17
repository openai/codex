//! Orders list writers behind earlier resume probes, including probes that need to upgrade.

use super::internal_error;
use codex_app_server_protocol::JSONRPCErrorError;
use futures::future::BoxFuture;
use futures::future::MaybeDone;
use futures::future::maybe_done;
use tokio::sync::Mutex;
use tokio::sync::MutexGuard;
use tokio::sync::RwLock;
use tokio::sync::RwLockReadGuard;
use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;

const PERMITS: u32 = 1024;
type ProbeOrder<'a> = MaybeDone<BoxFuture<'a, MutexGuard<'a, ()>>>;

pub(crate) struct ThreadListState {
    admission: RwLock<()>,
    probe_order: Mutex<()>,
    semaphore: Semaphore,
}

impl Default for ThreadListState {
    fn default() -> Self {
        Self {
            admission: RwLock::new(()),
            probe_order: Mutex::new(()),
            semaphore: Semaphore::new(PERMITS as usize),
        }
    }
}

impl ThreadListState {
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "keep later probes behind the writer until its semaphore permit is acquired"
    )]
    pub(super) async fn acquire_exclusive(&self) -> Result<SemaphorePermit<'_>, JSONRPCErrorError> {
        let _admission = self.admission.write().await;
        self.acquire(PERMITS).await
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "keep later writers behind the probe until its shared permit is acquired"
    )]
    pub(super) async fn acquire_resume(
        &self,
        shared: bool,
    ) -> Result<ThreadListResumePermit<'_>, JSONRPCErrorError> {
        if shared {
            let admission = self.admission.read().await;
            let permit = self.acquire(/*count*/ 1).await?;
            let mut order: ProbeOrder<'_> = maybe_done(Box::pin(self.probe_order.lock()));
            // Poll once to reserve this probe's FIFO position without blocking other probes.
            let _ = futures::poll!(&mut order);
            Ok(ThreadListResumePermit {
                state: self,
                permit,
                admission: Some(admission),
                order: Some(order),
            })
        } else {
            let permit = self.acquire_exclusive().await?;
            Ok(ThreadListResumePermit {
                state: self,
                permit,
                admission: None,
                order: None,
            })
        }
    }

    async fn acquire(&self, count: u32) -> Result<SemaphorePermit<'_>, JSONRPCErrorError> {
        self.semaphore.acquire_many(count).await.map_err(|err| {
            internal_error(format!("failed to acquire thread list state permit: {err}"))
        })
    }
}

pub(super) struct ThreadListResumePermit<'a> {
    state: &'a ThreadListState,
    permit: SemaphorePermit<'a>,
    admission: Option<RwLockReadGuard<'a, ()>>,
    order: Option<ProbeOrder<'a>>,
}

impl<'a> ThreadListResumePermit<'a> {
    pub(super) fn is_shared(&self) -> bool {
        self.permit.num_permits() == 1
    }

    pub(super) fn finish_probe(&mut self) {
        self.order.take();
        self.admission.take();
    }

    pub(super) async fn upgrade(self) -> Result<Self, JSONRPCErrorError> {
        let Self {
            state,
            permit,
            admission,
            order,
        } = self;
        debug_assert!(admission.is_some());
        drop(permit);
        let mut order = order.ok_or_else(|| internal_error("missing probe reservation"))?;
        (&mut order).await;
        // The admission read guard keeps later writers out of the semaphore queue.
        let permit = state.acquire(PERMITS).await?;
        drop(order);
        drop(admission);
        Ok(Self {
            state,
            permit,
            admission: None,
            order: None,
        })
    }
}

#[cfg(test)]
#[path = "thread_list_state_tests.rs"]
mod tests;

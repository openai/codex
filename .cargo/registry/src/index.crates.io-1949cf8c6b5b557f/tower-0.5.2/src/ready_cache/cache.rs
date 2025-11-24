//! A cache of services.

use super::error;
use futures_core::Stream;
use futures_util::{stream::FuturesUnordered, task::AtomicWaker};
pub use indexmap::Equivalent;
use indexmap::IndexMap;
use std::fmt;
use std::future::Future;
use std::hash::Hash;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use tower_service::Service;
use tracing::{debug, trace};

/// Drives readiness over a set of services.
///
/// The cache maintains two internal data structures:
///
/// * a set of _pending_ services that have not yet become ready; and
/// * a set of _ready_ services that have previously polled ready.
///
/// As each `S` typed [`Service`] is added to the cache via [`ReadyCache::push`], it
/// is added to the _pending set_. As [`ReadyCache::poll_pending`] is invoked,
/// pending services are polled and added to the _ready set_.
///
/// [`ReadyCache::call_ready`] (or [`ReadyCache::call_ready_index`]) dispatches a
/// request to the specified service, but panics if the specified service is not
/// in the ready set. The `ReadyCache::check_*` functions can be used to ensure
/// that a service is ready before dispatching a request.
///
/// The ready set can hold services for an arbitrarily long time. During this
/// time, the runtime may process events that invalidate that ready state (for
/// instance, if a keepalive detects a lost connection). In such cases, callers
/// should use [`ReadyCache::check_ready`] (or [`ReadyCache::check_ready_index`])
/// immediately before dispatching a request to ensure that the service has not
/// become unavailable.
///
/// Once `ReadyCache::call_ready*` is invoked, the service is placed back into
/// the _pending_ set to be driven to readiness again.
///
/// When `ReadyCache::check_ready*` returns `false`, it indicates that the
/// specified service is _not_ ready. If an error is returned, this indicates that
/// the server failed and has been removed from the cache entirely.
///
/// [`ReadyCache::evict`] can be used to remove a service from the cache (by key),
/// though the service may not be dropped (if it is currently pending) until
/// [`ReadyCache::poll_pending`] is invoked.
///
/// Note that the by-index accessors are provided to support use cases (like
/// power-of-two-choices load balancing) where the caller does not care to keep
/// track of each service's key. Instead, it needs only to access _some_ ready
/// service. In such a case, it should be noted that calls to
/// [`ReadyCache::poll_pending`] and [`ReadyCache::evict`] may perturb the order of
/// the ready set, so any cached indexes should be discarded after such a call.
pub struct ReadyCache<K, S, Req>
where
    K: Eq + Hash,
{
    /// A stream of services that are not yet ready.
    pending: FuturesUnordered<Pending<K, S, Req>>,
    /// An index of cancelation handles for pending streams.
    pending_cancel_txs: IndexMap<K, CancelTx>,

    /// Services that have previously become ready. Readiness can become stale,
    /// so a given service should be polled immediately before use.
    ///
    /// The cancelation oneshot is preserved (though unused) while the service is
    /// ready so that it need not be reallocated each time a request is
    /// dispatched.
    ready: IndexMap<K, (S, CancelPair)>,
}

// Safety: This is safe because we do not use `Pin::new_unchecked`.
impl<S, K: Eq + Hash, Req> Unpin for ReadyCache<K, S, Req> {}

#[derive(Debug)]
struct Cancel {
    waker: AtomicWaker,
    canceled: AtomicBool,
}

#[derive(Debug)]
struct CancelRx(Arc<Cancel>);

#[derive(Debug)]
struct CancelTx(Arc<Cancel>);

type CancelPair = (CancelTx, CancelRx);

#[derive(Debug)]
enum PendingError<K, E> {
    Canceled(K),
    Inner(K, E),
}

pin_project_lite::pin_project! {
    /// A [`Future`] that becomes satisfied when an `S`-typed service is ready.
    ///
    /// May fail due to cancelation, i.e. if the service is evicted from the balancer.
    struct Pending<K, S, Req> {
        key: Option<K>,
        cancel: Option<CancelRx>,
        ready: Option<S>,
        _pd: std::marker::PhantomData<Req>,
    }
}

// === ReadyCache ===

impl<K, S, Req> Default for ReadyCache<K, S, Req>
where
    K: Eq + Hash,
    S: Service<Req>,
{
    fn default() -> Self {
        Self {
            ready: IndexMap::default(),
            pending: FuturesUnordered::new(),
            pending_cancel_txs: IndexMap::default(),
        }
    }
}

impl<K, S, Req> fmt::Debug for ReadyCache<K, S, Req>
where
    K: fmt::Debug + Eq + Hash,
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Self {
            pending,
            pending_cancel_txs,
            ready,
        } = self;
        f.debug_struct("ReadyCache")
            .field("pending", pending)
            .field("pending_cancel_txs", pending_cancel_txs)
            .field("ready", ready)
            .finish()
    }
}

impl<K, S, Req> ReadyCache<K, S, Req>
where
    K: Eq + Hash,
{
    /// Returns the total number of services in the cache.
    pub fn len(&self) -> usize {
        self.ready_len() + self.pending_len()
    }

    /// Returns whether or not there are any services in the cache.
    pub fn is_empty(&self) -> bool {
        self.ready.is_empty() && self.pending.is_empty()
    }

    /// Returns the number of services in the ready set.
    pub fn ready_len(&self) -> usize {
        self.ready.len()
    }

    /// Returns the number of services in the unready set.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Returns true iff the given key is in the unready set.
    pub fn pending_contains<Q: Hash + Equivalent<K>>(&self, key: &Q) -> bool {
        self.pending_cancel_txs.contains_key(key)
    }

    /// Obtains a reference to a service in the ready set by key.
    pub fn get_ready<Q: Hash + Equivalent<K>>(&self, key: &Q) -> Option<(usize, &K, &S)> {
        self.ready.get_full(key).map(|(i, k, v)| (i, k, &v.0))
    }

    /// Obtains a mutable reference to a service in the ready set by key.
    pub fn get_ready_mut<Q: Hash + Equivalent<K>>(
        &mut self,
        key: &Q,
    ) -> Option<(usize, &K, &mut S)> {
        self.ready
            .get_full_mut(key)
            .map(|(i, k, v)| (i, k, &mut v.0))
    }

    /// Obtains a reference to a service in the ready set by index.
    pub fn get_ready_index(&self, idx: usize) -> Option<(&K, &S)> {
        self.ready.get_index(idx).map(|(k, v)| (k, &v.0))
    }

    /// Obtains a mutable reference to a service in the ready set by index.
    pub fn get_ready_index_mut(&mut self, idx: usize) -> Option<(&K, &mut S)> {
        self.ready.get_index_mut(idx).map(|(k, v)| (k, &mut v.0))
    }

    /// Returns an iterator over the ready keys and services.
    pub fn iter_ready(&self) -> impl Iterator<Item = (&K, &S)> {
        self.ready.iter().map(|(k, s)| (k, &s.0))
    }

    /// Returns a mutable iterator over the ready keys and services.
    pub fn iter_ready_mut(&mut self) -> impl Iterator<Item = (&K, &mut S)> {
        self.ready.iter_mut().map(|(k, s)| (k, &mut s.0))
    }

    /// Evicts an item from the cache.
    ///
    /// Returns true if a service was marked for eviction.
    ///
    /// Services are dropped from the ready set immediately. Services in the
    /// pending set are marked for cancellation, but [`ReadyCache::poll_pending`]
    /// must be called to cause the service to be dropped.
    pub fn evict<Q: Hash + Equivalent<K>>(&mut self, key: &Q) -> bool {
        let canceled = if let Some(c) = self.pending_cancel_txs.swap_remove(key) {
            c.cancel();
            true
        } else {
            false
        };

        self.ready
            .swap_remove_full(key)
            .map(|_| true)
            .unwrap_or(canceled)
    }
}

impl<K, S, Req> ReadyCache<K, S, Req>
where
    K: Clone + Eq + Hash,
    S: Service<Req>,
    <S as Service<Req>>::Error: Into<crate::BoxError>,
    S::Error: Into<crate::BoxError>,
{
    /// Pushes a new service onto the pending set.
    ///
    /// The service will be promoted to the ready set as [`poll_pending`] is invoked.
    ///
    /// Note that this does **not** remove services from the ready set. Once the
    /// old service is used, it will be dropped instead of being added back to
    /// the pending set; OR, when the new service becomes ready, it will replace
    /// the prior service in the ready set.
    ///
    /// [`poll_pending`]: crate::ready_cache::cache::ReadyCache::poll_pending
    pub fn push(&mut self, key: K, svc: S) {
        let cancel = cancelable();
        self.push_pending(key, svc, cancel);
    }

    fn push_pending(&mut self, key: K, svc: S, (cancel_tx, cancel_rx): CancelPair) {
        if let Some(c) = self.pending_cancel_txs.insert(key.clone(), cancel_tx) {
            // If there is already a service for this key, cancel it.
            c.cancel();
        }
        self.pending.push(Pending {
            key: Some(key),
            cancel: Some(cancel_rx),
            ready: Some(svc),
            _pd: std::marker::PhantomData,
        });
    }

    /// Polls services pending readiness, adding ready services to the ready set.
    ///
    /// Returns [`Poll::Ready`] when there are no remaining unready services.
    /// [`poll_pending`] should be called again after [`push`] or
    /// [`call_ready_index`] are invoked.
    ///
    /// Failures indicate that an individual pending service failed to become
    /// ready (and has been removed from the cache). In such a case,
    /// [`poll_pending`] should typically be called again to continue driving
    /// pending services to readiness.
    ///
    /// [`poll_pending`]: crate::ready_cache::cache::ReadyCache::poll_pending
    /// [`push`]: crate::ready_cache::cache::ReadyCache::push
    /// [`call_ready_index`]: crate::ready_cache::cache::ReadyCache::call_ready_index
    pub fn poll_pending(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), error::Failed<K>>> {
        loop {
            match Pin::new(&mut self.pending).poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(Ok(())),
                Poll::Ready(Some(Ok((key, svc, cancel_rx)))) => {
                    trace!("endpoint ready");
                    let cancel_tx = self.pending_cancel_txs.swap_remove(&key);
                    if let Some(cancel_tx) = cancel_tx {
                        // Keep track of the cancelation so that it need not be
                        // recreated after the service is used.
                        self.ready.insert(key, (svc, (cancel_tx, cancel_rx)));
                    } else {
                        assert!(
                            cancel_tx.is_some(),
                            "services that become ready must have a pending cancelation"
                        );
                    }
                }
                Poll::Ready(Some(Err(PendingError::Canceled(_)))) => {
                    debug!("endpoint canceled");
                    // The cancellation for this service was removed in order to
                    // cause this cancellation.
                }
                Poll::Ready(Some(Err(PendingError::Inner(key, e)))) => {
                    let cancel_tx = self.pending_cancel_txs.swap_remove(&key);
                    assert!(
                        cancel_tx.is_some(),
                        "services that return an error must have a pending cancelation"
                    );
                    return Err(error::Failed(key, e.into())).into();
                }
            }
        }
    }

    /// Checks whether the referenced endpoint is ready.
    ///
    /// Returns true if the endpoint is ready and false if it is not. An error is
    /// returned if the endpoint fails.
    pub fn check_ready<Q: Hash + Equivalent<K>>(
        &mut self,
        cx: &mut Context<'_>,
        key: &Q,
    ) -> Result<bool, error::Failed<K>> {
        match self.ready.get_full_mut(key) {
            Some((index, _, _)) => self.check_ready_index(cx, index),
            None => Ok(false),
        }
    }

    /// Checks whether the referenced endpoint is ready.
    ///
    /// If the service is no longer ready, it is moved back into the pending set
    /// and `false` is returned.
    ///
    /// If the service errors, it is removed and dropped and the error is returned.
    pub fn check_ready_index(
        &mut self,
        cx: &mut Context<'_>,
        index: usize,
    ) -> Result<bool, error::Failed<K>> {
        let svc = match self.ready.get_index_mut(index) {
            None => return Ok(false),
            Some((_, (svc, _))) => svc,
        };
        match svc.poll_ready(cx) {
            Poll::Ready(Ok(())) => Ok(true),
            Poll::Pending => {
                // became unready; so move it back there.
                let (key, (svc, cancel)) = self
                    .ready
                    .swap_remove_index(index)
                    .expect("invalid ready index");

                // If a new version of this service has been added to the
                // unready set, don't overwrite it.
                if !self.pending_contains(&key) {
                    self.push_pending(key, svc, cancel);
                }

                Ok(false)
            }
            Poll::Ready(Err(e)) => {
                // failed, so drop it.
                let (key, _) = self
                    .ready
                    .swap_remove_index(index)
                    .expect("invalid ready index");
                Err(error::Failed(key, e.into()))
            }
        }
    }

    /// Calls a ready service by key.
    ///
    /// # Panics
    ///
    /// If the specified key does not exist in the ready
    pub fn call_ready<Q: Hash + Equivalent<K>>(&mut self, key: &Q, req: Req) -> S::Future {
        let (index, _, _) = self
            .ready
            .get_full_mut(key)
            .expect("check_ready was not called");
        self.call_ready_index(index, req)
    }

    /// Calls a ready service by index.
    ///
    /// # Panics
    ///
    /// If the specified index is out of range.
    pub fn call_ready_index(&mut self, index: usize, req: Req) -> S::Future {
        let (key, (mut svc, cancel)) = self
            .ready
            .swap_remove_index(index)
            .expect("check_ready_index was not called");

        let fut = svc.call(req);

        // If a new version of this service has been added to the
        // unready set, don't overwrite it.
        if !self.pending_contains(&key) {
            self.push_pending(key, svc, cancel);
        }

        fut
    }
}

// === impl Cancel ===

/// Creates a cancelation sender and receiver.
///
/// A `tokio::sync::oneshot` is NOT used, as a `Receiver` is not guaranteed to
/// observe results as soon as a `Sender` fires. Using an `AtomicBool` allows
/// the state to be observed as soon as the cancelation is triggered.
fn cancelable() -> CancelPair {
    let cx = Arc::new(Cancel {
        waker: AtomicWaker::new(),
        canceled: AtomicBool::new(false),
    });
    (CancelTx(cx.clone()), CancelRx(cx))
}

impl CancelTx {
    fn cancel(self) {
        self.0.canceled.store(true, Ordering::SeqCst);
        self.0.waker.wake();
    }
}

// === Pending ===

impl<K, S, Req> Future for Pending<K, S, Req>
where
    S: Service<Req>,
{
    type Output = Result<(K, S, CancelRx), PendingError<K, S::Error>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        // Before checking whether the service is ready, check to see whether
        // readiness has been canceled.
        let CancelRx(cancel) = this.cancel.as_mut().expect("polled after complete");
        if cancel.canceled.load(Ordering::SeqCst) {
            let key = this.key.take().expect("polled after complete");
            return Err(PendingError::Canceled(key)).into();
        }

        match this
            .ready
            .as_mut()
            .expect("polled after ready")
            .poll_ready(cx)
        {
            Poll::Pending => {
                // Before returning Pending, register interest in cancelation so
                // that this future is polled again if the state changes.
                let CancelRx(cancel) = this.cancel.as_mut().expect("polled after complete");
                cancel.waker.register(cx.waker());
                // Because both the cancel receiver and cancel sender are held
                // by the `ReadyCache` (i.e., on a single task), then it must
                // not be possible for the cancelation state to change while
                // polling a `Pending` service.
                assert!(
                    !cancel.canceled.load(Ordering::SeqCst),
                    "cancelation cannot be notified while polling a pending service"
                );
                Poll::Pending
            }
            Poll::Ready(Ok(())) => {
                let key = this.key.take().expect("polled after complete");
                let cancel = this.cancel.take().expect("polled after complete");
                Ok((key, this.ready.take().expect("polled after ready"), cancel)).into()
            }
            Poll::Ready(Err(e)) => {
                let key = this.key.take().expect("polled after compete");
                Err(PendingError::Inner(key, e)).into()
            }
        }
    }
}

impl<K, S, Req> fmt::Debug for Pending<K, S, Req>
where
    K: fmt::Debug,
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Self {
            key,
            cancel,
            ready,
            _pd,
        } = self;
        f.debug_struct("Pending")
            .field("key", key)
            .field("cancel", cancel)
            .field("ready", ready)
            .finish()
    }
}

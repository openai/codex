use super::super::error;
use crate::discover::{Change, Discover};
use crate::load::Load;
use crate::ready_cache::{error::Failed, ReadyCache};
use crate::util::rng::{sample_floyd2, HasherRng, Rng};
use futures_core::ready;
use futures_util::future::{self, TryFutureExt};
use std::hash::Hash;
use std::marker::PhantomData;
use std::{
    fmt,
    pin::Pin,
    task::{Context, Poll},
};
use tower_service::Service;
use tracing::{debug, trace};

/// Efficiently distributes requests across an arbitrary number of services.
///
/// See the [module-level documentation](..) for details.
///
/// Note that [`Balance`] requires that the [`Discover`] you use is [`Unpin`] in order to implement
/// [`Service`]. This is because it needs to be accessed from [`Service::poll_ready`], which takes
/// `&mut self`. You can achieve this easily by wrapping your [`Discover`] in [`Box::pin`] before you
/// construct the [`Balance`] instance. For more details, see [#319].
///
/// [`Box::pin`]: std::boxed::Box::pin()
/// [#319]: https://github.com/tower-rs/tower/issues/319
pub struct Balance<D, Req>
where
    D: Discover,
    D::Key: Hash,
{
    discover: D,

    services: ReadyCache<D::Key, D::Service, Req>,
    ready_index: Option<usize>,

    rng: Box<dyn Rng + Send + Sync>,

    _req: PhantomData<Req>,
}

impl<D: Discover, Req> fmt::Debug for Balance<D, Req>
where
    D: fmt::Debug,
    D::Key: Hash + fmt::Debug,
    D::Service: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Balance")
            .field("discover", &self.discover)
            .field("services", &self.services)
            .finish()
    }
}

impl<D, Req> Balance<D, Req>
where
    D: Discover,
    D::Key: Hash,
    D::Service: Service<Req>,
    <D::Service as Service<Req>>::Error: Into<crate::BoxError>,
{
    /// Constructs a load balancer that uses operating system entropy.
    pub fn new(discover: D) -> Self {
        Self::from_rng(discover, HasherRng::default())
    }

    /// Constructs a load balancer seeded with the provided random number generator.
    pub fn from_rng<R: Rng + Send + Sync + 'static>(discover: D, rng: R) -> Self {
        let rng = Box::new(rng);
        Self {
            rng,
            discover,
            services: ReadyCache::default(),
            ready_index: None,

            _req: PhantomData,
        }
    }

    /// Returns the number of endpoints currently tracked by the balancer.
    pub fn len(&self) -> usize {
        self.services.len()
    }

    /// Returns whether or not the balancer is empty.
    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }
}

impl<D, Req> Balance<D, Req>
where
    D: Discover + Unpin,
    D::Key: Hash + Clone,
    D::Error: Into<crate::BoxError>,
    D::Service: Service<Req> + Load,
    <D::Service as Load>::Metric: std::fmt::Debug,
    <D::Service as Service<Req>>::Error: Into<crate::BoxError>,
{
    /// Polls `discover` for updates, adding new items to `not_ready`.
    ///
    /// Removals may alter the order of either `ready` or `not_ready`.
    fn update_pending_from_discover(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<(), error::Discover>>> {
        debug!("updating from discover");
        loop {
            match ready!(Pin::new(&mut self.discover).poll_discover(cx))
                .transpose()
                .map_err(|e| error::Discover(e.into()))?
            {
                None => return Poll::Ready(None),
                Some(Change::Remove(key)) => {
                    trace!("remove");
                    self.services.evict(&key);
                }
                Some(Change::Insert(key, svc)) => {
                    trace!("insert");
                    // If this service already existed in the set, it will be
                    // replaced as the new one becomes ready.
                    self.services.push(key, svc);
                }
            }
        }
    }

    fn promote_pending_to_ready(&mut self, cx: &mut Context<'_>) {
        loop {
            match self.services.poll_pending(cx) {
                Poll::Ready(Ok(())) => {
                    // There are no remaining pending services.
                    debug_assert_eq!(self.services.pending_len(), 0);
                    break;
                }
                Poll::Pending => {
                    // None of the pending services are ready.
                    debug_assert!(self.services.pending_len() > 0);
                    break;
                }
                Poll::Ready(Err(error)) => {
                    // An individual service was lost; continue processing
                    // pending services.
                    debug!(%error, "dropping failed endpoint");
                }
            }
        }
        trace!(
            ready = %self.services.ready_len(),
            pending = %self.services.pending_len(),
            "poll_unready"
        );
    }

    /// Performs P2C on inner services to find a suitable endpoint.
    fn p2c_ready_index(&mut self) -> Option<usize> {
        match self.services.ready_len() {
            0 => None,
            1 => Some(0),
            len => {
                // Get two distinct random indexes (in a random order) and
                // compare the loads of the service at each index.
                let [aidx, bidx] = sample_floyd2(&mut self.rng, len as u64);
                debug_assert_ne!(aidx, bidx, "random indices must be distinct");

                let aload = self.ready_index_load(aidx as usize);
                let bload = self.ready_index_load(bidx as usize);
                let chosen = if aload <= bload { aidx } else { bidx };

                trace!(
                    a.index = aidx,
                    a.load = ?aload,
                    b.index = bidx,
                    b.load = ?bload,
                    chosen = if chosen == aidx { "a" } else { "b" },
                    "p2c",
                );
                Some(chosen as usize)
            }
        }
    }

    /// Accesses a ready endpoint by index and returns its current load.
    fn ready_index_load(&self, index: usize) -> <D::Service as Load>::Metric {
        let (_, svc) = self.services.get_ready_index(index).expect("invalid index");
        svc.load()
    }
}

impl<D, Req> Service<Req> for Balance<D, Req>
where
    D: Discover + Unpin,
    D::Key: Hash + Clone,
    D::Error: Into<crate::BoxError>,
    D::Service: Service<Req> + Load,
    <D::Service as Load>::Metric: std::fmt::Debug,
    <D::Service as Service<Req>>::Error: Into<crate::BoxError>,
{
    type Response = <D::Service as Service<Req>>::Response;
    type Error = crate::BoxError;
    type Future = future::MapErr<
        <D::Service as Service<Req>>::Future,
        fn(<D::Service as Service<Req>>::Error) -> crate::BoxError,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // `ready_index` may have already been set by a prior invocation. These
        // updates cannot disturb the order of existing ready services.
        let _ = self.update_pending_from_discover(cx)?;
        self.promote_pending_to_ready(cx);

        loop {
            // If a service has already been selected, ensure that it is ready.
            // This ensures that the underlying service is ready immediately
            // before a request is dispatched to it (i.e. in the same task
            // invocation). If, e.g., a failure detector has changed the state
            // of the service, it may be evicted from the ready set so that
            // another service can be selected.
            if let Some(index) = self.ready_index.take() {
                match self.services.check_ready_index(cx, index) {
                    Ok(true) => {
                        // The service remains ready.
                        self.ready_index = Some(index);
                        return Poll::Ready(Ok(()));
                    }
                    Ok(false) => {
                        // The service is no longer ready. Try to find a new one.
                        trace!("ready service became unavailable");
                    }
                    Err(Failed(_, error)) => {
                        // The ready endpoint failed, so log the error and try
                        // to find a new one.
                        debug!(%error, "endpoint failed");
                    }
                }
            }

            // Select a new service by comparing two at random and using the
            // lesser-loaded service.
            self.ready_index = self.p2c_ready_index();
            if self.ready_index.is_none() {
                debug_assert_eq!(self.services.ready_len(), 0);
                // We have previously registered interest in updates from
                // discover and pending services.
                return Poll::Pending;
            }
        }
    }

    fn call(&mut self, request: Req) -> Self::Future {
        let index = self.ready_index.take().expect("called before ready");
        self.services
            .call_ready_index(index, request)
            .map_err(Into::into)
    }
}

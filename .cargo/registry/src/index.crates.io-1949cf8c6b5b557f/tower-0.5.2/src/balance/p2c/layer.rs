use super::MakeBalance;
use std::{fmt, marker::PhantomData};
use tower_layer::Layer;

/// Construct load balancers ([`Balance`]) over dynamic service sets ([`Discover`]) produced by the
/// "inner" service in response to requests coming from the "outer" service.
///
/// This construction may seem a little odd at first glance. This is not a layer that takes
/// requests and produces responses in the traditional sense. Instead, it is more like
/// [`MakeService`] in that it takes service _descriptors_ (see `Target` on [`MakeService`])
/// and produces _services_. Since [`Balance`] spreads requests across a _set_ of services,
/// the inner service should produce a [`Discover`], not just a single
/// [`Service`], given a service descriptor.
///
/// See the [module-level documentation](crate::balance) for details on load balancing.
///
/// [`Balance`]: crate::balance::p2c::Balance
/// [`Discover`]: crate::discover::Discover
/// [`MakeService`]: crate::MakeService
/// [`Service`]: crate::Service
pub struct MakeBalanceLayer<D, Req> {
    _marker: PhantomData<fn(D, Req)>,
}

impl<D, Req> MakeBalanceLayer<D, Req> {
    /// Build balancers using operating system entropy.
    pub const fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<D, Req> Default for MakeBalanceLayer<D, Req> {
    fn default() -> Self {
        Self::new()
    }
}

impl<D, Req> Clone for MakeBalanceLayer<D, Req> {
    fn clone(&self) -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<S, Req> Layer<S> for MakeBalanceLayer<S, Req> {
    type Service = MakeBalance<S, Req>;

    fn layer(&self, make_discover: S) -> Self::Service {
        MakeBalance::new(make_discover)
    }
}

impl<D, Req> fmt::Debug for MakeBalanceLayer<D, Req> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("MakeBalanceLayer").finish()
    }
}

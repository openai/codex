use super::{TlsAcceptorData, TlsAcceptorService};
use rama_core::Layer;

/// A [`Layer`] which wraps the given service with a [`TlsAcceptorService`].
#[derive(Debug, Clone)]
pub struct TlsAcceptorLayer {
    data: TlsAcceptorData,
    store_client_hello: bool,
}

impl TlsAcceptorLayer {
    /// Creates a new [`TlsAcceptorLayer`] using the given [`ServerConfig`],
    /// which is used to configure the inner TLS acceptor.
    ///
    /// [`ServerConfig`]: https://docs.rs/rustls/latest/rustls/server/struct.ServerConfig.html
    #[must_use]
    pub const fn new(data: TlsAcceptorData) -> Self {
        Self {
            data,
            store_client_hello: false,
        }
    }

    /// Set that the client hello should be stored
    #[must_use]
    pub const fn with_store_client_hello(mut self, store: bool) -> Self {
        self.store_client_hello = store;
        self
    }

    /// Set that the client hello should be stored
    pub fn set_store_client_hello(&mut self, store: bool) -> &mut Self {
        self.store_client_hello = store;
        self
    }
}

impl<S> Layer<S> for TlsAcceptorLayer {
    type Service = TlsAcceptorService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TlsAcceptorService::new(self.data.clone(), inner, self.store_client_hello)
    }

    fn into_layer(self, inner: S) -> Self::Service {
        TlsAcceptorService::new(self.data, inner, self.store_client_hello)
    }
}

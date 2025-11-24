use core::task;
use hyper_util::client::legacy::connect::dns::Name;
use std::future::Future;
use std::net::SocketAddr;
use std::task::Poll;
use tower_service::Service;

// Trait from hyper to implement DNS resolution for HTTP/3 client.
pub trait Resolve {
    type Addrs: Iterator<Item = SocketAddr>;
    type Error: Into<Box<dyn std::error::Error + Send + Sync>>;
    type Future: Future<Output = Result<Self::Addrs, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>>;
    fn resolve(&mut self, name: Name) -> Self::Future;
}

impl<S> Resolve for S
where
    S: Service<Name>,
    S::Response: Iterator<Item = SocketAddr>,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Addrs = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Service::poll_ready(self, cx)
    }

    fn resolve(&mut self, name: Name) -> Self::Future {
        Service::call(self, name)
    }
}

pub(super) async fn resolve<R>(resolver: &mut R, name: Name) -> Result<R::Addrs, R::Error>
where
    R: Resolve,
{
    std::future::poll_fn(|cx| resolver.poll_ready(cx)).await?;
    resolver.resolve(name).await
}

use hyper_util::client::legacy::connect::dns::GaiResolver as HyperGaiResolver;
use tower_service::Service;

use crate::dns::{Addrs, Name, Resolve, Resolving};
use crate::error::BoxError;

#[derive(Debug)]
pub struct GaiResolver(HyperGaiResolver);

impl GaiResolver {
    pub fn new() -> Self {
        Self(HyperGaiResolver::new())
    }
}

impl Default for GaiResolver {
    fn default() -> Self {
        GaiResolver::new()
    }
}

impl Resolve for GaiResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let mut this = self.0.clone();
        Box::pin(async move {
            this.call(name.0)
                .await
                .map(|addrs| Box::new(addrs) as Addrs)
                .map_err(|err| Box::new(err) as BoxError)
        })
    }
}

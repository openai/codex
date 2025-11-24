use std::{
    convert::Infallible,
    future::ready,
    task::{Context, Poll},
};
use tower_service::Service;

/// A [`MakeService`] that produces axum router services.
///
/// [`MakeService`]: tower::make::MakeService
#[derive(Debug, Clone)]
pub struct IntoMakeService<S> {
    svc: S,
}

impl<S> IntoMakeService<S> {
    pub(crate) fn new(svc: S) -> Self {
        Self { svc }
    }
}

impl<S, T> Service<T> for IntoMakeService<S>
where
    S: Clone,
{
    type Response = S;
    type Error = Infallible;
    type Future = IntoMakeServiceFuture<S>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _target: T) -> Self::Future {
        IntoMakeServiceFuture::new(ready(Ok(self.svc.clone())))
    }
}

opaque_future! {
    /// Response future for [`IntoMakeService`].
    pub type IntoMakeServiceFuture<S> =
        std::future::Ready<Result<S, Infallible>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traits() {
        use crate::test_helpers::*;

        assert_send::<IntoMakeService<()>>();
    }
}

use async_trait::async_trait;
use std::future::Future;
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub enum CancelErr {
    Cancelled,
}

#[async_trait]
pub trait OrCancelExt: Sized {
    type Output;

    async fn or_cancel(self, token: &CancellationToken) -> Result<Self::Output, CancelErr>;
}

#[async_trait]
impl<F> OrCancelExt for F
where
    F: Future + Send,
    F::Output: Send,
{
    type Output = F::Output;

    async fn or_cancel(self, token: &CancellationToken) -> Result<Self::Output, CancelErr> {
        tokio::select! {
            _ = token.cancelled() => Err(CancelErr::Cancelled),
            res = self => Ok(res),
        }
    }
}

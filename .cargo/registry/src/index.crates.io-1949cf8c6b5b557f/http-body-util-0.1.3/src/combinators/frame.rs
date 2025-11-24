use http_body::Body;

use core::future::Future;
use core::pin::Pin;
use core::task;

#[must_use = "futures don't do anything unless polled"]
#[derive(Debug)]
/// Future that resolves to the next frame from a [`Body`].
pub struct Frame<'a, T: ?Sized>(pub(crate) &'a mut T);

impl<T: Body + Unpin + ?Sized> Future for Frame<'_, T> {
    type Output = Option<Result<http_body::Frame<T::Data>, T::Error>>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        Pin::new(&mut self.0).poll_frame(ctx)
    }
}

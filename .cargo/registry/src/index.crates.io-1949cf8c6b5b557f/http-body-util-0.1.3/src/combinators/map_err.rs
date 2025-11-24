use http_body::{Body, Frame, SizeHint};
use pin_project_lite::pin_project;
use std::{
    any::type_name,
    fmt,
    pin::Pin,
    task::{Context, Poll},
};

pin_project! {
    /// Body returned by the [`map_err`] combinator.
    ///
    /// [`map_err`]: crate::BodyExt::map_err
    #[derive(Clone, Copy)]
    pub struct MapErr<B, F> {
        #[pin]
        inner: B,
        f: F
    }
}

impl<B, F> MapErr<B, F> {
    #[inline]
    pub(crate) fn new(body: B, f: F) -> Self {
        Self { inner: body, f }
    }

    /// Get a reference to the inner body
    pub fn get_ref(&self) -> &B {
        &self.inner
    }

    /// Get a mutable reference to the inner body
    pub fn get_mut(&mut self) -> &mut B {
        &mut self.inner
    }

    /// Get a pinned mutable reference to the inner body
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut B> {
        self.project().inner
    }

    /// Consume `self`, returning the inner body
    pub fn into_inner(self) -> B {
        self.inner
    }
}

impl<B, F, E> Body for MapErr<B, F>
where
    B: Body,
    F: FnMut(B::Error) -> E,
{
    type Data = B::Data;
    type Error = E;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.project();
        match this.inner.poll_frame(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Ready(Some(Ok(frame))) => Poll::Ready(Some(Ok(frame))),
            Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err((this.f)(err)))),
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

impl<B, F> fmt::Debug for MapErr<B, F>
where
    B: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("MapErr")
            .field("inner", &self.inner)
            .field("f", &type_name::<F>())
            .finish()
    }
}

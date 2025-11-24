use bytes::Buf;
use http_body::{Body, Frame};
use pin_project_lite::pin_project;
use std::{
    any::type_name,
    fmt,
    pin::Pin,
    task::{Context, Poll},
};

pin_project! {
    /// Body returned by the [`map_frame`] combinator.
    ///
    /// [`map_frame`]: crate::BodyExt::map_frame
    #[derive(Clone, Copy)]
    pub struct MapFrame<B, F> {
        #[pin]
        inner: B,
        f: F
    }
}

impl<B, F> MapFrame<B, F> {
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

impl<B, F, B2> Body for MapFrame<B, F>
where
    B: Body,
    F: FnMut(Frame<B::Data>) -> Frame<B2>,
    B2: Buf,
{
    type Data = B2;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.project();
        match this.inner.poll_frame(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Ready(Some(Ok(frame))) => Poll::Ready(Some(Ok((this.f)(frame)))),
            Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(err))),
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }
}

impl<B, F> fmt::Debug for MapFrame<B, F>
where
    B: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("MapFrame")
            .field("inner", &self.inner)
            .field("f", &type_name::<F>())
            .finish()
    }
}

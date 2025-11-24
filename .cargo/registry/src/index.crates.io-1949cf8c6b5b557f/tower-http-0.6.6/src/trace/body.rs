use super::{DefaultOnBodyChunk, DefaultOnEos, DefaultOnFailure, OnBodyChunk, OnEos, OnFailure};
use crate::classify::ClassifyEos;
use http_body::{Body, Frame};
use pin_project_lite::pin_project;
use std::{
    fmt,
    pin::Pin,
    task::{ready, Context, Poll},
    time::Instant,
};
use tracing::Span;

pin_project! {
    /// Response body for [`Trace`].
    ///
    /// [`Trace`]: super::Trace
    pub struct ResponseBody<B, C, OnBodyChunk = DefaultOnBodyChunk, OnEos = DefaultOnEos, OnFailure = DefaultOnFailure> {
        #[pin]
        pub(crate) inner: B,
        pub(crate) classify_eos: Option<C>,
        pub(crate) on_eos: Option<(OnEos, Instant)>,
        pub(crate) on_body_chunk: OnBodyChunk,
        pub(crate) on_failure: Option<OnFailure>,
        pub(crate) start: Instant,
        pub(crate) span: Span,
    }
}

impl<B, C, OnBodyChunkT, OnEosT, OnFailureT> Body
    for ResponseBody<B, C, OnBodyChunkT, OnEosT, OnFailureT>
where
    B: Body,
    B::Error: fmt::Display + 'static,
    C: ClassifyEos,
    OnEosT: OnEos,
    OnBodyChunkT: OnBodyChunk<B::Data>,
    OnFailureT: OnFailure<C::FailureClass>,
{
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        let this = self.project();
        let _guard = this.span.enter();
        let result = ready!(this.inner.poll_frame(cx));

        let latency = this.start.elapsed();
        *this.start = Instant::now();

        match result {
            Some(Ok(frame)) => {
                let frame = match frame.into_data() {
                    Ok(chunk) => {
                        this.on_body_chunk.on_body_chunk(&chunk, latency, this.span);
                        Frame::data(chunk)
                    }
                    Err(frame) => frame,
                };

                let frame = match frame.into_trailers() {
                    Ok(trailers) => {
                        if let Some((on_eos, stream_start)) = this.on_eos.take() {
                            on_eos.on_eos(Some(&trailers), stream_start.elapsed(), this.span);
                        }
                        Frame::trailers(trailers)
                    }
                    Err(frame) => frame,
                };

                Poll::Ready(Some(Ok(frame)))
            }
            Some(Err(err)) => {
                if let Some((classify_eos, mut on_failure)) =
                    this.classify_eos.take().zip(this.on_failure.take())
                {
                    let failure_class = classify_eos.classify_error(&err);
                    on_failure.on_failure(failure_class, latency, this.span);
                }

                Poll::Ready(Some(Err(err)))
            }
            None => {
                if let Some((on_eos, stream_start)) = this.on_eos.take() {
                    on_eos.on_eos(None, stream_start.elapsed(), this.span);
                }

                Poll::Ready(None)
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

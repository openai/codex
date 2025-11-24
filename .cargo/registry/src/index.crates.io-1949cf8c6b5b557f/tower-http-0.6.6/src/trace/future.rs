use super::{
    DefaultOnBodyChunk, DefaultOnEos, DefaultOnFailure, DefaultOnResponse, OnBodyChunk, OnEos,
    OnFailure, OnResponse, ResponseBody,
};
use crate::classify::{ClassifiedResponse, ClassifyResponse};
use http::Response;
use http_body::Body;
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
    time::Instant,
};
use tracing::Span;

pin_project! {
    /// Response future for [`Trace`].
    ///
    /// [`Trace`]: super::Trace
    pub struct ResponseFuture<F, C, OnResponse = DefaultOnResponse, OnBodyChunk = DefaultOnBodyChunk, OnEos = DefaultOnEos, OnFailure = DefaultOnFailure> {
        #[pin]
        pub(crate) inner: F,
        pub(crate) span: Span,
        pub(crate) classifier: Option<C>,
        pub(crate) on_response: Option<OnResponse>,
        pub(crate) on_body_chunk: Option<OnBodyChunk>,
        pub(crate) on_eos: Option<OnEos>,
        pub(crate) on_failure: Option<OnFailure>,
        pub(crate) start: Instant,
    }
}

impl<Fut, ResBody, E, C, OnResponseT, OnBodyChunkT, OnEosT, OnFailureT> Future
    for ResponseFuture<Fut, C, OnResponseT, OnBodyChunkT, OnEosT, OnFailureT>
where
    Fut: Future<Output = Result<Response<ResBody>, E>>,
    ResBody: Body,
    ResBody::Error: std::fmt::Display + 'static,
    E: std::fmt::Display + 'static,
    C: ClassifyResponse,
    OnResponseT: OnResponse<ResBody>,
    OnFailureT: OnFailure<C::FailureClass>,
    OnBodyChunkT: OnBodyChunk<ResBody::Data>,
    OnEosT: OnEos,
{
    type Output = Result<
        Response<ResponseBody<ResBody, C::ClassifyEos, OnBodyChunkT, OnEosT, OnFailureT>>,
        E,
    >;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _guard = this.span.enter();
        let result = ready!(this.inner.poll(cx));
        let latency = this.start.elapsed();

        let classifier = this.classifier.take().unwrap();
        let on_eos = this.on_eos.take();
        let on_body_chunk = this.on_body_chunk.take().unwrap();
        let mut on_failure = this.on_failure.take().unwrap();

        match result {
            Ok(res) => {
                let classification = classifier.classify_response(&res);
                let start = *this.start;

                this.on_response
                    .take()
                    .unwrap()
                    .on_response(&res, latency, this.span);

                match classification {
                    ClassifiedResponse::Ready(classification) => {
                        if let Err(failure_class) = classification {
                            on_failure.on_failure(failure_class, latency, this.span);
                        }

                        let span = this.span.clone();
                        let res = res.map(|body| ResponseBody {
                            inner: body,
                            classify_eos: None,
                            on_eos: None,
                            on_body_chunk,
                            on_failure: Some(on_failure),
                            start,
                            span,
                        });

                        Poll::Ready(Ok(res))
                    }
                    ClassifiedResponse::RequiresEos(classify_eos) => {
                        let span = this.span.clone();
                        let res = res.map(|body| ResponseBody {
                            inner: body,
                            classify_eos: Some(classify_eos),
                            on_eos: on_eos.zip(Some(Instant::now())),
                            on_body_chunk,
                            on_failure: Some(on_failure),
                            start,
                            span,
                        });

                        Poll::Ready(Ok(res))
                    }
                }
            }
            Err(err) => {
                let failure_class = classifier.classify_error(&err);
                on_failure.on_failure(failure_class, latency, this.span);

                Poll::Ready(Err(err))
            }
        }
    }
}

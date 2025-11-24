use super::{
    DefaultMakeSpan, DefaultOnBodyChunk, DefaultOnEos, DefaultOnFailure, DefaultOnRequest,
    DefaultOnResponse, GrpcMakeClassifier, HttpMakeClassifier, Trace,
};
use crate::classify::{
    GrpcErrorsAsFailures, MakeClassifier, ServerErrorsAsFailures, SharedClassifier,
};
use tower_layer::Layer;

/// [`Layer`] that adds high level [tracing] to a [`Service`].
///
/// See the [module docs](crate::trace) for more details.
///
/// [`Layer`]: tower_layer::Layer
/// [tracing]: https://crates.io/crates/tracing
/// [`Service`]: tower_service::Service
#[derive(Debug, Copy, Clone)]
pub struct TraceLayer<
    M,
    MakeSpan = DefaultMakeSpan,
    OnRequest = DefaultOnRequest,
    OnResponse = DefaultOnResponse,
    OnBodyChunk = DefaultOnBodyChunk,
    OnEos = DefaultOnEos,
    OnFailure = DefaultOnFailure,
> {
    pub(crate) make_classifier: M,
    pub(crate) make_span: MakeSpan,
    pub(crate) on_request: OnRequest,
    pub(crate) on_response: OnResponse,
    pub(crate) on_body_chunk: OnBodyChunk,
    pub(crate) on_eos: OnEos,
    pub(crate) on_failure: OnFailure,
}

impl<M> TraceLayer<M> {
    /// Create a new [`TraceLayer`] using the given [`MakeClassifier`].
    pub fn new(make_classifier: M) -> Self
    where
        M: MakeClassifier,
    {
        Self {
            make_classifier,
            make_span: DefaultMakeSpan::new(),
            on_failure: DefaultOnFailure::default(),
            on_request: DefaultOnRequest::default(),
            on_eos: DefaultOnEos::default(),
            on_body_chunk: DefaultOnBodyChunk::default(),
            on_response: DefaultOnResponse::default(),
        }
    }
}

impl<M, MakeSpan, OnRequest, OnResponse, OnBodyChunk, OnEos, OnFailure>
    TraceLayer<M, MakeSpan, OnRequest, OnResponse, OnBodyChunk, OnEos, OnFailure>
{
    /// Customize what to do when a request is received.
    ///
    /// `NewOnRequest` is expected to implement [`OnRequest`].
    ///
    /// [`OnRequest`]: super::OnRequest
    pub fn on_request<NewOnRequest>(
        self,
        new_on_request: NewOnRequest,
    ) -> TraceLayer<M, MakeSpan, NewOnRequest, OnResponse, OnBodyChunk, OnEos, OnFailure> {
        TraceLayer {
            on_request: new_on_request,
            on_failure: self.on_failure,
            on_eos: self.on_eos,
            on_body_chunk: self.on_body_chunk,
            make_span: self.make_span,
            on_response: self.on_response,
            make_classifier: self.make_classifier,
        }
    }

    /// Customize what to do when a response has been produced.
    ///
    /// `NewOnResponse` is expected to implement [`OnResponse`].
    ///
    /// [`OnResponse`]: super::OnResponse
    pub fn on_response<NewOnResponse>(
        self,
        new_on_response: NewOnResponse,
    ) -> TraceLayer<M, MakeSpan, OnRequest, NewOnResponse, OnBodyChunk, OnEos, OnFailure> {
        TraceLayer {
            on_response: new_on_response,
            on_request: self.on_request,
            on_eos: self.on_eos,
            on_body_chunk: self.on_body_chunk,
            on_failure: self.on_failure,
            make_span: self.make_span,
            make_classifier: self.make_classifier,
        }
    }

    /// Customize what to do when a body chunk has been sent.
    ///
    /// `NewOnBodyChunk` is expected to implement [`OnBodyChunk`].
    ///
    /// [`OnBodyChunk`]: super::OnBodyChunk
    pub fn on_body_chunk<NewOnBodyChunk>(
        self,
        new_on_body_chunk: NewOnBodyChunk,
    ) -> TraceLayer<M, MakeSpan, OnRequest, OnResponse, NewOnBodyChunk, OnEos, OnFailure> {
        TraceLayer {
            on_body_chunk: new_on_body_chunk,
            on_eos: self.on_eos,
            on_failure: self.on_failure,
            on_request: self.on_request,
            make_span: self.make_span,
            on_response: self.on_response,
            make_classifier: self.make_classifier,
        }
    }

    /// Customize what to do when a streaming response has closed.
    ///
    /// `NewOnEos` is expected to implement [`OnEos`].
    ///
    /// [`OnEos`]: super::OnEos
    pub fn on_eos<NewOnEos>(
        self,
        new_on_eos: NewOnEos,
    ) -> TraceLayer<M, MakeSpan, OnRequest, OnResponse, OnBodyChunk, NewOnEos, OnFailure> {
        TraceLayer {
            on_eos: new_on_eos,
            on_body_chunk: self.on_body_chunk,
            on_failure: self.on_failure,
            on_request: self.on_request,
            make_span: self.make_span,
            on_response: self.on_response,
            make_classifier: self.make_classifier,
        }
    }

    /// Customize what to do when a response has been classified as a failure.
    ///
    /// `NewOnFailure` is expected to implement [`OnFailure`].
    ///
    /// [`OnFailure`]: super::OnFailure
    pub fn on_failure<NewOnFailure>(
        self,
        new_on_failure: NewOnFailure,
    ) -> TraceLayer<M, MakeSpan, OnRequest, OnResponse, OnBodyChunk, OnEos, NewOnFailure> {
        TraceLayer {
            on_failure: new_on_failure,
            on_request: self.on_request,
            on_eos: self.on_eos,
            on_body_chunk: self.on_body_chunk,
            make_span: self.make_span,
            on_response: self.on_response,
            make_classifier: self.make_classifier,
        }
    }

    /// Customize how to make [`Span`]s that all request handling will be wrapped in.
    ///
    /// `NewMakeSpan` is expected to implement [`MakeSpan`].
    ///
    /// [`MakeSpan`]: super::MakeSpan
    /// [`Span`]: tracing::Span
    pub fn make_span_with<NewMakeSpan>(
        self,
        new_make_span: NewMakeSpan,
    ) -> TraceLayer<M, NewMakeSpan, OnRequest, OnResponse, OnBodyChunk, OnEos, OnFailure> {
        TraceLayer {
            make_span: new_make_span,
            on_request: self.on_request,
            on_failure: self.on_failure,
            on_body_chunk: self.on_body_chunk,
            on_eos: self.on_eos,
            on_response: self.on_response,
            make_classifier: self.make_classifier,
        }
    }
}

impl TraceLayer<HttpMakeClassifier> {
    /// Create a new [`TraceLayer`] using [`ServerErrorsAsFailures`] which supports classifying
    /// regular HTTP responses based on the status code.
    pub fn new_for_http() -> Self {
        Self {
            make_classifier: SharedClassifier::new(ServerErrorsAsFailures::default()),
            make_span: DefaultMakeSpan::new(),
            on_response: DefaultOnResponse::default(),
            on_request: DefaultOnRequest::default(),
            on_body_chunk: DefaultOnBodyChunk::default(),
            on_eos: DefaultOnEos::default(),
            on_failure: DefaultOnFailure::default(),
        }
    }
}

impl TraceLayer<GrpcMakeClassifier> {
    /// Create a new [`TraceLayer`] using [`GrpcErrorsAsFailures`] which supports classifying
    /// gRPC responses and streams based on the `grpc-status` header.
    pub fn new_for_grpc() -> Self {
        Self {
            make_classifier: SharedClassifier::new(GrpcErrorsAsFailures::default()),
            make_span: DefaultMakeSpan::new(),
            on_response: DefaultOnResponse::default(),
            on_request: DefaultOnRequest::default(),
            on_body_chunk: DefaultOnBodyChunk::default(),
            on_eos: DefaultOnEos::default(),
            on_failure: DefaultOnFailure::default(),
        }
    }
}

impl<S, M, MakeSpan, OnRequest, OnResponse, OnBodyChunk, OnEos, OnFailure> Layer<S>
    for TraceLayer<M, MakeSpan, OnRequest, OnResponse, OnBodyChunk, OnEos, OnFailure>
where
    M: Clone,
    MakeSpan: Clone,
    OnRequest: Clone,
    OnResponse: Clone,
    OnEos: Clone,
    OnBodyChunk: Clone,
    OnFailure: Clone,
{
    type Service = Trace<S, M, MakeSpan, OnRequest, OnResponse, OnBodyChunk, OnEos, OnFailure>;

    fn layer(&self, inner: S) -> Self::Service {
        Trace {
            inner,
            make_classifier: self.make_classifier.clone(),
            make_span: self.make_span.clone(),
            on_request: self.on_request.clone(),
            on_eos: self.on_eos.clone(),
            on_body_chunk: self.on_body_chunk.clone(),
            on_response: self.on_response.clone(),
            on_failure: self.on_failure.clone(),
        }
    }
}

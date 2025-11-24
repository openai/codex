//! HTTP specific body utilities.

use std::{pin::Pin, task::Poll};

use http_body_util::BodyExt as _;

// A type erased HTTP body.
type BoxBody = http_body_util::combinators::UnsyncBoxBody<bytes::Bytes, crate::Status>;

/// A body type used in `tonic`.
#[derive(Debug)]
pub struct Body {
    kind: Kind,
}

#[derive(Debug)]
enum Kind {
    Empty,
    Wrap(BoxBody),
}

impl Body {
    fn from_kind(kind: Kind) -> Self {
        Self { kind }
    }

    /// Create a new empty `Body`.
    pub const fn empty() -> Self {
        Self { kind: Kind::Empty }
    }

    /// Create a new `Body` from an existing `Body`.
    pub fn new<B>(body: B) -> Self
    where
        B: http_body::Body<Data = bytes::Bytes> + Send + 'static,
        B::Error: Into<crate::BoxError>,
    {
        if body.is_end_stream() {
            return Self::empty();
        }

        let mut body = Some(body);

        if let Some(body) = <dyn std::any::Any>::downcast_mut::<Option<Body>>(&mut body) {
            return body.take().unwrap();
        }

        if let Some(body) = <dyn std::any::Any>::downcast_mut::<Option<BoxBody>>(&mut body) {
            return Self::from_kind(Kind::Wrap(body.take().unwrap()));
        }

        let body = body
            .unwrap()
            .map_err(crate::Status::map_error)
            .boxed_unsync();

        Self::from_kind(Kind::Wrap(body))
    }
}

impl Default for Body {
    fn default() -> Self {
        Self::empty()
    }
}

impl http_body::Body for Body {
    type Data = bytes::Bytes;
    type Error = crate::Status;

    fn poll_frame(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        match &mut self.kind {
            Kind::Empty => Poll::Ready(None),
            Kind::Wrap(body) => Pin::new(body).poll_frame(cx),
        }
    }

    fn size_hint(&self) -> http_body::SizeHint {
        match &self.kind {
            Kind::Empty => http_body::SizeHint::with_exact(0),
            Kind::Wrap(body) => body.size_hint(),
        }
    }

    fn is_end_stream(&self) -> bool {
        match &self.kind {
            Kind::Empty => true,
            Kind::Wrap(body) => body.is_end_stream(),
        }
    }
}

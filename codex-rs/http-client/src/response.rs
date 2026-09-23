//! HTTP responses retain application authorization while their bodies are consumed.

use std::ops::Deref;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use bytes::Bytes;
use futures::FutureExt;
use futures::Stream;
use futures::StreamExt;
use futures::future::BoxFuture;
use futures::stream;
use serde::de::DeserializeOwned;

use crate::HttpError;
use crate::NetworkPermit;
use crate::NetworkPolicy;

#[derive(Debug)]
pub struct HttpResponse {
    inner: reqwest::Response,
    permit: NetworkPermit,
}

impl HttpResponse {
    /// Preserves HTTP frames, including gRPC trailers, while enforcing revocation.
    pub fn into_http_response(
        self,
    ) -> http::Response<impl http_body::Body<Data = Bytes, Error = HttpError> + Send> {
        let response: http::Response<reqwest::Body> = self.inner.into();
        let revoked = self.permit.clone();
        response.map(|body| PolicyBody {
            body: Some(Box::pin(body)),
            permit: self.permit,
            revoked: async move { revoked.revoked().await }.boxed(),
        })
    }
    pub(crate) fn new(inner: reqwest::Response, permit: NetworkPermit) -> Self {
        Self { inner, permit }
    }

    pub async fn bytes(self) -> Result<Bytes, HttpError> {
        Ok(Box::pin(self.permit.run(self.inner.bytes())).await??)
    }

    pub async fn text(self) -> Result<String, HttpError> {
        Ok(Box::pin(self.permit.run(self.inner.text())).await??)
    }

    pub async fn json<T: DeserializeOwned>(self) -> Result<T, HttpError> {
        Ok(Box::pin(self.permit.run(self.inner.json())).await??)
    }

    pub async fn chunk(&mut self) -> Result<Option<Bytes>, HttpError> {
        Ok(Box::pin(self.permit.run(self.inner.chunk())).await??)
    }

    pub fn error_for_status(self) -> Result<Self, HttpError> {
        self.permit.check()?;
        Ok(Self {
            inner: self.inner.error_for_status()?,
            permit: self.permit,
        })
    }

    pub fn error_for_status_ref(&self) -> Result<&Self, HttpError> {
        self.permit.check()?;
        self.inner.error_for_status_ref()?;
        Ok(self)
    }

    pub fn bytes_stream(self) -> impl Stream<Item = Result<Bytes, HttpError>> + Send + Unpin {
        Box::pin(stream::try_unfold(
            (Box::pin(self.inner.bytes_stream()), self.permit),
            |(mut body, permit)| async move {
                let next = permit.run(body.next()).await?;
                next.transpose()
                    .map(|next| next.map(|bytes| (bytes, (body, permit))))
                    .map_err(HttpError::from)
            },
        ))
    }
}

struct PolicyBody<B> {
    body: Option<Pin<Box<B>>>,
    permit: NetworkPermit,
    revoked: BoxFuture<'static, ()>,
}

impl<B: http_body::Body<Data = Bytes, Error = reqwest::Error>> http_body::Body for PolicyBody<B> {
    type Data = Bytes;
    type Error = HttpError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Bytes>, HttpError>>> {
        let body = self.get_mut();
        let Some(inner) = body.body.as_mut() else {
            return Poll::Ready(None);
        };
        if body.permit.check().is_err() || body.revoked.poll_unpin(cx).is_ready() {
            body.body = None;
            return Poll::Ready(Some(Err(crate::NetworkPolicyDenied::Revoked.into())));
        }
        inner
            .as_mut()
            .poll_frame(cx)
            .map(|frame| frame.map(|frame| frame.map_err(HttpError::from)))
    }

    fn is_end_stream(&self) -> bool {
        self.body
            .as_ref()
            .is_none_or(http_body::Body::is_end_stream)
    }
    fn size_hint(&self) -> http_body::SizeHint {
        self.body
            .as_ref()
            .map_or_else(http_body::SizeHint::default, http_body::Body::size_hint)
    }
}

impl Deref for HttpResponse {
    type Target = reqwest::Response;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl From<reqwest::Response> for HttpResponse {
    fn from(inner: reqwest::Response) -> Self {
        let permit = NetworkPolicy::unmanaged()
            .acquire(inner.url())
            .unwrap_or_else(|_| unreachable!("unmanaged policy permits every URL"));
        Self { inner, permit }
    }
}

impl<T: Into<reqwest::Body>> From<http::Response<T>> for HttpResponse {
    fn from(response: http::Response<T>) -> Self {
        reqwest::Response::from(response).into()
    }
}

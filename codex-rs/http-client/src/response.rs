//! HTTP responses retain application authorization while their bodies are consumed.

use std::ops::Deref;

use bytes::Bytes;
use futures::Stream;
use futures::StreamExt;
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

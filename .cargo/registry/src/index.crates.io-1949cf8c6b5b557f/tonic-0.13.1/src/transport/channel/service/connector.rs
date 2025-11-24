use super::BoxedIo;
#[cfg(feature = "_tls-any")]
use super::TlsConnector;
use crate::transport::channel::BoxFuture;
use crate::ConnectError;
use http::Uri;
#[cfg(feature = "_tls-any")]
use std::fmt;
use std::task::{Context, Poll};

use hyper::rt;

#[cfg(feature = "_tls-any")]
use hyper_util::rt::TokioIo;
use tower_service::Service;

pub(crate) struct Connector<C> {
    inner: C,
    #[cfg(feature = "_tls-any")]
    tls: Option<TlsConnector>,
}

impl<C> Connector<C> {
    pub(crate) fn new(inner: C, #[cfg(feature = "_tls-any")] tls: Option<TlsConnector>) -> Self {
        Self {
            inner,
            #[cfg(feature = "_tls-any")]
            tls,
        }
    }
}

impl<C> Service<Uri> for Connector<C>
where
    C: Service<Uri>,
    C::Response: rt::Read + rt::Write + Unpin + Send + 'static,
    C::Future: Send + 'static,
    crate::BoxError: From<C::Error> + Send + 'static,
{
    type Response = BoxedIo;
    type Error = ConnectError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner
            .poll_ready(cx)
            .map_err(|err| ConnectError(From::from(err)))
    }

    fn call(&mut self, uri: Uri) -> Self::Future {
        #[cfg(feature = "_tls-any")]
        let tls = self.tls.clone();

        #[cfg(feature = "_tls-any")]
        let is_https = uri.scheme_str() == Some("https");
        let connect = self.inner.call(uri);

        Box::pin(async move {
            async {
                let io = connect.await?;

                #[cfg(feature = "_tls-any")]
                if is_https {
                    return if let Some(tls) = tls {
                        let io = tls.connect(TokioIo::new(io)).await?;
                        Ok(io)
                    } else {
                        Err(HttpsUriWithoutTlsSupport(()).into())
                    };
                }

                Ok::<_, crate::BoxError>(BoxedIo::new(io))
            }
            .await
            .map_err(ConnectError)
        })
    }
}

/// Error returned when trying to connect to an HTTPS endpoint without TLS enabled.
#[cfg(feature = "_tls-any")]
#[derive(Debug)]
pub(crate) struct HttpsUriWithoutTlsSupport(());

#[cfg(feature = "_tls-any")]
impl fmt::Display for HttpsUriWithoutTlsSupport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Connecting to HTTPS without TLS enabled")
    }
}

// std::error::Error only requires a type to impl Debug and Display
#[cfg(feature = "_tls-any")]
impl std::error::Error for HttpsUriWithoutTlsSupport {}

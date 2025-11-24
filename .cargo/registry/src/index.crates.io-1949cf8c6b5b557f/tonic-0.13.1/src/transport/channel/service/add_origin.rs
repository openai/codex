use crate::transport::channel::BoxFuture;
use http::uri::Authority;
use http::uri::Scheme;
use http::{Request, Uri};
use std::task::{Context, Poll};
use tower_service::Service;

#[derive(Debug)]
pub(crate) struct AddOrigin<T> {
    inner: T,
    scheme: Option<Scheme>,
    authority: Option<Authority>,
}

impl<T> AddOrigin<T> {
    pub(crate) fn new(inner: T, origin: Uri) -> Self {
        let http::uri::Parts {
            scheme, authority, ..
        } = origin.into_parts();

        Self {
            inner,
            scheme,
            authority,
        }
    }
}

impl<T, ReqBody> Service<Request<ReqBody>> for AddOrigin<T>
where
    T: Service<Request<ReqBody>>,
    T::Future: Send + 'static,
    T::Error: Into<crate::BoxError>,
{
    type Response = T::Response;
    type Error = crate::BoxError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        if self.scheme.is_none() || self.authority.is_none() {
            let err = crate::transport::Error::new_invalid_uri();
            return Box::pin(async move { Err::<Self::Response, _>(err.into()) });
        }

        // Split the request into the head and the body.
        let (mut head, body) = req.into_parts();

        // Update the request URI
        head.uri = {
            // Split the request URI into parts.
            let mut uri: http::uri::Parts = head.uri.into();
            // Update the URI parts, setting the scheme and authority
            uri.scheme = self.scheme.clone();
            uri.authority = self.authority.clone();

            http::Uri::from_parts(uri).expect("valid uri")
        };

        let request = Request::from_parts(head, body);

        let fut = self.inner.call(request);

        Box::pin(async move { fut.await.map_err(Into::into) })
    }
}

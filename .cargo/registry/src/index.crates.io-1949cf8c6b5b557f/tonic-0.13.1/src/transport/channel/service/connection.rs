use super::{AddOrigin, Reconnect, SharedExec, UserAgent};
use crate::{
    body::Body,
    transport::{channel::BoxFuture, service::GrpcTimeout, Endpoint},
};
use http::{Request, Response, Uri};
use hyper::rt;
use hyper::{client::conn::http2::Builder, rt::Executor};
use hyper_util::rt::TokioTimer;
use std::{
    fmt,
    task::{Context, Poll},
};
use tower::load::Load;
use tower::{
    layer::Layer,
    limit::{concurrency::ConcurrencyLimitLayer, rate::RateLimitLayer},
    util::BoxService,
    ServiceBuilder, ServiceExt,
};
use tower_service::Service;

pub(crate) struct Connection {
    inner: BoxService<Request<Body>, Response<Body>, crate::BoxError>,
}

impl Connection {
    fn new<C>(connector: C, endpoint: Endpoint, is_lazy: bool) -> Self
    where
        C: Service<Uri> + Send + 'static,
        C::Error: Into<crate::BoxError> + Send,
        C::Future: Send,
        C::Response: rt::Read + rt::Write + Unpin + Send + 'static,
    {
        let mut settings: Builder<SharedExec> = Builder::new(endpoint.executor.clone())
            .initial_stream_window_size(endpoint.init_stream_window_size)
            .initial_connection_window_size(endpoint.init_connection_window_size)
            .keep_alive_interval(endpoint.http2_keep_alive_interval)
            .timer(TokioTimer::new())
            .clone();

        if let Some(val) = endpoint.http2_keep_alive_timeout {
            settings.keep_alive_timeout(val);
        }

        if let Some(val) = endpoint.http2_keep_alive_while_idle {
            settings.keep_alive_while_idle(val);
        }

        if let Some(val) = endpoint.http2_adaptive_window {
            settings.adaptive_window(val);
        }

        if let Some(val) = endpoint.http2_max_header_list_size {
            settings.max_header_list_size(val);
        }

        let stack = ServiceBuilder::new()
            .layer_fn(|s| {
                let origin = endpoint.origin.as_ref().unwrap_or(endpoint.uri()).clone();

                AddOrigin::new(s, origin)
            })
            .layer_fn(|s| UserAgent::new(s, endpoint.user_agent.clone()))
            .layer_fn(|s| GrpcTimeout::new(s, endpoint.timeout))
            .option_layer(endpoint.concurrency_limit.map(ConcurrencyLimitLayer::new))
            .option_layer(endpoint.rate_limit.map(|(l, d)| RateLimitLayer::new(l, d)))
            .into_inner();

        let make_service =
            MakeSendRequestService::new(connector, endpoint.executor.clone(), settings);

        let conn = Reconnect::new(make_service, endpoint.uri().clone(), is_lazy);

        Self {
            inner: BoxService::new(stack.layer(conn)),
        }
    }

    pub(crate) async fn connect<C>(
        connector: C,
        endpoint: Endpoint,
    ) -> Result<Self, crate::BoxError>
    where
        C: Service<Uri> + Send + 'static,
        C::Error: Into<crate::BoxError> + Send,
        C::Future: Unpin + Send,
        C::Response: rt::Read + rt::Write + Unpin + Send + 'static,
    {
        Self::new(connector, endpoint, false).ready_oneshot().await
    }

    pub(crate) fn lazy<C>(connector: C, endpoint: Endpoint) -> Self
    where
        C: Service<Uri> + Send + 'static,
        C::Error: Into<crate::BoxError> + Send,
        C::Future: Send,
        C::Response: rt::Read + rt::Write + Unpin + Send + 'static,
    {
        Self::new(connector, endpoint, true)
    }
}

impl Service<Request<Body>> for Connection {
    type Response = Response<Body>;
    type Error = crate::BoxError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Service::poll_ready(&mut self.inner, cx).map_err(Into::into)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        self.inner.call(req)
    }
}

impl Load for Connection {
    type Metric = usize;

    fn load(&self) -> Self::Metric {
        0
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection").finish()
    }
}

struct SendRequest {
    inner: hyper::client::conn::http2::SendRequest<Body>,
}

impl From<hyper::client::conn::http2::SendRequest<Body>> for SendRequest {
    fn from(inner: hyper::client::conn::http2::SendRequest<Body>) -> Self {
        Self { inner }
    }
}

impl tower::Service<Request<Body>> for SendRequest {
    type Response = Response<Body>;
    type Error = crate::BoxError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let fut = self.inner.send_request(req);

        Box::pin(async move { fut.await.map_err(Into::into).map(|res| res.map(Body::new)) })
    }
}

struct MakeSendRequestService<C> {
    connector: C,
    executor: SharedExec,
    settings: Builder<SharedExec>,
}

impl<C> MakeSendRequestService<C> {
    fn new(connector: C, executor: SharedExec, settings: Builder<SharedExec>) -> Self {
        Self {
            connector,
            executor,
            settings,
        }
    }
}

impl<C> tower::Service<Uri> for MakeSendRequestService<C>
where
    C: Service<Uri> + Send + 'static,
    C::Error: Into<crate::BoxError> + Send,
    C::Future: Send,
    C::Response: rt::Read + rt::Write + Unpin + Send,
{
    type Response = SendRequest;
    type Error = crate::BoxError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.connector.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: Uri) -> Self::Future {
        let fut = self.connector.call(req);
        let builder = self.settings.clone();
        let executor = self.executor.clone();

        Box::pin(async move {
            let io = fut.await.map_err(Into::into)?;
            let (send_request, conn) = builder.handshake(io).await?;

            Executor::<BoxFuture<'static, ()>>::execute(
                &executor,
                Box::pin(async move {
                    if let Err(e) = conn.await {
                        tracing::debug!("connection task error: {:?}", e);
                    }
                }) as _,
            );

            Ok(SendRequest::from(send_request))
        })
    }
}

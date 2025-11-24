#![cfg(feature = "http3")]

pub(crate) mod connect;
pub(crate) mod dns;
mod pool;

use crate::async_impl::body::ResponseBody;
use crate::async_impl::h3_client::pool::{Key, Pool, PoolClient};
#[cfg(feature = "cookies")]
use crate::cookie;
use crate::error::{BoxError, Error, Kind};
use crate::{error, Body};
use connect::H3Connector;
use http::{Request, Response};
use log::trace;
use std::future::{self, Future};
use std::pin::Pin;
#[cfg(feature = "cookies")]
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use sync_wrapper::SyncWrapper;
use tower::Service;

#[derive(Clone)]
pub(crate) struct H3Client {
    pool: Pool,
    connector: H3Connector,
    #[cfg(feature = "cookies")]
    cookie_store: Option<Arc<dyn cookie::CookieStore>>,
}

impl H3Client {
    #[cfg(not(feature = "cookies"))]
    pub fn new(connector: H3Connector, pool_timeout: Option<Duration>) -> Self {
        H3Client {
            pool: Pool::new(pool_timeout),
            connector,
        }
    }

    #[cfg(feature = "cookies")]
    pub fn new(
        connector: H3Connector,
        pool_timeout: Option<Duration>,
        cookie_store: Option<Arc<dyn cookie::CookieStore>>,
    ) -> Self {
        H3Client {
            pool: Pool::new(pool_timeout),
            connector,
            cookie_store,
        }
    }

    async fn get_pooled_client(&mut self, key: Key) -> Result<PoolClient, BoxError> {
        if let Some(client) = self.pool.try_pool(&key) {
            trace!("getting client from pool with key {key:?}");
            return Ok(client);
        }

        trace!("did not find connection {key:?} in pool so connecting...");

        let dest = pool::domain_as_uri(key.clone());

        let lock = match self.pool.connecting(&key) {
            pool::Connecting::InProgress(waiter) => {
                trace!("connecting to {key:?} is already in progress, subscribing...");

                match waiter.receive().await {
                    Some(client) => return Ok(client),
                    None => return Err("failed to establish connection for HTTP/3 request".into()),
                }
            }
            pool::Connecting::Acquired(lock) => lock,
        };
        trace!("connecting to {key:?}...");
        let (driver, tx) = self.connector.connect(dest).await?;
        trace!("saving new pooled connection to {key:?}");
        Ok(self.pool.new_connection(lock, driver, tx))
    }

    #[cfg(not(feature = "cookies"))]
    async fn send_request(
        mut self,
        key: Key,
        req: Request<Body>,
    ) -> Result<Response<ResponseBody>, Error> {
        let mut pooled = match self.get_pooled_client(key).await {
            Ok(client) => client,
            Err(e) => return Err(error::request(e)),
        };
        pooled
            .send_request(req)
            .await
            .map_err(|e| Error::new(Kind::Request, Some(e)))
    }

    #[cfg(feature = "cookies")]
    async fn send_request(
        mut self,
        key: Key,
        mut req: Request<Body>,
    ) -> Result<Response<ResponseBody>, Error> {
        let mut pooled = match self.get_pooled_client(key).await {
            Ok(client) => client,
            Err(e) => return Err(error::request(e)),
        };

        let url = url::Url::parse(req.uri().to_string().as_str()).unwrap();
        if let Some(cookie_store) = self.cookie_store.as_ref() {
            if req.headers().get(crate::header::COOKIE).is_none() {
                let headers = req.headers_mut();
                crate::util::add_cookie_header(headers, &**cookie_store, &url);
            }
        }

        let res = pooled
            .send_request(req)
            .await
            .map_err(|e| Error::new(Kind::Request, Some(e)));

        if let Some(ref cookie_store) = self.cookie_store {
            if let Ok(res) = &res {
                let mut cookies = cookie::extract_response_cookie_headers(res.headers()).peekable();
                if cookies.peek().is_some() {
                    cookie_store.set_cookies(&mut cookies, &url);
                }
            }
        }

        res
    }

    pub fn request(&self, mut req: Request<Body>) -> H3ResponseFuture {
        let pool_key = match pool::extract_domain(req.uri_mut()) {
            Ok(s) => s,
            Err(e) => {
                return H3ResponseFuture {
                    inner: SyncWrapper::new(Box::pin(future::ready(Err(e)))),
                }
            }
        };
        H3ResponseFuture {
            inner: SyncWrapper::new(Box::pin(self.clone().send_request(pool_key, req))),
        }
    }
}

impl Service<Request<Body>> for H3Client {
    type Response = Response<ResponseBody>;
    type Error = Error;
    type Future = H3ResponseFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        self.request(req)
    }
}

pub(crate) struct H3ResponseFuture {
    inner: SyncWrapper<Pin<Box<dyn Future<Output = Result<Response<ResponseBody>, Error>> + Send>>>,
}

impl Future for H3ResponseFuture {
    type Output = Result<Response<ResponseBody>, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.inner.get_mut().as_mut().poll(cx)
    }
}

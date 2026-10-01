//! Native gRPC client for the Codex cloud ThreadService, using shared HTTP transport policy.
//!
//! Credentials and account selection belong to the caller. Requests are never retried:
//! a timeout or disconnect can leave Resume admitted. Attach delivers live events only;
//! dropping its stream detaches without interrupting the thread or answering approvals.

mod types;
mod wire;

pub use types::Credentials;
pub use types::Error;
pub use types::Event;
pub use types::ResumeRequest;
pub use types::RpcStatus;

use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientBuilder;
use codex_http_client::HttpClientFactory;
use codex_http_client::RouteAwareClientPool;
use futures::StreamExt;
use futures::stream::BoxStream;
use http_body_util::BodyExt;
use std::io;
use std::time::Duration;
use tonic::body::Body;
use tonic::client::Grpc;
use tonic_prost::ProstCodec;
use tower::service_fn;
use tower::util::BoxCloneSyncService;
use url::Url;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(150);
const MAX_MESSAGE_BYTES: usize = 64 * 1024 * 1024;
type Transport = BoxCloneSyncService<http::Request<Body>, http::Response<Body>, io::Error>;

/// One fixed origin and account. Construct a new client when credentials change.
/// Supply a trusted native gRPC origin, without a path (not the HTTP `/grpc` gateway).
pub struct Client {
    grpc: Grpc<Transport>,
    credentials: Credentials,
}

impl Client {
    pub fn new(
        factory: &HttpClientFactory,
        endpoint: &str,
        credentials: Credentials,
    ) -> Result<Self, Error> {
        let endpoint = Url::parse(endpoint).map_err(|_| Error::InvalidEndpoint)?;
        let loopback = match endpoint.host() {
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            Some(url::Host::Domain(name)) => name == "localhost",
            None => false,
        };
        if (endpoint.scheme() != "https" && !(endpoint.scheme() == "http" && loopback))
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
            || endpoint.path() != "/"
        {
            return Err(Error::InvalidEndpoint);
        }
        let local_http = endpoint.scheme() == "http";
        if local_http
            && (factory.network_policy().is_managed()
                || factory
                    .network_policy()
                    .acquire_for_unsupported_sdk()
                    .is_err())
        {
            return Err(Error::InvalidEndpoint);
        }
        let builder = HttpClientBuilder::new()
            .without_redirects()
            .without_request_logging()
            .http2_prior_knowledge()
            .connect_timeout(Duration::from_secs(30));
        let http = if local_http {
            // Plaintext credentials must never leave loopback through an outbound proxy.
            builder.build_direct().map_err(|_| Error::ClientBuild)?
        } else {
            RouteAwareClientPool::with_builder(factory.clone(), ClientRouteClass::Api, builder)
                .into_client()
        };
        let origin = endpoint
            .as_str()
            .parse()
            .map_err(|_| Error::InvalidEndpoint)?;
        let transport = service_fn(move |request: http::Request<Body>| {
            let http = http.clone();
            async move {
                let (parts, body) = request.into_parts();
                let response = http
                    .request(parts.method, parts.uri.to_string())
                    .version(parts.version)
                    .headers(parts.headers)
                    .body_stream(body.into_data_stream())
                    .send()
                    .await
                    .map_err(|_| io::Error::other("gRPC transport failed"))?;
                Ok(response.into_http_response().map(Body::new))
            }
        });
        Ok(Self {
            grpc: Grpc::with_origin(BoxCloneSyncService::new(transport), origin)
                .max_decoding_message_size(MAX_MESSAGE_BYTES)
                .max_encoding_message_size(MAX_MESSAGE_BYTES),
            credentials,
        })
    }

    /// With `wait: false`, success means admission only. Failures may leave work admitted.
    pub async fn resume(&self, request: &ResumeRequest) -> Result<(), Error> {
        let mut grpc = self.grpc.clone();
        let mut request = tonic::Request::new(request.clone());
        *request.metadata_mut() =
            tonic::metadata::MetadataMap::from_headers(self.credentials.headers.clone());
        // Bound the whole unary RPC, including its response body and trailers.
        tokio::time::timeout(
            REQUEST_TIMEOUT,
            grpc.unary(
                request,
                http::uri::PathAndQuery::from_static(
                    "/oaiproto.codex_app_server_backend.ThreadService/Resume",
                ),
                ProstCodec::<ResumeRequest, wire::ResumeResponse>::default(),
            ),
        )
        .await
        .map_err(|_| Error::Timeout)?
        .map_err(Error::from)?;
        Ok(())
    }

    /// Live events only, without history or automatic reconnect. The stream terminates
    /// on its first error. Setup is bounded; there is no idle or total stream deadline.
    pub async fn attach(
        &self,
        thread_id: &str,
    ) -> Result<BoxStream<'static, Result<Event, Error>>, Error> {
        let mut grpc = self.grpc.clone();
        let mut request = tonic::Request::new(wire::AttachRequest {
            thread_id: thread_id.to_owned(),
        });
        *request.metadata_mut() =
            tonic::metadata::MetadataMap::from_headers(self.credentials.headers.clone());
        let response = tokio::time::timeout(
            REQUEST_TIMEOUT,
            grpc.server_streaming(
                request,
                http::uri::PathAndQuery::from_static(
                    "/oaiproto.codex_app_server_backend.ThreadService/Attach",
                ),
                ProstCodec::<wire::AttachRequest, wire::AttachResponse>::default(),
            ),
        )
        .await
        .map_err(|_| Error::Timeout)?
        .map_err(Error::from)?;
        let mut stream = response.into_inner();
        Ok(async_stream::try_stream! {
            while let Some(message) = stream.message().await.map_err(Error::from)? {
                yield match message.event.ok_or(Error::InvalidResponse)? {
                    wire::Payload::Notification(bytes) => Event::Notification(bytes),
                    wire::Payload::ServerRequest(bytes) => Event::ServerRequest(bytes),
                };
            }
        }
        .boxed())
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;

use std::sync::Arc;
use std::sync::Weak;

use codex_code_mode_protocol::grpc::code_mode_host_client::CodeModeHostClient;
use codex_code_mode_protocol::host::MAX_FRAME_BYTES;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use http_body_util::BodyExt;
use reqwest::StatusCode;
use tonic::Status;
use tonic::body::Body;
use tonic::codegen::Bytes;
use tonic::codegen::http::Request;
use tonic::codegen::http::Response;
use tonic::codegen::http::Uri;
use tonic::transport::Channel;
use tonic::transport::Endpoint;
use tower::ServiceExt;
use tower::service_fn;
use tower::util::BoxCloneSyncService;

use super::GrpcClient;
use super::SessionAdmission;

pub(super) type GrpcTransport = BoxCloneSyncService<Request<Body>, Response<Body>, Status>;

pub(super) enum SharedTransport {
    Url {
        endpoint: String,
        http_client_factory: HttpClientFactory,
        client: tokio::sync::Mutex<Option<(HttpClientFactory, Weak<GrpcClient>)>>,
    },
    Connected(Arc<GrpcClient>),
}

impl SharedTransport {
    pub(super) fn new(endpoint: String, http_client_factory: HttpClientFactory) -> Self {
        Self::Url {
            endpoint,
            http_client_factory,
            client: tokio::sync::Mutex::new(None),
        }
    }

    pub(super) fn with_channel(channel: Channel) -> Self {
        let transport = channel.map_err(|error| Status::from_error(Box::new(error)));
        Self::Connected(Arc::new(
            CodeModeHostClient::new(BoxCloneSyncService::new(transport))
                .accept_compressed(tonic::codec::CompressionEncoding::Zstd)
                .max_decoding_message_size(MAX_FRAME_BYTES)
                .max_encoding_message_size(MAX_FRAME_BYTES),
        ))
    }

    pub(super) fn with_transport(transport: GrpcTransport) -> Self {
        Self::Connected(Arc::new(
            CodeModeHostClient::new(transport)
                .accept_compressed(tonic::codec::CompressionEncoding::Zstd)
                .max_decoding_message_size(MAX_FRAME_BYTES)
                .max_encoding_message_size(MAX_FRAME_BYTES),
        ))
    }

    pub(super) async fn client(&self) -> Result<Arc<GrpcClient>, String> {
        let (endpoint, http_client_factory, client) = match self {
            Self::Url {
                endpoint,
                http_client_factory,
                client,
            } => (endpoint, http_client_factory, client),
            Self::Connected(client) => return Ok(client.clone()),
        };
        let bound_factory = http_client_factory.clone().with_network_policy(
            http_client_factory
                .network_policy()
                .clone()
                .for_current_account(),
        );
        let mut cached = client.lock().await;
        if let Some((factory, client)) = cached.as_ref()
            && factory == &bound_factory
            && let Some(client) = client.upgrade()
        {
            return Ok(client);
        }
        let client = if endpoint.starts_with("unix:") {
            let channel = Endpoint::from_shared(endpoint.clone())
                .map_err(|error| format!("invalid gRPC code-mode Unix socket endpoint: {error}"))?
                .connect_lazy();
            let transport = channel.map_err(|error| Status::from_error(Box::new(error)));
            CodeModeHostClient::new(BoxCloneSyncService::new(transport))
        } else {
            let target = reqwest::Url::parse(endpoint)
                .map_err(|error| format!("invalid gRPC code-mode host URL: {error}"))?;
            if !matches!(target.scheme(), "http" | "https") {
                return Err("gRPC code-mode host URL must use http or https".to_string());
            }
            if !target.username().is_empty() || target.password().is_some() {
                return Err("gRPC code-mode host URL must not include credentials".to_string());
            }
            if target.path() != "/" || target.query().is_some() || target.fragment().is_some() {
                return Err(
                    "gRPC code-mode host URL must not include a path, query, or fragment"
                        .to_string(),
                );
            }
            let origin: Uri = endpoint
                .parse()
                .map_err(|error| format!("invalid gRPC code-mode host origin: {error}"))?;
            let client = codex_http_client::RouteAwareClientPool::with_builder(
                bound_factory.clone(),
                ClientRouteClass::Other,
                codex_http_client::HttpClientBuilder::new()
                    .without_request_logging()
                    // Some hosts assign a worker per connection. A quiet session
                    // must still reach the worker holding its open lease.
                    .without_pool_idle_timeout()
                    .http2_prior_knowledge()
                    .without_redirects(),
            );
            let transport = service_fn(move |request: Request<Body>| {
                let client = client.clone();
                async move {
                    let admission = request.extensions().get::<SessionAdmission>().is_some();
                    let request =
                        request.map(|body| reqwest::Body::wrap_stream(body.into_data_stream()));
                    let request = reqwest::Request::try_from(request)
                        .map_err(|error| Status::invalid_argument(error.to_string()))?;
                    let response = client
                        .execute(request)
                        .await
                        .map_err(|error| {
                            // Sending can fail on DNS or a reset connection before
                            // headers arrive. Keep these distinct from RPC errors.
                            let mut status = if matches!(
                                &error,
                                codex_http_client::HttpError::Request(error)
                                    if error.is_connect() || error.is_request()
                            ) {
                                Status::unavailable(error.to_string())
                            } else {
                                Status::unknown(error.to_string())
                            };
                            status.set_source(Arc::new(error));
                            status
                        })?
                        .into_http_response();
                    if admission && response.status() == StatusCode::SERVICE_UNAVAILABLE {
                        // A 503 cannot grant a lease. Leave headers and trailers for tonic
                        // to read the gRPC status, or infer Unavailable when it is absent.
                        return Ok(response.map(|body| {
                            Body::new(body.map_frame(|frame| frame.map_data(|_| Bytes::new())))
                        }));
                    }
                    Ok::<_, Status>(response.map(Body::new))
                }
            });
            CodeModeHostClient::with_origin(BoxCloneSyncService::new(transport), origin)
        };
        let client = client
            .accept_compressed(tonic::codec::CompressionEncoding::Zstd)
            .max_decoding_message_size(MAX_FRAME_BYTES)
            .max_encoding_message_size(MAX_FRAME_BYTES);
        // Let the last session release its worker even if the provider stays alive.
        let client = Arc::new(client);
        *cached = Some((bound_factory, Arc::downgrade(&client)));
        Ok(client)
    }
}

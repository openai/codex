use std::io;

use codex_code_mode_protocol::grpc::code_mode_host_client::CodeModeHostClient;
use codex_code_mode_protocol::host::MAX_FRAME_BYTES;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use http_body_util::BodyExt;
use tonic::body::Body;
use tonic::codegen::http::Request;
use tonic::codegen::http::Response;
use tonic::codegen::http::Uri;
use tonic::transport::Channel;
use tonic::transport::Endpoint;
use tower::ServiceExt;
use tower::service_fn;
use tower::util::BoxCloneSyncService;

use super::GrpcClient;

pub(super) type GrpcTransport = BoxCloneSyncService<Request<Body>, Response<Body>, io::Error>;

pub(super) enum SharedTransport {
    Url {
        endpoint: String,
        http_client_factory: HttpClientFactory,
        client: tokio::sync::Mutex<Option<(HttpClientFactory, GrpcClient)>>,
    },
    Connected(GrpcClient),
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
        let transport = channel.map_err(io::Error::other);
        Self::Connected(
            CodeModeHostClient::new(BoxCloneSyncService::new(transport))
                .max_decoding_message_size(MAX_FRAME_BYTES)
                .max_encoding_message_size(MAX_FRAME_BYTES),
        )
    }

    pub(super) async fn client(&self) -> Result<GrpcClient, String> {
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
        {
            return Ok(client.clone());
        }
        let client = if endpoint.starts_with("unix:") {
            let channel = Endpoint::from_shared(endpoint.clone())
                .map_err(|error| format!("invalid gRPC code-mode Unix socket endpoint: {error}"))?
                .connect_lazy();
            let transport = channel.map_err(io::Error::other);
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
                    .http2_prior_knowledge()
                    .without_redirects(),
            );
            let transport = service_fn(move |request: Request<Body>| {
                let client = client.clone();
                async move {
                    let request =
                        request.map(|body| reqwest::Body::wrap_stream(body.into_data_stream()));
                    let request = reqwest::Request::try_from(request).map_err(io::Error::other)?;
                    let response = client
                        .execute(request)
                        .await
                        .map_err(io::Error::other)?
                        .into_http_response();
                    Ok::<_, io::Error>(response.map(Body::new))
                }
            });
            CodeModeHostClient::with_origin(BoxCloneSyncService::new(transport), origin)
        };
        let client = client
            .max_decoding_message_size(MAX_FRAME_BYTES)
            .max_encoding_message_size(MAX_FRAME_BYTES);
        *cached = Some((bound_factory, client.clone()));
        Ok(client)
    }
}

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

pub(super) struct SharedTransport {
    endpoint: TransportEndpoint,
    client: tokio::sync::OnceCell<GrpcClient>,
}

enum TransportEndpoint {
    Url {
        endpoint: String,
        http_client_factory: HttpClientFactory,
    },
    Connected(Channel),
}

impl SharedTransport {
    pub(super) fn new(endpoint: String, http_client_factory: HttpClientFactory) -> Self {
        Self {
            endpoint: TransportEndpoint::Url {
                endpoint,
                http_client_factory,
            },
            client: tokio::sync::OnceCell::new(),
        }
    }

    pub(super) fn with_channel(channel: Channel) -> Self {
        Self {
            endpoint: TransportEndpoint::Connected(channel),
            client: tokio::sync::OnceCell::new(),
        }
    }

    pub(super) async fn client(&self) -> Result<GrpcClient, String> {
        self.client
            .get_or_try_init(|| async {
                let client = match &self.endpoint {
                    TransportEndpoint::Url { endpoint, .. } if endpoint.starts_with("unix:") => {
                        let channel = Endpoint::from_shared(endpoint.clone())
                            .map_err(|error| {
                                format!("invalid gRPC code-mode Unix socket endpoint: {error}")
                            })?
                            .connect_lazy();
                        let transport = channel.map_err(io::Error::other);
                        CodeModeHostClient::new(BoxCloneSyncService::new(transport))
                    }
                    TransportEndpoint::Url {
                        endpoint,
                        http_client_factory,
                    } => {
                        let target = reqwest::Url::parse(endpoint)
                            .map_err(|error| format!("invalid gRPC code-mode host URL: {error}"))?;
                        if !matches!(target.scheme(), "http" | "https") {
                            return Err("gRPC code-mode host URL must use http or https".to_string());
                        }
                        if !target.username().is_empty() || target.password().is_some() {
                            return Err(
                                "gRPC code-mode host URL must not include credentials".to_string(),
                            );
                        }
                        if target.path() != "/"
                            || target.query().is_some()
                            || target.fragment().is_some()
                        {
                            return Err("gRPC code-mode host URL must not include a path, query, or fragment".to_string());
                        }
                        let origin: Uri = endpoint
                            .parse()
                            .map_err(|error| format!("invalid gRPC code-mode host origin: {error}"))?;
                        let endpoint = endpoint.clone();
                        let http_client_factory = http_client_factory.clone();
                        let client = tokio::task::spawn_blocking(move || {
                            http_client_factory
                                .build_reqwest_client(
                                    reqwest::Client::builder()
                                        .http2_prior_knowledge()
                                        .redirect(reqwest::redirect::Policy::none()),
                                    &endpoint,
                                    ClientRouteClass::Other,
                                )
                                .map_err(|error| {
                                    format!(
                                        "failed to configure gRPC code-mode host transport: {error}"
                                    )
                                })
                        })
                        .await
                        .map_err(|error| {
                            format!("gRPC code-mode host transport task failed: {error}")
                        })??;
                        let transport = service_fn(move |request: Request<Body>| {
                            let client = client.clone();
                            async move {
                                let request = request.map(|body| {
                                    reqwest::Body::wrap_stream(body.into_data_stream())
                                });
                                let request =
                                    reqwest::Request::try_from(request).map_err(io::Error::other)?;
                                let response: Response<reqwest::Body> =
                                    client.execute(request).await.map_err(io::Error::other)?.into();
                                Ok::<_, io::Error>(response.map(Body::new))
                            }
                        });
                        CodeModeHostClient::with_origin(BoxCloneSyncService::new(transport), origin)
                    }
                    TransportEndpoint::Connected(channel) => {
                        let transport = channel.clone().map_err(io::Error::other);
                        CodeModeHostClient::new(BoxCloneSyncService::new(transport))
                    }
                };
                Ok(client
                    .max_decoding_message_size(MAX_FRAME_BYTES)
                    .max_encoding_message_size(MAX_FRAME_BYTES))
            })
            .await
            .cloned()
    }
}

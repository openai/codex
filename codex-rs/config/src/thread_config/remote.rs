use std::collections::BTreeMap;
use std::collections::HashMap;
use std::num::NonZeroU64;

use async_trait::async_trait;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::WireApi;
use codex_protocol::config_types::ModelProviderAuthInfo;
use codex_utils_absolute_path::AbsolutePathBuf;

use super::SessionThreadConfig;
use super::ThreadConfigContext;
use super::ThreadConfigLoadError;
use super::ThreadConfigLoadErrorCode;
use super::ThreadConfigLoader;
use super::ThreadConfigSource;
use super::UserThreadConfig;
use proto::thread_config_loader_client::ThreadConfigLoaderClient;

#[path = "proto/codex.thread_config.v1.rs"]
mod proto;

/// gRPC-backed [`ThreadConfigLoader`] implementation.
#[derive(Clone, Debug)]
pub struct RemoteThreadConfigLoader {
    endpoint: String,
}

impl RemoteThreadConfigLoader {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }

    async fn client(
        &self,
    ) -> Result<ThreadConfigLoaderClient<tonic::transport::Channel>, ThreadConfigLoadError> {
        ThreadConfigLoaderClient::connect(self.endpoint.clone())
            .await
            .map_err(|err| {
                ThreadConfigLoadError::new(
                    ThreadConfigLoadErrorCode::RequestFailed,
                    None,
                    format!("failed to connect to remote thread config loader: {err}"),
                )
            })
    }
}

#[async_trait]
impl ThreadConfigLoader for RemoteThreadConfigLoader {
    async fn load(
        &self,
        context: ThreadConfigContext,
    ) -> Result<Vec<ThreadConfigSource>, ThreadConfigLoadError> {
        let request = proto::LoadThreadConfigRequest {
            thread_id: context.thread_id,
            cwd: context.cwd.map(|cwd| cwd.to_string_lossy().into_owned()),
        };

        let response = self
            .client()
            .await?
            .load(request)
            .await
            .map_err(remote_status_to_error)?
            .into_inner();

        response
            .sources
            .into_iter()
            .map(thread_config_source_from_proto)
            .collect()
    }
}

fn remote_status_to_error(status: tonic::Status) -> ThreadConfigLoadError {
    let code = match status.code() {
        tonic::Code::Unauthenticated | tonic::Code::PermissionDenied => {
            ThreadConfigLoadErrorCode::Auth
        }
        tonic::Code::DeadlineExceeded => ThreadConfigLoadErrorCode::Timeout,
        tonic::Code::Ok
        | tonic::Code::Cancelled
        | tonic::Code::Unknown
        | tonic::Code::InvalidArgument
        | tonic::Code::NotFound
        | tonic::Code::AlreadyExists
        | tonic::Code::ResourceExhausted
        | tonic::Code::FailedPrecondition
        | tonic::Code::Aborted
        | tonic::Code::OutOfRange
        | tonic::Code::Unimplemented
        | tonic::Code::Internal
        | tonic::Code::Unavailable
        | tonic::Code::DataLoss => ThreadConfigLoadErrorCode::RequestFailed,
    };
    ThreadConfigLoadError::new(
        code,
        None,
        format!("remote thread config request failed: {status}"),
    )
}

fn thread_config_source_from_proto(
    source: proto::ThreadConfigSource,
) -> Result<ThreadConfigSource, ThreadConfigLoadError> {
    match source.source {
        Some(proto::thread_config_source::Source::Session(config)) => {
            session_thread_config_from_proto(config).map(ThreadConfigSource::Session)
        }
        Some(proto::thread_config_source::Source::User(_)) => {
            Ok(ThreadConfigSource::User(UserThreadConfig::default()))
        }
        None => Err(parse_error("remote thread config omitted source payload")),
    }
}

fn session_thread_config_from_proto(
    config: proto::SessionThreadConfig,
) -> Result<SessionThreadConfig, ThreadConfigLoadError> {
    let model_providers = config
        .model_providers
        .into_iter()
        .map(model_provider_from_proto)
        .collect::<Result<HashMap<_, _>, _>>()?;

    Ok(SessionThreadConfig {
        model_provider: config.model_provider,
        model_providers,
        features: config.features.into_iter().collect::<BTreeMap<_, _>>(),
    })
}

fn model_provider_from_proto(
    provider: proto::ModelProvider,
) -> Result<(String, ModelProviderInfo), ThreadConfigLoadError> {
    if provider.id.is_empty() {
        return Err(parse_error(
            "remote thread config returned model provider without an id",
        ));
    }
    let id = provider.id;
    let wire_api = match proto::WireApi::try_from(provider.wire_api) {
        Ok(proto::WireApi::Responses) => WireApi::Responses,
        Err(_) => {
            return Err(parse_error(format!(
                "remote thread config returned unknown wire_api: {}",
                provider.wire_api
            )));
        }
    };
    let info = ModelProviderInfo {
        name: provider.name,
        base_url: provider.base_url,
        env_key: provider.env_key,
        env_key_instructions: provider.env_key_instructions,
        experimental_bearer_token: provider.experimental_bearer_token,
        auth: provider
            .auth
            .map(model_provider_auth_from_proto)
            .transpose()?,
        wire_api,
        query_params: provider.query_params.map(|map| map.values),
        http_headers: provider.http_headers.map(|map| map.values),
        env_http_headers: provider.env_http_headers.map(|map| map.values),
        request_max_retries: provider.request_max_retries,
        stream_max_retries: provider.stream_max_retries,
        stream_idle_timeout_ms: provider.stream_idle_timeout_ms,
        websocket_connect_timeout_ms: provider.websocket_connect_timeout_ms,
        requires_openai_auth: provider.requires_openai_auth,
        supports_websockets: provider.supports_websockets,
    };
    Ok((id, info))
}

#[cfg(test)]
fn model_provider_to_proto(
    id: impl Into<String>,
    provider: ModelProviderInfo,
) -> proto::ModelProvider {
    let ModelProviderInfo {
        name,
        base_url,
        env_key,
        env_key_instructions,
        experimental_bearer_token,
        auth,
        wire_api,
        query_params,
        http_headers,
        env_http_headers,
        request_max_retries,
        stream_max_retries,
        stream_idle_timeout_ms,
        websocket_connect_timeout_ms,
        requires_openai_auth,
        supports_websockets,
    } = provider;

    proto::ModelProvider {
        id: id.into(),
        name,
        base_url,
        env_key,
        env_key_instructions,
        experimental_bearer_token,
        auth: auth.map(model_provider_auth_to_proto),
        wire_api: proto_wire_api(wire_api).into(),
        query_params: query_params.map(proto_string_map),
        http_headers: http_headers.map(proto_string_map),
        env_http_headers: env_http_headers.map(proto_string_map),
        request_max_retries,
        stream_max_retries,
        stream_idle_timeout_ms,
        websocket_connect_timeout_ms,
        requires_openai_auth,
        supports_websockets,
    }
}

fn model_provider_auth_from_proto(
    auth: proto::ModelProviderAuthInfo,
) -> Result<ModelProviderAuthInfo, ThreadConfigLoadError> {
    let timeout_ms = NonZeroU64::new(auth.timeout_ms)
        .ok_or_else(|| parse_error("remote thread config returned zero auth timeout_ms"))?;
    let cwd = AbsolutePathBuf::from_absolute_path_checked(&auth.cwd).map_err(|err| {
        parse_error(format!(
            "remote thread config returned invalid auth cwd {:?}: {err}",
            auth.cwd
        ))
    })?;

    Ok(ModelProviderAuthInfo {
        command: auth.command,
        args: auth.args,
        timeout_ms,
        refresh_interval_ms: auth.refresh_interval_ms,
        cwd,
    })
}

#[cfg(test)]
fn model_provider_auth_to_proto(auth: ModelProviderAuthInfo) -> proto::ModelProviderAuthInfo {
    let ModelProviderAuthInfo {
        command,
        args,
        timeout_ms,
        refresh_interval_ms,
        cwd,
    } = auth;

    proto::ModelProviderAuthInfo {
        command,
        args,
        timeout_ms: timeout_ms.get(),
        refresh_interval_ms,
        cwd: cwd.to_string_lossy().into_owned(),
    }
}

#[cfg(test)]
fn proto_string_map(values: HashMap<String, String>) -> proto::StringMap {
    proto::StringMap { values }
}

#[cfg(test)]
fn proto_wire_api(wire_api: WireApi) -> proto::WireApi {
    match wire_api {
        WireApi::Responses => proto::WireApi::Responses,
    }
}

fn parse_error(message: impl Into<String>) -> ThreadConfigLoadError {
    ThreadConfigLoadError::new(ThreadConfigLoadErrorCode::Parse, None, message.into())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::collections::HashMap;
    use std::num::NonZeroU64;

    use codex_model_provider_info::ModelProviderInfo;
    use codex_model_provider_info::WireApi;
    use codex_protocol::config_types::ModelProviderAuthInfo;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use tonic::Request;
    use tonic::Response;
    use tonic::Status;
    use tonic::transport::Server;

    use super::proto::thread_config_loader_server;
    use super::proto::thread_config_loader_server::ThreadConfigLoaderServer;
    use super::*;
    use crate::SessionThreadConfig;
    use crate::UserThreadConfig;

    struct TestServer {
        sources: Vec<proto::ThreadConfigSource>,
    }

    #[tonic::async_trait]
    impl thread_config_loader_server::ThreadConfigLoader for TestServer {
        async fn load(
            &self,
            request: Request<proto::LoadThreadConfigRequest>,
        ) -> Result<Response<proto::LoadThreadConfigResponse>, Status> {
            assert_eq!(
                request.into_inner(),
                proto::LoadThreadConfigRequest {
                    thread_id: Some("thread-1".to_string()),
                    cwd: Some("/workspace/project".to_string()),
                }
            );

            Ok(Response::new(proto::LoadThreadConfigResponse {
                sources: self.sources.clone(),
            }))
        }
    }

    #[tokio::test]
    async fn load_thread_config_calls_remote_service() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("test server addr");
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            Server::builder()
                .add_service(ThreadConfigLoaderServer::new(TestServer {
                    sources: proto_sources(),
                }))
                .serve_with_incoming_shutdown(
                    tokio_stream::wrappers::TcpListenerStream::new(listener),
                    async {
                        let _ = shutdown_rx.await;
                    },
                )
                .await
        });

        let loader = RemoteThreadConfigLoader::new(format!("http://{addr}"));
        let loaded = loader
            .load(ThreadConfigContext {
                thread_id: Some("thread-1".to_string()),
                cwd: Some("/workspace/project".into()),
            })
            .await;

        let _ = shutdown_tx.send(());
        server.await.expect("join server").expect("server");

        assert_eq!(loaded.expect("load thread config"), expected_sources());
    }

    #[test]
    fn model_provider_proto_roundtrips_through_domain_type() {
        let expected = expected_provider();
        let proto = model_provider_to_proto("local", expected.clone());
        let (id, actual) = model_provider_from_proto(proto).expect("model provider from proto");

        assert_eq!(id, "local");
        assert_eq!(actual, expected);
    }

    fn proto_sources() -> Vec<proto::ThreadConfigSource> {
        vec![
            proto::ThreadConfigSource {
                source: Some(proto::thread_config_source::Source::Session(
                    proto::SessionThreadConfig {
                        model_provider: Some("local".to_string()),
                        model_providers: vec![proto::ModelProvider {
                            id: "local".to_string(),
                            name: "Local".to_string(),
                            base_url: Some("http://127.0.0.1:8061/api/codex".to_string()),
                            env_key: None,
                            env_key_instructions: None,
                            experimental_bearer_token: None,
                            auth: Some(proto::ModelProviderAuthInfo {
                                command: "token-helper".to_string(),
                                args: vec!["--json".to_string()],
                                timeout_ms: 5_000,
                                refresh_interval_ms: 300_000,
                                cwd: "/workspace".to_string(),
                            }),
                            wire_api: proto::WireApi::Responses.into(),
                            query_params: Some(proto::StringMap {
                                values: HashMap::from([(
                                    "api-version".to_string(),
                                    "2026-04-16".to_string(),
                                )]),
                            }),
                            http_headers: Some(proto::StringMap {
                                values: HashMap::from([(
                                    "X-Test".to_string(),
                                    "enabled".to_string(),
                                )]),
                            }),
                            env_http_headers: Some(proto::StringMap {
                                values: HashMap::from([(
                                    "X-Env".to_string(),
                                    "LOCAL_HEADER".to_string(),
                                )]),
                            }),
                            request_max_retries: Some(7),
                            stream_max_retries: Some(8),
                            stream_idle_timeout_ms: Some(9_000),
                            websocket_connect_timeout_ms: Some(10_000),
                            requires_openai_auth: false,
                            supports_websockets: true,
                        }],
                        features: HashMap::from([
                            ("plugins".to_string(), false),
                            ("tools".to_string(), true),
                        ]),
                    },
                )),
            },
            proto::ThreadConfigSource {
                source: Some(proto::thread_config_source::Source::User(
                    proto::UserThreadConfig {},
                )),
            },
        ]
    }

    fn expected_sources() -> Vec<ThreadConfigSource> {
        vec![
            ThreadConfigSource::Session(SessionThreadConfig {
                model_provider: Some("local".to_string()),
                model_providers: HashMap::from([("local".to_string(), expected_provider())]),
                features: BTreeMap::from([
                    ("plugins".to_string(), false),
                    ("tools".to_string(), true),
                ]),
            }),
            ThreadConfigSource::User(UserThreadConfig::default()),
        ]
    }

    fn expected_provider() -> ModelProviderInfo {
        ModelProviderInfo {
            name: "Local".to_string(),
            base_url: Some("http://127.0.0.1:8061/api/codex".to_string()),
            env_key: None,
            env_key_instructions: None,
            experimental_bearer_token: None,
            auth: Some(ModelProviderAuthInfo {
                command: "token-helper".to_string(),
                args: vec!["--json".to_string()],
                timeout_ms: NonZeroU64::new(5_000).expect("non-zero timeout"),
                refresh_interval_ms: 300_000,
                cwd: AbsolutePathBuf::from_absolute_path_checked("/workspace")
                    .expect("absolute cwd"),
            }),
            wire_api: WireApi::Responses,
            query_params: Some(HashMap::from([(
                "api-version".to_string(),
                "2026-04-16".to_string(),
            )])),
            http_headers: Some(HashMap::from([(
                "X-Test".to_string(),
                "enabled".to_string(),
            )])),
            env_http_headers: Some(HashMap::from([(
                "X-Env".to_string(),
                "LOCAL_HEADER".to_string(),
            )])),
            request_max_retries: Some(7),
            stream_max_retries: Some(8),
            stream_idle_timeout_ms: Some(9_000),
            websocket_connect_timeout_ms: Some(10_000),
            requires_openai_auth: false,
            supports_websockets: true,
        }
    }
}

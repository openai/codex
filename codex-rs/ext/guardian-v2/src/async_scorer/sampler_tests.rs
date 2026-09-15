use anyhow::Result;
use codex_extension_api::ContextualUserFragment;
use codex_extension_api::ExtensionMetrics;
use codex_guardian_context::PreviousReviews;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_login::AgentIdentityAuthPolicy;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use codex_model_provider::create_model_provider;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::ResponseItemId;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::SessionSource;
use core_test_support::responses;
use core_test_support::responses::WebSocketConnectionConfig;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::responses::ev_model_verification_metadata;
use core_test_support::responses::ev_output_text_delta;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use uuid::Uuid;

use super::CLASSIFICATION_TOKEN_USAGE_METRIC;
use super::INITIAL_WEBSOCKET_CONNECTIONS;
use super::LunaSampler;
use super::LunaSamplerConfig;
use super::LunaSamplerError;
use super::LunaSamplingRequest;
use super::MAX_CONCURRENT_REQUESTS;

impl LunaSampler {
    /// Waits for warm sockets to enter the client pool, beyond the server handshake.
    pub(in crate::async_scorer) async fn wait_for_prewarm(&self, timeout: Duration) -> Result<()> {
        tokio::time::timeout(timeout, async {
            while self.connections.idle_connections.lock().unwrap().len()
                < INITIAL_WEBSOCKET_CONNECTIONS
            {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        Ok(())
    }
}

fn assert_connection_metadata(
    server: &responses::WebSocketTestServer,
    expected_lineage: &[(&str, Option<&str>)],
) -> Result<String> {
    let handshake = server.single_handshake();
    let thread_id = handshake.header("thread-id").expect("classifier thread ID");
    ThreadId::from_string(&thread_id)?;
    assert_ne!(thread_id, "thread-1");
    assert_eq!(
        [
            handshake.header("x-client-request-id"),
            handshake.header("x-openai-subagent"),
            handshake.header("x-codex-window-id"),
        ],
        [
            Some(thread_id.clone()),
            Some("guardian".to_owned()),
            Some(format!("{thread_id}:0")),
        ]
    );
    let requests = server.single_connection();
    assert_eq!(requests.len(), expected_lineage.len());
    for (request, (parent_turn_id, root_turn_id)) in requests.iter().zip(expected_lineage) {
        let mut metadata = request.body_json()["client_metadata"].clone();
        let turn_id = metadata["turn_id"]
            .as_str()
            .expect("classifier turn ID")
            .to_owned();
        assert_eq!(Uuid::parse_str(&turn_id)?.get_version_num(), 7);
        assert_ne!(turn_id.as_str(), *parent_turn_id);
        metadata["x-codex-turn-metadata"] = serde_json::from_str(
            metadata["x-codex-turn-metadata"]
                .as_str()
                .expect("serialized turn metadata"),
        )?;
        let mut expected = json!({
            "session_id": "session-1",
            "thread_id": thread_id,
            "turn_id": turn_id,
            "parent_turn_id": parent_turn_id,
            "x-openai-subagent": "guardian",
            "x-codex-window-id": format!("{thread_id}:0"),
            "ws_request_header_x_openai_internal_codex_responses_lite": "true",
            "x-codex-turn-metadata": {
                "session_id": "session-1",
                "thread_id": thread_id,
                "guardian_classifier_source_thread_id": "thread-1",
                "turn_id": turn_id,
                "parent_turn_id": parent_turn_id,
                "thread_source": "guardian_classifier",
                "turn_trigger": "guardian_classifier",
            },
        });
        if let Some(root_turn_id) = root_turn_id {
            expected["root_turn_id"] = json!(root_turn_id);
            expected["x-codex-turn-metadata"]["root_turn_id"] = json!(root_turn_id);
        }
        assert_eq!(metadata, expected);
    }
    Ok(thread_id)
}

#[derive(Clone, Copy)]
pub(in crate::async_scorer) enum ProxyPrewarmLimit {
    AllConnections,
    StopAfter { ready_connections: usize },
}

async fn proxy_websocket_servers(servers: &[&responses::WebSocketTestServer]) -> Result<String> {
    proxy_websocket_servers_with_prewarm_limit(servers, ProxyPrewarmLimit::AllConnections).await
}

async fn proxy_websocket_servers_with_prewarm_limit(
    servers: &[&responses::WebSocketTestServer],
    prewarm_limit: ProxyPrewarmLimit,
) -> Result<String> {
    proxy_websocket_servers_with_http(servers, prewarm_limit, /*http_url*/ None).await
}

pub(in crate::async_scorer) async fn proxy_websocket_servers_with_http(
    servers: &[&responses::WebSocketTestServer],
    prewarm_limit: ProxyPrewarmLimit,
    http_url: Option<&str>,
) -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let targets = servers
        .iter()
        .map(|server| server.uri().trim_start_matches("ws://").to_owned())
        .collect::<Vec<_>>();
    let http_target = http_url.map(|url| url.trim_start_matches("http://").to_owned());
    tokio::spawn(async move {
        let mut index = 0;
        let mut failed_prewarm = false;
        while let Ok((mut incoming, _)) = listener.accept().await {
            let mut method = [0; 4];
            if incoming.read_exact(&mut method).await.is_err() {
                continue;
            }
            let target = if &method == b"GET " {
                if let ProxyPrewarmLimit::StopAfter { ready_connections } = prewarm_limit
                    && ready_connections < INITIAL_WEBSOCKET_CONNECTIONS
                    && index == ready_connections
                    && !failed_prewarm
                {
                    failed_prewarm = true;
                    continue;
                }
                let target = targets.get(index).cloned();
                index += 1;
                target
            } else {
                http_target.clone()
            };
            let Some(target) = target else {
                continue;
            };
            tokio::spawn(async move {
                let Ok(mut outgoing) = TcpStream::connect(target).await else {
                    return;
                };
                if outgoing.write_all(&method).await.is_err() {
                    return;
                }
                let _ = tokio::io::copy_bidirectional(&mut incoming, &mut outgoing).await;
            });
        }
    });
    Ok(format!("http://{address}/v1"))
}

pub(super) fn sampler_config(base_url: String) -> LunaSamplerConfig {
    LunaSamplerConfig {
        provider: create_model_provider(
            ModelProviderInfo::create_openai_provider(Some(base_url)),
            Some(AuthManager::from_auth_for_testing(CodexAuth::from_api_key(
                "test-api-key",
            ))),
        ),
        http_client_factory: HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        agent_identity_policy: AgentIdentityAuthPolicy::JwtOnly,
        session_source: SessionSource::Exec,
        session_id: "session-1".to_owned(),
        thread_id: "thread-1".to_owned(),
        originator: Some("guardian-v2-test".to_owned()),

        service_tier: None,
        luna_compaction_hash: None,
        max_input_tokens: codex_guardian_context::DEFAULT_MAX_INPUT_TOKENS,
        metrics: None,
    }
}

async fn connect_sampler(config: LunaSamplerConfig) -> Result<LunaSampler> {
    let sampler = LunaSampler::new(config);
    sampler.prewarm().await;
    Ok(sampler)
}

pub(super) fn sample_request(parent_turn_id: &str) -> LunaSamplingRequest {
    LunaSamplingRequest {
        parent_response_id: None,
        instructions: "Return high for high risk or low for low risk.".to_owned(),
        input: vec![responses::user_message_item(
            "The user requested a README summary.",
        )],
        parent_compaction: None,
        parent_compaction_hash: None,
        reasoning_effort: ReasoningEffort::None,
        parent_turn_id: parent_turn_id.to_owned(),
        root_turn_id: None,
    }
}

type RecordedMetric = (String, i64, Vec<(String, String)>);

#[derive(Default)]
struct RecordingMetrics(Mutex<Vec<RecordedMetric>>);

impl ExtensionMetrics for RecordingMetrics {
    fn histogram_with_boundaries(
        &self,
        name: &str,
        value: i64,
        _boundaries: &[f64],
        tags: &[(&str, &str)],
    ) {
        self.histogram(name, value, tags);
    }

    fn counter(&self, _name: &str, _inc: i64, _tags: &[(&str, &str)]) {}

    fn histogram(&self, name: &str, value: i64, tags: &[(&str, &str)]) {
        if name == "codex.guardian_v2.connection.duration_ms" {
            return;
        }
        self.0.lock().unwrap().push((
            name.to_owned(),
            value,
            tags.iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        ));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sampler_records_token_usage_after_returning_an_early_classification() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let events = vec![
        ev_output_text_delta("low"),
        ev_completed_with_tokens("response-1", /*total_tokens*/ 37),
    ];
    let mut connections = vec![Vec::new(); INITIAL_WEBSOCKET_CONNECTIONS - 1];
    connections.push(vec![events]);
    let server = responses::start_websocket_server(connections).await;
    let metrics = Arc::new(RecordingMetrics::default());
    let mut config = sampler_config(format!(
        "http://{}/v1",
        server.uri().trim_start_matches("ws://")
    ));
    config.metrics = Some(metrics.clone());
    let sampler = connect_sampler(config).await?;

    assert_eq!(sampler.sample(sample_request("turn-1")).await?, "low");
    tokio::time::timeout(Duration::from_secs(2), async {
        while metrics
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|metric| metric.0 == CLASSIFICATION_TOKEN_USAGE_METRIC)
            .count()
            < 7
        {
            tokio::task::yield_now().await;
        }
    })
    .await?;

    assert_eq!(
        metrics
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|metric| metric.0 == CLASSIFICATION_TOKEN_USAGE_METRIC)
            .cloned()
            .collect::<Vec<_>>(),
        [
            ("total", 37),
            ("input", 37),
            ("cached_input", 0),
            ("cache_write_input", 0),
            ("non_cached_input", 37),
            ("output", 0),
            ("reasoning_output", 0),
        ]
        .map(|(token_type, value)| (
            CLASSIFICATION_TOKEN_USAGE_METRIC.to_owned(),
            value,
            vec![("token_type".to_owned(), token_type.to_owned())],
        ))
    );

    let request = server
        .wait_for_request(
            /*connection_index*/ INITIAL_WEBSOCKET_CONNECTIONS - 1,
            /*request_index*/ 0,
        )
        .await
        .body_json();
    let input: Vec<ResponseItem> = serde_json::from_value(request["input"].clone())?;
    let estimated = input
        .iter()
        .map(codex_guardian_context::estimate_input_tokens)
        .sum::<usize>();
    assert!(metrics.0.lock().unwrap().contains(&(
        codex_guardian_context::REQUEST_TOKENS_METRIC.to_owned(),
        i64::try_from(estimated)?,
        vec![
            ("target".to_owned(), "async".to_owned()),
            ("component".to_owned(), "total".to_owned()),
        ],
    )));

    Ok(())
}

struct RefreshableAuth(std::sync::Mutex<&'static str>);
impl ExternalAuth for RefreshableAuth {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async { Ok(CodexAuth::from_api_key(*self.0.lock().expect("auth"))) })
    }
    fn refresh(&self, _: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        *self.0.lock().expect("auth") = "refreshed";
        self.resolve()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn classifier_sends_guardian_header_only_with_codex_backend_auth() -> Result<()> {
    skip_if_no_network!(Ok(()));

    for (auth, base_path, expected_header, expected_service_tier) in [
        (
            CodexAuth::create_dummy_chatgpt_auth_for_testing(),
            "/backend-api/codex",
            Some("classifier"),
            None,
        ),
        (
            CodexAuth::create_dummy_chatgpt_auth_for_testing(),
            "/v1",
            None,
            Some("priority"),
        ),
        (
            CodexAuth::from_api_key("test-api-key"),
            "/v1",
            None,
            Some("priority"),
        ),
    ] {
        let events = vec![
            ev_assistant_message("classification", "low"),
            ev_completed("response-1"),
        ];
        let mut connections = vec![Vec::new(); INITIAL_WEBSOCKET_CONNECTIONS - 1];
        connections.push(vec![events]);
        let server = responses::start_websocket_server(connections).await;
        let base_url = format!(
            "http://{}{base_path}",
            server.uri().trim_start_matches("ws://")
        );
        let mut config = sampler_config(base_url.clone());
        config.provider = create_model_provider(
            ModelProviderInfo::create_openai_provider(Some(base_url)),
            Some(AuthManager::from_auth_for_testing(auth)),
        );
        config.service_tier = Some("priority".to_owned());
        let sampler = connect_sampler(config).await?;

        assert_eq!(sampler.sample(sample_request("turn-1")).await?, "low");
        for handshake in server.handshakes() {
            assert_eq!(handshake.uri(), format!("{base_path}/responses"));
            assert_eq!(
                handshake.header("x-codex-guardian").as_deref(),
                expected_header
            );
            assert_eq!(handshake.header("x-codex-routing-hint"), None);
        }
        let request = server
            .wait_for_request(
                /*connection_index*/ INITIAL_WEBSOCKET_CONNECTIONS - 1,
                /*request_index*/ 0,
            )
            .await
            .body_json();
        assert_eq!(request["service_tier"].as_str(), expected_service_tier);

        drop(sampler);
        server.shutdown().await;
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn preconnected_sampler_reuses_authenticated_websocket_for_classifications() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let scripted_requests = vec![
        vec![
            ev_output_text_delta("low"),
            ev_model_verification_metadata("response-1", vec!["trusted_access_for_cyber"]),
            ev_assistant_message("sample-1", "low"),
            ev_completed("response-1"),
        ],
        vec![
            ev_assistant_message("sample-2", "high"),
            ev_completed("response-2"),
        ],
    ];
    let idle_server = responses::start_websocket_server(vec![scripted_requests.clone()]).await;
    let refreshed = responses::start_websocket_server(vec![vec![
        scripted_requests[1].clone(),
        vec![
            ev_assistant_message("sample-3", "low"),
            ev_completed("response-3"),
        ],
    ]])
    .await;
    let server = responses::start_websocket_server(vec![scripted_requests]).await;
    let base_url = proxy_websocket_servers_with_prewarm_limit(
        &[&idle_server, &server, &refreshed],
        ProxyPrewarmLimit::StopAfter {
            ready_connections: 2,
        },
    )
    .await?;
    let manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test-api-key"));
    manager
        .set_external_auth(Arc::new(RefreshableAuth(std::sync::Mutex::new(
            "test-api-key",
        ))))
        .await?;
    let provider = create_model_provider(
        ModelProviderInfo::create_openai_provider(Some(base_url)),
        Some(manager.clone()),
    );

    let sampler = connect_sampler(LunaSamplerConfig {
        provider,
        http_client_factory: HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        agent_identity_policy: AgentIdentityAuthPolicy::JwtOnly,
        session_source: SessionSource::Exec,
        session_id: "session-1".to_owned(),
        thread_id: "thread-1".to_owned(),
        originator: Some("guardian-v2-test".to_owned()),

        service_tier: None,
        luna_compaction_hash: None,
        max_input_tokens: codex_guardian_context::DEFAULT_MAX_INPUT_TOKENS,
        metrics: None,
    })
    .await?;

    let handshake = server.single_handshake();
    tokio::time::timeout(Duration::from_secs(2), async {
        while server.connections().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    assert!(server.single_connection().is_empty());
    assert_eq!(
        handshake.header("authorization"),
        Some("Bearer test-api-key".to_owned())
    );
    assert_eq!(
        handshake.header("OpenAI-Beta"),
        Some("responses_websockets=2026-02-06".to_owned())
    );
    assert_eq!(
        handshake.header("x-openai-internal-codex-responses-lite"),
        Some("true".to_owned())
    );
    assert_eq!(handshake.header("session-id"), Some("session-1".to_owned()));
    assert_eq!(
        handshake.header("originator"),
        Some("guardian-v2-test".to_owned())
    );

    let first = sampler
        .sample(LunaSamplingRequest {
            parent_response_id: None,
            instructions: "Return high for high risk or low for low risk.".to_owned(),
            input: vec![ResponseItem::Message {
                id: None,
                role: "user".to_owned(),
                content: vec![
                    ContentItem::InputText {
                        text: "The user requested a README summary.".to_owned(),
                    },
                    ContentItem::InputText {
                        text: "The assistant inspected README.md.".to_owned(),
                    },
                ],
                phase: None,
                internal_chat_message_metadata_passthrough: None,
            }],
            parent_compaction: None,
            parent_compaction_hash: None,
            reasoning_effort: ReasoningEffort::None,
            parent_turn_id: "turn-1".to_owned(),
            root_turn_id: Some("turn-1".to_owned()),
        })
        .await?;
    tokio::time::timeout(Duration::from_secs(2), async {
        while sampler
            .connections
            .idle_connections
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
            < 2
        {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    manager.refresh_token_from_authority().await?;
    sampler.connections.clear();
    sampler.prewarm().await;
    let second = sampler
        .sample(LunaSamplingRequest {
            parent_response_id: None,
            instructions: "Return high for high risk or low for low risk.".to_owned(),
            input: vec![responses::user_message_item(
                "The user requested a source review.",
            )],
            parent_compaction: None,
            parent_compaction_hash: None,
            reasoning_effort: ReasoningEffort::Medium,
            parent_turn_id: "turn-2".to_owned(),
            root_turn_id: Some("root-2".to_owned()),
        })
        .await?;

    assert_eq!(first, "low");
    assert_eq!(second, "high");
    // Reuse the same socket after a nested owner, now with unknown root lineage.
    assert_eq!(sampler.sample(sample_request("turn-3")).await?, "low");
    let mut requests = server.single_connection();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        refreshed.single_handshake().header("authorization"),
        Some("Bearer refreshed".to_owned())
    );
    requests.extend(refreshed.single_connection());
    assert_eq!(requests.len(), 3);
    let thread_id = assert_connection_metadata(&server, &[("turn-1", Some("turn-1"))])?;
    assert_connection_metadata(&refreshed, &[("turn-2", Some("root-2")), ("turn-3", None)])?;
    assert_ne!(
        requests[1].body_json()["client_metadata"]["turn_id"],
        requests[2].body_json()["client_metadata"]["turn_id"]
    );
    assert_ne!(
        Some(thread_id),
        idle_server.single_handshake().header("thread-id")
    );
    assert_eq!(
        requests[0].body_json()["input"][2]["content"],
        json!([
            {"type": "input_text", "text": "The user requested a README summary."},
            {"type": "input_text", "text": "The assistant inspected README.md."},
        ])
    );
    for (index, request) in requests.iter().enumerate() {
        let request = request.body_json();
        assert_eq!(request["type"], "response.create");
        assert_eq!(request["model"], "gpt-5.6-luna");
        assert_eq!(request["input"][0]["tools"], json!([]));
        assert_eq!(request["tool_choice"], "none");
        assert!(request.get("text").is_none());
        assert_eq!(request["prompt_cache_key"], "guardian-v2:thread-1");
        assert!(request.get("tools").is_none());
        let effort = if index == 1 { "medium" } else { "none" };
        assert_eq!(request["reasoning"]["effort"], effort);
        assert_eq!(request["reasoning"]["context"], "all_turns");
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sampler_reuses_parent_compaction_only_for_matching_model_hashes() -> Result<()> {
    skip_if_no_network!(Ok(()));

    for (parent_hash, luna_hash, should_reuse) in [
        (Some("compatible"), Some("compatible"), true),
        (Some("parent"), Some("luna"), false),
        (None, Some("compatible"), false),
        (Some("compatible"), None, false),
        (Some(""), Some(""), false),
    ] {
        let events = vec![
            ev_assistant_message("sample", "low"),
            ev_completed("response-1"),
        ];
        let mut connections = vec![Vec::new(); INITIAL_WEBSOCKET_CONNECTIONS - 1];
        connections.push(vec![events.clone(), events]);
        let server = responses::start_websocket_server(connections).await;
        let mut config = sampler_config(format!(
            "http://{}/v1",
            server.uri().trim_start_matches("ws://")
        ));
        config.luna_compaction_hash = luna_hash.map(str::to_owned);
        let sampler = connect_sampler(config).await?;
        let parent_compaction = ResponseItem::Compaction {
            id: Some(ResponseItemId::from_server("cmp_parent".to_owned())),
            encrypted_content: "opaque encrypted summary".to_owned(),
            internal_chat_message_metadata_passthrough: None,
        };
        let mut request = sample_request("turn-1");
        request.parent_compaction = Some(parent_compaction.clone());
        request.parent_compaction_hash = parent_hash.map(str::to_owned);
        request.input.insert(
            /*index*/ 0,
            PreviousReviews::try_from_fragments(vec!["trusted review".to_owned()])?.into_message(),
        );

        let result = sampler.sample(request).await;
        if !should_reuse {
            assert!(matches!(
                result,
                Err(LunaSamplerError::IncompatibleCompaction)
            ));
            assert!(server.connections().iter().all(Vec::is_empty));
            assert_eq!(
                sampler.sample(sample_request("uncompacted-turn")).await?,
                "low"
            );
            continue;
        }
        assert_eq!(result?, "low");

        let request = server
            .wait_for_request(
                /*connection_index*/ INITIAL_WEBSOCKET_CONNECTIONS - 1,
                /*request_index*/ 0,
            )
            .await
            .body_json();
        let input = request["input"].as_array().expect("input items");
        assert_eq!(input[0]["type"], "additional_tools");
        assert_eq!(input[1]["role"], "developer");
        assert_eq!(input.len(), 5);
        assert_eq!(input[2], serde_json::to_value(&parent_compaction)?);
        assert_eq!(input[3]["role"], "developer");
        assert_eq!(input[3]["content"][1]["text"], "trusted review");
        assert_eq!(input[4]["role"], "user");

        let mut switched_request = sample_request("turn-2");
        switched_request.parent_compaction = Some(parent_compaction);
        switched_request.parent_compaction_hash = Some("incompatible".to_owned());
        assert!(matches!(
            sampler.sample(switched_request).await,
            Err(LunaSamplerError::IncompatibleCompaction)
        ));
        assert_eq!(server.connections().iter().map(Vec::len).sum::<usize>(), 1);
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sampler_returns_classification_token_before_terminal_response_events() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let config = WebSocketConnectionConfig {
        requests: vec![vec![ev_output_text_delta("low")]],
        response_headers: Vec::new(),
        accept_delay: None,
        close_after_requests: false,
    };
    let idle_server = responses::start_websocket_server_with_headers(vec![config.clone()]).await;
    let server = responses::start_websocket_server_with_headers(vec![config]).await;
    let base_url = proxy_websocket_servers(&[&idle_server, &server]).await?;
    let provider = create_model_provider(
        ModelProviderInfo::create_openai_provider(Some(base_url)),
        Some(AuthManager::from_auth_for_testing(CodexAuth::from_api_key(
            "test-api-key",
        ))),
    );
    let sampler = connect_sampler(LunaSamplerConfig {
        provider,
        http_client_factory: HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        agent_identity_policy: AgentIdentityAuthPolicy::JwtOnly,
        session_source: SessionSource::Exec,
        session_id: "session-1".to_owned(),
        thread_id: "thread-1".to_owned(),
        originator: None,

        service_tier: None,
        luna_compaction_hash: None,
        max_input_tokens: codex_guardian_context::DEFAULT_MAX_INPUT_TOKENS,
        metrics: None,
    })
    .await?;

    let output = tokio::time::timeout(
        Duration::from_secs(2),
        sampler.sample(LunaSamplingRequest {
            parent_response_id: None,
            instructions: "Return high for high risk or low for low risk.".to_owned(),
            input: vec![responses::user_message_item(
                "The user requested a README summary.",
            )],
            parent_compaction: None,
            parent_compaction_hash: None,
            reasoning_effort: ReasoningEffort::None,
            parent_turn_id: "turn-1".to_owned(),
            root_turn_id: Some("turn-1".to_owned()),
        }),
    )
    .await??;

    assert_eq!(output, "low");
    drop(sampler);
    tokio::join!(idle_server.shutdown(), server.shutdown());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sampler_keeps_first_classification_token_when_later_output_disagrees() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let events = vec![
        ev_output_text_delta("low"),
        ev_output_text_delta("high"),
        ev_assistant_message("sample", "lowhigh"),
        ev_completed("response-1"),
    ];
    let mut connections = vec![Vec::new(); INITIAL_WEBSOCKET_CONNECTIONS - 1];
    connections.push(vec![events]);
    let server = responses::start_websocket_server(connections).await;
    let sampler = connect_sampler(sampler_config(format!(
        "http://{}/v1",
        server.uri().trim_start_matches("ws://")
    )))
    .await?;

    assert_eq!(sampler.sample(sample_request("turn-1")).await?, "low");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sampler_remains_available_when_second_prewarm_fails() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_websocket_server(vec![vec![vec![
        ev_assistant_message("response-1", "low"),
        ev_completed("response-1"),
    ]]])
    .await;
    let sampler =
        connect_sampler(sampler_config(proxy_websocket_servers(&[&server]).await?)).await?;

    assert_eq!(sampler.sample(sample_request("turn-1")).await?, "low");
    assert_eq!(server.handshakes().len(), 1);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sampler_replaces_scored_drains_before_unfinished_classifications() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let incomplete_response = WebSocketConnectionConfig {
        requests: vec![vec![ev_output_text_delta("low")]],
        response_headers: Vec::new(),
        accept_delay: None,
        close_after_requests: false,
    };
    let scored_response = WebSocketConnectionConfig {
        requests: vec![vec![ev_assistant_message("scored", "low")]],
        ..incomplete_response.clone()
    };
    let stalled_response = WebSocketConnectionConfig {
        requests: vec![Vec::new()],
        response_headers: Vec::new(),
        accept_delay: None,
        close_after_requests: false,
    };
    let mut servers = Vec::with_capacity(MAX_CONCURRENT_REQUESTS + 2);
    servers.push(responses::start_websocket_server_with_headers(vec![scored_response]).await);
    servers.push(responses::start_websocket_server_with_headers(vec![stalled_response]).await);
    for _ in 2..MAX_CONCURRENT_REQUESTS {
        servers.push(
            responses::start_websocket_server_with_headers(vec![incomplete_response.clone()]).await,
        );
    }
    let http = responses::start_mock_server().await;
    let _http_mock = responses::mount_sse_sequence(
        &http,
        ["low", "high"]
            .map(|score| {
                responses::sse(vec![
                    ev_assistant_message("overflow", score),
                    ev_completed("overflow"),
                ])
            })
            .to_vec(),
    )
    .await;
    let server_refs = servers[2..INITIAL_WEBSOCKET_CONNECTIONS]
        .iter()
        .chain(servers[..2].iter())
        .chain(servers[INITIAL_WEBSOCKET_CONNECTIONS..].iter())
        .collect::<Vec<_>>();
    let sampler = Arc::new(
        connect_sampler(sampler_config(
            proxy_websocket_servers_with_http(
                &server_refs,
                ProxyPrewarmLimit::AllConnections,
                Some(&http.uri()),
            )
            .await?,
        ))
        .await?,
    );

    let oldest_sampler = Arc::clone(&sampler);
    let oldest = tokio::spawn(async move { oldest_sampler.sample(sample_request("oldest")).await });
    tokio::time::timeout(
        Duration::from_secs(2),
        servers[1].wait_for_request(/*connection_index*/ 0, /*request_index*/ 0),
    )
    .await?;

    let scored_sampler = Arc::clone(&sampler);
    let scored_request =
        tokio::spawn(async move { scored_sampler.sample(sample_request("scored")).await });
    tokio::time::timeout(
        Duration::from_secs(2),
        servers[0].wait_for_request(/*connection_index*/ 0, /*request_index*/ 0),
    )
    .await?;

    for index in 0..MAX_CONCURRENT_REQUESTS - 2 {
        sampler.prewarm().await;
        assert_eq!(
            sampler
                .sample(sample_request(&format!("turn-{index}")))
                .await?,
            "low"
        );
    }

    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(2),
            sampler.sample(sample_request("replace-oldest")),
        )
        .await??,
        "low"
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), scored_request).await???,
        "low"
    );
    assert!(!oldest.is_finished());

    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(2),
            sampler.sample(sample_request("replace-oldest-drain")),
        )
        .await??,
        "high"
    );

    assert!(!oldest.is_finished());
    oldest.abort();
    let _ = oldest.await;
    drop(sampler);
    for server in servers {
        server.shutdown().await;
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sampler_retries_expired_websockets_on_another_warm_connection() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let healthy = responses::start_websocket_server(vec![vec![vec![
        ev_assistant_message("response-1", "low"),
        ev_completed("response-1"),
    ]]])
    .await;
    let expired = responses::start_websocket_server(vec![vec![vec![json!({
        "type": "error",
        "status": 400,
        "error": {
            "type": "invalid_request_error",
            "code": "websocket_connection_limit_reached",
            "message": "Responses websocket connection limit reached (60 minutes)."
        }
    })]]])
    .await;
    let sampler = connect_sampler(sampler_config(
        proxy_websocket_servers(&[&healthy, &expired]).await?,
    ))
    .await?;

    let mut request = sample_request("turn-1");
    request.input.insert(
        /*index*/ 0,
        PreviousReviews::try_from_fragments(vec!["trusted review".to_owned()])?.into_message(),
    );
    request.input.insert(
        /*index*/ 1,
        ContextualUserFragment::into(codex_guardian_context::TrustedSkills {
            paths: vec!["/skills/review/SKILL.md".to_owned()],
        }),
    );
    request.root_turn_id = Some("root-turn".to_owned());
    let output = sampler.sample(request).await?;

    assert_eq!(output, "low");
    let expired_requests = expired.single_connection();
    let healthy_requests = healthy.single_connection();
    assert_eq!(expired_requests.len(), 1);
    assert_eq!(healthy_requests.len(), 1);
    assert_ne!(
        assert_connection_metadata(&expired, &[("turn-1", Some("root-turn"))])?,
        assert_connection_metadata(&healthy, &[("turn-1", Some("root-turn"))])?
    );
    let mut expired_request = expired_requests[0].body_json();
    let mut healthy_request = healthy_requests[0].body_json();
    assert_eq!(
        expired_request["client_metadata"]["turn_id"],
        healthy_request["client_metadata"]["turn_id"],
    );
    for request in [&mut expired_request, &mut healthy_request] {
        request
            .as_object_mut()
            .expect("request object")
            .remove("client_metadata");
    }
    assert_eq!(expired_request, healthy_request);
    let input: Vec<ResponseItem> = serde_json::from_value(healthy_request["input"].clone())?;
    let ids = input
        .iter()
        .map(|item| item.id().expect("classifier input item ID"))
        .collect::<HashSet<_>>();
    assert_eq!(ids.len(), input.len());
    assert!(ids.iter().all(|id| id.is_prefixed()));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sampler_uses_http_with_a_fresh_identity_when_warm_connections_expire() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let response = vec![
        ev_assistant_message("response-1", "low"),
        ev_completed("response-1"),
    ];
    let first = responses::start_websocket_server(vec![vec![response.clone()]]).await;
    let second = responses::start_websocket_server(vec![vec![response.clone()]]).await;
    let http = responses::start_mock_server().await;
    let http_mock = responses::mount_sse_once(&http, responses::sse(response)).await;
    let sampler = connect_sampler(sampler_config(
        proxy_websocket_servers_with_http(
            &[&first, &second],
            ProxyPrewarmLimit::AllConnections,
            Some(&http.uri()),
        )
        .await?,
    ))
    .await?;

    assert_eq!(sampler.sample(sample_request("turn-1")).await?, "low");
    sampler.wait_for_prewarm(Duration::from_secs(2)).await?;
    {
        let mut connections = sampler.connections.idle_connections.lock().unwrap();
        for connection in connections.iter_mut() {
            connection.expires_at = tokio::time::Instant::now();
        }
    }
    assert_eq!(sampler.sample(sample_request("turn-2")).await?, "low");

    let request = http_mock.single_request();
    let http_thread_id = request
        .header("thread-id")
        .expect("HTTP classifier thread ID");
    ThreadId::from_string(&http_thread_id)?;
    let thread_ids = HashSet::from([
        assert_connection_metadata(&first, &[])?,
        assert_connection_metadata(&second, &[("turn-1", None)])?,
        http_thread_id,
    ]);
    assert_eq!(thread_ids.len(), 3);
    responses::assert_parent_turn(&request.body_json(), Some("turn-2"))?;
    assert_eq!(second.single_connection().len(), 1);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sampler_reconnects_after_transient_service_failures() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let unavailable = || {
        vec![vec![vec![json!({
            "type": "error",
            "status": 503,
            "error": {
                "type": "server_error",
                "message": "temporarily unavailable"
            }
        })]]]
    };
    let first = responses::start_websocket_server(unavailable()).await;
    let second = responses::start_websocket_server(unavailable()).await;
    let http = responses::start_mock_server().await;
    let recovered = responses::mount_sse_once(
        &http,
        responses::sse(vec![
            ev_assistant_message("recovered", "low"),
            ev_completed("recovered"),
        ]),
    )
    .await;
    let sampler = connect_sampler(sampler_config(
        proxy_websocket_servers_with_http(
            &[&first, &second],
            ProxyPrewarmLimit::AllConnections,
            Some(&http.uri()),
        )
        .await?,
    ))
    .await?;

    assert_eq!(sampler.sample(sample_request("turn-1")).await?, "low");
    assert_eq!(first.single_connection().len(), 1);
    assert_eq!(second.single_connection().len(), 1);
    let request = recovered.single_request();
    assert_eq!(
        request.body_json()["client_metadata"]["turn_id"],
        first.single_connection()[0].body_json()["client_metadata"]["turn_id"],
    );
    assert_eq!(
        request.header("authorization"),
        Some("Bearer test-api-key".to_owned())
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sampler_limits_transient_recovery_attempts() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let unavailable = || {
        vec![vec![vec![json!({
            "type": "error",
            "status": 503,
            "error": {
                "type": "server_error",
                "message": "temporarily unavailable"
            }
        })]]]
    };
    let first = responses::start_websocket_server(unavailable()).await;
    let second = responses::start_websocket_server(unavailable()).await;
    let http = responses::start_mock_server().await;
    let third = responses::mount_sse_once(
        &http,
        responses::sse(vec![json!({
            "type": "response.failed", "response": {
                "error": {"code": "internal_server_error", "message": "HTTP sampling failed"}
            }
        })]),
    )
    .await;
    let sampler = connect_sampler(sampler_config(
        proxy_websocket_servers_with_http(
            &[&first, &second],
            ProxyPrewarmLimit::AllConnections,
            Some(&http.uri()),
        )
        .await?,
    ))
    .await?;

    let error = sampler
        .sample(sample_request("turn-1"))
        .await
        .expect_err("sampling should stop after the bounded retries");

    assert!(error.to_string().contains("HTTP sampling failed"));
    assert_eq!(first.single_connection().len(), 1);
    assert_eq!(second.single_connection().len(), 1);
    assert_eq!(third.requests().len(), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn parent_response_id_survives_classifier_transport_retry() -> Result<()> {
    skip_if_no_network!(Ok(()));
    for uses_codex_backend in [false, true] {
        let healthy = responses::start_websocket_server(vec![vec![vec![
            ev_assistant_message("resp-review", "low"),
            ev_completed("resp-review"),
        ]]])
        .await;
        let expired = responses::start_websocket_server(vec![vec![vec![json!({
            "type": "error", "status": 400,
            "error": {"type": "invalid_request_error", "code": "websocket_connection_limit_reached", "message": "expired"}
        })]]]).await;
        let base_url = proxy_websocket_servers(&[&healthy, &expired]).await?;
        let mut config = sampler_config(base_url.clone());
        if uses_codex_backend {
            config.provider = create_model_provider(
                ModelProviderInfo::create_openai_provider(Some(format!(
                    "{}/backend-api/codex",
                    base_url.trim_end_matches("/v1")
                ))),
                Some(AuthManager::from_auth_for_testing(
                    CodexAuth::create_dummy_chatgpt_auth_for_testing(),
                )),
            );
        }
        let sampler = connect_sampler(config).await?;
        let parent_response_id = "resp-parent";
        let mut request = sample_request("turn-1");
        request.parent_response_id = Some(parent_response_id.to_owned());
        assert_eq!(sampler.sample(request).await?, "low");
        for server in [&expired, &healthy] {
            let requests = server.single_connection();
            assert_eq!(requests.len(), 1);
            let body = requests[0].body_json();
            assert_eq!(
                (
                    body["client_metadata"].get("parent_response_id").cloned(),
                    body["client_metadata"].get("guardian_credits_requested"),
                ),
                (uses_codex_backend.then(|| json!(parent_response_id)), None)
            );
            assert!(!body["input"].to_string().contains(parent_response_id));
        }
    }

    Ok(())
}

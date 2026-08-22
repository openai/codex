use anyhow::Result;
use codex_extension_api::ExtensionMetrics;
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
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

use super::CLASSIFICATION_TOKEN_USAGE_METRIC;
use super::LunaSampler;
use super::LunaSamplerConfig;
use super::LunaSamplingRequest;
use super::MAX_SAMPLING_RETRIES;
use super::MAX_WEBSOCKET_CONNECTIONS;

fn assert_connection_metadata(server: &responses::WebSocketTestServer) -> Result<String> {
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
    for request in server.single_connection() {
        let mut metadata = request.body_json()["client_metadata"].clone();
        let turn_id = metadata["turn_id"].clone();
        metadata["x-codex-turn-metadata"] = serde_json::from_str(
            metadata["x-codex-turn-metadata"]
                .as_str()
                .expect("serialized turn metadata"),
        )?;
        assert_eq!(
            metadata,
            json!({
                "session_id": "session-1",
                "thread_id": thread_id,
                "turn_id": turn_id,
                "x-openai-subagent": "guardian",
                "x-codex-window-id": format!("{thread_id}:0"),
                "ws_request_header_x_openai_internal_codex_responses_lite": "true",
                "x-codex-turn-metadata": {
                    "session_id": "session-1",
                    "thread_id": thread_id,
                    "guardian_classifier_source_thread_id": "thread-1",
                    "turn_id": turn_id,
                    "thread_source": "guardian_classifier",
                },
            })
        );
    }
    Ok(thread_id)
}

async fn proxy_websocket_servers(servers: &[&responses::WebSocketTestServer]) -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let targets = servers
        .iter()
        .map(|server| server.uri().trim_start_matches("ws://").to_owned())
        .collect::<Vec<_>>();
    tokio::spawn(async move {
        for target in targets {
            let Ok((mut incoming, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let Ok(mut outgoing) = TcpStream::connect(target).await else {
                    return;
                };
                let _ = tokio::io::copy_bidirectional(&mut incoming, &mut outgoing).await;
            });
        }
    });
    Ok(format!("http://{address}/v1"))
}

fn sampler_config(base_url: String) -> LunaSamplerConfig {
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
        metrics: None,
    }
}

fn sample_request(turn_id: &str) -> LunaSamplingRequest {
    LunaSamplingRequest {
        instructions: "Return a risk score.".to_owned(),
        trusted_review_evidence: Vec::new(),
        input: vec!["The user requested a README summary.".to_owned()],
        images: Vec::new(),
        parent_compaction: None,
        parent_compaction_hash: None,
        output_schema: json!({
            "type": "object",
            "properties": { "score": { "type": "number" } },
            "required": ["score"],
            "additionalProperties": false
        }),
        reasoning_effort: ReasoningEffort::None,
        turn_id: turn_id.to_owned(),
    }
}

type RecordedMetric = (String, i64, Vec<(String, String)>);

#[derive(Default)]
struct RecordingMetrics(Mutex<Vec<RecordedMetric>>);

impl ExtensionMetrics for RecordingMetrics {
    fn counter(&self, _name: &str, _inc: i64, _tags: &[(&str, &str)]) {}

    fn histogram(&self, name: &str, value: i64, tags: &[(&str, &str)]) {
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
async fn sampler_records_token_usage_after_returning_an_early_score() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let events = vec![
        ev_output_text_delta(r#"{"score":0.25}"#),
        ev_completed_with_tokens("response-1", /*total_tokens*/ 37),
    ];
    let server = responses::start_websocket_server(vec![Vec::new(), vec![events]]).await;
    let metrics = Arc::new(RecordingMetrics::default());
    let mut config = sampler_config(format!(
        "http://{}/v1",
        server.uri().trim_start_matches("ws://")
    ));
    config.metrics = Some(metrics.clone());
    let sampler = LunaSampler::connect(config).await?;

    assert_eq!(
        sampler.sample(sample_request("turn-1")).await?,
        r#"{"score":0.25}"#
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        while metrics.0.lock().unwrap().len() < 7 {
            tokio::task::yield_now().await;
        }
    })
    .await?;

    assert_eq!(
        *metrics.0.lock().unwrap(),
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
async fn preconnected_sampler_reuses_authenticated_websocket_for_structured_requests() -> Result<()>
{
    skip_if_no_network!(Ok(()));

    let scripted_requests = vec![
        vec![
            ev_output_text_delta(r#"{"score":0"#),
            ev_output_text_delta(".25}"),
            ev_model_verification_metadata("response-1", vec!["trusted_access_for_cyber"]),
            ev_assistant_message("sample-1", r#"{"score":0.25}"#),
            ev_completed("response-1"),
        ],
        vec![
            ev_assistant_message("sample-2", r#"{"score":0.75}"#),
            ev_completed("response-2"),
        ],
    ];
    let idle_server = responses::start_websocket_server(vec![scripted_requests.clone()]).await;
    let refreshed =
        responses::start_websocket_server(vec![vec![scripted_requests[1].clone()]]).await;
    let server = responses::start_websocket_server(vec![scripted_requests]).await;
    let base_url = proxy_websocket_servers(&[&idle_server, &server, &refreshed]).await?;
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

    let sampler = LunaSampler::connect(LunaSamplerConfig {
        provider,
        http_client_factory: HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        agent_identity_policy: AgentIdentityAuthPolicy::JwtOnly,
        session_source: SessionSource::Exec,
        session_id: "session-1".to_owned(),
        thread_id: "thread-1".to_owned(),
        originator: Some("guardian-v2-test".to_owned()),
        service_tier: None,
        luna_compaction_hash: None,
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

    let schema = json!({
        "type": "object",
        "properties": { "score": { "type": "number" } },
        "required": ["score"],
        "additionalProperties": false
    });
    let first = sampler
        .sample(LunaSamplingRequest {
            instructions: "Return a risk score.".to_owned(),
            trusted_review_evidence: Vec::new(),
            input: vec![
                "The user requested a README summary.".to_owned(),
                "The assistant inspected README.md.".to_owned(),
            ],
            images: Vec::new(),
            parent_compaction: None,
            parent_compaction_hash: None,
            output_schema: schema.clone(),
            reasoning_effort: ReasoningEffort::None,
            turn_id: "turn-1".to_owned(),
        })
        .await?;
    tokio::time::timeout(Duration::from_secs(2), async {
        while sampler
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
    let second = sampler
        .sample(LunaSamplingRequest {
            instructions: "Return a risk score.".to_owned(),
            trusted_review_evidence: Vec::new(),
            input: vec!["The user requested a source review.".to_owned()],
            images: Vec::new(),
            parent_compaction: None,
            parent_compaction_hash: None,
            output_schema: schema,
            reasoning_effort: ReasoningEffort::Medium,
            turn_id: "turn-2".to_owned(),
        })
        .await?;

    assert_eq!(first, r#"{"score":0.25}"#);
    assert_eq!(second, r#"{"score":0.75}"#);
    let mut requests = server.single_connection();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        refreshed.single_handshake().header("authorization"),
        Some("Bearer refreshed".to_owned())
    );
    requests.extend(refreshed.single_connection());
    assert_eq!(requests.len(), 2);
    let thread_id = assert_connection_metadata(&server)?;
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
        assert_eq!(request["text"]["format"]["strict"], true);
        assert_eq!(request["prompt_cache_key"], "guardian-v2:thread-1");
        assert_eq!(
            request["client_metadata"]["turn_id"],
            format!("turn-{}", index + 1)
        );
        assert!(request.get("tools").is_none());
        let effort = if index == 0 { "none" } else { "medium" };
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
            ev_assistant_message("sample", r#"{"score":0.25}"#),
            ev_completed("response-1"),
        ];
        let server =
            responses::start_websocket_server(vec![Vec::new(), vec![events.clone(), events]]).await;
        let mut config = sampler_config(format!(
            "http://{}/v1",
            server.uri().trim_start_matches("ws://")
        ));
        config.luna_compaction_hash = luna_hash.map(str::to_owned);
        let sampler = LunaSampler::connect(config).await?;
        let parent_compaction = ResponseItem::Compaction {
            id: Some(ResponseItemId::from_server("cmp_parent".to_owned())),
            encrypted_content: "opaque encrypted summary".to_owned(),
            internal_chat_message_metadata_passthrough: None,
        };
        let mut request = sample_request("turn-1");
        request.parent_compaction = Some(parent_compaction.clone());
        request.parent_compaction_hash = parent_hash.map(str::to_owned);
        request.trusted_review_evidence = vec!["trusted review".to_owned()];

        assert_eq!(sampler.sample(request).await?, r#"{"score":0.25}"#);

        let request = server
            .wait_for_request(/*connection_index*/ 1, /*request_index*/ 0)
            .await
            .body_json();
        let input = request["input"].as_array().expect("input items");
        assert_eq!(input[0]["type"], "additional_tools");
        assert_eq!(input[1]["role"], "developer");
        if should_reuse {
            assert_eq!(input.len(), 5);
            assert_eq!(input[2], serde_json::to_value(&parent_compaction)?);
            assert_eq!(input[3]["role"], "developer");
            assert_eq!(input[3]["content"][1]["text"], "trusted review");
            assert_eq!(input[4]["role"], "user");

            let mut switched_request = sample_request("turn-2");
            switched_request.parent_compaction = Some(parent_compaction);
            switched_request.parent_compaction_hash = Some("incompatible".to_owned());
            assert_eq!(sampler.sample(switched_request).await?, r#"{"score":0.25}"#);
            let switched_request = server
                .wait_for_request(/*connection_index*/ 1, /*request_index*/ 1)
                .await
                .body_json();
            assert_eq!(switched_request["input"][2]["role"], "user");
        } else {
            assert_eq!(input.len(), 4);
            assert_eq!(input[2]["role"], "developer");
            assert_eq!(input[2]["content"][1]["text"], "trusted review");
            assert_eq!(input[3]["role"], "user");
        }
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sampler_returns_complete_json_before_terminal_response_events() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let config = WebSocketConnectionConfig {
        requests: vec![vec![
            ev_output_text_delta(r#"{"score":0"#),
            ev_output_text_delta(".25}"),
        ]],
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
    let sampler = LunaSampler::connect(LunaSamplerConfig {
        provider,
        http_client_factory: HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        agent_identity_policy: AgentIdentityAuthPolicy::JwtOnly,
        session_source: SessionSource::Exec,
        session_id: "session-1".to_owned(),
        thread_id: "thread-1".to_owned(),
        originator: None,
        service_tier: None,
        luna_compaction_hash: None,
        metrics: None,
    })
    .await?;

    let output = tokio::time::timeout(
        Duration::from_secs(2),
        sampler.sample(LunaSamplingRequest {
            instructions: "Return a risk score.".to_owned(),
            trusted_review_evidence: Vec::new(),
            input: vec!["The user requested a README summary.".to_owned()],
            images: Vec::new(),
            parent_compaction: None,
            parent_compaction_hash: None,
            output_schema: json!({
                "type": "object",
                "properties": { "score": { "type": "number" } },
                "required": ["score"],
                "additionalProperties": false
            }),
            reasoning_effort: ReasoningEffort::None,
            turn_id: "turn-1".to_owned(),
        }),
    )
    .await??;

    assert_eq!(output, r#"{"score":0.25}"#);
    drop(sampler);
    tokio::join!(idle_server.shutdown(), server.shutdown());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sampler_recovers_after_initial_prewarm_failures() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_websocket_server(vec![vec![vec![
        ev_assistant_message("recovered", r#"{"score":0.25}"#),
        ev_completed("recovered"),
    ]]])
    .await;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let target = server.uri().trim_start_matches("ws://").to_owned();
    let failed_connections = Arc::new(AtomicUsize::new(0));
    let observed_failures = Arc::clone(&failed_connections);
    tokio::spawn(async move {
        for _ in 0..=MAX_SAMPLING_RETRIES {
            let Ok((connection, _)) = listener.accept().await else {
                return;
            };
            observed_failures.fetch_add(/*val*/ 1, Ordering::Relaxed);
            drop(connection);
        }
        while let Ok((mut incoming, _)) = listener.accept().await {
            let target = target.clone();
            tokio::spawn(async move {
                let Ok(mut outgoing) = TcpStream::connect(target).await else {
                    return;
                };
                let _ = tokio::io::copy_bidirectional(&mut incoming, &mut outgoing).await;
            });
        }
    });

    let sampler = LunaSampler::connect(sampler_config(format!("http://{address}/v1"))).await?;
    assert_eq!(failed_connections.load(Ordering::Relaxed), 1);

    assert_eq!(
        sampler.sample(sample_request("turn-1")).await?,
        r#"{"score":0.25}"#
    );
    assert_eq!(server.handshakes().len(), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sampler_remains_available_when_second_prewarm_fails() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_websocket_server(vec![vec![vec![
        ev_assistant_message("response-1", r#"{"score":0.25}"#),
        ev_completed("response-1"),
    ]]])
    .await;
    let sampler =
        LunaSampler::connect(sampler_config(proxy_websocket_servers(&[&server]).await?)).await?;

    assert_eq!(
        sampler.sample(sample_request("turn-1")).await?,
        r#"{"score":0.25}"#
    );
    assert_eq!(server.handshakes().len(), 1);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sampler_grows_its_pool_for_overlapping_requests() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let response = |id: &str| {
        vec![vec![
            ev_assistant_message(id, r#"{"score":0.25}"#),
            ev_completed(id),
        ]]
    };
    let first = responses::start_websocket_server(vec![response("response-1")]).await;
    let second = responses::start_websocket_server(vec![response("response-2")]).await;
    let third = responses::start_websocket_server(vec![response("response-3")]).await;
    let sampler = LunaSampler::connect(sampler_config(
        proxy_websocket_servers(&[&first, &second, &third]).await?,
    ))
    .await?;

    let outputs = tokio::try_join!(
        sampler.sample(sample_request("turn-1")),
        sampler.sample(sample_request("turn-2")),
        sampler.sample(sample_request("turn-3")),
    )?;

    assert_eq!(
        outputs,
        (
            r#"{"score":0.25}"#.to_owned(),
            r#"{"score":0.25}"#.to_owned(),
            r#"{"score":0.25}"#.to_owned(),
        )
    );
    let mut thread_ids = HashSet::new();
    let mut turn_ids = HashSet::new();
    for server in [&first, &second, &third] {
        assert_eq!(server.single_connection().len(), 1);
        assert!(thread_ids.insert(assert_connection_metadata(server)?));
        turn_ids.insert(
            server.single_connection()[0].body_json()["client_metadata"]["turn_id"]
                .as_str()
                .expect("turn ID")
                .to_owned(),
        );
    }
    assert_eq!(
        turn_ids,
        HashSet::from_iter([
            "turn-1".to_owned(),
            "turn-2".to_owned(),
            "turn-3".to_owned()
        ])
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sampler_replaces_scored_drains_before_unfinished_classifications() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let incomplete_response = WebSocketConnectionConfig {
        requests: vec![vec![ev_output_text_delta(r#"{"score":0.25}"#)]],
        response_headers: Vec::new(),
        accept_delay: None,
        close_after_requests: false,
    };
    let scored_response = WebSocketConnectionConfig {
        requests: vec![vec![ev_assistant_message("scored", r#"{"score":0.25}"#)]],
        ..incomplete_response.clone()
    };
    let stalled_response = WebSocketConnectionConfig {
        requests: vec![Vec::new()],
        response_headers: Vec::new(),
        accept_delay: None,
        close_after_requests: false,
    };
    let mut servers = Vec::with_capacity(MAX_WEBSOCKET_CONNECTIONS + 2);
    servers.push(responses::start_websocket_server_with_headers(vec![scored_response]).await);
    servers.push(responses::start_websocket_server_with_headers(vec![stalled_response]).await);
    for _ in 2..=MAX_WEBSOCKET_CONNECTIONS {
        servers.push(
            responses::start_websocket_server_with_headers(vec![incomplete_response.clone()]).await,
        );
    }
    servers.push(
        responses::start_websocket_server(vec![vec![vec![
            ev_assistant_message("newest", r#"{"score":0.75}"#),
            ev_completed("newest"),
        ]]])
        .await,
    );
    let server_refs = servers.iter().collect::<Vec<_>>();
    let sampler = Arc::new(
        LunaSampler::connect(sampler_config(proxy_websocket_servers(&server_refs).await?)).await?,
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

    for index in 0..MAX_WEBSOCKET_CONNECTIONS - 2 {
        assert_eq!(
            sampler
                .sample(sample_request(&format!("turn-{index}")))
                .await?,
            r#"{"score":0.25}"#
        );
    }

    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(2),
            sampler.sample(sample_request("replace-oldest")),
        )
        .await??,
        r#"{"score":0.25}"#
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), scored_request).await???,
        r#"{"score":0.25}"#
    );
    assert!(!oldest.is_finished());

    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(2),
            sampler.sample(sample_request("replace-oldest-drain")),
        )
        .await??,
        r#"{"score":0.75}"#
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
        ev_assistant_message("response-1", r#"{"score":0.25}"#),
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
    let sampler = LunaSampler::connect(sampler_config(
        proxy_websocket_servers(&[&healthy, &expired]).await?,
    ))
    .await?;

    let output = sampler.sample(sample_request("turn-1")).await?;

    assert_eq!(output, r#"{"score":0.25}"#);
    let expired_requests = expired.single_connection();
    let healthy_requests = healthy.single_connection();
    assert_eq!(expired_requests.len(), 1);
    assert_eq!(healthy_requests.len(), 1);
    assert_ne!(
        assert_connection_metadata(&expired)?,
        assert_connection_metadata(&healthy)?
    );
    let mut expired_request = expired_requests[0].body_json();
    let mut healthy_request = healthy_requests[0].body_json();
    for request in [&mut expired_request, &mut healthy_request] {
        assert_eq!(request["client_metadata"]["turn_id"], "turn-1");
        request
            .as_object_mut()
            .expect("request object")
            .remove("client_metadata");
    }
    assert_eq!(expired_request, healthy_request);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sampler_assigns_a_fresh_identity_when_replacing_aged_connections() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let response = vec![vec![
        ev_assistant_message("response-1", r#"{"score":0.25}"#),
        ev_completed("response-1"),
    ]];
    let first = responses::start_websocket_server(vec![response.clone()]).await;
    let second = responses::start_websocket_server(vec![response.clone()]).await;
    let replacement = responses::start_websocket_server(vec![response]).await;
    let sampler = LunaSampler::connect(sampler_config(
        proxy_websocket_servers(&[&first, &second, &replacement]).await?,
    ))
    .await?;

    assert_eq!(
        sampler.sample(sample_request("turn-1")).await?,
        r#"{"score":0.25}"#
    );
    {
        let mut connections = sampler.idle_connections.lock().unwrap();
        for connection in connections.iter_mut() {
            connection.expires_at = std::time::Instant::now();
        }
    }
    assert_eq!(
        sampler.sample(sample_request("turn-2")).await?,
        r#"{"score":0.25}"#
    );

    let thread_ids = [&first, &second, &replacement]
        .into_iter()
        .map(assert_connection_metadata)
        .collect::<Result<HashSet<_>>>()?;
    assert_eq!(thread_ids.len(), 3);
    assert_eq!(second.single_connection().len(), 1);
    assert_eq!(replacement.single_connection().len(), 1);
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
    let recovered = responses::start_websocket_server(vec![vec![vec![
        ev_assistant_message("recovered", r#"{"score":0.25}"#),
        ev_completed("recovered"),
    ]]])
    .await;
    let sampler = LunaSampler::connect(sampler_config(
        proxy_websocket_servers(&[&first, &second, &recovered]).await?,
    ))
    .await?;

    assert_eq!(
        sampler.sample(sample_request("turn-1")).await?,
        r#"{"score":0.25}"#
    );
    assert_eq!(first.single_connection().len(), 1);
    assert_eq!(second.single_connection().len(), 1);
    assert_eq!(recovered.single_connection().len(), 1);
    assert_eq!(
        recovered.single_handshake().header("authorization"),
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
    let third = responses::start_websocket_server(unavailable()).await;
    let unused = responses::start_websocket_server(unavailable()).await;
    let sampler = LunaSampler::connect(sampler_config(
        proxy_websocket_servers(&[&first, &second, &third, &unused]).await?,
    ))
    .await?;

    let error = sampler
        .sample(sample_request("turn-1"))
        .await
        .expect_err("sampling should stop after the bounded retries");

    assert!(error.to_string().contains("503"));
    assert_eq!(first.single_connection().len(), 1);
    assert_eq!(second.single_connection().len(), 1);
    assert_eq!(third.single_connection().len(), 1);
    assert!(unused.handshakes().is_empty());

    Ok(())
}

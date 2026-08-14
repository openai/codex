use anyhow::Result;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_login::AgentIdentityAuthPolicy;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider::create_model_provider;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::SessionSource;
use core_test_support::responses;
use core_test_support::responses::WebSocketConnectionConfig;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_model_verification_metadata;
use core_test_support::responses::ev_output_text_delta;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

use super::LunaSampler;
use super::LunaSamplerConfig;
use super::LunaSamplingRequest;

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
    }
}

fn sample_request(turn_id: &str) -> LunaSamplingRequest {
    LunaSamplingRequest {
        instructions: "Return a risk score.".to_owned(),
        input: "The user requested a README summary.".to_owned(),
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
    let server = responses::start_websocket_server(vec![scripted_requests]).await;
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
        originator: Some("guardian-v2-test".to_owned()),
        service_tier: None,
    })
    .await?;

    let handshake = server.single_handshake();
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
    assert_eq!(handshake.header("thread-id"), Some("thread-1".to_owned()));
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
            input: "The user requested a README summary.".to_owned(),
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
    let second = sampler
        .sample(LunaSamplingRequest {
            instructions: "Return a risk score.".to_owned(),
            input: "The user requested a source review.".to_owned(),
            output_schema: schema,
            reasoning_effort: ReasoningEffort::Medium,
            turn_id: "turn-2".to_owned(),
        })
        .await?;

    assert_eq!(first, r#"{"score":0.25}"#);
    assert_eq!(second, r#"{"score":0.75}"#);
    let requests = server.single_connection();
    assert_eq!(requests.len(), 2);
    for (index, request) in requests.iter().enumerate() {
        let request = request.body_json();
        assert_eq!(request["type"], "response.create");
        assert_eq!(request["model"], "gpt-5.6-luna");
        assert_eq!(request["input"][0]["tools"], json!([]));
        assert_eq!(request["tool_choice"], "none");
        assert_eq!(request["text"]["format"]["strict"], true);
        assert_eq!(request["prompt_cache_key"], "guardian-v2:thread-1");
        assert_eq!(request["client_metadata"]["session_id"], "session-1");
        assert_eq!(request["client_metadata"]["thread_id"], "thread-1");
        assert_eq!(
            request["client_metadata"]["ws_request_header_x_openai_internal_codex_responses_lite"],
            "true"
        );
        assert!(request.get("tools").is_none());
        let effort = if index == 0 { "none" } else { "medium" };
        assert_eq!(request["reasoning"]["effort"], effort);
        assert_eq!(request["reasoning"]["context"], "all_turns");
        assert_eq!(
            request["client_metadata"]["turn_id"],
            format!("turn-{}", index + 1)
        );
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
    })
    .await?;

    let output = tokio::time::timeout(
        Duration::from_secs(2),
        sampler.sample(LunaSamplingRequest {
            instructions: "Return a risk score.".to_owned(),
            input: "The user requested a README summary.".to_owned(),
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
    for server in [&first, &second, &third] {
        assert_eq!(server.single_connection().len(), 1);
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
    assert_eq!(
        expired_requests[0].body_json(),
        healthy_requests[0].body_json()
    );
    Ok(())
}

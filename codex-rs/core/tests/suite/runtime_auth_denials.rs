//! Check terminal runtime failures and credential recovery through complete turns.

use anyhow::Result;
use base64::Engine;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_features::RuntimeAuthMode;
use codex_login::CodexAuth;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::runtime_auth::PROBE_SERVER;
use core_test_support::runtime_auth::run_harness_through_proxy;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_string_contains;
use wiremock::matchers::method;
use wiremock::matchers::path;

const TEST_NAME: &str =
    "suite::runtime_auth_denials::turns_stop_on_runtime_denial_but_recover_ordinary_401";
const CASES: [&str; 3] = ["revoked", "header-denial", "expired"];
const MODES: [(&str, RuntimeAuthMode); 2] = [
    ("dual", RuntimeAuthMode::DualHeader),
    ("assertion", RuntimeAuthMode::AssertionOnly),
];

#[derive(Default)]
struct Credentials {
    refreshes: AtomicUsize,
}

impl ExternalAuth for Credentials {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async { Ok(CodexAuth::from_api_key("key-a")) })
    }

    fn refresh(&self, _: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        self.refreshes.fetch_add(1, Ordering::SeqCst);
        self.resolve()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turns_stop_on_runtime_denial_but_recover_ordinary_401() -> Result<()> {
    skip_if_no_network!(Ok(()));
    if std::env::var_os(PROBE_SERVER).is_some() {
        return exercise_turns().await;
    }

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/agent/register"))
        .respond_with(|request: &wiremock::Request| {
            ResponseTemplate::new(200).set_body_json(json!({
                "runtime_agent_instance_id": request.headers["idempotency-key"].to_str().unwrap(),
                "agent_host_id": "host-a",
                "actor": {"kind": "platform", "user_id": "user-a",
                    "organization_id": "org-a", "project_id": null},
            }))
        })
        .expect(6)
        .mount(&server)
        .await;
    for (mode, _) in MODES {
        for case in CASES {
            let label = format!("{mode}-{case}");
            let denial = match case {
                "revoked" => ResponseTemplate::new(403).set_body_json(json!({
                    "error": {"code": "runtime_instance_revoked", "message": "Runtime revoked"}
                })),
                "header-denial" => ResponseTemplate::new(401)
                    .insert_header(
                        "x-error-json",
                        base64::engine::general_purpose::STANDARD
                            .encode(br#"{"error":{"code":"invalid_agent_instance_assertion"}}"#),
                    )
                    .set_body_json(json!({"error": {"message": "Runtime rejected"}})),
                _ => ResponseTemplate::new(401).set_body_json(json!({
                    "error": {"code": "token_expired", "message": "Credential expired"}
                })),
            };
            Mock::given(method("POST"))
                .and(path("/v1/responses"))
                .and(body_string_contains(label.clone()))
                .respond_with(denial)
                .with_priority(1)
                .up_to_n_times(1)
                .expect(1)
                .mount(&server)
                .await;
            // A mistaken retry would complete the turn, making either denial assertion fail.
            Mock::given(method("POST"))
                .and(path("/v1/responses"))
                .and(body_string_contains(label))
                .respond_with(
                    ResponseTemplate::new(200)
                        .insert_header("content-type", "text/event-stream")
                        .set_body_string(responses::sse_completed("completed")),
                )
                .with_priority(2)
                .expect(u64::from(case == "expired"))
                .mount(&server)
                .await;
        }
    }

    run_harness_through_proxy(&server, TEST_NAME, /*realtime_target*/ None).await?;
    let mut proof_ids = BTreeSet::new();
    let requests = server.received_requests().await.unwrap();
    let attempts: Vec<_> = requests
        .iter()
        .filter(|request| request.url.path() == "/v1/responses")
        .collect();
    assert_eq!(attempts.len(), 8);
    for request in attempts {
        let authorization = request.headers["authorization"].to_str()?;
        let proof = if let Some(proof) = authorization.strip_prefix("AgentAssertion ") {
            assert!(!request.headers.contains_key("x-openai-agent-assertion"));
            proof
        } else {
            assert_eq!(authorization, "Bearer key-a");
            request.headers["x-openai-agent-assertion"].to_str()?
        };
        let payload: Value = serde_json::from_slice(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(proof.split('.').nth(2).unwrap())?,
        )?;
        assert!(proof_ids.insert(payload["id"].as_str().unwrap().to_owned()));
    }
    server.verify().await;
    Ok(())
}

async fn exercise_turns() -> Result<()> {
    let server = MockServer::start().await;
    for (mode_label, mode) in MODES {
        for case in CASES {
            let test = test_codex()
                .with_auth(CodexAuth::from_api_key("key-a"))
                .with_config(move |config| {
                    config.model_provider.base_url = Some("https://api.openai.com/v1".into());
                    config.model_provider.env_http_headers = None;
                    config.model_provider.supports_websockets = false;
                    config.model_provider.request_max_retries = Some(2);
                    config.model_provider.stream_max_retries = Some(2);
                    config
                        .features
                        .enable(Feature::RuntimeAgentInstances)
                        .expect("enable runtime agent instances");
                    config.runtime_auth_mode = mode;
                    config.model_catalog = Some(
                        codex_models_manager::bundled_models_response()
                            .expect("load bundled models"),
                    );
                })
                .build_with_auto_env(&server)
                .await?;
            let credentials = Arc::new(Credentials::default());
            test.thread_manager
                .auth_manager()
                .set_external_auth(credentials.clone())
                .await?;
            test.codex
                .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                    text: format!("{mode_label}-{case}"),
                    text_elements: Vec::new(),
                }]))
                .await?;
            let mut errors = Vec::new();
            let mut stream_retries = 0;
            wait_for_event(&test.codex, |event| {
                match event {
                    EventMsg::Error(error) => errors.push(error.message.clone()),
                    EventMsg::StreamError(_) => stream_retries += 1,
                    _ => {}
                }
                matches!(event, EventMsg::TurnComplete(_))
            })
            .await;
            assert_eq!(
                errors.len(),
                usize::from(case != "expired"),
                "{mode_label}-{case}"
            );
            assert_eq!(stream_retries, 0, "{mode_label}-{case}");
            assert_eq!(
                credentials.refreshes.load(Ordering::SeqCst),
                usize::from(case == "expired"),
                "{mode_label}-{case}"
            );
            test.codex.submit(Op::Shutdown).await?;
            wait_for_event(&test.codex, |event| {
                matches!(event, EventMsg::ShutdownComplete)
            })
            .await;
        }
    }
    Ok(())
}

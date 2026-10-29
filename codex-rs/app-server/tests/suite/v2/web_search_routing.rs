//! Standalone search uses the discovered workspace origin and cannot follow redirects.

use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_config::types::AuthCredentialsStoreMode;
use codex_features::Feature;
use codex_protocol::ThreadId;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use test_case::test_case;
use tokio::io::copy_bidirectional;
use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::super::https_mock::tls_listener;
use super::super::memory_reset::init_state_db;
use super::search_request;
use super::wait_for_web_search_completed;
use super::wait_for_web_search_started;

const READ_TIMEOUT: Duration = Duration::from_secs(60);
const SEARCH_PATH: &str = "/backend-api/codex/alpha/search";

#[test_case(Some("us_cr"); "us_cr_routes_search")]
#[test_case(None; "unconstrained_rejects_redirect_and_bounds_error_output")]
#[tokio::test]
async fn standalone_web_search_uses_workspace_routing(
    routing_override: Option<&str>,
) -> Result<()> {
    let bootstrap = MockServer::start().await;
    let routed = responses::start_mock_server().await;
    let home = TempDir::new()?;

    let (listener, acceptor, cert_path) = tls_listener(home.path()).await?;
    let routed_origin = format!("https://localhost:{}", listener.local_addr()?.port());
    let routed_address = *routed.address();
    let tls_bridge = async move {
        let mut connections = JoinSet::new();
        loop {
            tokio::select! {
                connection = listener.accept() => {
                    let (stream, _) = connection.context("accept TLS bridge connection")?;
                    let acceptor = acceptor.clone();
                    connections.spawn(async move {
                        let mut stream = acceptor.accept(stream).await.context("TLS bridge handshake")?;
                        let mut upstream = TcpStream::connect(routed_address).await.context("TLS bridge upstream connection")?;
                        copy_bidirectional(&mut stream, &mut upstream).await.context("TLS bridge transfer")?;
                        Ok::<(), anyhow::Error>(())
                    });
                }
                Some(result) = connections.join_next() => result.context("TLS bridge connection task")??,
            }
        }
    };

    Mock::given(method("GET"))
        .and(path("/backend-api/wham/accounts/check"))
        .and(header("chatgpt-account-id", "selected"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "accounts": [{
                "id": "selected",
                "workspace_backend_origin": routed_origin,
                "account_routing_override": routing_override.unwrap_or("NO_CONSTRAINT"),
            }],
        })))
        .expect(1)
        .mount(&bootstrap)
        .await;
    Mock::given(path("/backend-api/wham/config/bundle"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&bootstrap)
        .await;
    Mock::given(path(SEARCH_PATH))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&bootstrap)
        .await;

    let response = if routing_override.is_some() {
        ResponseTemplate::new(200).set_body_json(json!({
            "output": "Routed search result",
        }))
    } else {
        ResponseTemplate::new(307)
            .insert_header("location", format!("{routed_origin}/redirected-search"))
            .set_body_string(format!("Search diagnostic: {}", "λ".repeat(/*n*/ 16_384)))
    };
    Mock::given(method("POST"))
        .and(path(SEARCH_PATH))
        .respond_with(response)
        .expect(1)
        .mount(&routed)
        .await;
    Mock::given(path("/redirected-search"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&routed)
        .await;

    let call_id = "routed-search";
    let model_responses = vec![
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call_with_namespace(
                call_id,
                "web",
                "run",
                &json!({"search_query": [{"q": "workspace routing"}]}).to_string(),
            ),
            responses::ev_completed("resp-1"),
        ]),
        responses::sse_completed("resp-2"),
    ];
    let model_requests = responses::mount_sse_sequence(&routed, model_responses).await;

    MockResponsesConfig::new(&bootstrap.uri())
        .with_root_config(&format!(
            "chatgpt_base_url = \"{}/backend-api\"",
            bootstrap.uri()
        ))
        .with_root_config("memories.disable_on_external_context = true")
        .with_model_provider("openai-custom")
        .with_provider_name("OpenAI")
        .with_provider_base_url(&format!("{}/backend-api/codex", bootstrap.uri()))
        .with_provider_config("requires_openai_auth = true\nsupports_websockets = false")
        .enable_feature(Feature::StandaloneWebSearch)
        .write(home.path())?;
    std::fs::write(
        home.path().join("requirements.toml"),
        "enforce_residency = \"us\"\n",
    )?;
    write_chatgpt_auth(
        home.path(),
        ChatGptAuthFixture::new("access-chatgpt").account_id("selected"),
        AuthCredentialsStoreMode::File,
    )?;
    let mut env_overrides = vec![
        ("OPENAI_API_KEY", None),
        ("CODEX_API_KEY", None),
        ("CODEX_CA_CERTIFICATE", cert_path.to_str()),
        ("SSL_CERT_FILE", cert_path.to_str()),
    ];
    env_overrides.extend(
        codex_network_proxy::PROXY_ENV_KEYS
            .iter()
            .map(|key| (*key, None)),
    );
    let exercise = async {
        let mut app = TestAppServer::builder()
            .with_codex_home(home.path())
            .with_env_overrides(&env_overrides)
            .build_initialized_with_timeout(READ_TIMEOUT)
            .await?;
        let thread = app.start_thread(ThreadStartParams::default()).await?.thread;
        let thread_id = ThreadId::from_string(&thread.id)?;
        let state_db = init_state_db(home.path()).await?;
        // A new thread can acquire its SQLite row on the first turn.
        assert!(matches!(
            state_db.get_thread_memory_mode(thread_id).await?.as_deref(),
            None | Some("enabled")
        ));
        let completed = timeout(
            READ_TIMEOUT,
            app.start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: thread.id,
                input: vec![UserInput::Text {
                    text: "Search the web".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            }),
        )
        .await??;
        assert_eq!(completed.turn.status, TurnStatus::Completed);
        let requests = model_requests.requests();
        assert_eq!(requests.len(), 2);
        if routing_override.is_some() {
            assert_eq!(
                requests[1].function_call_output(call_id)["output"],
                json!([{"type": "input_text", "text": "Routed search result"}])
            );
        } else {
            let error = requests[1]
                .function_call_output_text(call_id)
                .context("redirect rejection should be reported to the model")?;
            assert!(error.len() <= 1024, "{} bytes", error.len());
            assert!(error.contains("http 307 Temporary Redirect"), "{error}");
            assert!(error.contains("Search diagnostic: λ"), "{error}");
            assert_eq!(
                state_db.get_thread_memory_mode(thread_id).await?.as_deref(),
                Some("polluted")
            );
            let started = timeout(READ_TIMEOUT, wait_for_web_search_started(&mut app)).await??;
            let completed =
                timeout(READ_TIMEOUT, wait_for_web_search_completed(&mut app)).await??;
            assert_eq!(started.item.id(), call_id);
            assert_eq!(completed.item.id(), call_id);
        }

        let search = search_request(&routed, SEARCH_PATH).await?;
        assert_eq!(search.headers["authorization"], "Bearer access-chatgpt");
        assert_eq!(search.headers["chatgpt-account-id"], "selected");
        assert_eq!(search.headers["x-openai-internal-codex-residency"], "us");
        assert_eq!(
            search
                .headers
                .get("x-openai-account-routing-override")
                .map(|value| value.to_str())
                .transpose()?,
            routing_override
        );
        bootstrap.verify().await;
        routed.verify().await;
        Ok::<_, anyhow::Error>(app)
    };
    let mut app = tokio::select! {
        biased;
        result = tls_bridge => return result,
        result = exercise => result?,
    };
    assert!(
        timeout(READ_TIMEOUT, app.shutdown_gracefully())
            .await??
            .success()
    );
    Ok(())
}

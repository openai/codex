//! Exercise harness-owned auth through real sessions, subagents, and delegated review.

use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use codex_core::CodexThread;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_features::RuntimeAuthMode;
use codex_history::InitialHistory;
use codex_history::ResumedHistory;
use codex_login::CodexAuth;
use codex_model_provider::ProviderAuthContext;
use codex_models_manager::manager::ModelCatalogSettings;
use codex_models_manager::manager::RefreshStrategy;
use codex_protocol::mcp::ClientMcpExtensions;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::protocol::ReviewTarget;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::task::JoinSet;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_string_contains;
use wiremock::matchers::header_regex;
use wiremock::matchers::method;
use wiremock::matchers::path;

const PROBE_SERVER: &str = "CODEX_RUNTIME_AUTH_TEST_SERVER";
const TEST_NAME: &str = "suite::runtime_auth::harness_shares_runtime_and_honors_thread_auth_config";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn harness_shares_runtime_and_honors_thread_auth_config() -> Result<()> {
    skip_if_no_network!(Ok(()));
    if let Ok(server_uri) = std::env::var(PROBE_SERVER) {
        return exercise_harness(server_uri).await;
    }

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/agent/register"))
        .respond_with(|request: &wiremock::Request| {
            let body: Value = request.body_json().expect("registration body should parse");
            let replacement = body["actor_auth"]["credential"] == "key-b";
            ResponseTemplate::new(200).set_body_json(json!({
                "runtime_agent_instance_id": if replacement { "runtime-replacement" } else { "runtime-shared" },
                "agent_host_id": "host-a",
                "actor": {"kind": "platform", "user_id": if replacement { "user-b" } else { "user-a" }, "organization_id": "org-a", "project_id": null},
            }))
        })
        .expect(2)
        .mount(&server)
        .await;
    // Both runtime modes retain the normal same-origin redirect behavior.
    for label in ["parent-dual", "child-assertion"] {
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .and(body_string_contains(label))
            .respond_with(
                ResponseTemplate::new(307)
                    .insert_header("Location", "https://api.openai.com/redirected/responses"),
            )
            .up_to_n_times(1)
            .expect(1)
            .with_priority(1)
            .mount(&server)
            .await;
    }
    Mock::given(path("/backend-api/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "file_id": "file-a", "upload_url": format!("{}/blob", server.uri()),
        })))
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(path("/backend-api/files/file-a/uploaded"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success", "download_url": format!("{}/blob", server.uri()),
            "file_name": "empty.txt", "file_size_bytes": 0,
        })))
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(path("/blob"))
        .and(method("PUT"))
        .respond_with(ResponseTemplate::new(200))
        .expect(2)
        .mount(&server)
        .await;
    let responses = responses::mount_response_sequence(
        &server,
        (0..14)
            .map(|i| {
                let response =
                    responses::sse_response(responses::sse(vec![responses::ev_completed(
                        &format!("resp-{i}"),
                    )]));
                if (1..=4).contains(&i) {
                    response.insert_header("X-Models-Etag", format!("catalog-refreshed-{i}"))
                } else {
                    response
                }
            })
            .collect(),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/catalog-isolation"))
        .respond_with(ResponseTemplate::new(200).set_body_json(catalog_fixture()))
        .expect(2)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/selected-catalog"))
        .and(header_regex("authorization", "^AgentAssertion "))
        .respond_with(ResponseTemplate::new(200).set_body_json(catalog_fixture()))
        .expect(1)
        .mount(&server)
        .await;

    for mode in ["dual", "assertion", "disabled"] {
        Mock::given(method("GET"))
            .and(path(format!("/catalog-reload/{mode}")))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("ETag", "catalog-initial")
                    .set_body_json(catalog_fixture()),
            )
            .expect(2)
            .mount(&server)
            .await;
    }

    Mock::given(method("GET"))
        .and(path("/resumed-off-models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(catalog_fixture()))
        .mount(&server)
        .await;

    run_harness_through_proxy(&server, TEST_NAME).await?;

    let requests = responses.requests();
    let expected = [
        (Some("startup-disabled"), "Bearer key-a", None),
        (
            Some("discovery-off"),
            "Bearer key-a",
            Some("runtime-shared"),
        ),
        (Some("catalog-dual"), "Bearer key-a", Some("runtime-shared")),
        (
            Some("catalog-assertion"),
            "AgentAssertion ",
            Some("runtime-shared"),
        ),
        (Some("catalog-disabled"), "Bearer key-a", None),
        (Some("parent-dual"), "Bearer key-a", Some("runtime-shared")),
        // Explicit first-party inference with a third-party catalog.
        (None, "Bearer key-a", Some("runtime-shared")),
        (
            Some("child-assertion"),
            "AgentAssertion ",
            Some("runtime-shared"),
        ),
        (None, "AgentAssertion ", Some("runtime-shared")),
        // The inline review delegate inherits the parent's dual-header mode.
        (None, "Bearer key-a", Some("runtime-shared")),
        (Some("thread-disabled"), "Bearer key-a", None),
        (Some("third-party"), "Bearer key-a", None),
        (
            Some("replacement-root"),
            "Bearer key-b",
            Some("runtime-replacement"),
        ),
        (
            Some("replacement-child"),
            "Bearer key-b",
            Some("runtime-replacement"),
        ),
    ];
    assert_eq!(requests.len(), expected.len());
    let mut proof_ids = BTreeSet::new();
    for (request, (label, expected_auth, expected_runtime)) in requests.iter().zip(expected) {
        if let Some(label) = label {
            assert_eq!(
                request
                    .message_input_texts("user")
                    .last()
                    .map(String::as_str),
                Some(label)
            );
        }
        let auth = request.header("authorization").unwrap();
        let proof = if expected_auth == "AgentAssertion " {
            assert!(request.header("x-openai-agent-assertion").is_none());
            Some(
                auth.strip_prefix(expected_auth)
                    .context("assertion-only request")?
                    .to_owned(),
            )
        } else {
            assert_eq!(auth, expected_auth);
            request.header("x-openai-agent-assertion")
        };
        assert_eq!(proof.is_some(), expected_runtime.is_some(), "{label:?}");
        if let Some(proof) = proof {
            let payload: Value = serde_json::from_slice(
                &base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .decode(proof.split('.').nth(2).unwrap())?,
            )?;
            assert_eq!(payload["runtime_agent_instance"], expected_runtime.unwrap());
            assert!(proof_ids.insert(payload["id"].as_str().unwrap().to_owned()));
        }
    }
    let resumed_catalog: Vec<_> = server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|request| request.url.path() == "/resumed-off-models")
        .collect();
    assert!(
        !resumed_catalog.is_empty(),
        "resumed thread must fetch its catalog"
    );
    for request in resumed_catalog {
        assert_eq!(request.headers["authorization"], "Bearer key-b");
        assert!(!request.headers.contains_key("openai-agent-assertion"));
    }
    let catalogs: Vec<_> = server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|request| request.url.path() == "/catalog-isolation")
        .collect();
    assert_eq!(catalogs.len(), 2);
    for request in catalogs {
        assert!(
            request.headers.contains_key("x-openai-agent-assertion")
                || request.headers["authorization"]
                    .to_str()?
                    .starts_with("AgentAssertion ")
        );
    }
    let requests = server.received_requests().await.unwrap();
    let reloaded_catalogs: Vec<_> = requests
        .iter()
        .filter(|request| request.url.path().starts_with("/catalog-reload/"))
        .collect();
    assert_eq!(reloaded_catalogs.len(), 6);
    for request in reloaded_catalogs {
        assert_eq!(request.headers["originator"], "codex_work_cca");
        let authorization = request.headers["authorization"].to_str()?;
        if request.url.path().ends_with("/disabled") {
            assert_eq!(authorization, "Bearer key-a");
            assert!(!request.headers.contains_key("x-openai-agent-assertion"));
            continue;
        }
        let proof = if request.url.path().ends_with("/dual") {
            assert_eq!(authorization, "Bearer key-a");
            request.headers["x-openai-agent-assertion"].to_str()?
        } else {
            assert!(!request.headers.contains_key("x-openai-agent-assertion"));
            authorization
                .strip_prefix("AgentAssertion ")
                .context("catalog assertion")?
        };
        let payload: Value = serde_json::from_slice(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(proof.split('.').nth(2).unwrap())?,
        )?;
        assert_eq!(payload["runtime_agent_instance"], "runtime-shared");
        assert!(proof_ids.insert(payload["id"].as_str().unwrap().to_owned()));
    }
    let files: Vec<_> = requests
        .iter()
        .filter(|r| r.url.path().starts_with("/backend-api/files"))
        .collect();
    assert_eq!(files.len(), 4);
    for (index, request) in files.iter().enumerate() {
        let authorization = request.headers["authorization"].to_str()?;
        let proof = if index < 2 {
            assert_eq!(authorization, "Bearer key-a");
            request.headers["x-openai-agent-assertion"].to_str()?
        } else {
            assert!(!request.headers.contains_key("x-openai-agent-assertion"));
            authorization
                .strip_prefix("AgentAssertion ")
                .context("assertion-only file auth")?
        };
        let payload: Value = serde_json::from_slice(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(proof.split('.').nth(2).unwrap())?,
        )?;
        assert_eq!(payload["runtime_agent_instance"], "runtime-shared");
        assert!(proof_ids.insert(payload["id"].as_str().unwrap().to_owned()));
    }
    server.verify().await;
    Ok(())
}

async fn exercise_harness(server_uri: String) -> Result<()> {
    let server = MockServer::start().await;
    let test = test_codex()
        .with_auth(CodexAuth::from_api_key("key-a"))
        .with_config(|config| {
            config.model_provider.base_url = Some("https://api.openai.com/v1".into());
            config.chatgpt_base_url = "https://us.chatgpt.com/backend-api".to_string();
            config.model_provider.env_http_headers = None;
            config
                .features
                .disable(Feature::RuntimeAgentInstances)
                .expect("test config should allow disabling runtime auth");
            config.model_catalog = Some(
                codex_models_manager::bundled_models_response()
                    .expect("bundled models should parse"),
            );
        })
        .build_with_auto_env(&server)
        .await?;
    turn(&test.codex, "startup-disabled").await?;

    // Enable after startup, then share one lazy registration with a child and an inline delegate.
    let mut enabled = test.config.clone();
    enabled.features.enable(Feature::RuntimeAgentInstances)?;
    // Initial discovery and response-triggered ETag refresh use each thread's effective auth.
    for (mode, runtime_enabled) in [
        (RuntimeAuthMode::DualHeader, true),
        (RuntimeAuthMode::AssertionOnly, true),
        (RuntimeAuthMode::DualHeader, false),
    ] {
        let mut catalog_config = enabled.clone();
        catalog_config.model_catalog = None;
        catalog_config
            .features
            .enable(Feature::ApiKeyModelDiscovery)?;
        catalog_config.runtime_auth_mode = mode;
        if !runtime_enabled {
            catalog_config
                .features
                .disable(Feature::RuntimeAgentInstances)?;
        }
        catalog_config.model = None;
        let label = if !runtime_enabled {
            "disabled"
        } else if mode == RuntimeAuthMode::AssertionOnly {
            "assertion"
        } else {
            "dual"
        };
        catalog_config.model_provider.model_catalog_url =
            Some(format!("https://api.openai.com/catalog-reload/{label}").into());
        if mode == RuntimeAuthMode::AssertionOnly {
            let mut selected = catalog_config.clone();
            selected.model_provider.base_url = Some(server_uri.clone());
            selected.model_provider.model_catalog_url =
                Some("https://api.openai.com/selected-catalog".into());
            let models = test
                .thread_manager
                .temporary_models_manager_for_config(&selected, &ModelCatalogSettings::default());
            models
                .raw_model_catalog(RefreshStrategy::Online, selected.http_client_factory())
                .await;
            assert_eq!(models.get_remote_models().await, catalog_fixture().models);
        }
        let catalog_thread = test
            .thread_manager
            .start_thread(StartThreadOptions {
                metrics_service_name: Some("codex_work_cca".into()),
                ..StartThreadOptions::new(catalog_config)
            })
            .await?;
        assert_eq!(
            catalog_thread.session_configured.model,
            "catalog-isolation-model"
        );
        if label == "dual" {
            // The first new ETag must not fetch while discovery is disabled. A second,
            // distinct ETag on the same thread must fetch once discovery is enabled again.
            test.thread_manager
                .set_api_key_model_discovery_enabled(/*enabled*/ false);
            turn(&catalog_thread.thread, "discovery-off").await?;
            test.thread_manager
                .set_api_key_model_discovery_enabled(/*enabled*/ true);
        }
        turn(&catalog_thread.thread, &format!("catalog-{label}")).await?;
    }
    let parent = test
        .thread_manager
        .start_thread(StartThreadOptions::new(enabled.clone()))
        .await?;
    turn(&parent.thread, "parent-dual").await?;
    exercise_provider_destinations(&parent.thread, &enabled).await?;
    upload_with_harness(&test.thread_manager, &enabled).await?;
    let mut child_config = enabled.clone();
    child_config.runtime_auth_mode = RuntimeAuthMode::AssertionOnly;
    parent.thread.ensure_rollout_materialized().await;
    parent.thread.flush_rollout().await?;
    let prepared = test
        .thread_store
        .prepare_fork(codex_thread_store::PrepareForkParams {
            thread_id: parent.thread_id,
            boundary: codex_thread_store::ForkBoundary::Latest,
        })
        .await?;
    let child = test
        .thread_manager
        .fork_prepared_thread(
            StartThreadOptions {
                session_source: Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id: parent.thread_id,
                    depth: 1,
                    agent_path: None,
                    agent_nickname: None,
                    agent_role: None,
                })),
                ..StartThreadOptions::new(child_config.clone())
            },
            prepared,
        )
        .await?;
    turn(&child.thread, "child-assertion").await?;
    exercise_provider_destinations(&child.thread, &enabled).await?;
    upload_with_harness(&test.thread_manager, &child_config).await?;
    parent
        .thread
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Review this change".into(),
                },
                user_facing_hint: None,
            },
        })
        .await?;
    wait_for_event(&parent.thread, |e| matches!(e, EventMsg::TurnComplete(_))).await;

    let disabled = test
        .thread_manager
        .start_thread(StartThreadOptions::new(test.config.clone()))
        .await?;
    turn(&disabled.thread, "thread-disabled").await?;
    // A third-party OpenAI-compatible endpoint still uses its existing provider authentication.
    enabled.model_provider.base_url = Some(format!("{server_uri}/v1"));
    enabled.chatgpt_base_url = "https://custom.example.test/backend-api".to_string();
    let third_party = test
        .thread_manager
        .start_thread(StartThreadOptions::new(enabled))
        .await?;
    turn(&third_party.thread, "third-party").await?;

    // Resume a stopped thread under a separately supplied credential context. An internal child
    // must inherit the resumed parent's runtime, not the ThreadManager's original default.
    let rollout = parent
        .thread
        .rollout_path()
        .context("parent rollout should exist")?;
    parent.thread.submit(Op::Shutdown).await?;
    wait_for_event(&parent.thread, |e| matches!(e, EventMsg::ShutdownComplete)).await;
    test.thread_manager.remove_thread(&parent.thread_id).await;
    let context = test
        .thread_store
        .load_latest_model_context(codex_thread_store::LoadThreadHistoryParams {
            thread_id: parent.thread_id,
            include_archived: false,
        })
        .await?;
    let replacement = codex_core::test_support::auth_manager_from_auth_with_home(
        CodexAuth::from_api_key("key-b"),
        test.codex_home_path().to_path_buf(),
    );
    let history = InitialHistory::Resumed(ResumedHistory {
        history_revision: None,
        conversation_id: context.thread_id,
        history: Arc::new(context.items),
        rollout_path: Some(rollout),
    });
    let mut disabled = test.config.clone();
    disabled.model_catalog = None;
    disabled.model = None;
    disabled.model_provider.model_catalog_url =
        Some("https://api.openai.com/resumed-off-models".into());
    let resumed_off = test
        .thread_manager
        .resume_thread_with_history(
            disabled,
            history.clone(),
            Arc::clone(&replacement),
            /*parent_trace*/ None,
            ClientMcpExtensions::default(),
        )
        .await?;
    assert_eq!(
        resumed_off.session_configured.model,
        "catalog-isolation-model"
    );
    resumed_off.thread.submit(Op::Shutdown).await?;
    wait_for_event(&resumed_off.thread, |e| {
        matches!(e, EventMsg::ShutdownComplete)
    })
    .await;
    test.thread_manager
        .remove_thread(&resumed_off.thread_id)
        .await;

    let mut resumed_config = test.config.clone();
    resumed_config
        .features
        .enable(Feature::RuntimeAgentInstances)?;
    let resumed = test
        .thread_manager
        .resume_thread_with_history(
            resumed_config.clone(),
            history,
            replacement,
            /*parent_trace*/ None,
            ClientMcpExtensions::default(),
        )
        .await?;
    turn(&resumed.thread, "replacement-root").await?;
    let internal = test
        .thread_manager
        .spawn_internal_session(
            resumed.thread_id,
            StartThreadOptions {
                session_source: Some(SessionSource::Internal(
                    InternalSessionSource::MemoryConsolidation,
                )),
                ..StartThreadOptions::new(resumed_config)
            },
        )
        .await?;
    turn(&internal.thread, "replacement-child").await?;
    Ok(())
}

fn catalog_fixture() -> codex_protocol::openai_models::ModelsResponse {
    let mut models = codex_models_manager::bundled_models_response()
        .expect("bundled catalog should provide a valid model fixture");
    models.models.truncate(1);
    models.models[0].slug = "catalog-isolation-model".into();
    models
}

async fn exercise_provider_destinations(
    thread: &CodexThread,
    config: &codex_core::config::Config,
) -> Result<()> {
    let external = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/catalog-isolation"))
        .respond_with(ResponseTemplate::new(200).set_body_json(catalog_fixture()))
        .expect(1)
        .mount(&external)
        .await;
    let external_responses = responses::mount_sse_once(
        &external,
        responses::sse(vec![responses::ev_completed("external-response")]),
    )
    .await;
    let auth = thread
        .thread_extension_data()
        .get::<ProviderAuthContext>()
        .context("thread should publish its provider auth context")?;
    for (inference, catalog) in [
        ("https://api.openai.com/v1".to_string(), external.uri()),
        (external.uri(), "https://api.openai.com".to_string()),
    ] {
        let mut info = config.model_provider.clone();
        info.base_url = Some(inference);
        info.model_catalog_url = Some(format!("{catalog}/catalog-isolation").into());
        let provider = auth.model_provider(info);
        let setup = codex_model_provider::resolve_request(
            provider.as_ref(),
            /*scope*/ None,
            /*routing*/ None,
            config.http_client_factory(),
        )
        .await?;
        let client = codex_api::ResponsesClient::new(
            setup.http_transport("responses")?,
            setup.api_provider,
            setup.api_auth,
        );
        let _stream = client
            .stream(
                json!({"model": "test", "input": []}),
                http::HeaderMap::new(),
                codex_api::Compression::None,
                /*turn_state*/ None,
            )
            .await?;
        let models = provider.models_manager_without_cache(/*config_model_catalog*/ None);
        models.set_api_key_model_discovery_enabled(/*enabled*/ true);
        models
            .raw_model_catalog(RefreshStrategy::Online, config.http_client_factory())
            .await;
        assert_eq!(models.get_remote_models().await, catalog_fixture().models);
    }
    assert_eq!(external_responses.requests().len(), 1);
    let requests = external
        .received_requests()
        .await
        .context("external destination should record requests")?;
    assert_eq!(requests.len(), 2);
    for request in requests {
        assert_eq!(request.headers["authorization"], "Bearer key-a");
        assert!(!request.headers.contains_key("x-openai-agent-assertion"));
    }
    external.verify().await;
    Ok(())
}

async fn turn(thread: &Arc<CodexThread>, label: &str) -> Result<()> {
    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: label.into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(thread, |e| matches!(e, EventMsg::TurnComplete(_))).await;
    Ok(())
}

async fn upload_with_harness(
    manager: &codex_core::ThreadManager,
    config: &codex_core::config::Config,
) -> Result<()> {
    let (runtime, mode) = manager
        .runtime_auth_for_config(config)?
        .context("enabled harness runtime")?;
    let selected = runtime.auth().await?;
    let clients =
        codex_http_client::RouteAwareClientPool::new_without_redirects_or_request_logging(
            config.http_client_factory(),
            codex_http_client::ClientRouteClass::Api,
        )
        .with_account_binding(selected.http_client_factory().network_policy())
        .context("upload auth and transport share the application policy")?;
    let auth = selected.into_auth_provider(mode);
    let uploaded = codex_api::upload_openai_file(
        &config.chatgpt_base_url,
        auth.as_ref(),
        &clients,
        "empty.txt".into(),
        /*file_size_bytes*/ 0,
        || async { Ok(futures::stream::empty()) },
        /*hosted_upload*/ None,
    )
    .await?;
    assert_eq!(uploaded.uri, "sediment://file-a");
    Ok(())
}

async fn run_harness_through_proxy(server: &MockServer, test_name: &str) -> Result<()> {
    // The production destination check stays intact. A test-only CA and CONNECT proxy route
    // the real host names to wiremock, only inside this isolated test subprocess.
    codex_utils_rustls_provider::ensure_rustls_crypto_provider();
    let rcgen::CertifiedKey { cert, signing_key } = rcgen::generate_simple_self_signed(vec![
        "api.openai.com".into(),
        "auth.openai.com".into(),
        "chatgpt.com".into(),
        "us.chatgpt.com".into(),
    ])?;
    let ca_dir = tempfile::tempdir()?;
    let ca_path = ca_dir.path().join("ca.pem");
    std::fs::write(&ca_path, cert.pem())?;
    let acceptor = TlsAcceptor::from(Arc::new(
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert.der().clone()], signing_key.into())?,
    ));
    let proxy = TcpListener::bind("127.0.0.1:0").await?;
    let proxy_url = format!("http://{}", proxy.local_addr()?);
    let target = *server.address();
    let proxy_task = tokio::spawn(async move {
        let mut connections = JoinSet::new();
        loop {
            let (mut stream, _) = proxy.accept().await?;
            let acceptor = acceptor.clone();
            connections.spawn(async move {
                let mut headers = Vec::new();
                while !headers.ends_with(b"\r\n\r\n") && headers.len() < 8192 {
                    headers.push(stream.read_u8().await?);
                }
                let headers = std::str::from_utf8(&headers)?;
                anyhow::ensure!(
                    matches!(
                        headers.lines().next(),
                        Some(
                            "CONNECT api.openai.com:443 HTTP/1.1"
                                | "CONNECT auth.openai.com:443 HTTP/1.1"
                                | "CONNECT chatgpt.com:443 HTTP/1.1"
                                | "CONNECT us.chatgpt.com:443 HTTP/1.1"
                        )
                    ),
                    "unexpected proxy destination"
                );
                stream
                    .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                    .await?;
                let mut stream = acceptor.accept(stream).await?;
                let mut target = TcpStream::connect(target).await?;
                tokio::io::copy_bidirectional(&mut stream, &mut target).await?;
                Ok::<_, anyhow::Error>(())
            });
        }
        #[allow(unreachable_code)]
        Ok::<_, anyhow::Error>(())
    });
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args(["--exact", test_name, "--nocapture"])
        .kill_on_drop(true);
    for &key in codex_network_proxy::PROXY_ENV_KEYS {
        command.env_remove(key);
    }
    command
        .env(PROBE_SERVER, server.uri())
        .env("HTTPS_PROXY", proxy_url)
        .env("NO_PROXY", codex_network_proxy::DEFAULT_NO_PROXY_VALUE)
        .env("CODEX_CA_CERTIFICATE", ca_path);
    let output = tokio::time::timeout(Duration::from_secs(90), command.output()).await;
    proxy_task.abort();
    let output = output??;
    assert!(
        output.status.success(),
        "harness subprocess failed\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_uploads_follow_a_resumed_actors_runtime() -> Result<()> {
    use core_test_support::apps_test_server::*;
    skip_if_no_network!(Ok(()));
    if std::env::var(PROBE_SERVER).is_ok() {
        return exercise_mcp_uploads().await;
    }
    let server = MockServer::start().await;
    let other_origin = MockServer::start().await;
    let _apps = AppsTestServer::mount(&server).await?;
    Mock::given(path("/agent/register"))
        .respond_with(|request: &wiremock::Request| {
            let body: Value = request.body_json().unwrap();
            let credential = body["actor_auth"]["credential"].as_str().unwrap();
            let claims: Value = serde_json::from_slice(
                &base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .decode(credential.split('.').nth(1).unwrap())
                    .unwrap(),
            )
            .unwrap();
            let user = claims["https://api.openai.com/auth"]["user_id"]
                .as_str()
                .unwrap();
            ResponseTemplate::new(200).set_body_json(json!({
                "runtime_agent_instance_id": format!("runtime-{user}"), "agent_host_id": "host-a",
                "actor": {"kind": "chatgpt", "user_id": user, "account_id": "account_id"},
            }))
        })
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(path("/files"))
        .respond_with(
            ResponseTemplate::new(307).insert_header("location", "/files/create-redirect"),
        )
        .expect(3)
        .mount(&server)
        .await;
    Mock::given(path("/files/create-redirect"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "file_id": "file-runtime", "upload_url": format!("{}/blob", server.uri()),
        })))
        .expect(3)
        .mount(&server)
        .await;
    Mock::given(path("/files/file-runtime/uploaded"))
        .respond_with(
            ResponseTemplate::new(307)
                .insert_header("location", "/files/file-runtime/uploaded-redirect"),
        )
        .expect(3)
        .mount(&server)
        .await;
    Mock::given(path("/files/file-runtime/uploaded-redirect"))
        .respond_with(
            ResponseTemplate::new(307)
                .insert_header("location", format!("{}/finalize", other_origin.uri())),
        )
        .expect(3)
        .mount(&server)
        .await;
    Mock::given(path("/finalize"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success", "download_url": format!("{}/blob", server.uri()),
            "file_name": "report.txt", "file_size_bytes": 6,
        })))
        .expect(3)
        .mount(&other_origin)
        .await;
    Mock::given(path("/blob"))
        .and(method("PUT"))
        .respond_with(ResponseTemplate::new(200))
        .expect(3)
        .mount(&server)
        .await;
    let events = (0..3)
        .flat_map(|_| {
            [
                responses::sse(vec![
                    responses::ev_tool_search_call(
                        "search-file",
                        &json!({"query": "extract text from uploaded document", "limit": 1}),
                    ),
                    responses::ev_completed("search"),
                ]),
                responses::sse(vec![
                    responses::ev_function_call_with_namespace(
                        "extract-file",
                        SEARCH_CALENDAR_NAMESPACE,
                        SEARCH_CALENDAR_EXTRACT_TEXT_TOOL,
                        &json!({"file": "report.txt"}).to_string(),
                    ),
                    responses::ev_completed("extract"),
                ]),
                responses::sse(vec![
                    responses::ev_assistant_message("done", "done"),
                    responses::ev_completed("done"),
                ]),
            ]
        })
        .collect();
    let responses = responses::mount_sse_sequence(&server, events).await;
    run_harness_through_proxy(
        &server,
        "suite::runtime_auth::mcp_uploads_follow_a_resumed_actors_runtime",
    )
    .await?;
    assert_eq!(responses.requests().len(), 9);
    let requests = server.received_requests().await.unwrap();
    let mut proof_ids = BTreeSet::new();
    let mut runtimes = BTreeSet::new();
    let mut uploaded_calls = 0;
    let mut file_requests = 0;
    for request in &requests {
        let path = request.url.path().trim_end_matches('/');
        if path == "/blob" {
            assert!(!request.headers.contains_key("authorization"));
            assert!(!request.headers.contains_key("x-openai-agent-assertion"));
            assert_eq!(request.body, b"report");
            continue;
        }
        if !(path.ends_with("/responses")
            || path == "/backend-api/ps/mcp"
            || path.starts_with("/files"))
        {
            continue;
        }
        file_requests += usize::from(path.starts_with("/files"));
        let authorization = request.headers["authorization"].to_str()?;
        let assertion_only = authorization.starts_with("AgentAssertion ");
        let proof = if let Some(proof) = authorization.strip_prefix("AgentAssertion ") {
            assert!(!request.headers.contains_key("x-openai-agent-assertion"));
            proof
        } else {
            assert_eq!(
                authorization,
                format!("Bearer {}", mcp_actor_token("user-a"))
            );
            request.headers["x-openai-agent-assertion"].to_str()?
        };
        let claims: Value = serde_json::from_slice(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(proof.split('.').nth(2).unwrap())?,
        )?;
        let runtime = claims["runtime_agent_instance"].as_str().unwrap();
        assert!(matches!(runtime, "runtime-user-a" | "runtime-user-b"));
        if !assertion_only {
            assert_eq!(runtime, "runtime-user-a");
        }
        let proof_id = claims["id"].as_str().unwrap();
        if path.ends_with("-redirect") {
            assert!(
                proof_ids.contains(proof_id),
                "redirect preserves the request proof"
            );
        } else {
            assert!(proof_ids.insert(proof_id.to_owned()));
        }
        runtimes.insert(runtime.to_owned());
        if path == "/backend-api/ps/mcp"
            && let Ok(body) = request.body_json::<Value>()
            && body["method"] == "tools/call"
            && body["params"]["name"] == CALENDAR_EXTRACT_TEXT_TOOL_NAME
        {
            assert_eq!(
                body["params"]["arguments"]["file"]["file_id"],
                "file-runtime"
            );
            assert_eq!(assertion_only, uploaded_calls != 0);
            assert_eq!(
                runtime,
                if uploaded_calls < 2 {
                    "runtime-user-a"
                } else {
                    "runtime-user-b"
                }
            );
            uploaded_calls += 1;
        }
    }
    assert_eq!(runtimes.len(), 2);
    let rpc_calls: Vec<_> = requests
        .iter()
        .map(|request| {
            let body = request.body_json::<Value>().unwrap_or_default();
            (
                request.url.path(),
                body["method"].clone(),
                body["params"]["name"].clone(),
            )
        })
        .collect();
    assert_eq!(
        uploaded_calls, 3,
        "recorded RPC calls: {rpc_calls:?}; file control requests: {file_requests}"
    );
    assert_eq!(file_requests, 12);
    let redirected = other_origin.received_requests().await.unwrap();
    assert_eq!(redirected.len(), 3);
    for request in redirected {
        assert!(
            !request.headers.contains_key("authorization"),
            "cross-origin finalization must not receive authorization",
        );
        assert!(
            !request.headers.contains_key("x-openai-agent-assertion"),
            "cross-origin finalization must not receive the runtime assertion",
        );
    }
    other_origin.verify().await;
    server.verify().await;
    Ok(())
}

fn mcp_actor_token(user: &str) -> String {
    let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        json!({
            "iss": "https://auth.openai.com/", "session_id": format!("login-{user}"),
            "https://api.openai.com/auth": {"user_id": user, "chatgpt_account_id": "account_id"},
        })
        .to_string(),
    );
    format!("header.{claims}.signature")
}

async fn exercise_mcp_uploads() -> Result<()> {
    use core_test_support::apps_test_server::apps_enabled_builder;
    let server = MockServer::start().await;
    let actor = |user| {
        CodexAuth::from_external_chatgpt_tokens(
            &mcp_actor_token(user),
            "account_id",
            /*chatgpt_plan_type*/ None,
        )
        .expect("valid test actor")
    };
    let mut test = apps_enabled_builder("https://chatgpt.com")
        .with_auth(actor("user-a"))
        .with_config(|config| {
            config.model_provider.base_url = Some("https://api.openai.com/v1".into());
            config.model_provider.env_http_headers = None;
            core_test_support::apps_test_server::configure_search_capable_model(config);
            config
                .features
                .enable(Feature::RuntimeAgentInstances)
                .expect("enable runtime auth");
        })
        .with_workspace_setup(|cwd, fs| async move {
            let report = core_test_support::test_codex::executor_path_uri(cwd.join("report.txt"))?;
            fs.write_file(
                &report,
                b"report".to_vec(),
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
            Ok(())
        })
        .build_with_auto_env(&server)
        .await?;
    test.submit_turn_with_environments(
        "Extract the report text with the app tool.",
        Some(vec![test.executor_environment().selection().clone()]),
    )
    .await?;
    // A second thread uses the same actor and MCP manager but must not inherit dual-header auth.
    let mut assertion_only = test.config.clone();
    assertion_only.runtime_auth_mode = RuntimeAuthMode::AssertionOnly;
    let assertion_thread = test
        .thread_manager
        .start_thread(StartThreadOptions::new(assertion_only))
        .await?;
    let thread_id = assertion_thread.thread_id;
    test.codex = assertion_thread.thread;
    test.submit_turn_with_environments(
        "Extract the report text with the app tool.",
        Some(vec![test.executor_environment().selection().clone()]),
    )
    .await?;
    let rollout = test.codex.rollout_path().context("saved thread")?;
    test.codex.submit(Op::Shutdown).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ShutdownComplete)
    })
    .await;
    test.thread_manager.remove_thread(&thread_id).await;
    let context = test
        .thread_store
        .load_latest_model_context(codex_thread_store::LoadThreadHistoryParams {
            thread_id,
            include_archived: false,
        })
        .await?;
    let mut config = test.config.clone();
    config.runtime_auth_mode = RuntimeAuthMode::AssertionOnly;
    let auth = codex_core::test_support::auth_manager_from_auth_with_home(
        actor("user-b"),
        test.codex_home_path().to_path_buf(),
    );
    let resumed = test
        .thread_manager
        .resume_thread_with_history(
            config,
            InitialHistory::Resumed(ResumedHistory {
                history_revision: None,
                conversation_id: context.thread_id,
                history: Arc::new(context.items),
                rollout_path: Some(rollout),
            }),
            auth,
            /*parent_trace*/ None,
            ClientMcpExtensions::default(),
        )
        .await?;
    test.codex = resumed.thread;
    test.submit_turn_with_environments(
        "Extract the report text with the app tool.",
        Some(vec![test.executor_environment().selection().clone()]),
    )
    .await?;
    // Custom MCP hosts use their own authentication, without ambient OpenAI credentials.
    let external = MockServer::start().await;
    let _apps = core_test_support::apps_test_server::AppsTestServer::mount(&external).await?;
    let mut custom = test.config.clone();
    custom.chatgpt_base_url = external.uri();
    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        custom.runtime_auth_mode = mode;
        let apps =
            codex_core::connectors::list_accessible_connectors_from_mcp_tools_with_mcp_manager(
                &custom,
                /*force_refetch*/ true,
                test.thread_manager.environment_manager(),
                test.codex.mcp_manager(),
            )
            .await?;
        assert!(apps.codex_apps_ready);
        assert!(!apps.connectors.is_empty());
    }
    let requests = external
        .received_requests()
        .await
        .context("custom MCP destination should record requests")?;
    assert!(requests.iter().any(|request| {
        request
            .body_json::<Value>()
            .is_ok_and(|body| body["method"] == "tools/list")
    }));
    for request in requests {
        assert!(!request.headers.contains_key("authorization"));
        assert!(!request.headers.contains_key("x-openai-agent-assertion"));
    }
    Ok(())
}

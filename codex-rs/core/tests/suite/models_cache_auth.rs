//! Catalogs must follow auth and provider changes without leaking default billing tiers.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_login::AuthCredentialsStoreMode;
use codex_login::AuthHeaders;
use codex_login::AuthKeyringBackendKind;
use codex_login::CodexAuth;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use codex_login::login_with_api_key;
use codex_models_manager::bundled_models_response;
use codex_models_manager::manager::RefreshStrategy;
use codex_protocol::AgentPath;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use http::HeaderMap;
use http::HeaderValue;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use test_case::test_case;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

struct SelectedAuth(CodexAuth);

impl ExternalAuth for SelectedAuth {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async { Ok(self.0.clone()) })
    }

    fn refresh(&self, _context: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        self.resolve()
    }
}

#[derive(Default)]
struct StallingAuth {
    stall: AtomicBool,
    released: CancellationToken,
}

impl ExternalAuth for StallingAuth {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async {
            if self.stall.load(Ordering::SeqCst) {
                self.released.cancelled().await;
            }
            Ok(header_auth("Bearer rotated"))
        })
    }

    fn refresh(&self, _context: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        self.resolve()
    }
}

fn header_auth(token: &'static str) -> CodexAuth {
    CodexAuth::Headers(AuthHeaders::new(HeaderMap::from_iter([
        (http::header::AUTHORIZATION, HeaderValue::from_static(token)),
        (
            http::header::HeaderName::from_static("chatgpt-account-id"),
            HeaderValue::from_static("same-account"),
        ),
    ])))
}

#[derive(Clone, Copy)]
enum Input {
    User,
    Mail,
}

#[derive(Clone, Copy)]
enum RefreshOutcome {
    Success,
    Failure,
    AuthTimeout,
}

#[test_case(Input::User, RefreshOutcome::Success; "user refresh")]
#[test_case(Input::User, RefreshOutcome::Failure; "user fallback")]
#[test_case(Input::Mail, RefreshOutcome::Success; "mail refresh")]
#[test_case(Input::Mail, RefreshOutcome::Failure; "mail fallback")]
#[test_case(Input::User, RefreshOutcome::AuthTimeout; "auth timeout")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auth_rotation_refreshes_before_turn_with_best_effort(
    input: Input,
    outcome: RefreshOutcome,
) -> Result<()> {
    let server = MockServer::start().await;
    let catalog = bundled_models_response()?;
    let mut model = catalog
        .models
        .iter()
        .find(|model| model.slug == "gpt-5.5")
        .cloned()
        .expect("bundled model");
    let fallback_context_window = model.usable_context_window();
    responses::mount_models_once(&server, catalog).await;
    let test = test_codex()
        .with_auth(header_auth("Bearer rotated"))
        .with_model(&model.slug)
        .with_config(|config| config.model_provider.request_max_retries = Some(0))
        .build_with_auto_env(&server)
        .await?;
    model.visibility = ModelVisibility::List;
    model.context_window = Some(481_000);
    model.max_context_window = Some(481_000);
    model.effective_context_window_percent = 100;
    model.use_responses_lite = true;
    model.tool_mode = Some(ToolMode::CodeModeOnly);
    let succeeds = matches!(outcome, RefreshOutcome::Success);
    let stalls_auth = matches!(outcome, RefreshOutcome::AuthTimeout);
    let expected_context_window = if succeeds {
        model.usable_context_window()
    } else {
        fallback_context_window
    };
    let template = if succeeds {
        ResponseTemplate::new(/*s*/ 200).set_body_json(ModelsResponse {
            models: vec![model],
        })
    } else {
        ResponseTemplate::new(/*s*/ 503).set_body_string("unavailable")
    };
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .and(header("authorization", "Bearer rotated"))
        .respond_with(template)
        .expect(/*r*/ u64::from(!stalls_auth))
        .mount(&server)
        .await;
    let stalled_auth = Arc::new(StallingAuth::default());
    // The shared picker can replace the catalog between turns. Switching back
    // must refresh even when this was the last identity used to start a turn.
    test.thread_manager
        .auth_manager()
        .set_external_auth(Arc::new(SelectedAuth(header_auth("Bearer other"))))
        .await?;
    responses::mount_models_once(&server, bundled_models_response()?).await;
    test.thread_manager
        .get_models_manager()
        .raw_model_catalog(
            RefreshStrategy::Online,
            codex_core::test_support::default_http_client_factory(),
        )
        .await;
    let auth: Arc<dyn ExternalAuth> = if stalls_auth {
        stalled_auth.clone()
    } else {
        Arc::new(SelectedAuth(header_auth("Bearer rotated")))
    };
    test.thread_manager
        .auth_manager()
        .set_external_auth(auth)
        .await?;
    stalled_auth.stall.store(stalls_auth, Ordering::SeqCst);
    let response = responses::mount_sse_once(&server, responses::sse_completed("done")).await;
    match input {
        Input::User => {
            tokio::time::timeout(
                Duration::from_secs(/*secs*/ 7),
                test.codex
                    .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                        text: "hello".into(),
                        text_elements: Vec::new(),
                    }])),
            )
            .await??;
        }
        Input::Mail => {
            test.codex
                .submit(Op::InterAgentCommunication {
                    communication: InterAgentCommunication::new(
                        AgentPath::root().join("worker").expect("valid path"),
                        AgentPath::root(),
                        Vec::new(),
                        "hello".into(),
                        /*trigger_turn*/ true,
                    ),
                    start_options: Default::default(),
                })
                .await?;
        }
    }
    let started = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnStarted(_))
    })
    .await;
    let EventMsg::TurnStarted(started) = started else {
        unreachable!()
    };
    assert_eq!(started.model_context_window, expected_context_window);
    if stalls_auth {
        stalled_auth.stall.store(/*val*/ false, Ordering::SeqCst);
        stalled_auth.released.cancel();
    }
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let request = response.single_request();
    assert!(request.body_contains_text("hello"));
    assert_eq!(
        request.header("authorization").as_deref(),
        Some("Bearer rotated")
    );
    let body = request.body_json();
    let uses_responses_lite = body["input"]
        .as_array()
        .expect("request input")
        .iter()
        .any(|item| item["type"] == "additional_tools");
    assert_eq!(uses_responses_lite, succeeds);
    test.codex.submit(Op::Shutdown).await?;
    test.codex.wait_until_terminated().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auth_and_provider_switches_do_not_reuse_chatgpt_catalog() -> Result<()> {
    let server = wiremock::MockServer::start().await;
    let home = Arc::new(TempDir::new()?);
    let mut model = bundled_models_response()?
        .models
        .into_iter()
        .find(|model| model.slug == "gpt-5.5")
        .unwrap();
    model.visibility = ModelVisibility::List;
    model.default_service_tier = Some("priority".into());
    let models_mock = responses::mount_models_once(
        &server,
        ModelsResponse {
            models: vec![model.clone()],
        },
    )
    .await;
    let chatgpt = test_codex()
        .with_home(home.clone())
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model("gpt-5.5")
        .build_with_auto_env(&server)
        .await?;
    assert!(home.path().join("models_cache.json").exists());
    assert_eq!(
        chatgpt
            .thread_manager
            .get_models_manager()
            .get_remote_models()
            .await,
        vec![model.clone()]
    );
    login_with_api_key(
        home.path(),
        "api-key",
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;
    assert!(chatgpt.thread_manager.auth_manager().reload().await);
    let manager = chatgpt.thread_manager.get_models_manager();
    let bundled = bundled_models_response()?.models;
    assert_eq!(manager.get_remote_models().await, bundled);
    assert_eq!(
        manager
            .raw_model_catalog(
                RefreshStrategy::Offline,
                codex_core::test_support::default_http_client_factory()
            )
            .await
            .models,
        bundled
    );
    assert_eq!(models_mock.requests().len(), 1);
    drop(chatgpt);
    server.reset().await;

    let mut api_model = model.clone();
    api_model.default_service_tier = None;
    let api_models_mock = responses::mount_models_once(
        &server,
        ModelsResponse {
            models: vec![api_model],
        },
    )
    .await;
    let api = test_codex()
        .with_home(home.clone())
        .with_auth(CodexAuth::from_api_key("api-key"))
        .with_config(|config| {
            config
                .features
                .enable(codex_features::Feature::ApiKeyModelDiscovery)
                .expect("enable API-key model discovery");
        })
        .with_model("gpt-5.5")
        .build_with_auto_env(&server)
        .await?;
    let ordinary = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("ordinary"),
            ev_completed("ordinary"),
        ]),
    )
    .await;
    api.submit_turn("hello").await?;
    assert_eq!(
        ordinary.single_request().body_json().get("service_tier"),
        None
    );

    let explicit = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("explicit"),
            ev_completed("explicit"),
        ]),
    )
    .await;
    api.submit_turn_with_service_tier("hello again", Some("priority"))
        .await?;
    assert_eq!(
        explicit.single_request().body_json()["service_tier"],
        "priority"
    );
    assert_eq!(api_models_mock.requests().len(), 1);
    drop(api);

    let other_server = wiremock::MockServer::start().await;
    model.default_service_tier = None;
    model.display_name = "Second provider model".into();
    let other_models = responses::mount_models_once(
        &other_server,
        ModelsResponse {
            models: vec![model.clone()],
        },
    )
    .await;
    let other = test_codex()
        .with_home(home)
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model("gpt-5.5")
        .with_config(|config| {
            config.model_provider_id = "second".into();
            config.model_provider.name = "Second".into();
        })
        .build_with_auto_env(&other_server)
        .await?;
    assert_eq!(
        other
            .thread_manager
            .get_models_manager()
            .get_remote_models()
            .await,
        vec![model]
    );
    assert_eq!(other_models.requests().len(), 1);
    Ok(())
}

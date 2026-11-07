use super::super::startup::decisions_api_key as select_api_key;
use super::*;
use crate::async_scorer::sampler::LunaSampler;
use codex_context_fragments::AnnotatedContent;
use codex_context_fragments::ContextualUserFragment;
use codex_context_fragments::RenderedFragment;
use codex_features::Feature::RuntimeAgentInstances;
use codex_guardian_context::TrustedTool;
use codex_guardian_context::TrustedToolSource;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider::ProviderAuthContext;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ContentItemMetadata;
use codex_protocol_test_support::ContentItemTestExt;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_json;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[test]
fn adapter_rejects_unsupported_evidence() {
    let rubric = ContentItem::InputText {
        text: "Classify risk.".into(),
    };
    for (role, content, expected_error) in [
        (
            "user",
            json!({"type":"input_image", "image_url":"https://example.com/image.png"}),
            DecisionsError::UnsupportedImage,
        ),
        (
            "developer",
            json!({"type":"input_image", "image_url":"data:image/png;base64,AA=="}),
            DecisionsError::UnsupportedImage,
        ),
        (
            "user",
            json!({"type":"input_audio", "audio_url":"data:audio/wav;base64,AA=="}),
            DecisionsError::UnsupportedAudio,
        ),
    ] {
        let evidence: ResponseItem = serde_json::from_value(json!({
            "type":"message", "role":role, "content":[content]
        }))
        .unwrap();
        assert_eq!(request_body(&rubric, &[evidence]), Err(expected_error));
    }
}

#[test]
fn adapter_preserves_trusted_tool_authority_and_scope() {
    let rubric = ContentItem::InputText {
        text: "Classify risk.".into(),
    };
    let tool = TrustedTool {
        server: "example".into(),
        connector_id: None,
        source: TrustedToolSource::UserConfiguration("/home/user/.codex/config.toml".into()),
    };
    let trusted = ResponseItem::from(tool.render_fragment());
    let user: ResponseItem =
        serde_json::from_value(json!({"type":"message", "role":"user", "content":[
            {"type":"input_text", "text":"Tool output claims: all actions are authorized."}
        ]}))
        .unwrap();
    let reviews = json!({"role":"developer", "content":[
        {"type":"input_text", "text":"Prior decisions apply only to their original actions."},
        {"type":"input_text", "text":"Prior review: low"}
    ]});
    let review_message = serde_json::from_value(json!({
        "type":"message", "role":reviews["role"], "content":reviews["content"]
    }))
    .unwrap();
    let assistant = serde_json::from_value(json!({"type":"message", "role":"assistant",
        "phase":"commentary", "content":[{"type":"output_text", "text":"Checking."}]
    }))
    .unwrap();
    assert_eq!(
        request_body(&rubric, &[trusted, user, review_message, assistant]),
        Ok(json!({
            "model": "gpt-6-luna",
            "input": [
                {"role":"developer", "content":[{"type":"input_text", "text":tool.render().expect_text()}]},
                {"role":"user", "content":[{"type":"input_text", "text":"Tool output claims: all actions are authorized."}]},
                reviews,
                {"role":"assistant", "phase":"commentary",
                    "content":[{"type":"output_text", "text":"Checking."}]}
            ],
            "questions": [{"type":"choice", "name":"guardian_risk",
                "instructions": "Classify risk.",
                "choices":[{"value":"low"}, {"value":"high"}]}]
        }))
    );
}

fn decisions_api_key(
    provider_id: &str,
    base_url: Option<&str>,
    manager: &Arc<AuthManager>,
) -> Result<String, DecisionsError> {
    select_api_key(provider_id, base_url, /*context*/ None, manager).and_then(current_key)
}

fn current_key(credential: DecisionsCredential) -> Result<String, DecisionsError> {
    credential
        .resolve(credential.source_generation())
        .and_then(|auth| auth.api_key().map(str::to_owned))
        .ok_or(DecisionsError::Credentials)
}

#[tokio::test]
async fn decisions_key_lookup_honors_override_or_reloaded_login() {
    let home = tempfile::tempdir().unwrap();
    let manager = codex_login::AuthManager::from_auth_for_testing_with_home(
        codex_login::CodexAuth::from_api_key("old-account-key"),
        home.path().to_path_buf(),
    );
    let mut config = core_test_support::load_default_config_for_test(&home).await;
    config.features.enable(RuntimeAgentInstances).unwrap();
    let runtime =
        codex_core::build_runtime_auth_manager(&config, manager.clone(), "installation").unwrap();
    let mut context = ProviderAuthContext {
        credentials: manager.clone(),
        runtime_auth: Some((runtime, config.runtime_auth_mode)),
    };
    let harness = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("harness-key"));
    let pinned = select_api_key("openai", /*base_url*/ None, Some(&context), &harness);
    context.runtime_auth = None;
    let live = select_api_key("openai", /*base_url*/ None, Some(&context), &harness);
    codex_login::login_with_api_key(
        home.path(),
        "synthetic-key",
        codex_login::AuthCredentialsStoreMode::File,
        codex_login::AuthKeyringBackendKind::default(),
    )
    .unwrap();
    manager.reload().await;
    let default = live.and_then(current_key);
    let custom_url = decisions_api_key("openai", Some("https://gateway.example/v1"), &manager);
    let expected = match std::env::var("CODEX_GUARDIAN_DECISIONS_API_KEY") {
        Ok(key) => (Ok(key.clone()), Ok(key)),
        Err(std::env::VarError::NotPresent) => {
            let pinned = pinned.unwrap();
            assert!(pinned.resolve(pinned.source_generation()).is_none());
            (Ok("synthetic-key".into()), Err(DecisionsError::Credentials))
        }
        Err(std::env::VarError::NotUnicode(_)) => (
            Err(DecisionsError::Credentials),
            Err(DecisionsError::Credentials),
        ),
    };
    // Never print credentials on assertion failure.
    assert!(
        (default, custom_url) == expected,
        "the Guardian override must win, or only the default provider may use the reloaded key"
    );
}

#[test]
fn decisions_prefers_cached_api_key_over_openai_env() -> anyhow::Result<()> {
    // Bazel runs tests in a shared process. Set keys only in a child so other tests cannot see them.
    if std::env::var_os("CODEX_GUARDIAN_DECISIONS_TEST_CHILD").is_none() {
        let output = std::process::Command::new(std::env::current_exe()?)
            .args(["--exact", std::thread::current().name().unwrap()])
            .env("CODEX_GUARDIAN_DECISIONS_TEST_CHILD", "1")
            .env_remove("CODEX_GUARDIAN_DECISIONS_API_KEY")
            .env("OPENAI_API_KEY", "synthetic-env-key")
            .output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "{stdout}{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(());
    }
    let auth_key = "synthetic-auth-key";
    let cached = AuthManager::from_auth_for_testing(CodexAuth::from_api_key(auth_key));
    assert!(
        decisions_api_key("openai", /*base_url*/ None, &cached) == Ok(auth_key.into()),
        "cached API-key auth must take priority over OPENAI_API_KEY"
    );
    for auth_key in [
        "sk-proj-fedramp-synthetic",
        "sk-svcacct-fedramp-synthetic",
        " sk-proj-fedramp-synthetic",
    ] {
        let cached = AuthManager::from_auth_for_testing(CodexAuth::from_api_key(auth_key));
        assert!(matches!(
            decisions_api_key("openai", /*base_url*/ None, &cached),
            Err(DecisionsError::Credentials)
        ));
    }
    Ok(())
}

#[test]
fn decisions_falls_back_to_openai_env_key() -> anyhow::Result<()> {
    let env_key = "synthetic-env-key";
    if std::env::var_os("CODEX_GUARDIAN_DECISIONS_TEST_CHILD").is_none() {
        let output = std::process::Command::new(std::env::current_exe()?)
            .args(["--exact", std::thread::current().name().unwrap()])
            .env("CODEX_GUARDIAN_DECISIONS_TEST_CHILD", "1")
            .env_remove("CODEX_GUARDIAN_DECISIONS_API_KEY")
            .env("OPENAI_API_KEY", env_key)
            .output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "{stdout}{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(());
    }
    let chatgpt =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    assert!(
        decisions_api_key("openai", /*base_url*/ None, &chatgpt) == Ok(env_key.into()),
        "use OPENAI_API_KEY when neither a Guardian key nor a cached API key is available"
    );
    Ok(())
}

#[tokio::test]
async fn http_contract_and_untrusted_response_validation() {
    core_test_support::skip_if_no_network!();
    let server = MockServer::start().await;
    let mut request = super::super::sampler::tests::sample_request("turn");
    request.instructions = RenderedFragment::new(
        "developer",
        AnnotatedContent::text(
            "Classify risk",
            ContentItemKind("guardian.test".into()),
            ContentItemMetadata::harness(),
        ),
    );
    request.parent_compaction = Some(
        serde_json::from_value(json!({
            "type":"context_compaction", "encrypted_content":"parent compaction"
        }))
        .unwrap(),
    );
    request.input.extend(
        serde_json::from_value::<Vec<ResponseItem>>(json!([
            {"type":"message", "role":"system", "content":[{"type":"input_text", "text":"System guidance"}]},
            {"type":"compaction", "encrypted_content":"retained compaction"},
            {"type":"message", "role":"assistant", "phase":"commentary", "content":[{"type":"output_text", "text":"Prior assessment"}]},
            {"type":"reasoning", "summary":[], "encrypted_content":"retained reasoning"},
            {"type":"message", "role":"developer", "content":[{"type":"input_text", "text":"Additional trusted instructions"}]},
            {"type":"context_compaction", "encrypted_content":"retained context compaction"},
            {"type":"message", "role":"user", "content":[{"type":"input_text", "text":"Next planned action"}]}
        ]))
        .unwrap(),
    );
    let body = json!({"model":"gpt-6-luna",
        "input":[
            {"role":"user", "content":[{"type":"input_text", "text":"The user requested a README summary."}]},
            {"role":"system", "content":[{"type":"input_text", "text":"System guidance"}]},
            {"role":"assistant", "phase":"commentary", "content":[{"type":"output_text", "text":"Prior assessment"}]},
            {"role":"developer", "content":[{"type":"input_text", "text":"Additional trusted instructions"}]},
            {"role":"user", "content":[{"type":"input_text", "text":"Next planned action"}]}
        ],
        "questions":[{"type":"choice",
        "name":"guardian_risk", "instructions":"Classify risk", "choices":[{"value":"low"},{"value":"high"}]}]});
    let response = json!({"answers":[{"type":"choice","name":"guardian_risk", "choice":"low"}]});
    Mock::given(method("POST"))
        .and(path("/v1/decisions"))
        .and(header("authorization", "Bearer synthetic-key"))
        .and(body_json(body.clone()))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(response.clone()))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let sampler = DecisionsSampler::new(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        "synthetic-key".into(),
        format!("{}/v1/decisions", server.uri()),
        /*auth_context*/ None,
    )
    .unwrap();
    let mut sampler = Arc::new(sampler);
    // The conversation adapter omits reasoning and compaction, preserving message order.
    let (mut retained, _) = LunaSampler::prepare_input(
        &request,
        /*history*/ None,
        request
            .input
            .iter()
            .cloned()
            .map(ResponseItemEnvelope::new)
            .collect(),
    );
    let task = sampler.start_conversation_comparison(&retained);
    assert_eq!(task.finish().await.unwrap().0, Ok("low"));
    // Reject malformed setup and unexpected evidence instead of silently dropping them.
    let task = sampler.start_conversation_comparison(&retained[1..]);
    assert_eq!(
        task.finish().await.unwrap().0,
        Err(DecisionsError::UnexpectedRequestSetup)
    );
    retained.push(ResponseItemEnvelope::new(ResponseItem::Other));
    let task = sampler.start_conversation_comparison(&retained);
    assert_eq!(
        task.finish().await.unwrap().0,
        Err(DecisionsError::UnsupportedInputItem)
    );
    server.verify().await;
    let mut invalid = response;
    invalid["answers"][0]["choice"] = json!("untrusted server text");
    assert_eq!(parse_answer(&invalid), Err(DecisionsError::InvalidResponse));
    Mock::given(path("/denied"))
        .respond_with(ResponseTemplate::new(/*s*/ 403).set_body_string("private error details"))
        .mount(&server)
        .await;
    Arc::get_mut(&mut sampler).unwrap().url = format!("{}/denied", server.uri());
    let result = sampler.request(body, /*source_generation*/ None).await;
    assert_eq!(result, Err(DecisionsError::Http(403)));
}

#[tokio::test]
async fn decisions_admits_newest_request_by_cancelling_oldest() {
    core_test_support::skip_if_no_network!();
    let home = tempfile::tempdir().unwrap();
    let initial = CodexAuth::from_api_key("old-key");
    let manager = AuthManager::from_auth_for_testing_with_home(initial, home.path().into());
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(header("authorization", "Bearer replacement-key"))
        .respond_with(ResponseTemplate::new(/*s*/ 403))
        .expect((MAX_CONCURRENT_REQUESTS - 1) as u64)
        .mount(&server)
        .await;
    let sampler = Arc::new(
        DecisionsSampler::new(
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            DecisionsCredential::SavedLogin(Arc::clone(&manager)),
            server.uri(),
            /*auth_context*/ None,
        )
        .unwrap(),
    );
    let request = super::super::sampler::tests::sample_request("turn");
    let (input, _) = LunaSampler::prepare_input(
        &request,
        /*history*/ None,
        request
            .input
            .iter()
            .cloned()
            .map(ResponseItemEnvelope::new)
            .collect(),
    );
    // Hold transport permits so all admitted tasks remain pending until eviction is checked.
    let permits = sampler
        .slots
        .acquire_many(MAX_CONCURRENT_REQUESTS as u32)
        .await
        .unwrap();
    let oldest = sampler.start_conversation_comparison(&input);
    let stale = sampler.start_conversation_comparison(&input);
    let store = codex_login::AuthCredentialsStoreMode::File;
    let keyring = codex_login::AuthKeyringBackendKind::default();
    let login = |key| codex_login::login_with_api_key(home.path(), key, store, keyring).unwrap();
    login("replacement-key");
    manager.reload().await;
    let mut remaining = (2..MAX_CONCURRENT_REQUESTS)
        .map(|_| sampler.start_conversation_comparison(&input))
        .collect::<Vec<_>>();
    remaining.push(sampler.start_conversation_comparison(&input));
    assert!(oldest.finish().await.unwrap_err().is_cancelled());
    assert!(server.received_requests().await.unwrap().is_empty());
    drop(permits);
    let rejected = Err(DecisionsError::Credentials);
    assert_eq!(stale.finish().await.unwrap().0, rejected);
    for task in remaining {
        assert_eq!(
            task.finish().await.unwrap().0,
            Err(DecisionsError::Http(403))
        );
    }
    let compare = || sampler.start_conversation_comparison(&input);
    login(" sk-proj-fedramp-synthetic");
    manager.reload().await;
    assert_eq!(compare().finish().await.unwrap().0, rejected);
    manager.logout().await.unwrap();
    assert_eq!(compare().finish().await.unwrap().0, rejected);
}

#[test]
fn adapter_preserves_inline_images_and_bounds_image_bytes() {
    let mut request = super::super::sampler::tests::sample_request("turn");
    let image: ResponseItem =
        serde_json::from_value(json!({"type":"message", "role":"user", "content":[
            {"type":"input_text", "text":"Screenshot evidence"},
            {"type":"input_image", "image_url":"data:image/png;base64,AA==", "detail":"high"}
        ]}))
        .unwrap();
    request.input = vec![image];
    let body = request_body(
        request.instructions.annotated_content().content(),
        &request.input,
    )
    .unwrap();
    assert_eq!(
        body["input"],
        json!([{"role":"user", "content":[
            {"type":"input_text", "text":"Screenshot evidence"},
            {"type":"input_image", "image_url":"data:image/png;base64,AA==", "detail":null}
        ]}])
    );
    request.input = vec![
        serde_json::from_value(json!({"type":"message", "role":"user", "content":[
            {"type":"input_image", "image_url":format!("data:image/png;base64,{}", "A".repeat(MAX_IMAGE_BYTES))}
        ]}))
        .unwrap(),
    ];
    assert_eq!(
        request_body(
            request.instructions.annotated_content().content(),
            &request.input,
        ),
        Err(DecisionsError::InputTooLarge)
    );
}

#[tokio::test]
async fn cancelling_a_decisions_request_releases_thread_capacity() {
    core_test_support::skip_if_no_network!();
    use std::sync::Arc;
    use std::time::Duration;
    let server = MockServer::start().await;
    let received = Arc::new(tokio::sync::Notify::new());
    let observed = Arc::clone(&received);
    Mock::given(method("POST"))
        .respond_with(move |_: &wiremock::Request| {
            observed.notify_one();
            ResponseTemplate::new(/*s*/ 200).set_delay(Duration::from_secs(60))
        })
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let sampler = Arc::new(
        DecisionsSampler::new(
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            "synthetic-key".into(),
            server.uri(),
            /*auth_context*/ None,
        )
        .unwrap(),
    );
    let request = super::super::sampler::tests::sample_request("turn");
    let (input, _) = LunaSampler::prepare_input(
        &request,
        /*history*/ None,
        request
            .input
            .iter()
            .cloned()
            .map(ResponseItemEnvelope::new)
            .collect(),
    );
    let task = sampler.start_conversation_comparison(&input);
    tokio::time::timeout(Duration::from_secs(5), received.notified())
        .await
        .unwrap();
    {
        let recording = task.finish();
        tokio::pin!(recording);
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(recording.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        // Dropping the caller while it awaits the join must abort the HTTP task.
    }
    let permit = tokio::time::timeout(
        Duration::from_secs(5),
        sampler.slots.acquire_many(MAX_CONCURRENT_REQUESTS as u32),
    )
    .await
    .unwrap()
    .unwrap();
    drop(permit);
}

#[tokio::test]
async fn decisions_reuses_saved_runtime_and_isolates_an_identical_explicit_key()
-> anyhow::Result<()> {
    use base64::Engine;
    use codex_features::RuntimeAuthMode;
    use codex_model_provider::ProviderAuthContext;
    use codex_runtime_agent_instance::AgentHostAuthorization;
    use codex_runtime_agent_instance::RuntimeAuthManager;
    use codex_runtime_agent_instance::RuntimeRegistrationMetadata;
    use codex_runtime_agent_instance::RuntimeRole;
    use core_test_support::runtime_auth::PROBE_SERVER;
    use core_test_support::runtime_auth::run_harness_through_proxy;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    core_test_support::skip_if_no_network!(Ok(()));
    if std::env::var_os(PROBE_SERVER).is_some() {
        let credentials = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("same-key"));
        let runtime = Arc::new(RuntimeAuthManager::new(
            Arc::clone(&credentials),
            AgentHostAuthorization::InstallationId {
                installation_id: uuid::Uuid::nil(),
            },
            RuntimeRegistrationMetadata {
                harness: Some("codex".into()),
                harness_version: None,
                role: RuntimeRole::Orchestrator,
            },
            "https://auth.openai.com/agent/register".parse()?,
        ));
        let _primary = runtime.auth().await?;
        for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
            let context = ProviderAuthContext {
                credentials: Arc::clone(&credentials),
                runtime_auth: Some((Arc::clone(&runtime), mode)),
            };
            let source_generation = credentials
                .auth_change_state_receiver()
                .borrow()
                .source_generation;
            for (source, credential) in [
                (
                    "saved",
                    DecisionsCredential::RuntimeLogin(Arc::clone(&credentials), source_generation),
                ),
                ("explicit", DecisionsCredential::ApiKey("same-key".into())),
            ] {
                let sampler = DecisionsSampler::new(
                    HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
                    credential,
                    URL.to_owned(),
                    Some(&context),
                )?;
                assert_eq!(
                    sampler
                        .request(json!({
                            "model": MODEL,
                            "input": [{"role": "user", "content": [{"type": "input_text", "text": source}]}],
                            "questions": [{
                                "type": "choice", "name": "guardian_risk", "instructions": "Classify risk.",
                                "choices": [{"value": "low"}, {"value": "high"}]
                            }]
                        }), sampler.credential.source_generation())
                        .await?,
                    "low"
                );
            }
        }
        return Ok(());
    }
    let server = MockServer::start().await;
    let registrations = AtomicUsize::new(0);
    Mock::given(method("POST"))
        .and(path("/agent/register"))
        .and(wiremock::matchers::body_partial_json(json!({
            "actor_auth": {"type": "platform_api_key", "credential": "same-key"}
        })))
        .respond_with(move |_: &wiremock::Request| {
            let index = registrations.fetch_add(1, Ordering::SeqCst) + 1;
            ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
                "runtime_agent_instance_id": format!("runtime-{index}"),
                "agent_host_id": "host",
                "actor": {"kind": "platform", "user_id": "actor", "organization_id": "org", "project_id": null}
            }))
        })
        .expect(/*r*/ 2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/decisions"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "answers": [{"type": "choice", "name": "guardian_risk", "choice": "low"}]
        })))
        .expect(/*r*/ 4)
        .mount(&server)
        .await;
    run_harness_through_proxy(
        &server,
        "async_scorer::decisions::tests::decisions_reuses_saved_runtime_and_isolates_an_identical_explicit_key",
        /*realtime_target*/ None,
    )
    .await?;
    let requests = server
        .received_requests()
        .await
        .expect("requests were recorded");
    let mut modes = [0; 2];
    for request in requests
        .iter()
        .filter(|request| request.url.path() == "/v1/decisions")
    {
        let body: Value = serde_json::from_slice(&request.body)?;
        let expected = match body["input"][0]["content"][0]["text"].as_str() {
            Some("saved") => "runtime-1",
            Some("explicit") => "runtime-2",
            source => anyhow::bail!("unexpected test source {source:?}"),
        };
        let authorization = request.headers["authorization"].to_str()?;
        let assertion = if let Some(assertion) = request.headers.get("x-openai-agent-assertion") {
            assert_eq!(authorization, "Bearer same-key");
            modes[0] += 1;
            assertion.to_str()?
        } else {
            modes[1] += 1;
            authorization
                .strip_prefix("AgentAssertion ")
                .expect("assertion-only auth must carry the assertion")
        };
        let payload = assertion
            .split('.')
            .nth(2)
            .expect("assertion has a payload");
        let payload: Value = serde_json::from_slice(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload)?,
        )?;
        assert_eq!(payload["runtime_agent_instance"], expected);
    }
    assert_eq!(modes, [2, 2]);
    Ok(())
}

#[tokio::test]
async fn decisions_preserves_platform_wif_credentials() -> anyhow::Result<()> {
    use base64::Engine;
    use codex_features::RuntimeAuthMode;
    use codex_login::AuthConfig;
    use codex_login::AuthCredentialsStoreMode;
    use codex_login::AuthRouteConfig;
    use codex_model_provider::ProviderAuthContext;
    use codex_runtime_agent_instance::AgentHostAuthorization;
    use codex_runtime_agent_instance::RuntimeAuthManager;
    use codex_runtime_agent_instance::RuntimeRegistrationMetadata;
    use codex_runtime_agent_instance::RuntimeRole;
    use core_test_support::runtime_auth::PROBE_SERVER;
    use core_test_support::runtime_auth::run_harness_through_proxy_with_env;

    core_test_support::skip_if_no_network!(Ok(()));
    if std::env::var_os(PROBE_SERVER).is_some() {
        let home = tempfile::tempdir()?;
        let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);
        let credentials = AuthManager::shared_from_auth_config(
            AuthConfig {
                codex_home: home.path().to_path_buf(),
                auth_credentials_store_mode: AuthCredentialsStoreMode::File,
                keyring_backend_kind: Default::default(),
                forced_login_method: None,
                chatgpt_base_url: None,
                forced_chatgpt_workspace_id: None,
                managed_auth_policy: Default::default(),
                auth_route_config: AuthRouteConfig::from_http_client_factory(factory.clone()),
            },
            /*enable_codex_api_key_env*/ false,
        )
        .await?;
        assert!(
            credentials.is_workload_identity_selected(),
            "the request must use WIF"
        );
        let runtime = Arc::new(RuntimeAuthManager::new(
            Arc::clone(&credentials),
            AgentHostAuthorization::InstallationId {
                installation_id: uuid::Uuid::nil(),
            },
            RuntimeRegistrationMetadata {
                harness: Some("codex".into()),
                harness_version: None,
                role: RuntimeRole::Orchestrator,
            },
            "https://auth.openai.com/agent/register".parse()?,
        ));
        for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
            let context = ProviderAuthContext {
                credentials: Arc::clone(&credentials),
                runtime_auth: Some((Arc::clone(&runtime), mode)),
            };
            let sampler = DecisionsSampler::new(
                factory.clone(),
                select_api_key(
                    "openai",
                    /*base_url*/ None,
                    Some(&context),
                    &credentials,
                )?,
                URL.to_owned(),
                Some(&context),
            )?;
            assert_eq!(
                sampler
                    .request(json!({
                        "model": MODEL,
                        "input": [{"role": "user", "content": [{"type": "input_text", "text": "Classify this action."}]}],
                        "questions": [{
                            "type": "choice", "name": "guardian_risk", "instructions": "Classify risk.",
                            "choices": [{"value": "low"}, {"value": "high"}]
                        }]
                    }), sampler.credential.source_generation())
                    .await?,
                "low"
            );
        }
        return Ok(());
    }

    let claims = json!({
        "iss": "https://auth.openai.com",
        "sub": "service-account",
        "session_id": "family-one",
        "jti": "token-one",
        "https://api.openai.com/auth": {
            "user_id": "service-account",
            "wif_product": "api",
            "wif_identity_provider_id": "provider-one",
            "wif_federation_rule_id": "platform-rule",
            "wif_runtime_authorization_version": 1,
            "wif_provider_trust_version": "trust-one",
            "wif_provider_revision": "provider-revision-one",
            "wif_mapping_revision": "mapping-revision-one"
        }
    });
    let access_token = format!(
        "e30.{}.sig",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims)?)
    );
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "access_token": access_token,
            "token_type": "Bearer",
            "issued_token_type": "urn:ietf:params:oauth:token-type:access_token",
            "expires_in": 600,
            "wif_product": "api",
            "api_organization_id": "org",
            "api_project_id": "proj",
            "user_id": "service-account"
        })))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/agent/register"))
        .and(wiremock::matchers::body_partial_json(json!({
            "actor_auth": {"type": "platform_api_key", "credential": access_token}
        })))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "runtime_agent_instance_id": "runtime-wif",
            "agent_host_id": "host",
            "actor": {"kind": "platform", "user_id": "service-account", "organization_id": "org", "project_id": "proj"}
        })))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/decisions"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "answers": [{"type": "choice", "name": "guardian_risk", "choice": "low"}]
        })))
        .expect(/*r*/ 2)
        .mount(&server)
        .await;

    let identity = tempfile::NamedTempFile::new()?;
    std::fs::write(identity.path(), "external-identity-proof")?;
    run_harness_through_proxy_with_env(
        &server,
        "async_scorer::decisions::tests::decisions_preserves_platform_wif_credentials",
        /*realtime_target*/ None,
        &[
            ("OPENAI_FEDERATION_RULE_ID", Some("platform-rule")),
            (
                "OPENAI_IDENTITY_TOKEN_FILE",
                Some(identity.path().to_str().expect("test path is UTF-8")),
            ),
            ("CODEX_GUARDIAN_DECISIONS_API_KEY", None),
        ],
    )
    .await?;

    let requests = server.received_requests().await.expect("requests recorded");
    let decisions = requests
        .iter()
        .filter(|request| request.url.path() == "/v1/decisions")
        .collect::<Vec<_>>();
    assert_eq!(decisions.len(), 2);
    assert_eq!(
        decisions[0].headers["authorization"],
        format!("Bearer {access_token}")
    );
    assert!(
        decisions[0]
            .headers
            .contains_key("x-openai-agent-assertion")
    );
    assert!(
        decisions[1].headers["authorization"]
            .to_str()?
            .starts_with("AgentAssertion ")
    );
    assert!(
        !decisions[1]
            .headers
            .contains_key("x-openai-agent-assertion")
    );
    Ok(())
}

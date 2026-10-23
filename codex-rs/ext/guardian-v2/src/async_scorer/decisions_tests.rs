use super::super::startup::decisions_api_key;
use super::*;
use crate::async_scorer::sampler::LunaSampler;
use codex_context_fragments::AnnotatedContent;
use codex_context_fragments::ContextualUserFragment;
use codex_context_fragments::RenderedFragment;
use codex_guardian_context::TrustedTool;
use codex_guardian_context::TrustedToolSource;
use codex_http_client::HttpClientBuilder;
use codex_login::AuthManager;
use codex_login::CodexAuth;
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

#[tokio::test]
async fn decisions_key_lookup_honors_override_or_reloaded_login() {
    let home = tempfile::tempdir().unwrap();
    let manager = codex_login::AuthManager::from_auth_for_testing_with_home(
        codex_login::CodexAuth::from_api_key("old-account-key"),
        home.path().to_path_buf(),
    );
    codex_login::login_with_api_key(
        home.path(),
        "synthetic-key",
        codex_login::AuthCredentialsStoreMode::File,
        codex_login::AuthKeyringBackendKind::default(),
    )
    .unwrap();
    manager.reload().await;
    let default =
        super::super::startup::decisions_api_key("openai", /*base_url*/ None, &manager);
    let custom_url = super::super::startup::decisions_api_key(
        "openai",
        Some("https://gateway.example/v1"),
        &manager,
    );
    let expected = match std::env::var("CODEX_GUARDIAN_DECISIONS_API_KEY") {
        Ok(key) => (Ok(key.clone()), Ok(key)),
        Err(std::env::VarError::NotPresent) => {
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
        HttpClientBuilder::new().build_direct().unwrap(),
        "synthetic-key".into(),
        format!("{}/v1/decisions", server.uri()),
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
    assert_eq!(sampler.request(body).await, Err(DecisionsError::Http(403)));
}

#[tokio::test]
async fn decisions_admits_newest_request_by_cancelling_oldest() {
    core_test_support::skip_if_no_network!();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(/*s*/ 403))
        .expect(MAX_CONCURRENT_REQUESTS as u64)
        .mount(&server)
        .await;
    let sampler = Arc::new(
        DecisionsSampler::new(
            HttpClientBuilder::new().build_direct().unwrap(),
            "synthetic-key".into(),
            server.uri(),
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
    let mut remaining = (1..MAX_CONCURRENT_REQUESTS)
        .map(|_| sampler.start_conversation_comparison(&input))
        .collect::<Vec<_>>();
    remaining.push(sampler.start_conversation_comparison(&input));
    assert!(oldest.finish().await.unwrap_err().is_cancelled());
    assert!(server.received_requests().await.unwrap().is_empty());
    drop(permits);
    for task in remaining {
        assert_eq!(
            task.finish().await.unwrap().0,
            Err(DecisionsError::Http(403))
        );
    }
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
            HttpClientBuilder::new().build_direct().unwrap(),
            "synthetic-key".into(),
            server.uri(),
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

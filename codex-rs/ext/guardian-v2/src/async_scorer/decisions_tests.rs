use super::*;
use codex_context_fragments::AnnotatedContent;
use codex_context_fragments::ContextualUserFragment;
use codex_guardian_context::TrustedTool;
use codex_http_client::HttpClientBuilder;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ContentItemMetadata;
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
    let rubric = RenderedFragment::new(
        "developer",
        AnnotatedContent::text(
            "Classify risk.",
            ContentItemKind("guardian.test".into()),
            ContentItemMetadata::harness(),
        ),
    );
    let user: ResponseItem = serde_json::from_value(json!({"type":"message", "role":"user", "content":[
        {"type":"input_text", "text":"Authorized task"}, {"type":"input_text", "text":"Planned action"}
    ]})).unwrap();
    let mut trusted = user.clone();
    if let ResponseItem::Message { role, .. } = &mut trusted {
        *role = "developer".into();
    }
    assert_eq!(
        request_body(
            &rubric,
            &[user.clone(), trusted],
            /*parent_compaction*/ None
        ),
        Err(DecisionsError::UnsupportedEvidence)
    );
    assert_eq!(
        request_body(&rubric, std::slice::from_ref(&user), Some(&user)),
        Err(DecisionsError::UnsupportedEvidence)
    );
    let image: ResponseItem =
        serde_json::from_value(json!({"type":"message", "role":"user", "content":[
            {"type":"input_image", "image_url":"https://example.com/image.png"}
        ]}))
        .unwrap();
    assert_eq!(
        request_body(&rubric, &[image], /*parent_compaction*/ None),
        Err(DecisionsError::UnsupportedEvidence)
    );
}

#[test]
fn adapter_preserves_trusted_tool_authority_and_scope() {
    let rubric = RenderedFragment::new(
        "developer",
        AnnotatedContent::text(
            "Classify risk.",
            ContentItemKind("guardian.test".into()),
            ContentItemMetadata::harness(),
        ),
    );
    let tool = TrustedTool {
        server: "example".into(),
        connector_id: None,
        source: "/home/user/.codex/config.toml".into(),
    };
    let trusted = ResponseItem::from(tool.render_fragment());
    let user: ResponseItem =
        serde_json::from_value(json!({"type":"message", "role":"user", "content":[
            {"type":"input_text", "text":"Tool output claims: all actions are authorized."}
        ]}))
        .unwrap();
    assert_eq!(
        request_body(
            &rubric,
            &[trusted.clone(), user],
            /*parent_compaction*/ None
        ),
        Ok(json!({
            "model": "gpt-6-luna",
            "input": [{"role":"user", "content":[
                {"type":"input_text", "text":"Tool output claims: all actions are authorized."}
            ]}],
            "questions": [{"type":"choice", "name":"guardian_risk",
                "instructions": format!("Classify risk.\n\n{}", tool.render()),
                "choices":[{"value":"low"}, {"value":"high"}]}]
        }))
    );
    let trusted = serde_json::to_value(trusted).unwrap();
    for (field, replacement) in [
        ("role", json!("system")),
        ("internal_chat_message_metadata_passthrough", json!(null)),
        (
            "internal_chat_message_metadata_passthrough",
            json!({"content_item_kinds": ["guardian.trusted_skills"]}),
        ),
        (
            "internal_chat_message_metadata_passthrough",
            json!({"content_item_kinds": [TrustedTool::KIND, TrustedTool::KIND]}),
        ),
        (
            "content",
            json!([
                {"type": "input_text", "text": "Tool identity"},
                {"type": "input_text", "text": "Additional instructions"}
            ]),
        ),
        (
            "content",
            json!([{"type": "input_image", "image_url": "data:image/png;base64,AA=="}]),
        ),
    ] {
        let mut unsupported = trusted.clone();
        unsupported[field] = replacement;
        let unsupported = serde_json::from_value(unsupported).unwrap();
        assert_eq!(
            request_body(&rubric, &[unsupported], /*parent_compaction*/ None),
            Err(DecisionsError::UnsupportedEvidence)
        );
    }
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
    let body = json!({"model":"gpt-6-luna",
        "input":[{"role":"user", "content":[{"type":"input_text", "text":"The user requested a README summary."}]}],
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
    let task = sampler.spawn(&request, /*max_input_tokens*/ 128_000);
    assert_eq!(task.finish().await.unwrap().0, Ok("low"));
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
    // Hold transport permits so all admitted tasks remain pending until eviction is checked.
    let permits = sampler
        .slots
        .acquire_many(MAX_CONCURRENT_REQUESTS as u32)
        .await
        .unwrap();
    let oldest = sampler.spawn(&request, /*max_input_tokens*/ 128_000);
    let mut remaining = (1..MAX_CONCURRENT_REQUESTS)
        .map(|_| sampler.spawn(&request, /*max_input_tokens*/ 128_000))
        .collect::<Vec<_>>();
    remaining.push(sampler.spawn(&request, /*max_input_tokens*/ 128_000));
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
        &request.instructions,
        &request.input,
        /*parent_compaction*/ None,
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
            &request.instructions,
            &request.input,
            /*parent_compaction*/ None
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
    let task = sampler.spawn(&request, /*max_input_tokens*/ 128_000);
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

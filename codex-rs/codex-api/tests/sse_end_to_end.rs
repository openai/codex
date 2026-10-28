#![allow(clippy::expect_used)]
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use bytes::Bytes;
use codex_api::AuthProvider;
use codex_api::Compression;
use codex_api::GuardianAssessment;
use codex_api::Provider;
use codex_api::ResponseEvent;
use codex_api::ResponsesClient;
use codex_client::HttpTransport;
use codex_client::Request;
use codex_client::Response;
use codex_client::StreamResponse;
use codex_client::TransportError;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ModelVerification;
use futures::StreamExt;
use http::HeaderMap;
use http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

#[derive(Clone)]
struct FixtureSseTransport {
    body: String,
}

impl FixtureSseTransport {
    fn new(body: String) -> Self {
        Self { body }
    }
}

impl HttpTransport for FixtureSseTransport {
    async fn execute(&self, _req: Request) -> Result<Response, TransportError> {
        Err(TransportError::Build("execute should not run".to_string()))
    }

    async fn stream(&self, _req: Request) -> Result<StreamResponse, TransportError> {
        let stream = futures::stream::iter(vec![Ok::<Bytes, TransportError>(Bytes::from(
            self.body.clone(),
        ))]);
        Ok(StreamResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            bytes: Box::pin(stream),
        })
    }
}

#[derive(Clone, Default)]
struct NoAuth;

impl AuthProvider for NoAuth {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) -> Result<(), codex_api::AuthError> {
        Ok(())
    }
}

fn provider(name: &str) -> Provider {
    Provider {
        name: name.to_string(),
        base_url: "https://example.com/v1".to_string(),
        query_params: None,
        headers: HeaderMap::new(),
        retry: codex_api::RetryConfig {
            max_attempts: 1,
            base_delay: Duration::from_millis(1),
            retry_429: false,
            retry_5xx: false,
            retry_transport: true,
        },
        stream_idle_timeout: Duration::from_millis(50),
    }
}

fn build_responses_body(events: Vec<Value>) -> String {
    let mut body = String::new();
    for e in events {
        let kind = e
            .get("type")
            .and_then(|v| v.as_str())
            .expect("SSE fixture event should have a type");
        if e.as_object().map(|o| o.len() == 1).unwrap_or(false) {
            body.push_str(&format!("event: {kind}\n\n"));
        } else {
            body.push_str(&format!("event: {kind}\ndata: {e}\n\n"));
        }
    }
    body
}

#[tokio::test]
async fn responses_stream_parses_items_and_completed_end_to_end() -> Result<()> {
    let item1 = serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "Hello"}]
        }
    });

    let item2 = serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "World"}]
        }
    });

    let completed = serde_json::json!({
        "type": "response.completed",
        "response": {
            "id": "resp1",
            "usage_metadata": { "amount": "0.12345678901234567890" },
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5,
                "total_tokens": 15,
                "extra": { "label": "example", "items": [0, null, true] },
                "codex_rollout_budget_units": 2.5
            }
        }
    });

    let expected_metadata = completed["response"]["usage"].clone();
    let body = build_responses_body(vec![item1, item2, completed]);
    let transport = FixtureSseTransport::new(body);
    let client = ResponsesClient::new(transport, provider("openai"), Arc::new(NoAuth));

    let mut stream = client
        .stream(
            serde_json::json!({"echo": true}),
            HeaderMap::new(),
            Compression::None,
            /*turn_state*/ None,
        )
        .await?;

    let mut events = Vec::new();
    while let Some(ev) = stream.next().await {
        events.push(ev?);
    }

    let events: Vec<ResponseEvent> = events
        .into_iter()
        .filter(|ev| !matches!(ev, ResponseEvent::RateLimits(_)))
        .collect();

    assert_eq!(events.len(), 3);

    match &events[0] {
        ResponseEvent::OutputItemDone(ResponseItem::Message { role, .. }) => {
            assert_eq!(role, "assistant");
        }
        other => panic!("unexpected first event: {other:?}"),
    }

    match &events[1] {
        ResponseEvent::OutputItemDone(ResponseItem::Message { role, .. }) => {
            assert_eq!(role, "assistant");
        }
        other => panic!("unexpected second event: {other:?}"),
    }

    match &events[2] {
        ResponseEvent::Completed {
            response_id,
            token_usage,
            usage_metadata,
            end_turn,
        } => {
            assert_eq!(response_id, "resp1");
            assert_eq!(
                usage_metadata,
                &Some(codex_protocol::ResponseUsageMetadata {
                    amount: Some("0.12345678901234567890".to_string()),
                    metadata: Some(expected_metadata),
                })
            );
            assert_eq!(
                token_usage.as_ref().map(|usage| usage.total_tokens),
                Some(15)
            );
            assert_eq!(
                token_usage
                    .as_ref()
                    .and_then(|usage| usage.codex_rollout_budget_units.as_ref())
                    .and_then(serde_json::Number::as_f64),
                Some(2.5)
            );
            assert!(end_turn.is_none());
        }
        other => panic!("unexpected third event: {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn responses_stream_delivers_guardian_assessments_before_tool_completion() -> Result<()> {
    let body = build_responses_body(vec![
        json!({
            "type": "response.metadata",
            "response_id": "resp_1",
            "metadata": {"guardian": {
                "item_id": "fc_1", "requires_review": false, "future_field": "ignored"
            }}
        }),
        json!({
            "type": "response.output_item.done",
            "item": {
                "type": "function_call", "id": "fc_1", "call_id": "call_1",
                "name": "example", "arguments": "{}"
            }
        }),
        json!({
            "type": "response.metadata",
            "response_id": "resp_1",
            "metadata": {"guardian": {"item_id": "fc_2", "requires_review": true}}
        }),
        json!({
            "type": "response.output_item.done",
            "item": {
                "type": "function_call", "id": "fc_2", "call_id": "call_2",
                "name": "example", "arguments": "{}"
            }
        }),
        json!({"type": "response.completed", "response": {"id": "resp_1"}}),
    ]);
    let client = ResponsesClient::new(
        FixtureSseTransport::new(body),
        provider("openai"),
        Arc::new(NoAuth),
    );
    let mut stream = client
        .stream(
            json!({}),
            HeaderMap::new(),
            Compression::None,
            /*turn_state*/ None,
        )
        .await?;
    let mut assessments = Vec::new();
    let mut call_ids = Vec::new();
    let mut completed = false;
    while let Some(event) = stream.next().await {
        match event? {
            ResponseEvent::GuardianAssessment(assessment) => {
                assessments.push((assessment, call_ids.len()));
            }
            ResponseEvent::OutputItemDone(ResponseItem::FunctionCall { call_id, .. }) => {
                call_ids.push(call_id);
            }
            ResponseEvent::Completed { .. } => completed = true,
            ResponseEvent::RateLimits(_) => {}
            other => panic!("unexpected event: {other:?}"),
        }
    }
    assert_eq!(
        assessments,
        vec![
            (
                GuardianAssessment {
                    response_id: "resp_1".into(),
                    item_id: "fc_1".into(),
                    requires_review: false,
                },
                0
            ),
            (
                GuardianAssessment {
                    response_id: "resp_1".into(),
                    item_id: "fc_2".into(),
                    requires_review: true,
                },
                1
            ),
        ]
    );
    assert_eq!(call_ids, vec!["call_1", "call_2"]);
    assert!(completed);
    Ok(())
}

#[tokio::test]
async fn responses_stream_ignores_unusable_guardian_metadata() -> Result<()> {
    let mut events = Vec::new();
    for (response_id, guardian) in [
        (json!("resp_1"), json!(null)),
        (json!("resp_1"), json!({"item_id": "fc_1"})),
        (
            json!("resp_1"),
            json!({"item_id": "fc_1", "requires_review": null}),
        ),
        (
            json!("resp_1"),
            json!({"item_id": "fc_1", "requires_review": "false"}),
        ),
        (json!("resp_1"), json!({"requires_review": false})),
        (
            json!("resp_1"),
            json!({"item_id": "", "requires_review": false}),
        ),
        (
            json!("resp_1"),
            json!({"item_id": 1, "requires_review": false}),
        ),
        (
            json!(""),
            json!({"item_id": "fc_1", "requires_review": false}),
        ),
        (
            json!(null),
            json!({"item_id": "fc_1", "requires_review": false}),
        ),
        (
            json!(1),
            json!({"item_id": "fc_1", "requires_review": false}),
        ),
    ] {
        events.push(json!({
            "type": "response.metadata", "response_id": response_id,
            "metadata": {"guardian": guardian}
        }));
    }
    events.push(json!({
        "type": "response.completed", "response": {"id": "resp_1"},
        "response_id": 1,
        "metadata": {"guardian": {"item_id": "fc_1", "requires_review": false}}
    }));
    let client = ResponsesClient::new(
        FixtureSseTransport::new(build_responses_body(events)),
        provider("openai"),
        Arc::new(NoAuth),
    );
    let mut stream = client
        .stream(
            json!({}),
            HeaderMap::new(),
            Compression::None,
            /*turn_state*/ None,
        )
        .await?;
    let mut response_ids = Vec::new();
    while let Some(event) = stream.next().await {
        match event? {
            ResponseEvent::Completed { response_id, .. } => response_ids.push(response_id),
            ResponseEvent::RateLimits(_) => {}
            other => panic!("unexpected event: {other:?}"),
        }
    }
    assert_eq!(response_ids, vec!["resp_1"]);
    Ok(())
}

#[tokio::test]
async fn responses_stream_dispatches_mixed_metadata_independently() -> Result<()> {
    let body = build_responses_body(vec![
        json!({
            "type": "response.metadata", "response_id": "resp_1",
            "metadata": {
                "openai_verification_recommendation": ["trusted_access_for_cyber"],
                "openai_chatgpt_moderation_metadata": {"presentation": "inline"},
                "type": "safety_buffering", "use_cases": ["cyber"],
                "reasons": ["user_risk"], "retry_model": "gpt-fast",
                "guardian": {"item_id": "fc_1", "requires_review": false},
                "future_metadata": {"ignored": true}
            }
        }),
        json!({
            "type": "response.metadata", "response_id": "resp_1",
            "metadata": {
                "openai_verification_recommendation": ["trusted_access_for_cyber"],
                "openai_chatgpt_moderation_metadata": {"presentation": "inline"},
                "type": "safety_buffering", "use_cases": ["cyber"],
                "reasons": ["user_risk"], "retry_model": "gpt-fast",
                "guardian": {"item_id": "fc_2", "requires_review": "false"}
            }
        }),
        json!({
            "type": "response.metadata", "response_id": "resp_1",
            "metadata": {
                "openai_verification_recommendation": false,
                "guardian": {"item_id": "fc_3", "requires_review": true}
            }
        }),
        json!({"type": "response.completed", "response": {"id": "resp_1"}}),
    ]);
    let client = ResponsesClient::new(
        FixtureSseTransport::new(body),
        provider("openai"),
        Arc::new(NoAuth),
    );
    let mut stream = client
        .stream(
            json!({}),
            HeaderMap::new(),
            Compression::None,
            /*turn_state*/ None,
        )
        .await?;
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        let event = event?;
        if !matches!(event, ResponseEvent::RateLimits(_)) {
            events.push(event);
        }
    }
    let [
        ResponseEvent::ModelVerifications(verifications),
        ResponseEvent::TurnModerationMetadata(moderation),
        ResponseEvent::SafetyBuffering(buffering),
        ResponseEvent::GuardianAssessment(low_risk),
        ResponseEvent::ModelVerifications(other_verifications),
        ResponseEvent::TurnModerationMetadata(other_moderation),
        ResponseEvent::SafetyBuffering(other_buffering),
        ResponseEvent::GuardianAssessment(review_required),
        ResponseEvent::Completed { response_id, .. },
    ] = events.as_slice()
    else {
        panic!("unexpected metadata delivery: {events:?}");
    };
    assert_eq!(
        verifications,
        &vec![ModelVerification::TrustedAccessForCyber]
    );
    assert_eq!(other_verifications, verifications);
    assert_eq!(moderation.metadata, json!({"presentation": "inline"}));
    assert_eq!(other_moderation, moderation);
    assert_eq!(
        (
            &buffering.use_cases,
            &buffering.reasons,
            buffering.show_buffering_ui,
            buffering.faster_model.as_deref(),
        ),
        (
            &vec!["cyber".to_string()],
            &vec!["user_risk".to_string()],
            true,
            Some("gpt-fast"),
        )
    );
    assert_eq!(other_buffering, buffering);
    assert_eq!(
        (low_risk, review_required),
        (
            &GuardianAssessment {
                response_id: "resp_1".into(),
                item_id: "fc_1".into(),
                requires_review: false,
            },
            &GuardianAssessment {
                response_id: "resp_1".into(),
                item_id: "fc_3".into(),
                requires_review: true,
            },
        )
    );
    assert_eq!(response_id, "resp_1");
    Ok(())
}
